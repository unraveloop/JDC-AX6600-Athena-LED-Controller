// ==========================================
// 🚀 main.rs — 程序入口与命令行参数
// 代码结构 (v2.3.0 模块化拆分):
//   led_screen.rs     — LED 屏硬件驱动 (cdev/sysfs 双后端, 仅 Linux)
//   led_screen_sim.rs — Windows 本地调试用虚拟屏幕
//   char_dict.rs      — 点阵字模字典
//   monitor.rs        — 本地系统数据采集 (CPU/内存/温度/网速...)
//   net_agent.rs      — 后台网络数据代理 (天气/IP/股票/HTTP/延迟/日出日落)
//   scheduler.rs      — 轮播调度引擎 (Profile 解析/渲染/休眠/插播)
//   button.rs         — 物理按键监听 (长按/短按/双击)
//   control.rs        — 运行时控制接口 (127.0.0.1 TCP)
//   mqtt.rs           — MQTT 订阅上屏 (HA 集成)
//   lunar.rs / sun.rs — 农历 / 日出日落 (纯本地计算)
// ==========================================
#[cfg(unix)]
mod led_screen;
#[cfg(not(unix))]
#[path = "led_screen_sim.rs"]
mod led_screen;
#[cfg(unix)]
mod char_dict;

mod button;
mod control;
mod lunar;
mod monitor;
mod mqtt;
mod net_agent;
mod scheduler;
mod sun;

use anyhow::{Context, Result};
use clap::Parser;

