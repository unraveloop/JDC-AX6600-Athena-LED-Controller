#[cfg(unix)]
mod led_screen;
#[cfg(unix)]
mod char_dict;

// --------------------------------------------------------
// 2. 如果是在 Windows 下本地测试，加载“终端虚拟屏幕”
// --------------------------------------------------------
#[cfg(not(unix))]
pub mod led_screen {
    use anyhow::Result;
    
    pub struct LedScreen {}
    
    impl LedScreen {
        // 模拟屏幕初始化
        pub fn new(_: u32, _: u32, _: u32, _: u32) -> Result<Self> {
            println!("========================================");
            println!("💻 [Windows 模拟环境] 虚拟 LED 屏幕已启动");
            println!("========================================");
            Ok(Self {})
        }
        
        // 模拟电源和亮度控制
        pub fn power(&mut self, on: bool, level: u8) -> Result<()> {
            if on {
                println!("💡 [虚拟屏幕] 屏幕点亮 (亮度: {})", level);
            } else {
                println!("💤 [虚拟屏幕] 屏幕彻底熄灭");
            }
            Ok(())
        }
        
        // 模拟屏幕数据写入
        pub async fn write_data(&mut self, data: &[u8], flag: u8) -> Result<()> {
            // 把字节转回字符串
            let text = String::from_utf8_lossy(data);
            println!("📺 [屏幕输出 | 状态灯:{}] => {}", flag, text);
            Ok(())
        }
        pub async fn play_animation(&mut self, file_name: &str, duration_secs: u64, status: u8) -> Result<()> {
            println!("🎬 [虚拟屏幕 | 状态灯:{}] 开始模拟播放动画: {} (时长: {}秒)", status, file_name, duration_secs);
            
            // 模拟动画播放时的耗时，让本地测试时的控制台也能像真机一样停顿
            tokio::time::sleep(std::time::Duration::from_secs(duration_secs)).await;
            
            println!("✅ [虚拟屏幕] 动画 {} 播放结束", file_name);
            Ok(())
        }
    }
}



use anyhow::{Context, Result};
use clap::Parser;           
use std::fs;
use std::time::{Duration, Instant};
// use tokio::signal::unix::{signal, SignalKind}; 
use chrono::{Local, NaiveTime};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use regex::Regex;
use std::collections::HashMap;



// --- [新] uapis.cn 天气结构体 ---
#[derive(Deserialize, Debug)]
struct WeatherResponse {
    // 天气现象 (例如: "多云", "晴", "小雨")
    weather: String, 
    // 当前温度
    temperature: f64,
    // 最高温 (仅 forecast=true 时返回)
    #[serde(default)] 
    temp_max: Option<f64>,
    // 最低温 (仅 forecast=true 时返回)
    #[serde(default)]
    temp_min: Option<f64>,
}

// --- [新增] 心知天气结构体 ---
#[derive(Deserialize, Debug)]
struct SeniverseResponse {
    results: Vec<SeniverseResult>,
}
#[derive(Deserialize, Debug)]
struct SeniverseResult {
    daily: Vec<SeniverseDaily>,
}
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct SeniverseLocation {

}
#[derive(Deserialize, Debug)]
struct SeniverseDaily {
    high: String, 
    low: String,
    code_day: String, 
}

#[derive(Deserialize, Debug)]
struct WttrResult {
    current_condition: Vec<WttrCurrent>,
    weather: Vec<WttrDaily>, 
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)] 
struct WttrCurrent {
    temp_C: String, 
    weatherDesc: Vec<WttrValue>,
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct WttrDaily {
    maxtempC: String,
    mintempC: String,
}

#[derive(Deserialize, Debug)]
struct WttrValue {
    value: String,
}

// --- [最简版] Open-Meteo 结构体 ---
#[derive(Deserialize, Debug)]
struct OmGeoResponse {
    results: Option<Vec<OmLocation>>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct OmLocation {
    name: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize, Debug)]
struct OmWeatherResponse {
    current_weather: OmCurrentWeather,
}

#[derive(Deserialize, Debug)]
struct OmCurrentWeather {
    temperature: f64,
    weathercode: u8,
}


// --- [核心] 全能系统监控器 ---
// 这个结构体负责保存所有需要“记忆”的数据，比如上一次的流量、上一次的CPU快照
struct SystemMonitor {
    net_interface: String,
    http_client: Client,
    
    // 网络流量记录
    // --- 【换成这 1 个】 ---
    // 🌟 [终极升级] 独立记忆每个网卡的 (rx_bytes, tx_bytes, last_time)
    net_speed_cache: HashMap<String, (u64, u64, std::time::Instant)>,
    
    // CPU 记录
    last_cpu_total: u64,
    last_cpu_idle: u64,

    // [新增] 必须补上这个字段，否则后面代码找不到它
    last_stock_price: f64, 

    // [新增] 缓存字段
    cached_weather: String,      // 存天气文字
    last_weather_time: Instant,  // 上次查天气的时间
    
    cached_ip: String,           // 存 IP 文字
    last_ip_time: Instant,       // 上次查 IP 的时间

    // [新增] HTTP 请求缓存
    http_cache_text: String,
    http_cache_time: Instant,
    //天气api获取定位城市
    auto_location: String,

    // [新增] 4 盏 LED 用的独立状态引擎变量
    led_last_rx: u64,
    led_last_tx: u64,
    led_last_time: Instant,
    led_rx_speed: f64,
    led_tx_speed: f64,
    
    led_clock_state: bool,
    led_clock_timer: Instant,
    led_medal_state: bool,
    led_medal_timer: Instant,
    led_up_state: bool,
    led_up_timer: Instant,
    led_down_state: bool,
    led_down_timer: Instant,


    led_last_cpu_total: u64,
    led_last_cpu_idle: u64,
    led_cpu_usage: f64,
}

impl SystemMonitor {
    
    // [修复版] 构造函数
    // 注意：返回值改为 Result<Self> 以支持 anyhow 的 ? 操作符
    // [修复版] 构造函数：合并了 HTTP/缓存 和 CPU/网络计数器
    fn new(net_dev: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Athena-LED Router)")
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http_client: client,
            net_interface: net_dev,

            // [修改点] 删除了减去 24 小时的代码，直接用当前时间
            // 因为内容包含 "Wait" 或 "Err"，后续缓存检查会自动放行并立刻去请求网络
            cached_weather: "Wait...".to_string(),
            last_weather_time: Instant::now(), 
            
            cached_ip: "IP:Err".to_string(),
            last_ip_time: Instant::now(),

            // 让初始时间回到过去，确保第一次必定请求
            http_cache_text: String::new(),
            http_cache_time: Instant::now(),


            net_speed_cache: HashMap::new(),
            
            last_cpu_total: 0,
            last_cpu_idle: 0,
            
            last_stock_price: 0.0,
            // [新增] 初始化 LED 引擎
            led_last_rx: 0,
            led_last_tx: 0,
            led_last_time: Instant::now(),
            led_rx_speed: 0.0,
            led_tx_speed: 0.0,
            
            led_clock_state: false,
            led_clock_timer: Instant::now(),
            led_medal_state: false,
            led_medal_timer: Instant::now(),
            led_up_state: false,
            led_up_timer: Instant::now(),
            led_down_state: false,
            led_down_timer: Instant::now(),

            // [新增] 自动定位缓存，避免频繁请求 IP 接口
            auto_location: String::new(),


            led_last_cpu_total: 0,
            led_last_cpu_idle: 0,
            led_cpu_usage:  0.0,
        })

        
    }

    // ==========================================
    // [灯光大脑 1] 计算 CPU 精准负载 (用于时钟灯)
    // ==========================================
    fn get_cpu_usage_f64(&mut self) -> f64 {
        let (curr_total, curr_idle) = self.read_cpu_stats();
        let diff_total = curr_total.saturating_sub(self.last_cpu_total);
        let diff_idle = curr_idle.saturating_sub(self.last_cpu_idle);
        self.last_cpu_total = curr_total;
        self.last_cpu_idle = curr_idle;
        if diff_total == 0 { return 0.0; }
        100.0 * (1.0 - (diff_idle as f64 / diff_total as f64))
    }

    // ==========================================
    // [灯光大脑 2] 零流量底层路由探测 (用于奖牌灯)
    // ==========================================
    fn check_default_route(&self) -> bool {
        if let Ok(content) = fs::read_to_string("/proc/net/route") {
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // Destination 为 00000000 代表存在默认网关
                if parts.len() > 1 && parts[1] == "00000000" { return true; }
            }
        }
        false
    }

    // ==========================================
    // [灯光大脑 3] 综合输出当前 100ms 的 4 灯状态
    // ==========================================
