use anyhow::{anyhow, Context, Result};
use crate::char_dict::CHAR_DICT;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LOW: u8 = 0x00;
const HIGH: u8 = 0x01;

// ==========================================
// 🌟 AX6600 屏幕在主控 TLMM 上的硬件引脚偏移
// 这是物理编号，与内核版本无关 (字符设备后端直接使用；
// sysfs 后端需要加上 gpiochip base 换算成全局编号)
// ==========================================
pub const PIN_STB_LEFT: u32 = 69;
pub const PIN_STB_RIGHT: u32 = 70;
pub const PIN_CLK: u32 = 73;
pub const PIN_DIO: u32 = 74;

// Display mode commands
const COMMAND1: u8 = 0b00000011; // Display mode
const COMMAND2: u8 = 0b01000000; // Data mode
const COMMAND3: u8 = 0b11000000; // Display address

// ==========================================
// 🌟 [兼容性修复] GPIO 基址自动探测 (sysfs 后端用)
// 不同内核版本的 gpiochip base 完全不同：
//   - 内核 6.1+ (GPIO_DYNAMIC_BASE): base = 512  -> 屏幕引脚 581/582/585/586
//   - 内核 5.x  (自顶向下分配):      base = 432  -> 屏幕引脚 501/502/505/506
//   - 老内核 (静态分配):             base = 0    -> 屏幕引脚 69/70/73/74
// 以前写死 581 系列导致 QWRT / iStoreOS 等固件上屏幕完全不亮。
// 这里扫描 /sys/class/gpio/gpiochip*/ 动态找出主控 (TLMM/pinctrl) 的真实 base。
// ==========================================
pub fn detect_gpio_base() -> u64 {
    // (base, ngpio, 是否主控芯片)
    let mut best: Option<(u64, u64, bool)> = None;

    if let Ok(entries) = fs::read_dir("/sys/class/gpio") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("gpiochip") {
                continue;
            }
            let path = entry.path();
            let read_num = |file: &str| -> Option<u64> {
                fs::read_to_string(path.join(file))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
            };

            let base = match read_num("base") {
                Some(b) => b,
                None => continue,
            };
            let ngpio = read_num("ngpio").unwrap_or(0);
            let label = fs::read_to_string(path.join("label"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            // IPQ60xx 的主控引脚控制器 label 通常含 "pinctrl" 或 "tlmm"
            let is_main = label.contains("pinctrl") || label.contains("tlmm");

            let better = match &best {
                None => true,
                Some((_, best_n, best_main)) => {
                    (is_main && !best_main) || (is_main == *best_main && ngpio > *best_n)
                }
            };
            if better {
                best = Some((base, ngpio, is_main));
            }
        }
    }

    match best {
        Some((base, ngpio, _)) => {
            println!("🔍 [GPIO] 自动探测到主控芯片 base={} (ngpio={})", base, ngpio);
            base
        }
        None => {
            // 探测不到时保持老版本行为 (内核 6.1+ 的 512)
            println!("⚠️ [GPIO] 未能探测到 gpiochip，回退默认 base=512");
            512
        }
    }
}

// ==========================================
// 🌟 [新] 查找主控芯片的字符设备路径 (/dev/gpiochipN)
// 优先选 label 含 pinctrl/tlmm 的芯片，否则选线数最多的
// ==========================================
pub fn find_main_chip() -> Option<PathBuf> {
    let mut best: Option<(PathBuf, u32, bool)> = None;

    if let Ok(entries) = fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("gpiochip") {
                continue;
            }
            let path = entry.path();
            let info = match gpiocdev::Chip::from_path(&path).and_then(|c| c.info()) {
                Ok(i) => i,
                Err(_) => continue,
            };
            let label = info.label.to_lowercase();
            let is_main = label.contains("pinctrl") || label.contains("tlmm");

            let better = match &best {
                None => true,
                Some((_, best_n, best_main)) => {
                    (is_main && !best_main) || (is_main == *best_main && info.num_lines > *best_n)
                }
            };
            if better {
                best = Some((path, info.num_lines, is_main));
            }
        }
    }

    best.map(|(p, n, _)| {
        println!("🔍 [GPIO] 找到主控字符设备: {} (lines={})", p.display(), n);
        p
    })
}