// --- 参数定义 ---
// 🌟 Clone: 网络代理后台任务需要持有一份参数副本
#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    // --- 基础设置 ---
    #[arg(long, default_value_t = 5)]
    pub seconds: u64, // 每个模块显示的秒数

    #[arg(long, default_value_t = 5)]
    pub light_level: u8, // 亮度 (0-7)

    // 🌟 [v2.3.1 新增] 定时亮度: 夜间时段自动降低亮度 (不熄屏)
    // 起止为空 = 功能关闭；支持跨午夜 (如 22:00 ~ 07:00)
    #[arg(long, default_value = "")]
    pub night_start: String,

    #[arg(long, default_value = "")]
    pub night_end: String,

    #[arg(long, default_value_t = 1)]
    pub night_level: u8, // 夜间亮度 (0-7)

    // [新增] 允许用户自定义按键 GPIO
    #[arg(long, default_value = "71")]
    pub button_gpio: String,

    // 🌟 [兼容性修复] GPIO 基址 (仅 sysfs 后端使用)
    // "auto" = 自动探测 gpiochip base (推荐，可兼容 QWRT/iStoreOS 等不同内核的固件)
    // 也可以直接填数字强制指定: "512" (内核6.1+) / "432" (内核5.x) / "0" (老内核静态分配)
    #[arg(long, default_value = "auto")]
    pub gpio_base: String,

    // 🌟 [长期兼容] GPIO 后端选择
    // "auto"  = 优先 /dev/gpiochipN 字符设备 (现代内核标准接口)，失败回退 sysfs
    // "cdev"  = 强制字符设备
    // "sysfs" = 强制老式 sysfs (内核已废弃，仅老固件用)
    #[arg(long, default_value = "auto")]
    pub gpio_backend: String,

    // [核心升级] 智能 Profile 数组！
    // 允许传入多个 --profile，比如：
    // --profile "time_group:time_sec#10 weather#10" --profile "netspeed_down:wan#5"
    #[arg(
        long = "profile",
        num_args = 1..,
        default_values = [
            "time_group:time_sec#10 weather#10", // 第 1 台：时间与天气
            "cpu#5 mem#5 temp_single:4#5",       // 第 2 台：系统监控 (默认看 CPU 温度)
            "traffic_split:br-lan#5 nic#5"       // 第 3 台：网络状态 (默认看 br-lan)
        ]
    )]
    pub profile: Vec<String>,

    // --- 网络与接口配置 ---
    #[arg(long, default_value = "br-lan")]
    pub net_interface: String,

    // --- 各个模块的专属配置 ---

    // 1. IP 查询接口
    #[arg(long, default_value = "http://members.3322.org/dyndns/getip")]
    pub ip_url: String,

    // 2. 自定义文本 (对应以前的 value)
    #[arg(long, default_value = "")]
    pub custom_text: String,

    // 3. 自定义 HTTP 内容获取 (对应以前的 url)
    #[arg(long, default_value = "")]
    pub custom_http_url: String,

    // [新增] HTTP 自定义 API 的缓存时间（默认 60 秒）
    #[arg(long, default_value_t = 60)]
    pub http_cache_secs: u64,

    // [新增] HTTP 结果截断长度
    #[arg(long, default_value_t = 15)]
    pub http_length: usize,

    #[arg(long, default_value = "auto")]
    pub weather_city: String,

    #[arg(long, default_value = "uapis")]
    pub weather_source: String,

    // 🌟 [安全] 不再内置公共测试 key (曾被滥用风险高)，用户需自行申请填入
    #[arg(long, default_value = "")]
    pub seniverse_key: String,

    // 5. 股票接口 (预留，建议用返回简单文本的 API)
    #[arg(long, default_value = "")]
    pub stock_url: String,

    #[arg(long, default_value = "4")]
    pub temp_flag: String, // 用于温度显示

    // --- 定时开关机 ---
    #[arg(long, default_value = "")]
    pub sleep_start: String,

    #[arg(long, default_value = "")]
    pub sleep_end: String,

    #[arg(long, default_value = "simple")]
    pub weather_format: String,

    // [新增] 4 盏全局状态指示灯的独立开关
    #[arg(long)]
    pub disable_led_clock: bool, // 禁用时钟灯 (CPU)

    #[arg(long)]
    pub disable_led_medal: bool, // 禁用奖牌灯 (连通性)

    #[arg(long)]
    pub disable_led_up: bool,    // 禁用上箭头 (上传)

    #[arg(long)]
    pub disable_led_down: bool,  // 禁用下箭头 (下载)

    // ==========================================
    // 🌟 [v2.4.0 新增]
    // ==========================================
    // 温度告警阈值 (°C)，0 = 关闭。超过阈值时插播闪烁警示 (3°C 滞回)
    #[arg(long, default_value_t = 0)]
    pub temp_alert: u32,

    // 温度告警监控的传感器 ID (thermal_zone 编号，AX6600 CPU = 4)
    #[arg(long, default_value = "4")]
    pub temp_alert_sensor: String,

    // 运行时控制接口端口 (监听 127.0.0.1，0 = 关闭)
    // 用法: echo "show 10 HELLO" | nc 127.0.0.1 <端口>
    #[arg(long, default_value_t = 0)]
    pub control_port: u16,

    // MQTT 集成 (broker 为空 = 关闭)。配合 "mqtt" 显示模块使用
    #[arg(long, default_value = "")]
    pub mqtt_broker: String, // host 或 host:port (默认 1883)

    #[arg(long, default_value = "athena-led/display")]
    pub mqtt_topic: String,

    #[arg(long, default_value = "")]
    pub mqtt_user: String,

    #[arg(long, default_value = "")]
    pub mqtt_pass: String,
}

