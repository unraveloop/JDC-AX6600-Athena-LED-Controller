mod led_screen;
mod char_dict;

use anyhow::{Context, Result};
use clap::Parser;
use std::env;             
use std::fs;
use std::time::{Duration, Instant};
use tokio::time;
use tokio::signal::unix::{signal, SignalKind}; 
use chrono::{Local, NaiveTime, Timelike};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use regex::Regex;

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
    last_rx_bytes: u64,
    last_tx_bytes: u64,
    last_net_check: std::time::Instant,
    
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

            // 缓存字段
            cached_weather: "Wait...".to_string(),
            last_weather_time: Instant::now() - Duration::from_secs(3600 * 24),
            cached_ip: "Checking...".to_string(),
            last_ip_time: Instant::now() - Duration::from_secs(3600 * 24),

            // 统计字段 (注意：删除了 initial_rx/tx)
            last_rx_bytes: 0,
            last_tx_bytes: 0,
            last_net_check: Instant::now(),
            
            last_cpu_total: 0,
            last_cpu_idle: 0,
            
            last_stock_price: 0.0,
        })
    }

 
    
    
    // 初始化数据（避免第一次显示数值暴涨）
    fn init(&mut self) {
        let (rx, tx) = self.read_net_bytes();
        self.last_rx_bytes = rx;
        self.last_tx_bytes = tx;
        
        let (total, idle) = self.read_cpu_stats();
        self.last_cpu_total = total;
        self.last_cpu_idle = idle;
    }

    // --- 底层读取函数 ---
    
    // 读取 /proc/net/dev 获取原始字节数
    fn get_total_traffic(&self) -> String {
        // 直接读取当前总数值
        let (rx, tx) = self.read_net_bytes(); 
        
        // 辅助闭包：自动把字节转成 GB/MB
        let format_bytes = |bytes: u64| -> String {
            if bytes > 1024 * 1024 * 1024 {
                format!("{:.1}G", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
            } else {
                format!("{:.0}M", bytes as f64 / 1024.0 / 1024.0)
            }
        };

        let rx_str = format_bytes(rx);
        let tx_str = format_bytes(tx);

        // 显示格式： "T:1.2G/500M"
        format!("T:{}/{}", rx_str, tx_str)
    }

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

    // --- [补全] 读取 /proc/net/dev 原始数据 ---
    // (如果你的代码里已经有 read_net_bytes 了，就不用复制这一个)
    fn read_net_bytes(&self) -> (u64, u64) {
        let path = "/proc/net/dev";
        let content = fs::read_to_string(path).unwrap_or_default();
        
        for line in content.lines() {
            if line.contains(&self.net_interface) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // 适配不同格式：有的系统接口名后紧跟冒号 "eth0:"，有的是 "eth0 :"
                let rx_idx = if parts[0].contains(':') { 1 } else { 2 };
                let tx_idx = rx_idx + 8;
                
                if parts.len() > tx_idx {
                    let rx = parts[rx_idx].parse::<u64>().unwrap_or(0);
                    let tx = parts[tx_idx].parse::<u64>().unwrap_or(0);
                    return (rx, tx);
                }
            }
        }
        (0, 0)
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

    // --- 业务逻辑函数 ---

    // 1. 获取实时网速字符串 (如 "5.2M") - 专门用于显示下行
    // mode: 0=Download, 1=Upload
    fn get_speed_string(&mut self, mode: u8) -> String {
        let (curr_rx, curr_tx) = self.read_net_bytes();
        let now = Instant::now();
        let duration = now.duration_since(self.last_net_check).as_secs_f64();
        
        // [修复 1] 防止除以0，也防止间隔过短导致计算抖动
        if duration < 0.1 { return "...".to_string(); }

        // [修复 2] 核心修复：防止启动瞬间出现 "20000MB/s" 的巨额数值
        // 如果 last_rx_bytes 为 0 (说明 init 没成功或者刚启动)
        // 或者 duration 异常大 (说明程序暂停了很久)，
        // 我们不进行计算，而是直接重置基准值，并返回 0。
        if self.last_rx_bytes == 0 || self.last_tx_bytes == 0 || duration > 30.0 {
            self.last_rx_bytes = curr_rx;
            self.last_tx_bytes = curr_tx;
            self.last_net_check = now;
            return format_bytes_speed(0.0);
        }

        let speed = if mode == 0 {
            // saturating_sub 防止计数器溢出/回滚导致崩溃
            (curr_rx.saturating_sub(self.last_rx_bytes)) as f64 / duration
        } else {
            (curr_tx.saturating_sub(self.last_tx_bytes)) as f64 / duration
        };

        // 更新状态
        self.last_rx_bytes = curr_rx;
        self.last_tx_bytes = curr_tx;
        self.last_net_check = now;

        format_bytes_speed(speed)
    }

    // 2. 获取累计流量
    fn get_traffic_total_string(&self) -> String {
        let (curr_rx, curr_tx) = self.read_net_bytes();
        // 简单显示总和，或者你可以改成轮播 "In: 10G" "Out: 5G"
        format!("T:{}", format_bytes_total(curr_rx + curr_tx))
    }
    // --- [新增] 获取累计下载流量 (Total Download) ---
    // 返回格式如: "TD:1.5T"
    fn get_total_rx_string(&self) -> String {
        let (curr_rx, _) = self.read_net_bytes();
        // 直接使用 curr_rx 表示自开机以来的总量
        format!("TD:{}", format_bytes_total(curr_rx))
    }

    // --- [新增] 获取累计上传流量 (Total Upload) ---
    // 返回格式如: "TU:50G"
    fn get_total_tx_string(&self) -> String {
        let (_, curr_tx) = self.read_net_bytes();
        format!("TU:{}", format_bytes_total(curr_tx))
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

    // --- [新增] 统计在线设备 (ARP表) ---
    fn get_online_devices(&self) -> String {
        if let Ok(content) = fs::read_to_string("/proc/net/arp") {
            // 第一行是标题，所以从第二行开始算
            // 每一行代表一个设备 (IP + MAC)
            let count = content.lines().count();
            if count > 1 {
                return format!("Dev:{}", count - 1);
            }
        }
        "Dev:0".to_string()
    }

    // --- [修改后] 通用 HTTP 文本获取 ---
    // 增加了 max_len 参数，并修复了 UTF-8 切片可能导致的崩溃问题
    pub async fn get_http_text(&self, url: &str, prefix: &str, max_len: usize) -> String {
        if url.is_empty() {
            return String::new();
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
    async fn get_smart_weather(&mut self, location: &str, source: &str, key: &str) -> String {
        // 1. [缓存检查]
        // 如果距离上次更新不到 30 分钟 (1800秒)，且缓存内容不是错误信息，直接返回旧数据
        if self.last_weather_time.elapsed() < Duration::from_secs(1800) {
            if !self.cached_weather.contains("Err") && !self.cached_weather.contains("Wait") {
                return self.cached_weather.clone();
            }
        }

        // 2. [网络请求] 根据源选择不同的函数
        // 注意：这里调用的是我们之前定义好的那些函数
        let result = match source {
            "seniverse" => self.get_weather_from_seniverse(location, key).await,
            "openmeteo" => self.get_weather_from_open_meteo(location).await, // OpenMeteo 不需要 Key
            "uapis" => self.get_weather_from_uapis(location).await,
            _ => self.get_weather_from_wttr(location).await, // 默认 fallback 到 wttr
        };

        // 3. [更新缓存]
        // 只有获取成功 (不包含 Err 且不包含 Wait) 才更新时间
        // 如果失败了，下次循环会立即重试，而不会等 30 分钟
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
        println!("DEBUG: Requesting Wttr: {}", url); // [调试]

        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                // 1. 检查 HTTP 状态码 (关键！wttr 经常封 IP 返回 429 或 503)
                if !resp.status().is_success() {
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

    // 2. 构造今天的唤醒时间点
    let mut target_dt = now.date_naive().and_time(wake_time).and_local_timezone(Local).unwrap();

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

// --- 参数定义 ---
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    // --- 基础设置 ---
    #[arg(long, default_value_t = 5)]
    seconds: u64, // 每个模块显示的秒数

    #[arg(long, default_value_t = 5)]
    light_level: u8, // 亮度 (0-7)

    // --- 核心：显示顺序与内容 ---
    // 用户可以在这里自由排序，比如: "time weather cpu stock traffic_down"
    #[arg(long, default_value = "date timeBlink weather stock uptime netspeed_down netspeed_up cpu")]
    display_order: String,

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

    // [新增] HTTP 结果截断长度
    #[arg(long, default_value_t = 15)]
    http_length: usize,

    // 4. 天气接口 (我们可以用 wttr.in 这种返回纯文本的，简单方便)
    // 默认为北京天气，%t表示只显示温度+符号
    #[arg(long, default_value = "Beijing")]
    weather_city: String,

    #[arg(long, default_value = "uapis")]
    weather_source: String,

    // [新增] 心知天气 API Key (免费申请)
    // 这是一个公用的测试 Key，但不保证永久有效，建议自己申请
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
}

// ... 这里保留原来的 set_timezone_from_config 函数 ...
fn set_timezone_from_config() -> Result<()> {
    // (代码省略，保持原样即可)
    let content = fs::read_to_string("/etc/config/system")?;
    for line in content.lines() {
        if line.contains("CST-8") { env::set_var("TZ", "Asia/Shanghai"); return Ok(()); }
    }
    env::set_var("TZ", "UTC");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 设置时区 (如果有这个函数的话)
    // set_timezone_from_config().unwrap_or(());
    
    // 2. 解析参数
    let args = Args::parse();
    
    // 3. 初始化屏幕
    let mut screen = led_screen::LedScreen::new(581, 582, 585, 586)
        .context("Failed to init screen")?;
    // 注意：args.light_level 必须是存在的参数
    screen.power(true, args.light_level)?;
    
    // 4. 初始化系统监控 (只写一次！)
    let mut monitor = SystemMonitor::new(args.net_interface.clone())
        .context("Failed to initialize system monitor")?;
    
    // 5. 信号处理
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    // 6. 主循环
    loop {
        tokio::select! {
            // 收到终止信号，关屏退出
            _ = sigterm.recv() => { screen.power(false, 0)?; break; },
            _ = sigint.recv() => { screen.power(false, 0)?; break; },
            
            // 核心循环：传入 screen, args 和 monitor
            _ = process_loop(&mut screen, &args, &mut monitor) => {},
        }
    }
    Ok(())
}

async fn process_loop(
    screen: &mut led_screen::LedScreen, 
    args: &Args, 
    monitor: &mut SystemMonitor
) -> Result<()> {
    
    // [注意] 这里删除了原来的 "开头检查逻辑"
    // 我们把它移到了下面的 for 循环里，为了消除延迟
    
    // 解析用户输入的排序字符串
    let modules: Vec<&str> = args.display_order.split_whitespace().collect();

    for module in modules {
        // ================= [修改] 优雅的休眠守卫 =================
        // 1. 位置优化：放在循环内部。
        //    这样每显示完一个模块(5秒)就会检查一次，而不是等一整圈(30秒)。
        if is_sleep_time(&args.sleep_start, &args.sleep_end) {
            
            // A. 彻底灭灯 (写入空格清屏)
            screen.write_data(b"        ", 0)?; 
            
            // B. [优雅优化] 计算还需要睡多久才能醒来
            //    直接计算出距离 args.sleep_end 还有多少秒
            let sleep_sec = get_seconds_until_wake(&args.sleep_end);
            
            // C. 长睡眠 (CPU 占用率为 0)
            //    不再是每60秒醒来一次，而是直接睡到天亮
            tokio::time::sleep(tokio::time::Duration::from_secs(sleep_sec)).await;
            
            // D. 醒来后，直接返回，重新开始新的一轮 loop
            return Ok(()); 
        }
        // =======================================================

        let mut current_flag = 0; // 默认不亮灯
        let mut text_to_show = String::new();

        match module {
            // --- 基础时间类 ---
            "date" => text_to_show = Local::now().format("%m-%d").to_string(),
            
            "time" => {
                text_to_show = Local::now().format("%H:%M").to_string();
                current_flag |= 1; // [图标] Bit 0: 时钟图标
            }
            
            // [特殊处理] timeBlink 包含了自己的循环逻辑，需要直接 return 或 continue
            "timeBlink" => {
                current_flag |= 1;
                let start = Instant::now(); // 注意用 std::time::Instant
                let mut time_flag = false;
                
                // 在 args.seconds 时间内循环闪烁
                while start.elapsed() < Duration::from_secs(args.seconds) {
                    let mut time_str = Local::now().format("%H:%M").to_string();
                    if time_flag {
                        // 冒号变分号 (隐形)
                        time_str = time_str.replace(':', ";"); 
                    }
                    
                    // 直接写入屏幕，不经过外面的 write_data
                    screen.write_data(time_str.as_bytes(), current_flag)?;
                    
                    time_flag = !time_flag;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                // 循环结束，直接跳过后面通用的 write_data
                continue; 
            }

            // --- 系统信息类 ---
            // 注意：如果 monitor 没有 get_uptime_string，需要自己实现或补上
            "uptime" => text_to_show = monitor.get_uptime_string(),
            
            "cpu" => text_to_show = monitor.get_cpu_usage_string(),
            
            "mem" => text_to_show = monitor.get_mem_string(),
            
            "load" => text_to_show = monitor.get_load_string(),
            
            // [已修复] 传入 temp_flag 字符串
            "temp" => text_to_show = monitor.get_temps_by_ids(&args.temp_flag),

            // --- 网络信息类 ---
            "ip" => {
                // [已修复] 传入 ip_url
                text_to_show = monitor.get_public_ip(&args.ip_url).await;
            }

            "netspeed_down" => {
                text_to_show = monitor.get_speed_string(0); // 0 = RX (下行)
                current_flag |= 8; // [图标] Bit 3: 向下箭头
            }
            "netspeed_up" => {
                text_to_show = monitor.get_speed_string(1); // 1 = TX (上行)
                current_flag |= 4; // [图标] Bit 2: 向上箭头
            }
            

            
            // [已修复] 在线设备数
            "dev" => text_to_show = monitor.get_online_devices(),

            // --- 扩展功能类 ---
            "banner" => {
                if !args.custom_text.is_empty() {
                    text_to_show = args.custom_text.clone();
                } else {
                    text_to_show = "Welcome".to_string(); 
                }
            }
            
            "http_custom" => {
                // 此时不需要在这里调用 screen.write_data
                // 而是把处理好的字符串给 text_to_show，交给后面的统一逻辑处理
                text_to_show = monitor.get_http_text(
                    &args.custom_http_url,     // 或者是 &args.custom_http_url，取决于你的定义
                    "", 
                    args.http_length    // [新增] 传入截断长度
                ).await;
            }

            // [唤醒 2] 单独显示总下载流量 (T-RX)
            "traffic_down" => {
                text_to_show = monitor.get_total_rx_string();
                current_flag |= 8; // 向下箭头
            }

            // [唤醒 3] 单独显示总上传流量 (T-TX)
            "traffic_up" => {
                text_to_show = monitor.get_total_tx_string();
                current_flag |= 4; // 向上箭头
            }

"weather" => {
                let full_text = monitor.get_smart_weather(
                    &args.weather_city, 
                    &args.weather_source, 
                    &args.seniverse_key
                ).await;

                let (static_icon, raw_rest) = match full_text.split_once(' ') {
                    Some((icon, rest)) => (icon, rest),
                    None => {
                        screen.write_data(full_text.as_bytes(), current_flag)?;
                        continue;
                    }
                };
                
                let clean_rest = raw_rest.trim();

                // [步骤 A] 预先计算好“温度部分的字符串”
                let temp_part_str = if args.weather_format == "simple" {
                    // === 简易模式 ===
                    let mut temp_val = String::new();
                    for (i, c) in clean_rest.chars().enumerate() {
                        if (i == 0 && c == '-') || c.is_ascii_digit() || c == '.' {
                            temp_val.push(c);
                        } else {
                            break; 
                        }
                    }
                    if temp_val.starts_with('-') {
                        temp_val // 负温 "-5"
                    } else {
                        format!("{}℃", temp_val) // 正温 "28摄氏度"
                    }
                } else {
                    // === 完整模式 ===
                    // 保留原样 "28℃ 22-30"
                    // 并在前面加一个空格用于和图标隔开
                    format!(" {}", clean_rest) 
                };

                // [步骤 B] 进入动画循环
                let start = Instant::now();
                let mut frame_flag = true;
                
                while start.elapsed() < Duration::from_secs(args.seconds) {
                    let dynamic_icon = monitor.get_animated_icon(static_icon, frame_flag);
                    
                    // [步骤 C] 最终拼接
                    // Simple: "☀" + "28°" -> "☀28°"
                    // Full:   "☀" + " 28℃..." -> "☀ 28℃..."
                    let display_text = format!("{}{}", dynamic_icon, temp_part_str);

                    screen.write_data(display_text.as_bytes(), current_flag)?;
                    
                    frame_flag = !frame_flag;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                continue;
            }

            "stock" => {
                let (txt, flag) = monitor.get_stock_trend(&args.stock_url).await;
                text_to_show = txt;
                current_flag |= flag;
            }

            _ => continue, // 未知模块直接跳过
        }
        
    if !text_to_show.is_empty() {
            screen.write_data(text_to_show.as_bytes(), current_flag)?;
            
            // 注意：请确认你的 Args 字段名为 seconds 还是 duration
            // 这里用你刚才发给我的 args.seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(args.seconds)).await;
        }
    } // for loop 结束
    Ok(())
}