// ==========================================
// 🌟 [双后端架构] GPIO 总线抽象
// cdev  = /dev/gpiochipN 字符设备 (现代内核标准接口，优先)
// sysfs = /sys/class/gpio (内核已废弃，仅作老固件回退)
// 注意: CLK/DIO 是左右两屏共享的，所以必须由总线统一持有，
//       不能像旧版那样每个屏各导出一份 (cdev 下会 EBUSY)
// ==========================================
#[derive(Clone, Copy, PartialEq)]
enum Line {
    StbLeft,
    StbRight,
    Clk,
    Dio,
}

enum GpioBus {
    Cdev {
        req: gpiocdev::Request,
    },
    Sysfs {
        stb_l: sysfs_gpio::Pin,
        stb_r: sysfs_gpio::Pin,
        clk: sysfs_gpio::Pin,
        dio: sysfs_gpio::Pin,
    },
}

impl GpioBus {
    fn set(&mut self, line: Line, level: u8) -> Result<()> {
        match self {
            GpioBus::Cdev { req } => {
                let offset = match line {
                    Line::StbLeft => PIN_STB_LEFT,
                    Line::StbRight => PIN_STB_RIGHT,
                    Line::Clk => PIN_CLK,
                    Line::Dio => PIN_DIO,
                };
                let value = if level == LOW {
                    gpiocdev::line::Value::Inactive
                } else {
                    gpiocdev::line::Value::Active
                };
                req.set_value(offset, value)?;
            }
            GpioBus::Sysfs { stb_l, stb_r, clk, dio } => {
                let pin = match line {
                    Line::StbLeft => stb_l,
                    Line::StbRight => stb_r,
                    Line::Clk => clk,
                    Line::Dio => dio,
                };
                pin.set_value(level)?;
            }
        }
        Ok(())
    }
}

impl Drop for GpioBus {
    fn drop(&mut self) {
        // cdev 的 Request 析构时内核自动释放线；sysfs 需要手动 unexport
        if let GpioBus::Sysfs { stb_l, stb_r, clk, dio } = self {
            let _ = stb_l.unexport();
            let _ = stb_r.unexport();
            let _ = clk.unexport();
            let _ = dio.unexport();
        }
    }
}

pub struct LedScreen {
    bus: GpioBus,
}

impl LedScreen {
    // 🌟 [新签名] backend: "auto"/"cdev"/"sysfs"; gpio_base: "auto"/数字 (仅 sysfs 用)
    pub fn new(backend: &str, gpio_base: &str) -> Result<Self> {
        let bus = match backend.trim() {
            "cdev" => Self::open_cdev()?,
            "sysfs" => Self::open_sysfs(gpio_base)?,
            // auto: 优先字符设备，失败自动回退 sysfs
            _ => match Self::open_cdev() {
                Ok(bus) => bus,
                Err(e) => {
                    println!("⚠️ [GPIO] 字符设备后端不可用 ({})，回退 sysfs 后端", e);
                    Self::open_sysfs(gpio_base)?
                }
            },
        };

        let mut screen = Self { bus };
        screen.set_show_model()?;
        screen.set_data_model()?;
        Ok(screen)
    }

    fn open_cdev() -> Result<GpioBus> {
        let chip = find_main_chip().ok_or_else(|| anyhow!("未找到 /dev/gpiochip* 字符设备"))?;
        let req = gpiocdev::Request::builder()
            .on_chip(chip.clone())
            .with_consumer("athena-led")
            .with_lines(&[PIN_STB_LEFT, PIN_STB_RIGHT, PIN_CLK, PIN_DIO])
            .as_output(gpiocdev::line::Value::Inactive)
            .request()
            .with_context(|| format!("在 {} 上请求屏幕 GPIO 线失败", chip.display()))?;
        println!("🔌 [GPIO] 屏幕使用字符设备后端: {}", chip.display());
        Ok(GpioBus::Cdev { req })
    }