// ==========================================
// [终极时区修复] 直接读取 OpenWrt 底层配置，免装 zoneinfo 包
// ==========================================
fn set_timezone_from_config() -> Result<()> {
    if let Ok(content) = std::fs::read_to_string("/etc/config/system") {
        for line in content.lines() {
            // 匹配 option timezone 'CST-8' 这种格式
            if line.contains("option timezone") {
                let parts: Vec<&str> = line.split('\'').collect();
                if parts.len() >= 3 {
                    let tz_str = parts[1];
                    std::env::set_var("TZ", tz_str);
                    return Ok(());
                }
            }
        }
    }
    // 如果没读到，默认兜底使用北京时间
    std::env::set_var("TZ", "CST-8");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 生成 PID 文件
    let pid = std::process::id();
    if let Err(e) = std::fs::write("/var/run/athena-led.pid", pid.to_string()) {
        println!("⚠️ [警告] 无法写入 PID 文件: {}", e);
    } else {
        println!("📝 [系统] 进程 PID ({}) 已写入 /var/run/athena-led.pid", pid);
    }

    let _ = set_timezone_from_config();
    let args = Args::parse();

    // ==========================================
    // 🌟 优雅退出的核心开关（有且只能有这一组！）
    // ==========================================
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running_for_listener = std::sync::Arc::clone(&running);

    // ==========================================
    // 🌟 [双后端] 初始化屏幕
    // AX6600 屏幕在主控 TLMM 上的硬件引脚偏移固定为:
    //   STB左=69, STB右=70, CLK=73, DIO=74 (按键=71)
    // 后端策略与 base 换算全部在 led_screen 内部处理:
    //   auto -> 优先 /dev/gpiochipN 字符设备，失败回退 sysfs (base 自动探测)
    // ==========================================
    let mut screen = led_screen::LedScreen::new(&args.gpio_backend, &args.gpio_base)
        .context("Failed to init screen")?;
    screen.power(true, args.light_level)?;

    // 初始化本地系统监控 (纯 /proc、/sys 读取，不会失败)
    let mut monitor = monitor::SystemMonitor::new(args.net_interface.clone());

    // 🌟 [v2.3.1] 启动后台网络代理：天气/IP/股票/HTTP/延迟全部后台刷新，
    // 渲染循环只读快照，彻底告别"缓存过期瞬间屏幕冻结 30 秒"
    let net = net_agent::spawn_net_agent(args.clone());

    // 🌟 [v2.4.0] MQTT 集成 (broker 未配置时零开销)
    let mqtt = mqtt::spawn_mqtt(&args.mqtt_broker, &args.mqtt_topic, &args.mqtt_user, &args.mqtt_pass);

    // 初始化通信频道
    let (tx, mut rx) = tokio::sync::watch::channel(1i32);

    // 🌟 [v2.4.0] 共享控制状态 (按键双击 / 控制接口共用)
    let control_state = control::new_shared();

    // 🌟 [v2.4.0] 运行时控制接口 (port=0 时不启动)
    control::spawn_control_server(args.control_port, tx.clone(), std::sync::Arc::clone(&control_state));

    // ==========================================
    // 🌟 启动监听器（有且只能调用一次！）
    // ==========================================
    button::spawn_button_listener(
        tx.clone(),
        running_for_listener,
        args.button_gpio.clone(),
        args.gpio_base.clone(),
        std::sync::Arc::clone(&control_state),
    );

    loop {
        tokio::select! {
            // 赛道 1：监听 Ctrl+C (你在电脑或 SSH 里手动调试时触发)
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 收到 Ctrl+C 信号，准备关屏退出...");
                break; // 跳出循环，去执行下面的统一收尾代码
            },

            // 赛道 2：🌟 监听 OpenWrt 停止服务发出的 SIGTERM 信号
            _ = async {
                #[cfg(unix)]
                {
                    if let Ok(mut sigterm) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                        sigterm.recv().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                }
                #[cfg(not(unix))]
                std::future::pending::<()>().await;
            } => {
                println!("🛑 收到 OpenWrt 停止服务信号 (SIGTERM)，准备关屏退出...");
                break; // 跳出循环，去执行下面的统一收尾代码
            },

            // 赛道 3：进入超级调度引擎 (死循环)
            _ = scheduler::process_loop(&mut screen, &args, &mut monitor, &net, &mqtt, &control_state, &mut rx) => {
                // 如果 process_loop 意外崩溃退出了，就在这里打个日志，然后重新进入下一次 loop 恢复运行
                println!("⚠️ [警告] 渲染引擎意外退出，准备自动重启渲染循环...");
            },
        }
    }

    // ==========================================
    // 🧹 统一的优雅退出收尾工作 (跳出 loop 后一定会执行这里)
    // ==========================================
    // 1. 告诉后台监听线程该下班了
    running.store(false, std::sync::atomic::Ordering::SeqCst);

    // 2. 关屏逻辑：清空残影，彻底断电
    let _ = screen.write_data(b"        ", 0).await;
    let _ = screen.power(false, 0);

    // 3. 稍微等一下（100ms），给后台线程“跳出循环并关闭文件”的时间
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // 4. 删掉 PID 文件（保持系统整洁）
    let _ = std::fs::remove_file("/var/run/athena-led.pid");

    println!("👋 [系统] Athena LED 服务已安全关闭。");
    Ok(())
}