// ==========================================
    // [灯光大脑 3] 综合输出当前 100ms 的 4 灯状态 (状态隔离版)
    // ==========================================
    pub fn get_global_led_flag(&mut self) -> u8 {
        let mut flag = 0;
        let now = Instant::now();

        // 🔄 核心修复：每 0.25 秒统一刷新一次底层硬件数据 (网络 + CPU)
        let net_duration = now.duration_since(self.led_last_time).as_secs_f64();
        if net_duration >= 0.25 {
            // 1. 独立计算网速 (防止减法溢出)
            let (curr_rx, curr_tx) = self.read_net_bytes_for(&self.net_interface);
            if self.led_last_rx > 0 && curr_rx >= self.led_last_rx {
                self.led_rx_speed = (curr_rx - self.led_last_rx) as f64 / net_duration;
                self.led_tx_speed = (curr_tx - self.led_last_tx) as f64 / net_duration;
            } else {
                self.led_rx_speed = 0.0;
                self.led_tx_speed = 0.0;
            }
            self.led_last_rx = curr_rx;
            self.led_last_tx = curr_tx;
            
            // 2. 🌟 独立计算 CPU (不再和屏幕显示的 CPU 抢夺数据！)
            let (curr_cpu_total, curr_cpu_idle) = self.read_cpu_stats();
            let diff_total = curr_cpu_total.saturating_sub(self.led_last_cpu_total);
            let diff_idle = curr_cpu_idle.saturating_sub(self.led_last_cpu_idle);
            self.led_last_cpu_total = curr_cpu_total;
            self.led_last_cpu_idle = curr_cpu_idle;
            if diff_total > 0 {
                self.led_cpu_usage = 100.0 * (1.0 - (diff_idle as f64 / diff_total as f64));
            }
            
            self.led_last_time = now;
        }

        // 🕒 1. 时钟灯 (Bit 0, Val 1): 绑定专属 CPU 负载
        let cpu_interval = (1000.0 - (self.led_cpu_usage * 8.0)).max(100.0) as u128;
        if now.duration_since(self.led_clock_timer).as_millis() > cpu_interval {
            self.led_clock_state = !self.led_clock_state;
            self.led_clock_timer = now;
        }
        if self.led_clock_state { flag |= 1; }

        // 🏅 2. 奖牌灯 (Bit 1, Val 2): 绑定路由连通性 (每2秒查一次)
        if now.duration_since(self.led_medal_timer).as_secs() >= 2 {
            self.led_medal_state = self.check_default_route();
            self.led_medal_timer = now;
        }
        if self.led_medal_state { flag |= 2; }

        // ⬆️ 3. 上箭头 (Bit 2, Val 4): 绑定上传速度
        if self.led_tx_speed > 10240.0 { 
            let speed_ratio = (self.led_tx_speed / 10_485_760.0).min(1.0);
            let tx_interval = (800.0 - (speed_ratio * 700.0)) as u128;
            if now.duration_since(self.led_up_timer).as_millis() > tx_interval {
                self.led_up_state = !self.led_up_state;
                self.led_up_timer = now;
            }
            if self.led_up_state { flag |= 4; }
        } else { 
            self.led_up_state = false; 
        }

        // ⬇️ 4. 下箭头 (Bit 3, Val 8): 绑定下载速度
        if self.led_rx_speed > 10240.0 { 
            let speed_ratio = (self.led_rx_speed / 10_485_760.0).min(1.0);
            let rx_interval = (800.0 - (speed_ratio * 700.0)) as u128;
            if now.duration_since(self.led_down_timer).as_millis() > rx_interval {
                self.led_down_state = !self.led_down_state;
                self.led_down_timer = now;
            }
            if self.led_down_state { flag |= 8; }
        } else { 
            self.led_down_state = false; 
        }

        flag
    }
    
    


    // --- 底层读取函数 ---
    


    fn get_animated_icon(&self, static_icon: &str, frame_toggle: bool) -> String {
        match static_icon {
            // 1. 晴天 ☀ -> ☀ / ☼ (旋转)
            "☀" => if frame_toggle { "☀".to_string() } else { "☼".to_string() },
            
            // 2. 下雨 ☂ -> ☂ / ☔ (下落)
            "☂" => if frame_toggle { "☂".to_string() } else { "☔".to_string() },
            
            // 3. 多云 ☁ -> ☁ / 🌥 (飘动)
            "☁" => if frame_toggle { "☁".to_string() } else { "🌥".to_string() },
            
            // 4. 雪 ❄ -> ❄ / ❅ (飘落)
            "❄" => if frame_toggle { "❄".to_string() } else { "❅".to_string() },
            
            // 5. 雷 ⚡ -> ⚡ / ☇ (闪烁)
            "⚡" => if frame_toggle { "⚡".to_string() } else { "☇".to_string() },
            
            // 6. 雾 🌫 -> 保持静态 (或者你可以加动画)
            "🌫" => "🌫".to_string(),

            // 其他未定义图标，直接原样返回，不闪烁
            _ => static_icon.to_string(),
        }
    }



    // 读取 /proc/stat 获取 CPU 数据
    fn read_cpu_stats(&self) -> (u64, u64) {
        let content = fs::read_to_string("/proc/stat").unwrap_or_default();
        if let Some(line) = content.lines().next() { // 第一行通常是 total cpu
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // parts[0] is "cpu"
                // parts[1]..parts[4] = user, nice, system, idle
                // parts[5].. = iowait, irq, softirq, etc.
                if parts.len() >= 5 {
                    let user: u64 = parts[1].parse().unwrap_or(0);
                    let nice: u64 = parts[2].parse().unwrap_or(0);
                    let system: u64 = parts[3].parse().unwrap_or(0);
                    let idle: u64 = parts[4].parse().unwrap_or(0);
                    let iowait: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let irq: u64 = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
                    let softirq: u64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
                    
                    let total = user + nice + system + idle + iowait + irq + softirq;
                    return (total, idle);
                }
            }
        }
        (0, 0)
    }

  
    //网络部分
    // ==========================================
    // [终极网速修复] 完美适配所有网卡动态切换
    // ==========================================
    // 🌟 [修改点] 接收指定的网卡名 target_iface
    fn read_net_bytes_for(&self, target_iface: &str) -> (u64, u64) {
        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines() {
                if line.contains(target_iface) {
                    if let Some((_, data)) = line.split_once(':') {
                        let parts: Vec<&str> = data.split_whitespace().collect();
                        if parts.len() >= 9 {
                            let rx = parts[0].parse::<u64>().unwrap_or(0);
                            let tx = parts[8].parse::<u64>().unwrap_or(0);
                            return (rx, tx);
                        }
                    }
                }
            }
        }
        (0, 0)
    }


    // 🌟 获取实时网速 (V2.0 终极版：支持多网卡同时、独立测速)
    pub fn get_speed_string_for(&mut self, mode: u8, target_iface: &str) -> String {
        let (curr_rx, curr_tx) = self.read_net_bytes_for(target_iface);
        let now = Instant::now();

        // 🌟 核心魔法：去字典里拿这个专属网卡的数据。如果没拿过（第一次），就存入当前数据
        let (last_rx, last_tx, last_time) = self.net_speed_cache
            .entry(target_iface.to_string())
            .or_insert((curr_rx, curr_tx, now));

        let duration = now.duration_since(*last_time).as_secs_f64();

        // 防抖与异常防护
        if duration < 0.1 || duration > 30.0 || *last_rx == 0 {
            // 更新记忆，返回 0
            self.net_speed_cache.insert(target_iface.to_string(), (curr_rx, curr_tx, now));
            return format_bytes_speed(0.0);
        }

        // 独立计算该网卡的网速
        let speed = if mode == 0 {
            (curr_rx.saturating_sub(*last_rx)) as f64 / duration
        } else {
            (curr_tx.saturating_sub(*last_tx)) as f64 / duration
        };

        // 更新这块网卡的专属记忆
        self.net_speed_cache.insert(target_iface.to_string(), (curr_rx, curr_tx, now));

        format_bytes_speed(speed)
    }

    // --- 各类累计流量的动态查询 ---
    pub fn get_total_rx_string_for(&self, target_iface: &str) -> String {
        let (curr_rx, _) = self.read_net_bytes_for(target_iface);
        format!("TD:{}", format_bytes_total(curr_rx))
    }

    pub fn get_total_tx_string_for(&self, target_iface: &str) -> String {
        let (_, curr_tx) = self.read_net_bytes_for(target_iface);
        format!("TU:{}", format_bytes_total(curr_tx))
    }

    pub fn get_traffic_total_string_for(&self, target_iface: &str) -> String {
        let (curr_rx, curr_tx) = self.read_net_bytes_for(target_iface);
        format!("T:{}", format_bytes_total(curr_rx + curr_tx))
    }

    pub fn get_total_traffic_for(&self, target_iface: &str) -> String {
        let (rx, tx) = self.read_net_bytes_for(target_iface); 
        let format_bytes = |bytes: u64| -> String {
            if bytes > 1024 * 1024 * 1024 {
                format!("{:.1}G", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
            } else {
                format!("{:.0}M", bytes as f64 / 1024.0 / 1024.0)
            }
        };
        format!("T:{}/{}", format_bytes(rx), format_bytes(tx))
    }

    // --- [新增] 获取上下行同显实时网速 (updl) ---
    // 利用后台 LED 引擎的高频测速数据，不干扰原有的独立模块计数器
    pub fn get_updl_string(&self) -> String {
        let short_fmt = |s: f64| -> String {
            if s >= 1_048_576.0 { format!("{:.1}M", s / 1_048_576.0) } 
            else if s >= 1024.0 { format!("{:.0}K", s / 1024.0) } 
            else { format!("{:.0}B", s) }
        };
        // 返回格式: 1.2M/500K (上行在前，下行在后)
        format!("{}/{}", short_fmt(self.led_tx_speed), short_fmt(self.led_rx_speed))
    }


    // --- [新增] 物理网口链路状态 (nic) 自适应版 ---
    pub fn get_nic_status(&self) -> String {
        // 定义两套最常见的网口命名体系
        let dsa_interfaces = ["wan", "lan1", "lan2", "lan3", "lan4"];
        let legacy_interfaces = ["eth0", "eth1", "eth2", "eth3", "eth4"];
        
        // 动态嗅探：如果系统中存在 wan 或 lan1，就判定为新款 DSA 架构
        let is_dsa = std::path::Path::new("/sys/class/net/wan").exists() || 
                     std::path::Path::new("/sys/class/net/lan1").exists();
                     
        let target_interfaces = if is_dsa { &dsa_interfaces } else { &legacy_interfaces };
        
        let mut result = String::new();
        
        for iface in target_interfaces {
            // 💡 极客细节：使用 carrier 来判断底层物理连接，比 operstate 准确 100 倍
            let carrier_path = format!("/sys/class/net/{}/carrier", iface);
            let speed_path = format!("/sys/class/net/{}/speed", iface);
            
            // 如果 carrier 不是 1（比如文件不存在、值为 0），说明没插网线
            if std::fs::read_to_string(&carrier_path).unwrap_or_default().trim() != "1" {
                result.push('O'); 
                continue;
            }
            
            // 读取协商速率
            match std::fs::read_to_string(&speed_path).unwrap_or_default().trim() {
                "10" => result.push('B'),    // Base (10M)
                "100" => result.push('H'),   // Hundred (100M)
                "1000" => result.push('G'),  // Gigabit (1000M)
                "2500" => result.push('S'),  // Super (2.5G)
                "10000" => result.push('T'), // Ten Gigabit (10G) - 万兆战未来
                _ => result.push('?'),       // 链路通了但速度未知
            }
        }
        
        if result.is_empty() { "NIC:Err".to_string() } else { result }
    }

    // 3. 获取 CPU 占用率
    fn get_cpu_usage_string(&mut self) -> String {
        let (curr_total, curr_idle) = self.read_cpu_stats();
        let diff_total = curr_total.saturating_sub(self.last_cpu_total);
        let diff_idle = curr_idle.saturating_sub(self.last_cpu_idle);
        
        self.last_cpu_total = curr_total;
        self.last_cpu_idle = curr_idle;

        if diff_total == 0 { return "CPU:-".to_string(); }
        
        let usage = 100.0 * (1.0 - (diff_idle as f64 / diff_total as f64));
        format!("C:{:.0}%", usage)
    }

    // --- [新增] 内存监控 (读取 /proc/meminfo) ---
    fn get_mem_string(&self) -> String {
        let content = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let mut total = 0.0;
        let mut available = 0.0;

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 { continue; }
            match parts[0] {
                "MemTotal:" => total = parts[1].parse().unwrap_or(0.0),
                "MemAvailable:" => available = parts[1].parse().unwrap_or(0.0),
                _ => {}
            }
        }

        if total > 0.0 {
            let usage_percent = 100.0 * (1.0 - (available / total));
            // 用 "M:" 代表 Memory (RAM)
            format!("M:{:.0}%", usage_percent)
        } else {
            "M:Err".to_string()
        }
    }

    // --- [新增] 负载监控 (读取 /proc/loadavg) ---
    fn get_load_string(&self) -> String {
        let content = fs::read_to_string("/proc/loadavg").unwrap_or_default();
        let parts: Vec<&str> = content.split_whitespace().collect();
        if !parts.is_empty() {
            // 只取第一个数 (1分钟负载)
            // 用 "L:" 代表 Load
            format!("L:{}", parts[0])
        } else {
            "L:Err".to_string()
        }
    }
    fn get_uptime_string(&self) -> String {
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(sec_str) = content.split_whitespace().next() {
                if let Ok(seconds) = sec_str.parse::<f64>() {
                    let secs = seconds as u64;
                    let days = secs / 86400;
                    let hours = (secs % 86400) / 3600;
                    let mins = (secs % 3600) / 60;

                    if days > 0 {
                        return format!("Up:{}d{}h", days, hours);
                    } else if hours > 0 {
                        return format!("Up:{}h{}m", hours, mins);
                    } else {
                        return format!("Up:{}m", mins);
                    }
                }
            }
        }
        "Up:Err".to_string()
    }

    // --- [LuCI 指定版] 根据 ID 列表读取温度 ---
    fn get_temps_by_ids(&self, ids: &str) -> String {
        let mut results = Vec::new();

        // 1. 分割 ID 字符串 (支持空格或逗号分隔)
        // LuCI 的 MultiValue 通常是用空格分隔的，比如 "0 4"
        let id_list: Vec<&str> = ids.split(|c| c == ' ' || c == ',')
                                    .filter(|s| !s.is_empty())
                                    .collect();

        for id in id_list {
            // 构造路径
            let type_path = format!("/sys/class/thermal/thermal_zone{}/type", id);
            let temp_path = format!("/sys/class/thermal/thermal_zone{}/temp", id);

            // 2. 读取名字 (用于显示标签，如 "cpu", "nss")
            if let Ok(type_name_raw) = fs::read_to_string(&type_path) {
                // 简化名字：去掉 "-thermal" 后缀，转小写
                let label = type_name_raw.trim().to_lowercase().replace("-thermal", "");
                
                // 3. 读取温度
                if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                    if let Ok(raw_temp) = temp_str.trim().parse::<f64>() {
                        // 标准化：OpenWrt 通常是毫摄氏度 (55000 -> 55)
                        // 有些特殊的可能是直接摄氏度 (55 -> 55)
                        let val = if raw_temp > 1000.0 { raw_temp / 1000.0 } else { raw_temp };
                        
                        // 格式化单个温度: "cpu:55C"
                        results.push(format!("{}:{:.0}℃", label, val));
                    }
                }
            }
        }

        if results.is_empty() {
            "Temp:--".to_string()
        } else {
            // 如果选了多个，用空格连接: "cpu:55C ddr:45C"
            results.join(" ")
        }
    }

    // ==========================================
    // [新增] 极简而精准的单体温度探针
    // ==========================================
    pub fn get_single_temp(&self, sensor_id: &str) -> String {
        // OpenWrt 规范：温度文件存放在 /sys/class/thermal/thermal_zoneX/temp
        let path = format!("/sys/class/thermal/thermal_zone{}/temp", sensor_id);
        
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(temp_millideg) = content.trim().parse::<f64>() {
                let temp_c = temp_millideg / 1000.0;
                
                // 自动分配高逼格前缀 (严格控制在 1~2 个字符，防止撑爆屏幕)
                let prefix = match sensor_id {
                    "0" => "N0", // NSS-Top
                    "1" => "N1", // NSS
                    "2" => "W0", // Wi-Fi PHY0 (2.4G/5G)
                    "3" => "W1", // Wi-Fi PHY1 (5G-Game)
                    "4" => "C",  // CPU
                    "5" => "L",  // LPASS
                    "6" => "D",  // DDR
                    _ => "?",
                };
                
                return format!("{}:{:.1}C", prefix, temp_c);
            }
        }
        "T:Err".to_string()
    }

    // ==========================================
    // [终极设备数修复] 直接数内核 ARP 活体记录
    // ==========================================
    pub fn get_online_devices(&self) -> String {
        let mut count = 0;
        if let Ok(content) = std::fs::read_to_string("/proc/net/arp") {
            // 跳过第一行表头
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // 确保有完整的 6 列，并且 MAC 地址不是 00:00:00:00:00:00
                if parts.len() >= 4 && parts[3] != "00:00:00:00:00:00" {
                    // 还可以判断 Flags 字段 (parts[2]) 是否包含 "0x2" 代表可达
                    if parts[2].contains("0x2") {
                        count += 1;
                    }
                }
            }
            return format!("DEV:{}", count);
        }
        "DEV:Err".to_string()
    }

    // --- [修改后] 通用 HTTP 文本获取 ---
    // 增加了 max_len 参数，并修复了 UTF-8 切片可能导致的崩溃问题
    pub async fn get_http_text(&mut self, url: &str, prefix: &str, max_len: usize, cache_secs: u64) -> String {
        if url.is_empty() {
            return String::new();
        }

        if self.http_cache_time.elapsed().as_secs() < cache_secs && !self.http_cache_text.is_empty() {
            return self.http_cache_text.clone();
        }
        
        // 设置超时，防止卡死
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap_or(self.http_client.clone());

        match client.get(url).send().await {
            Ok(resp) => {
                match resp.text().await {
                    Ok(text) => {
                        let clean_text = text.trim();
                        
                        // [关键修复] 使用 chars() 迭代器进行安全的字符截断
                        // 这样即使 max_len 设置为 5，遇到中文也能正确截取 5 个汉字，而不是 5 个字节
                        let truncated: String = clean_text.chars().take(max_len).collect();
                        
                        format!("{}{}", prefix, truncated)
                    }
                    Err(_) => format!("{}Err", prefix),
                }
            }
            Err(_) => format!("{}Wait", prefix),
        }
    }

    async fn get_public_ip(&mut self, ip_url: &str) -> String {
        // [缓存策略] IP 变化很少，缓存 60 分钟都可以
        if self.last_ip_time.elapsed() < Duration::from_secs(3600) {
            if !self.cached_ip.contains("Err") {
                return self.cached_ip.clone();
            }
        }

        // --- 真的去请求网络 ---
        // 注意：这里用参数里的 ip_url，不要用 self.ip_url 了（如果你之前存了的话）
        #[cfg(debug_assertions)]
        println!("DEBUG: Fetching IP from network..."); 
        
        let mut new_ip = "IP:Err".to_string();
        
        match self.http_client.get(ip_url).send().await {
            Ok(resp) => {
                if let Ok(text) = resp.text().await {
                    let re = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap();
                    if let Some(mat) = re.find(&text) {
                        new_ip = format!("IP:{}", mat.as_str());
                    }
                }
            }
            Err(e) => println!("IP Request error: {:?}", e),
        }

        // [更新缓存]
        if !new_ip.contains("Err") {
            self.cached_ip = new_ip.clone();
            self.last_ip_time = Instant::now();
        }

        new_ip
    }

    async fn get_stock_trend(&mut self, url: &str) -> (String, u8) {
        if url.is_empty() { return (String::new(), 0); }

        match self.http_client.get(url).send().await {
            Ok(resp) => {
                // 解析 JSON
                if let Ok(json_val) = resp.json::<Value>().await {
                    // 尝试找 price/last/close 字段
                    let price_opt = json_val["price"].as_f64()
                        .or_else(|| json_val["price"].as_str().and_then(|s| s.parse::<f64>().ok()))
                        .or_else(|| json_val["last"].as_f64())
                        .or_else(|| json_val["close"].as_f64());

                    if let Some(current_price) = price_opt {
                        // 【核心灯光逻辑】
                        // 默认亮 Bit 1 (值 2, 奖牌灯)，表示初始状态或持平
                        let mut flag = 2; 

                        if self.last_stock_price > 0.0 {
                            if current_price > self.last_stock_price {
                                flag = 4; // 涨 -> 亮 Bit 2 (上箭头)
                            } else if current_price < self.last_stock_price {
                                flag = 8; // 跌 -> 亮 Bit 3 (下箭头)
                            }
                        }

                        // 更新缓存
                        self.last_stock_price = current_price;
                        
                        // 屏幕只显示纯数字
                        let text = if current_price > 1000.0 {
                            format!("{:.0}", current_price) // 大数不显小数
                        } else {
                            format!("{:.2}", current_price) // 小数显2位
                        };
                        
                        return (text, flag);
                    }
                }
            }
            Err(_) => {}
        }
        ("Err".to_string(), 0)
    }


    // --- [入口] 统一智能天气接口 ---
    pub async fn get_smart_weather(&mut self, location: &str, source: &str, key: &str) -> String {
        // 1. [缓存检查]
        if self.last_weather_time.elapsed() < Duration::from_secs(1800) {
            if !self.cached_weather.contains("Err") && !self.cached_weather.contains("Wait") {
                return self.cached_weather.clone();
            }
        }

        // ==========================================
        // 🌟 [新增] 紫辰精准 IP 定位 (加固版)
        // ==========================================
        let mut target_location = location.to_string();
        if target_location.to_lowercase() == "auto" || target_location.is_empty() {
            if self.auto_location.is_empty() {
                // 1. 构建带伪装和超时的 Client（防止路由器无限卡死）
                let client = reqwest::Client::builder()
                    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Athena-LED/2.0")
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .unwrap_or_default();

                // 2. 发起请求并捕获错误
                match client.get("http://app.zichen.zone/api/geoip/api.php").send().await {
                    Ok(resp) => {
                        if let Ok(text) = resp.text().await {
                            #[cfg(debug_assertions)]
                            println!("🌍 [定位调试] 紫辰原始返回: {}", text); // 👈 关键！看这里到底返回了啥
                            
                            // 3. 提取 city
                            if let Some(city_part) = text.split("\"city\"").nth(1) {
                                if let Some(city) = city_part.split('"').nth(1) {
                                    let trimmed = city.trim().to_string();
                                    if !trimmed.is_empty() {
                                        self.auto_location = trimmed;
                                        #[cfg(debug_assertions)]
                                        println!("✅ [定位成功] 城市: {}", self.auto_location);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => println!("❌ [定位失败] 网络请求报错: {}", e),
                }
            }
            
            // 兜底策略
            target_location = if self.auto_location.is_empty() { 
                #[cfg(debug_assertions)]
                println!("⚠️ [定位兜底] 启用默认城市: 北京");
                "北京".to_string() 
            } else { 
                self.auto_location.clone() 
            };
        }
        // ==========================================
        // 🌟 [新增] 默认数据源接管
        // 如果用户没选数据源，或者填了 auto，直接强制使用 uapis
        let target_source = if source.is_empty() || source == "auto" { 
            "uapis" 
        } else { 
            source 
        };

        // 2. [网络请求] 根据源选择不同的函数
        // ⚠️ target_source 记得加 .as_str() 才能匹配
        let result = match target_source {
            "seniverse" => self.get_weather_from_seniverse(&target_location, key).await,
            "openmeteo" => self.get_weather_from_open_meteo(&target_location).await,
            "uapis" => self.get_weather_from_uapis(&target_location).await,
            "wttr" => self.get_weather_from_wttr(&target_location).await, // 👈 赶紧把信仰加回来！
            // 如果用户乱填了一个不存在的名字，兜底走 uapis
            _ => self.get_weather_from_uapis(&target_location).await, 
        };

        // 3. [更新缓存]
        if !result.contains("Err") && !result.contains("Wait") {
            self.cached_weather = result.clone();
            self.last_weather_time = Instant::now();
        }
        
        result
    }

    // --- [通道1] uapis.cn (适合国内，支持中文名) ---
    async fn get_weather_from_uapis(&self, city: &str) -> String {
        let url = format!("https://uapis.cn/api/v1/misc/weather?city={}&forecast=true", city);
        
        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(data) = resp.json::<WeatherResponse>().await {
                    let temp = data.temperature;
                    let max = data.temp_max.unwrap_or(temp); // 如果没返回最高温，就用当前温暂代
                    let min = data.temp_min.unwrap_or(temp);
                    
                    let desc = data.weather; 
                    let icon = if desc.contains("雨") { "☂" }
                    else if desc.contains("雪") { "❄" }
                    else if desc.contains("云") || desc.contains("阴") || desc.contains("雾") || desc.contains("霾") { "☁" }
                    else { "☀" };

                    return format!("{} {:.0}℃ {:.0}-{:.0}", icon, temp, min, max);
                }
            }
            Err(_) => {}
        }
        "W:Err(U)".to_string()
    }

    // --- [修复版] Wttr 天气获取 ---
    async fn get_weather_from_wttr(&self, city: &str) -> String {
        // format=j1 返回 JSON
        let url = format!("https://wttr.in/{}?format=j1", city);
        #[cfg(debug_assertions)]
        println!("DEBUG: Requesting Wttr: {}", url); // [调试]

        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                // 1. 检查 HTTP 状态码 (关键！wttr 经常封 IP 返回 429 或 503)
                if !resp.status().is_success() {
                    #[cfg(debug_assertions)]
                    println!("DEBUG: Wttr failed status: {}", resp.status());
                    return format!("W:Err({})", resp.status().as_u16());
                }

                // 2. 解析 JSON
                match resp.json::<WttrResult>().await {
                    Ok(json) => {
                        // 安全获取数据 (使用 first() 防止数组为空崩溃)
                        if let (Some(curr), Some(daily)) = (json.current_condition.first(), json.weather.first()) {
                            let temp = &curr.temp_C;
                            let max = &daily.maxtempC;
                            let min = &daily.mintempC;
                            
                            // 获取天气描述
                            let desc = curr.weatherDesc.first()
                                .map(|d| d.value.to_lowercase())
                                .unwrap_or_else(|| "unknown".to_string());

                            // 图标映射
                            let icon = if desc.contains("rain") || desc.contains("shower") || desc.contains("drizzle") { "☂" }
                            else if desc.contains("snow") || desc.contains("ice") || desc.contains("hail") { "❄" }
                            else if desc.contains("thunder") { "⚡" }
                            else if desc.contains("cloud") || desc.contains("overcast") { "☁" }
                            else if desc.contains("mist") || desc.contains("fog") { "🌫" }
                            else { "☀" };

                            // 返回: ☀ 25℃ 20-30
                            return format!("{} {}℃ {}-{}", icon, temp, min, max);
                        }
                        println!("DEBUG: Wttr JSON structure mismatch (empty arrays)");
                        "W:DataErr".to_string()
                    }
                    Err(e) => {
                        println!("DEBUG: Wttr JSON Parse Error: {:?}", e);
                        "W:JsonErr".to_string()
                    }
                }
            }
            Err(e) => {
                println!("DEBUG: Wttr Network Error: {:?}", e);
                "W:NetErr".to_string()
            }
        }
    }
    // --- [新增] 通道3: 心知天气 (直接支持城市名) ---
    async fn get_weather_from_seniverse(&self, location: &str, key: &str) -> String {
        // start=0&days=1 表示只查今天
        let url = format!(
            "https://api.seniverse.com/v3/weather/daily.json?key={}&location={}&language=en&unit=c&start=0&days=1",
            key, location
        );

        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<SeniverseResponse>().await {
                    if let Some(daily) = json.results.get(0).and_then(|r| r.daily.get(0)) {
                        // 解析温度 (字符串 -> f64)
                        let max = daily.high.parse::<f64>().unwrap_or(0.0);
                        let min = daily.low.parse::<f64>().unwrap_or(0.0);
                        // 算出当前大概温度 (取平均值，因为免费版日预报不返回实时温度，但够用了)
                        // 或者你可以再调一次 realtime 接口，但我觉得没必要浪费请求次数
                        let temp = (max + min) / 2.0;

                        // 解析图标代码
                        // 0-3: 晴, 4-9: 云, 10-19: 雨, 20-29: 雪
                        let code = daily.code_day.parse::<i32>().unwrap_or(99);
                        let icon = match code {
                            0..=3 => "☀",
                            4..=9 => "☁",
                            10..=19 => "☂",
                            20..=29 => "❄",
                            30..=36 => "☁", // 雾霾风
                            _ => "☀",
                        };

                        return format!("{} {:.0}℃ {:.0}-{:.0}", icon, temp, min, max);
                    }
                }
            }
            Err(_) => {}
        }
        "W:Err(S)".to_string()
    }

    // --- [最简版] Open-Meteo (只看当前) ---
    async fn get_weather_from_open_meteo(&self, city: &str) -> String {
        // Step 1: 查坐标
        let geo_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh&format=json",
            city
        );
        
        // 获取经纬度
        let (lat, lon) = match self.http_client.get(&geo_url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() { return "W:GeoErr".to_string(); }
                match resp.json::<OmGeoResponse>().await {
                    Ok(data) => {
                        if let Some(results) = data.results {
                            if let Some(loc) = results.first() {
                                (loc.latitude, loc.longitude)
                            } else { return "W:NoCity".to_string(); }
                        } else { return "W:NoCity".to_string(); }
                    }
                    Err(_) => return "W:GeoJson".to_string(),
                }
            }
            Err(_) => return "W:GeoNet".to_string(),
        };

        // Step 2: 查当前天气 (current_weather=true)
        let weather_url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
            lat, lon
        );

        match self.http_client.get(&weather_url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() { return "W:ApiErr".to_string(); }
                match resp.json::<OmWeatherResponse>().await {
                    Ok(data) => {
                        let temp = data.current_weather.temperature;
                        let code = data.current_weather.weathercode;

                        // 图标转换
                        let icon = match code {
                            0 => "☀", 
                            1 | 2 | 3 => "☁", 
                            45 | 48 => "🌫", 
                            51..=67 | 80..=82 => "☂", 
                            71..=77 | 85..=86 => "❄", 
                            95..=99 => "⚡", 
                            _ => "?",
                        };

                        // 返回格式: ☀ 26.5℃
                        return format!("{} {:.1}℃", icon, temp);
                    }
                    Err(_) => "W:JsonErr".to_string(),
                }
            }
            Err(_) => "W:NetErr".to_string(),
        }
    }

}