    fn open_sysfs(gpio_base: &str) -> Result<GpioBus> {
        let base: u64 = match gpio_base.trim() {
            "" | "auto" => detect_gpio_base(),
            s => s.parse().unwrap_or_else(|_| {
                println!("⚠️ [GPIO] --gpio-base 参数 '{}' 无法解析，改用自动探测", s);
                detect_gpio_base()
            }),
        };

        let make_pin = |offset: u32, name: &str| -> Result<sysfs_gpio::Pin> {
            let num = base + offset as u64;
            let pin = sysfs_gpio::Pin::new(num);
            // 🌟 [修复] export 失败时明确报出是哪个引脚，方便在 logread 里排查固件兼容性
            pin.export()
                .with_context(|| format!("导出 GPIO{} ({}) 失败，请检查 --gpio-base 是否正确", num, name))?;
            pin.set_direction(sysfs_gpio::Direction::Out)
                .with_context(|| format!("设置 GPIO{} 方向失败", num))?;
            Ok(pin)
        };

        let stb_l = make_pin(PIN_STB_LEFT, "STB_L")?;
        let stb_r = make_pin(PIN_STB_RIGHT, "STB_R")?;
        let clk = make_pin(PIN_CLK, "CLK")?;
        let dio = make_pin(PIN_DIO, "DIO")?;

        println!(
            "🔌 [GPIO] 屏幕使用 sysfs 后端 (base={}, 引脚 {}/{}/{}/{})",
            base,
            base + PIN_STB_LEFT as u64,
            base + PIN_STB_RIGHT as u64,
            base + PIN_CLK as u64,
            base + PIN_DIO as u64
        );
        Ok(GpioBus::Sysfs { stb_l, stb_r, clk, dio })
    }

    pub fn set_show_model(&mut self) -> Result<()> {
        self.unit_write(Line::StbLeft, COMMAND1, &[])?;
        self.unit_write(Line::StbRight, COMMAND1, &[])?;
        Ok(())
    }

    pub fn set_data_model(&mut self) -> Result<()> {
        self.unit_write(Line::StbLeft, COMMAND2, &[])?;
        self.unit_write(Line::StbRight, COMMAND2, &[])?;
        Ok(())
    }

    pub fn power(&mut self, run: bool, light_level: u8) -> Result<()> {
        let command = if run {
            (light_level << 5 >> 5 | 0b11111000) & 0b10001111
        } else {
            0b10000000
        };
        self.unit_write(Line::StbLeft, command, &[])?;
        self.unit_write(Line::StbRight, command, &[])?;
        Ok(())
    }

    // 2. 这里也要加上 async 关键字
    pub async fn write_data(&mut self, text: &[u8], status: u8) -> Result<()> {
        let mut display_data = Vec::new();

        let content = std::str::from_utf8(text).unwrap_or("");

        for ch in content.chars() {
            let key = ch.to_ascii_uppercase();

            if let Some(bytes) = CHAR_DICT.get(&key) {
                display_data.extend_from_slice(bytes);
                display_data.push(0x00); // 加空格
            }
        }

        // 修复：砍掉最后一个多余的尾部空格！
        // 这样 28 列的 "10:10:10" 就会瞬间变成 27 列！
        if !display_data.is_empty() {
            display_data.pop();
        }

        // 判断逻辑完全不需要改，保持 27 即可
        if display_data.len() > 27 {
            self.flow(&display_data, status).await?;
        } else {
            self.static_display(&display_data, status)?;
        }
        Ok(())
    }

