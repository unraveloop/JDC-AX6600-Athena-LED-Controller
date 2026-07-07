// ==========================================
// 🎮 button.rs — 物理按键监听器 (长短按分离 + 睡眠感知)
// 双后端: GPIO 字符设备直读 (优先) / debugfs 文本解析 (兜底)
// 通过 watch channel 与调度器通信: -1=息屏Toggle, +N=切台
// ==========================================

// ==========================================
// 🐧 Linux 环境下的监听器
// ==========================================
#[cfg(unix)]
pub fn spawn_button_listener(
    tx: tokio::sync::watch::Sender<i32>,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    gpio_pin: String,  // 🌟 比如 "71" (TLMM 引脚偏移，也允许直接填全局编号)
    gpio_base: String, // 🌟 "auto" 或数字，仅 debugfs 兜底路径需要换算全局编号
    control: crate::control::SharedControl, // 🌟 [v2.4.0] 双击回首页需要写共享控制状态
) {
    use crate::led_screen;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    tokio::task::spawn_blocking(move || {
        let pin_num: u32 = gpio_pin.trim().parse().unwrap_or(71);

        // ==========================================
        // 🌟 [双后端] 后端 1: GPIO 字符设备直读 (现代内核标准接口)
        // 注意: 如果按键被内核 gpio-keys 驱动占用会返回 EBUSY，自动落入后端 2
        // ==========================================
        let cdev_req = led_screen::find_main_chip().and_then(|chip| {
            gpiocdev::Request::builder()
                .on_chip(chip)
                .with_consumer("athena-led-btn")
                .with_line(pin_num)
                .as_input()
                .request()
                .ok()
        });

        // ==========================================
        // 后端 2: debugfs 文本解析 (兜底，兼容老内核与被 gpio-keys 占用的引脚)
        // 不同内核 /sys/kernel/debug/gpio 的行格式完全不同：
        //   QSDK 老内核:  "gpio71  : in  low"
        //   标准新内核:   " gpio-583 (...|switch  ) in  lo"    (583 = base 512 + 71)
        // ==========================================
        let mut debugfs_file: Option<File> = None;
        let mut pin_patterns: Vec<String> = Vec::new();

        if cdev_req.is_some() {
            println!("🎮 [系统] 按键监听启动 (字符设备后端, 引脚 {})", pin_num);
        } else {
            match File::open("/sys/kernel/debug/gpio") {
                Ok(f) => {
                    let base: u64 = match gpio_base.trim() {
                        "" | "auto" => led_screen::detect_gpio_base(),
                        s => s.parse().unwrap_or_else(|_| led_screen::detect_gpio_base()),
                    };
                    let global_num = base + pin_num as u64;
                    // 行内“引脚名”匹配模式 (任意一个命中即认为是目标引脚所在行)
                    pin_patterns = vec![
                        format!("gpio{}  :", pin_num),      // QSDK 老格式
                        format!("gpio-{} ", pin_num),       // 用户直接填了全局编号
                        format!("gpio-{} ", global_num),    // 标准格式: base + 偏移
                        format!("gpio-{}(", global_num),
                    ];
                    debugfs_file = Some(f);
                    println!("🎮 [系统] 按键监听启动 (debugfs 后端, 引脚 {} / 全局 {})", pin_num, global_num);
                }
                Err(e) => {
                    println!("⚠️ [警告] 按键监听不可用: 字符设备请求失败，debugfs 也无法打开 ({})", e);
                    return;
                }
            }
        }

        // 判断单行是否报告“输入 + 低电平” (按键按下通常拉低)
        let line_is_pressed = |line: &str| -> bool {
            if !line.contains(" in ") { return false; }
            // 新内核输出 " lo"，老内核输出 " low"
            let trimmed = line.trim_end();
            line.contains(" low") || line.contains(" lo ") || trimmed.ends_with(" lo")
        };

        let mut buffer = String::with_capacity(4096);

        // 🌟 状态机变量
        let mut press_start: Option<Instant> = None;
        let mut long_press_handled = false;
        // 🌟 [v2.4.0] 双击检测: 第一次短按松开后等待 350ms，
        // 期间再次按下 = 双击 (回频道 1)；超时未按 = 单击 (切下一台)
        let mut pending_click_deadline: Option<Instant> = None;

        while running.load(Ordering::SeqCst) {
            // --- 读取当前按键电平 (按下 = 物理低电平) ---
            let is_pressed = if let Some(req) = &cdev_req {
                matches!(req.value(pin_num), Ok(gpiocdev::line::Value::Inactive))
            } else if let Some(file) = debugfs_file.as_mut() {
                buffer.clear();
                let _ = file.seek(SeekFrom::Start(0));
                if file.read_to_string(&mut buffer).is_ok() {
                    // 🌟 逐行扫描：先定位目标引脚所在行，再判断电平状态
                    buffer.lines().any(|line| {
                        pin_patterns.iter().any(|p| line.contains(p.as_str())) && line_is_pressed(line)
                    })
                } else {
                    false
                }
            } else {
                false
            };

            if is_pressed {
                // 1️⃣ 刚刚按下瞬间，记录时间点
                if press_start.is_none() {
                    press_start = Some(Instant::now());
                    long_press_handled = false;

                    // 🌟 [双击] 在等待窗口内再次按下 = 双击，立即回频道 1
                    if pending_click_deadline.is_some() {
                        pending_click_deadline = None;
                        #[cfg(debug_assertions)]
                        println!("⏮️ [硬件交互] 双击触发！回到频道 1");
                        if let Ok(mut st) = control.lock() {
                            st.go_home = true;
                        }
                        let current = *tx.borrow();
                        let _ = tx.send(if current < 0 { 1 } else { current + 1 });
                        // 标记本次按压已消费，松开时不再进入单击判定
                        long_press_handled = true;
                    }
                }
                // 2️⃣ 一直按着没松手，检查是否达到长按阈值 (2 秒)
                else if !long_press_handled {
                    if press_start.unwrap().elapsed() >= Duration::from_secs(2) {
                        #[cfg(debug_assertions)]
                        println!("🌙 [硬件交互] 检测到长按 2 秒！发送息屏/亮屏切换指令！");

                        // 🌟 约定 -1 为“休眠/唤醒”的 Toggle 指令
                        let _ = tx.send(-1);

                        // 标记已处理，防止一直触发
                        long_press_handled = true;
                    }
                }
            } else {
                // 3️⃣ 松开按键
                if let Some(start) = press_start {
                    let hold_time = start.elapsed();

                    // 如果没有触发过长按/双击，并且按下的时间大于 50ms (防物理抖动)
                    if !long_press_handled && hold_time > Duration::from_millis(50) {
                        let current = *tx.borrow();
                        if current < 0 {
                            // 休眠状态: 任何短按立即唤醒 (不做双击等待)
                            println!("☀️ [硬件交互] 夜间休眠被打断，唤醒屏幕！");
                            let _ = tx.send(1);
                            pending_click_deadline = None;
                        } else {
                            // 🌟 [双击] 先挂起 350ms，看是否有第二击
                            pending_click_deadline = Some(Instant::now() + Duration::from_millis(350));
                        }
                    }

                    // 重置状态机，准备迎接下一次按键
                    press_start = None;
                }

                // 🌟 [双击] 等待窗口超时且无第二击 -> 判定为单击，正常切台
                if let Some(deadline) = pending_click_deadline {
                    if Instant::now() >= deadline {
                        pending_click_deadline = None;
                        #[cfg(debug_assertions)]
                        println!("➡️ [硬件交互] 短按触发！准备切换频道...");
                        let current = *tx.borrow();
                        let _ = tx.send(if current < 0 { 1 } else { current + 1 });
                    }
                }
            }

            // 保持 100ms 的轮询频率，兼顾灵敏度与低 CPU 占用
            std::thread::sleep(Duration::from_millis(100));
        }

        println!("👋 [系统] 按钮监听线程已安全退出。");
    });
}

// ==========================================
// 🪟 Windows 环境下的“空壳”监听器 (防报错)
// ==========================================
#[cfg(not(unix))]
pub fn spawn_button_listener(
    _tx: tokio::sync::watch::Sender<i32>,
    _running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _gpio_pin: String,
    _gpio_base: String,
    _control: crate::control::SharedControl,
) {
    // Windows 模拟器不需要物理按键监听
    println!("📺 [Windows 模拟器] 按键监听已就绪（空跑模式）");
}