// 辅助格式化函数
fn format_bytes_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec > 1_048_576.0 {
        format!("{:.1}M", bytes_per_sec / 1_048_576.0)
    } else if bytes_per_sec > 1024.0 {
        format!("{:.0}K", bytes_per_sec / 1024.0)
    } else {
        format!("{:.0}B", bytes_per_sec)
    }
}

fn format_bytes_total(bytes: u64) -> String {
    let b = bytes as f64;
    if b > 1_099_511_627_776.0 { // 1TB
        format!("{:.2}T", b / 1_099_511_627_776.0)
    } else if b > 1_073_741_824.0 { // 1GB
        format!("{:.2}G", b / 1_073_741_824.0)
    } else if b > 1_048_576.0 { // 1MB
        format!("{:.1}M", b / 1_048_576.0)
    } else {
        format!("{:.0}K", b / 1024.0)
    }
}

fn get_seconds_until_wake(wake_time_str: &str) -> u64 {
    let now = Local::now();
    
    // 1. 解析目标唤醒时间
    let wake_time = match NaiveTime::parse_from_str(wake_time_str, "%H:%M") {
        Ok(t) => t,
        Err(_) => return 60, // 解析失败兜底
    };

    // 2. 构造今天的唤醒时间点 (安全处理夏令时/不存在的时间)
    let mut target_dt = match now.date_naive().and_time(wake_time).and_local_timezone(Local).latest() {
        Some(dt) => dt,
        None => return 60, // 如果遇到极其罕见的夏令时跳跃导致时间不存在，兜底睡 60 秒后重试
    };

    // 3. 如果唤醒时间比现在早 (比如现在23:00, 唤醒是07:00)，说明是"明天"
    if target_dt <= now {
        target_dt = target_dt + chrono::Duration::days(1);
    }

    // 4. 计算秒数差
    let duration = target_dt.signed_duration_since(now).num_seconds();
    
    // 5. 加上 2 秒缓冲，确保醒来时肯定过了时间点
    if duration > 0 {
        (duration as u64) + 2
    } else {
        60
    }
}