    // ==========================================
    // 🎬 动画播放引擎 (0 CPU 消耗，直接内存推流)
    // ==========================================
    pub async fn play_animation(&mut self, file_name: &str, duration_secs: u64, status: u8) -> Result<()> {
        let file_path = format!("/etc/athena_led/anim/{}", file_name);

        if let Ok(metadata) = fs::metadata(&file_path) {
            // 🌟 [修复] 限制和提示统一为 5MB (以前代码限制 50MB 但提示 5MB，自相矛盾；
            // 路由器内存有限，5MB ≈ 3 小时动画，完全够用)
            if metadata.len() > 5 * 1024 * 1024 {
                eprintln!("❌ 动画文件过大 (超过 5MB)，拒绝加载: {}", file_path);
                return self.static_display(b"TOO LARGE", status);
            }
        }
        // 1. 一次性把整个动画文件读进内存
        let anim_data = match fs::read(&file_path) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("❌ 无法读取动画文件 {}: {}", file_path, e);
                // 读不到文件时防呆：显示一个错误提示并退出
                return self.static_display(b"FILE ERR", status);
            }
        };

        let frames_count = anim_data.len() / 27;
        if frames_count == 0 {
            eprintln!("❌ 动画文件为空或已损坏: {}", file_path);
            return Ok(());
        }

        let start_time = Instant::now();
        let total_duration = Duration::from_secs(duration_secs);

        // 2. 设定帧间隔 (15 FPS = 约 66 毫秒)
        let frame_interval = Duration::from_millis(66);

        // 3. 切片读取：每次精准切出 27 个字节！
        // .cycle() 魔法：播到底部自动从头循环，直到总时长结束！
        let mut frame_iter = anim_data.chunks_exact(27).cycle();

        // 4. 开始无情推流
        while start_time.elapsed() < total_duration {
            if let Some(frame_chunk) = frame_iter.next() {
                // 震惊！由于 .bin 已经做好了列映射，我们直接把这 27 字节塞给底层！
                self.do_write_data(frame_chunk, status)?;
            }

            // 异步休眠，挂起当前任务，立刻将 CPU 交还给按键监听线程！
            tokio::time::sleep(frame_interval).await;
        }

        Ok(())
    }

    // 🌟 专为动态模块（天气、时间）设计的“强制静态、完美居中、零浪费”特化方法
    // (当前调度器未使用，保留给外部/未来模块调用)
    #[allow(dead_code)]
    pub async fn write_data_static(&mut self, text: &[u8], status: u8) -> Result<()> {
        let mut display_data = Vec::new();
        let content = std::str::from_utf8(text).unwrap_or("");

        for ch in content.chars() {
            let key = ch.to_ascii_uppercase();
            if let Some(bytes) = CHAR_DICT.get(&key) {
                display_data.extend_from_slice(bytes);
                display_data.push(0x00);
            }
        }

        if !display_data.is_empty() {
            display_data.pop();
        }

        self.static_display(&display_data, status)?;
        Ok(())
    }

    // 1. 加上 async 关键字
    async fn flow(&mut self, data: &[u8], status: u8) -> Result<()> {
        let mut start = 0;
        for i in 1..=data.len() {
            let mut off = [0u8; 27];
            if i > 27 {
                start += 1;
            }
            off[..i.min(27)].copy_from_slice(&data[start..start + i.min(27)]);
            self.do_write_data(&off, status)?;

            // 🚨 核心修复：把原先的 std::thread::sleep 换成 tokio 的异步 sleep！
            // 这样休眠时，程序立刻把控制权交还给主线程去检查按键！
            tokio::time::sleep(std::time::Duration::from_millis(128)).await;
        }
        Ok(())
    }

    fn static_display(&mut self, data: &[u8], status: u8) -> Result<()> {
        let mut display_data = [0u8; 27];
        if data.len() < 27 {
            let offset = (27 - data.len()) / 2;
            display_data[offset..offset + data.len()].copy_from_slice(data);
        } else {
            display_data[..27].copy_from_slice(&data[..27]);
        }
        self.do_write_data(&display_data, status)?;
        Ok(())
    }

    fn do_write_data(&mut self, values: &[u8], status: u8) -> Result<()> {
        // 左屏显示前 14 列
        let left: Vec<u8> = values[..14].to_vec();
        self.unit_write(Line::StbLeft, COMMAND3, &left)?;
        // 右屏显示后 13 列 + 状态灯字节
        let mut right_data = values[14..27].to_vec();
        right_data.push(status);
        self.unit_write(Line::StbRight, COMMAND3, &right_data)?;
        Ok(())
    }

    // ==========================================
    // TM1628A 底层协议 (STB 选中 -> 命令字节 -> 数据字节 -> STB 释放)
    // ==========================================
    fn unit_write(&mut self, stb: Line, command: u8, values: &[u8]) -> Result<()> {
        self.bus.set(stb, LOW)?;
        self.write_command_byte(command)?;

        for (i, &value) in values.iter().enumerate() {
            self.write_data_byte(value, i % 2 != 0)?;
        }

        self.bus.set(stb, HIGH)?;
        Ok(())
    }

    fn write_command_byte(&mut self, value: u8) -> Result<()> {
        for i in 0..8 {
            let bit = (value >> i) & 0x01;
            self.write_bit(bit)?;
        }
        Ok(())
    }

    fn write_data_byte(&mut self, value: u8, fill_data: bool) -> Result<()> {
        for i in 0..5 {
            let bit = (value >> i) & 0x01;
            self.write_bit(bit)?;
        }

        if fill_data {
            for _ in 0..6 {
                self.write_bit(LOW)?;
            }
        }
        Ok(())
    }

    fn write_bit(&mut self, bit: u8) -> Result<()> {
        self.bus.set(Line::Clk, LOW)?;
        self.bus.set(Line::Dio, bit)?;
        self.bus.set(Line::Clk, HIGH)?;
        Ok(())
    }
}