/// 判断当前时间是否在休眠区间内
/// 支持跨午夜设置，例如 start="23:00", end="07:00"
fn is_sleep_time(start_str: &str, end_str: &str) -> bool {
    // 1. 如果参数为空（LuCI未勾选），直接返回 false
    if start_str.is_empty() || end_str.is_empty() {
        return false;
    }

    // 2. 尝试解析时间
    let start = match NaiveTime::parse_from_str(start_str, "%H:%M") {
        Ok(t) => t,
        Err(_) => return false, // 格式错误当作不休眠
    };
    let end = match NaiveTime::parse_from_str(end_str, "%H:%M") {
        Ok(t) => t,
        Err(_) => return false,
    };

    let now = Local::now().time();

    // 3. 判断逻辑
    if start < end {
        // 同一天内：例如 12:00 睡 - 14:00 醒
        now >= start && now < end
    } else {
        // 跨午夜：例如 23:00 睡 - 07:00 醒
        // 当前时间比 23:00 晚，或者比 07:00 早
        now >= start || now < end
    }
}


// ==========================================
// [智能调度引擎] 专属配置结构 (V2.0 动态参数版)
// ==========================================
#[derive(Debug, Clone)]
struct ModuleConfig {
    name: String,
    param: String, // 🌟 新增：用于存放冒号后面的二级参数 (如 "wan", "time_sec", "4")
    duration: u64,
}

#[derive(Debug, Clone)]
struct ProfileConfig {
    modules: Vec<ModuleConfig>,
}

// --- 参数定义 ---
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    // --- 基础设置 ---
    #[arg(long, default_value_t = 5)]
    seconds: u64, // 每个模块显示的秒数

    #[arg(long, default_value_t = 5)]
    light_level: u8, // 亮度 (0-7)

    // [新增] 允许用户自定义按键 GPIO
    #[arg(long, default_value = "71")]
    pub button_gpio: String,


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
    profile: Vec<String>,

    // --- 网络与接口配置 ---
    #[arg(long, default_value = "br-lan")]
    net_interface: String,

    // --- 各个模块的专属配置 ---

    // 1. IP 查询接口
    #[arg(long, default_value = "http://members.3322.org/dyndns/getip")]
    ip_url: String,

    // 2. 自定义文本 (对应以前的 value)
    #[arg(long, default_value = "")]
    custom_text: String,

    // 3. 自定义 HTTP 内容获取 (对应以前的 url)
    #[arg(long, default_value = "")]
    custom_http_url: String,

    // [新增] HTTP 自定义 API 的缓存时间（默认 60 秒）
    #[arg(long, default_value_t = 60)]
    pub http_cache_secs: u64,

    // [新增] HTTP 结果截断长度
    #[arg(long, default_value_t = 15)]
    http_length: usize,


    #[arg(long, default_value = "auto")]
    pub weather_city: String,

    #[arg(long, default_value = "uapis")]
    pub weather_source: String,

    #[arg(long, default_value = "S140W1C6_1_8R8_8c")] 
    seniverse_key: String,

    // 5. 股票接口 (预留，建议用返回简单文本的 API)
    #[arg(long, default_value = "")]
    stock_url: String,

    #[arg(long, default_value = "4")]
    temp_flag: String, // 用于温度显示

    // --- 定时开关机 ---
    #[arg(long, default_value = "")]
    sleep_start: String,

    #[arg(long, default_value = "")]
    sleep_end: String,

    #[arg(long, default_value = "simple")]
    weather_format: String,

    // [新增] 4 盏全局状态指示灯的独立开关
    #[arg(long)]
    pub disable_led_clock: bool, // 禁用时钟灯 (CPU)
    
    #[arg(long)]
    pub disable_led_medal: bool, // 禁用奖牌灯 (连通性)
    
    #[arg(long)]
    pub disable_led_up: bool,    // 禁用上箭头 (上传)
    
    #[arg(long)]
    pub disable_led_down: bool,  // 禁用下箭头 (下载)
}

// ... 这里保留原来的 set_timezone_from_config 函数 ...
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
    
    // 初始化屏幕
    let mut screen = led_screen::LedScreen::new(581, 582, 585, 586)
        .context("Failed to init screen")?;
    screen.power(true, args.light_level)?;
    
    // 初始化系统监控
    let mut monitor = SystemMonitor::new(args.net_interface.clone())
        .context("Failed to initialize system monitor")?;
    
    // 初始化通信频道
    let (tx, mut rx) = tokio::sync::watch::channel(1i32);

    // ==========================================
    // 🌟 启动监听器（有且只能调用一次！）
    // ==========================================
    spawn_button_listener(tx.clone(), running_for_listener, args.button_gpio.clone());
    


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
            _ = process_loop(&mut screen, &args, &mut monitor, &mut rx) => {
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

    // 4. 删掉 PID 文件（强迫症福音，保持系统整洁）
    let _ = std::fs::remove_file("/var/run/athena-led.pid");
    
    println!("👋 [系统] Athena LED 服务已安全关闭。");
    Ok(())
}

// ==========================================
// 🐧 Linux 环境下的【长短按分离 + 睡眠感知】监听器
// ==========================================
#[cfg(unix)]
fn spawn_button_listener(
    tx: tokio::sync::watch::Sender<i32>, 
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    gpio_pin: String // 🌟 传进来！比如 "71"
    // 🌟 [可选] 如果你想让按键感知当前是否在休眠，可以传这个参数；
    // 也可以不传，全靠发送不同的 i32 信号让主线程去判断。
    // 这里我们假设直接通过 tx 发送特定的特殊值来通信。
) {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::time::{Duration, Instant};
    use std::sync::atomic::Ordering;

tokio::task::spawn_blocking(move || {
        // 🌟 打印日志也变成动态的！
        println!("🎮 [系统] 启动终极 GPIO{} 硬件雷达监听模式 (支持长短按分离)...", gpio_pin);

        let mut file = match File::open("/sys/kernel/debug/gpio") {
            Ok(f) => f,
            Err(e) => {
                println!("⚠️ [警告] 无法打开底层 GPIO 调试接口: {}", e);
                return;
            }
        };

        let mut buffer = String::with_capacity(4096);
        
        // 🌟 状态机变量
        let mut press_start: Option<Instant> = None;
        let mut long_press_handled = false;

        // 🌟 提前组装好要搜索的字符串，不用每次循环都 format，压榨极致性能！
        let search_target = format!("gpio{}  : in  low", gpio_pin);

        while running.load(Ordering::SeqCst) {
            buffer.clear();
            let _ = file.seek(SeekFrom::Start(0));

            if file.read_to_string(&mut buffer).is_ok() {
                // 🌟 核心修复：检查 buffer 里有没有我们刚刚组装好的那个目标字符串
                let is_pressed = buffer.contains(&search_target);

                if is_pressed {
                    // 1️⃣ 刚刚按下瞬间，记录时间点
                    if press_start.is_none() {
                        press_start = Some(Instant::now());
                        long_press_handled = false;
                    } 
                    // 2️⃣ 一直按着没松手，检查是否达到长按阈值 (比如 2 秒)
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
                        
                        // 如果没有触发过长按，并且按下的时间大于 50ms (防物理抖动)
                        if !long_press_handled && hold_time > Duration::from_millis(50) {
                            #[cfg(debug_assertions)]
                            println!("➡️ [硬件交互] 短按触发！准备切换频道...");
                            
                            let current = *tx.borrow();
                            // 如果当前处于休眠状态，短按直接唤醒，从 1 开始
                            if current < 0 {
                                println!("☀️ [硬件交互] 夜间休眠被打断，唤醒屏幕！");
                                let _ = tx.send(1);
                            } else {
                                // 正常切台
                                let _ = tx.send(current + 1);
                            }
                        }
                        
                        // 重置状态机，准备迎接下一次按键
                        press_start = None;
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
// 🌟 增加第二个参数声明，哪怕不用它，也要让签名保持一致
fn spawn_button_listener(
    _tx: tokio::sync::watch::Sender<i32>, 
    _running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _gpio_pin: String
) {
    // Windows 模拟器不需要物理按键监听，所以这里保持空或者加行打印
    println!("📺 [Windows 模拟器] 按键监听已就绪（空跑模式）");
}



// 🌟 完美去掉了 pub，解决了可见性报错！
async fn process_loop(
    screen: &mut led_screen::LedScreen, 
    args: &Args, 
    monitor: &mut SystemMonitor,
    rx: &mut tokio::sync::watch::Receiver<i32> 
) -> Result<()> {
    
    // --- 1. 动态解析用户的智能配置 ---
    let mut profiles: Vec<ProfileConfig> = Vec::new();
    let default_profiles = vec!["date timeBlink weather stock uptime netspeed_down netspeed_up cpu".to_string()];
    let profile_args = if args.profile.is_empty() { &default_profiles } else { &args.profile };

    for p_str in profile_args {
        let mut modules = Vec::new();
        for m_str in p_str.split_whitespace() {
            // 1. 先用 '#' 切割，分离出【模块主体(含参数)】和【时长】
            let parts: Vec<&str> = m_str.split('#').collect();
            let name_with_param = parts[0];
            
            // 2. 提取时长
            let duration = if parts.len() > 1 {
                parts[1].parse::<u64>().unwrap_or(args.seconds)
            } else {
                args.seconds
            };

            // 🌟 3. 核心修复：用 ':' 切割主体，同时生成 name 和 param！
            let (name, param) = match name_with_param.split_once(':') {
                Some((n, p)) => (n.to_string(), p.to_string()),
                None => (name_with_param.to_string(), String::new()), // 没冒号就给 param 塞个空字符串
            };

            // 4. 组装发车！现在 name 和 param 都实打实存在了
            modules.push(ModuleConfig { name, param, duration });
        }
        if !modules.is_empty() { profiles.push(ProfileConfig { modules }); }
    }

    let profiles_count = profiles.len();
    let mut current_profile_idx = 0;
    
    // 🌟 [新增] 夜间被按键唤醒后的“临时免死金牌”时间
    let mut manual_wake_expire: Option<std::time::Instant> = None;

    // --- 2. 状态机死循环 ---
    loop {
        // 🌟 [处理长按息屏] (由监听器发送 -1 触发)
        if *rx.borrow() < 0 {
            let _ = screen.write_data(b"        ", 0).await; 
            screen.power(false, 0).unwrap_or_default(); 
            // 陷入沉睡，直到监听到大于 0 的短按唤醒信号
            let _ = rx.wait_for(|&val| val > 0).await; 
            screen.power(true, args.light_level).unwrap_or_default(); 
            continue; 
        }

        // 🌟 [新增] 判断当前是否处于“临时唤醒”保护期
        let is_manual_awake = manual_wake_expire.map_or(false, |exp| exp > std::time::Instant::now());

        // 🌟 [处理夜间休眠] (仅在保护期外，且满足时间时才休眠)
        if !is_manual_awake && is_sleep_time(&args.sleep_start, &args.sleep_end) {
            let _ = screen.write_data(b"        ", 0).await; 
            screen.power(false, 0).unwrap_or_default(); 
            let sleep_sec = get_seconds_until_wake(&args.sleep_end);
            
            tokio::select! {
                // 1. 正常睡到天亮自动醒
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(sleep_sec)) => {
                    screen.power(true, args.light_level).unwrap_or_default();
                    continue; 
                }
                // 2. 半夜被起夜的用户按了按钮
                Ok(_) = rx.changed() => {
                    // 赋予 60 秒免死金牌，这 60 秒内正常轮播配置
                    manual_wake_expire = Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
                    screen.power(true, args.light_level).unwrap_or_default(); 
                    continue; 
                }
            }
        }

        let profile = &profiles[current_profile_idx];
        let mut module_idx = 0;

// --- 3. 模块级渲染与打断 (纯净版：专注于 4 盏灯改造) ---
        while module_idx < profile.modules.len() {
            let module = &profile.modules[module_idx];
            let mut text_to_show = String::new();
            let mut module_interrupted = false;

            // 💡 [新增]：全局灯光掩码过滤器
            let get_leds = |monitor: &mut SystemMonitor, args: &Args| -> u8 {
                let mut raw_flag = monitor.get_global_led_flag();
                if args.disable_led_clock { raw_flag &= !1; } // 1: 时钟
                if args.disable_led_medal { raw_flag &= !2; } // 2: 奖牌
                if args.disable_led_up    { raw_flag &= !4; } // 4: 上箭头
                if args.disable_led_down  { raw_flag &= !8; } // 8: 下箭头
                raw_flag
            };

            // 提取静态文本（注意：我把你原代码里的 current_flag |= xx 全删了！）
            match module.name.as_str() {
                "uptime" => text_to_show = monitor.get_uptime_string(),
                "cpu" => text_to_show = monitor.get_cpu_usage_string(),
                "mem" => text_to_show = monitor.get_mem_string(),
                "load" => text_to_show = monitor.get_load_string(),

                "temp" => text_to_show = monitor.get_temps_by_ids(&args.temp_flag),
                // 这是本次新增的单体温度专属通道：
                "temp_single" => {
                    let sensor_id = if module.param.is_empty() { "4" } else { &module.param };
                    text_to_show = monitor.get_single_temp(sensor_id); 
                }

                "ip" => text_to_show = monitor.get_public_ip(&args.ip_url).await,
                // ==========================================
                // 🌟 3. [升级] 动态网口流量组
                // ==========================================
                "netspeed_down" | "netspeed_up" | "traffic_down" | "traffic_up" | "traffic_total" | "traffic_split" => {
                    // 如果前端选了网卡就用前端传入的 (比如 param="wan")，没传就用全局默认的 net_interface
                    let target_iface = if module.param.is_empty() { &args.net_interface } else { &module.param };
                    
                    text_to_show = match module.name.as_str() {
                        "netspeed_down" => monitor.get_speed_string_for(0, target_iface),
                        "netspeed_up"   => monitor.get_speed_string_for(1, target_iface),
                        "traffic_down"  => monitor.get_total_rx_string_for(target_iface),
                        "traffic_up"    => monitor.get_total_tx_string_for(target_iface),
                        "traffic_total" => monitor.get_traffic_total_string_for(target_iface),
                        "traffic_split" => monitor.get_total_traffic_for(target_iface),
                        _ => String::new(),
                    };
                }
                
                // 保留旧代码中存在的 updl (以防废弃不彻底)
                "updl" => text_to_show = monitor.get_updl_string(),
                "nic" => text_to_show = monitor.get_nic_status(),
                "dev" => text_to_show = monitor.get_online_devices(),
                "banner" => {
                    if !args.custom_text.is_empty() { text_to_show = args.custom_text.clone(); } 
                    else { text_to_show = "Welcome".to_string(); }
                }
                "http_custom" => {
                    // [修改点] 在参数最后加上了 args.http_cache_secs
                    text_to_show = monitor.get_http_text(&args.custom_http_url, "", args.http_length, args.http_cache_secs).await;
                }
                
                // ==========================================
                // 🌟 1. [向下兼容合并] 时间与日期组
                // ==========================================
                // 把新版的 time_group 和所有旧版的散装名字全部拦截下来
                "time_group" | "timeBlink" | "time_sec" | "weekday" | "time" | "date" | "date_y" | "date_Y" | "week_only" => {
                    // 智能提取格式：如果是新版的 time_group，就用冒号后面的 param；如果是旧版直传的，就直接用旧版名字
                    let format = if module.name == "time_group" {
                        if module.param.is_empty() { "timeBlink" } else { module.param.as_str() }
                    } else {
                        module.name.as_str() 
                    };

                    match format {
                        "time_sec" => {
                            let start = Instant::now();
                            while start.elapsed() < Duration::from_secs(module.duration) {
                                let time_str = Local::now().format("%H^%M^%S").to_string();
                                let _ = screen.write_data(time_str.as_bytes(), get_leds(monitor, args)).await;
                                tokio::select! {
                                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                                    Ok(_) = rx.changed() => { module_interrupted = true; break; }
                                }
                            }
                        }
                        "timeBlink" => {
                            let start = Instant::now(); 
                            let mut time_flag = false;
                            let mut last_tick = Instant::now();
                            while start.elapsed() < Duration::from_secs(module.duration) {
                                let mut time_str = Local::now().format("%H:%M").to_string();
                                if time_flag { time_str = time_str.replace(':', ";"); }
                                let _ = screen.write_data(time_str.as_bytes(), get_leds(monitor, args)).await;
                                if last_tick.elapsed().as_secs() >= 1 {
                                    time_flag = !time_flag;
                                    last_tick = Instant::now();
                                }
                                tokio::select! {
                                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                                    Ok(_) = rx.changed() => { module_interrupted = true; break; }
                                }
                            }
                        }
                        "weekday" => {
                            let start = Instant::now();
                            while start.elapsed() < Duration::from_secs(module.duration) {
                                let elapsed_ms = start.elapsed().as_millis();
                                let cycle = elapsed_ms % 4000; 
                                let display_text = if cycle < 1500 {
                                    Local::now().format("%a").to_string().to_uppercase()
                                } else {
                                    Local::now().format("%H:%M").to_string()
                                };
                                let _ = screen.write_data(display_text.as_bytes(), get_leds(monitor, args)).await;
                                tokio::select! {
                                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                                    Ok(_) = rx.changed() => { module_interrupted = true; break; }
                                }
                            }
                        }
                        // --- 静态时间与日期 ---
                        "time" => text_to_show = Local::now().format("%H:%M").to_string(),
                        "date" => text_to_show = Local::now().format("%m-%d").to_string(),
                        "date_y" => text_to_show = Local::now().format("%y-%m-%d").to_string(),
                        "date_Y" => text_to_show = Local::now().format("%Y.%m.%d").to_string(),
                        "week_only" => text_to_show = Local::now().format("%a").to_string().to_uppercase(),
                        _ => text_to_show = Local::now().format("%H:%M").to_string(), // 兜底
                    }
                }
                

                // --- 动态模块 2: 天气动画 (智能双模版：静态防抖 + 循环滚动) ---
                "weather" => {
                    let full_text = monitor.get_smart_weather(&args.weather_city, &args.weather_source, &args.seniverse_key).await;
                    let (static_icon, raw_rest) = match full_text.split_once(' ') {
                        Some((icon, rest)) => (icon, rest),
                        None => {
                            // [修改点] 解析失败时，使用静态防抖函数直接显示，防止极端乱码卡死
                            let _ = screen.write_data(full_text.as_bytes(), get_leds(monitor, args)).await;
                            module_idx += 1;
                            continue;
                        }
                    };
                    
                    let clean_rest = raw_rest.trim();
                    let status_leds = get_leds(monitor, args);
                    
                    // 🌟 记录这个模块开始的绝对时间，用于整体倒计时控制
                    let start_module = Instant::now();

                    if args.weather_format == "simple" {
                        // ==========================================
                        // 【策略 A】精简模式：截取数字，原地动画 + 强制静态
                        // ==========================================
                        let mut temp_val = String::new();
                        for (i, c) in clean_rest.chars().enumerate() {
                            if (i == 0 && c == '-') || c.is_ascii_digit() || c == '.' { temp_val.push(c); } 
                            else { break; }
                        }
                        let temp_part_str = if temp_val.starts_with('-') { temp_val } else { format!("{}℃", temp_val) };

                        let mut frame_flag = true;
                        let mut last_frame = Instant::now();
                        
                        while start_module.elapsed() < Duration::from_secs(module.duration) {
                            let dynamic_icon = monitor.get_animated_icon(static_icon, frame_flag);
                            let display_text = format!("{}{}", dynamic_icon, temp_part_str);
                            
                            // [修改点] 强制静态锁死，彻底解决图标闪烁导致的左右横跳
                            let _ = screen.write_data(display_text.as_bytes(), get_leds(monitor, args)).await;
                            
                            if last_frame.elapsed().as_millis() >= 500 {
                                frame_flag = !frame_flag;
                                last_frame = Instant::now();
                            }

                            // 100ms 智能监听按键
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                                Ok(_) = rx.changed() => { module_interrupted = true; break; }
                            }
                        }
                    } else {
                        // ==========================================
                        // 【策略 B】完整模式：长文本滚动 + 停顿 1 秒循环
                        // ==========================================
                        let display_text = format!("{} {}", static_icon, clean_rest);
                        
                        while start_module.elapsed() < Duration::from_secs(module.duration) {
                            tokio::select! {
                                // 🌟 核心逻辑：滚动动作 + 1秒停顿 作为一个整体异步执行
                                _ = async {
                                    // 1. 调用原生 write_data 执行完整的从右到左滚动
                                    let _ = screen.write_data(display_text.as_bytes(), get_leds(monitor, args)).await;
                                    // 2. 滚出屏幕后，静止等待 1 秒
                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                } => {
                                    // 一轮完整的“滚+停”正常结束，进入下一次 while 循环，重新开始滚
                                }
                                // 🌟 随时监听：无论是正在滚动，还是正在 1 秒停顿中，按键都能秒切
                                Ok(_) = rx.changed() => { 
                                    module_interrupted = true; 
                                    break; 
                                }
                            }
                        }
                    }
                }
                "stock" => {
                    let (txt, _) = monitor.get_stock_trend(&args.stock_url).await; // [修改点] 忽略原来的 flag
                    text_to_show = txt;
                }
                // ==========================================
                // 🎬 动画模块专属分支 (支持按键秒切)
                // ==========================================
                "anim" => {
                    let file_name = module.param.clone(); 
        
                    if file_name.is_empty() {
                        eprintln!("⚠️ 警告: 动画模块未指定文件");
                        text_to_show = "NO FILE".to_string();
                    } else {
                        let duration_secs = module.duration;
            
                        // 🌟 核心修复：引入 tokio::select! 神级打断机制
                        tokio::select! {
                            // 🏃‍♂️ 赛道 1：默默播放动画，播满设定的时长后自然结束
                            _ = screen.play_animation(&file_name, duration_secs, get_leds(monitor, args)) => {}
                            
                            // 🏃‍♂️ 赛道 2：按键狙击手！一旦检测到按键被按下，瞬间“击杀”赛道 1 的动画进程！
                            Ok(_) = rx.changed() => {
                                module_interrupted = true;
                                // 打上中断标记后跳出 select，外层的打断接管逻辑会立刻执行切台！
                            }
                        }
                    }
                }
                _ => {
                    module_idx += 1;
                    continue; 
                }
            } // match 结束

            // === 统一神级中断接管 ===
            if module_interrupted {
                let new_val = *rx.borrow();
                if new_val < 0 { break; } // 长按息屏，回溯外层休眠
                
                if profiles_count == 1 {
                    module_idx += 1; // 只有1个配置：行为=切歌
                } else {
                    current_profile_idx = (current_profile_idx + 1) % profiles_count;
                    break; // 有多个配置：行为=换台 (打破当前模块列表，去下个配置组)
                }
                continue;
            }

            // === [核心剥离] 静态模块的智能渲染层 ===
            if !text_to_show.is_empty() {
                // 🌟 新增：文字内容刷新计时器
                let mut last_refresh_time = Instant::now();
                let module_start_time = Instant::now();
                while module_start_time.elapsed() < Duration::from_secs(module.duration) {
                    // 🌟 新增：每隔 1 秒，重新抓取一次动态数据
                    if last_refresh_time.elapsed().as_secs() >= 1 {
                        let target_iface = if module.param.is_empty() { &args.net_interface } else { &module.param };
                        
                        match module.name.as_str() {
                            // --- 🌐 网速组 ---
                            "netspeed_down" => text_to_show = monitor.get_speed_string_for(0, target_iface),
                            "netspeed_up"   => text_to_show = monitor.get_speed_string_for(1, target_iface),
                            "updl"          => text_to_show = monitor.get_updl_string(),
                            
                            // --- 💻 系统组 ---
                            "cpu"           => text_to_show = monitor.get_cpu_usage_string(),
                            "mem"           => text_to_show = monitor.get_mem_string(),
                            "load"          => text_to_show = monitor.get_load_string(),
                            "temp"          => text_to_show = monitor.get_temps_by_ids(&args.temp_flag),
                            "temp_single"   => {
                                let sensor_id = if module.param.is_empty() { "4" } else { &module.param };
                                text_to_show = monitor.get_single_temp(sensor_id); 
                            },
                            "dev"           => text_to_show = monitor.get_online_devices(),
                            
                            // --- 🕒 时间组 (防止跨分) ---
                            "time"          => text_to_show = Local::now().format("%H:%M").to_string(),
                            
                            _ => {} // 纯静态模块（如 Banner）不处理
                        }
                        last_refresh_time = Instant::now(); // 重置计时器
                    }

                    
                    // 🌟 神级打断逻辑：画图和按键同时进行！
                    tokio::select! {
                        // 🏃‍♂️ 赛道 1：执行画图（即使滚很久）并休眠 100ms
                        _res = async {
                            let _ = screen.write_data(text_to_show.as_bytes(), get_leds(monitor, args)).await;
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        } => {
                            // 赛道 1 正常跑完（一帧画完了），什么都不做，继续下一轮循环
                        }
                        
                        // 🏃‍♂️ 赛道 2：按键狙击手！只要一按，瞬间掐死赛道 1 的画图过程！
                        Ok(_) = rx.changed() => { 
                            module_interrupted = true; 
                            break; 
                        }
                    }
                }
                
                if module_interrupted {
                    let new_val = *rx.borrow();
                    if new_val < 0 { break; } 
                    
                    if profiles_count == 1 {
                        module_idx += 1; 
                    } else {
                        current_profile_idx = (current_profile_idx + 1) % profiles_count;
                        break; 
                    }
                    continue;
                }
                module_idx += 1;
            } else {
                // 动画模块已经放完
                module_idx += 1;
            }
        } // 内层 module_idx while 结束
        
        // 🚨 魔法发生地：如果 while 自然结束（没被按钮打断），
        // current_profile_idx 没变，它就会无限循环当前的 Profile 频道！
    }
}



