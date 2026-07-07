// ==========================================
// 🛰️ net_agent.rs — 后台网络数据代理 (v2.3.1 新增)
//
// 🌟 [核心修复] 渲染与网络彻底解耦：
// 以前天气/IP/股票/HTTP 的网络请求是在渲染循环里 await 的，
// 缓存一过期，那次请求会把屏幕卡在上一帧最长 30 秒（连按键都没反应）。
// 现在所有网络请求由本模块的后台任务定时刷新，写入共享快照；
// 渲染层只读快照，永不等待网络。
// ==========================================
use crate::control::{Alert, SharedControl};
use crate::Args;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// --- uapis.cn 天气结构体 ---
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

// --- 心知天气结构体 ---
#[derive(Deserialize, Debug)]
struct SeniverseResponse {
    results: Vec<SeniverseResult>,
}
#[derive(Deserialize, Debug)]
struct SeniverseResult {
    daily: Vec<SeniverseDaily>,
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

// --- Open-Meteo 结构体 ---
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
    // 当天最高/最低温 (可选，接口异常时不影响当前温度显示)
    #[serde(default)]
    daily: Option<OmDaily>,
}

#[derive(Deserialize, Debug)]
struct OmCurrentWeather {
    temperature: f64,
    weathercode: u8,
}

#[derive(Deserialize, Debug)]
struct OmDaily {
    #[serde(default)]
    temperature_2m_max: Vec<f64>,
    #[serde(default)]
    temperature_2m_min: Vec<f64>,
}

// ==========================================
// 📸 共享快照：后台任务写，渲染层读 (临界区极短，无网络等待)
// ==========================================
struct NetSnapshot {
    weather: String,
    ip: String,
    http_text: String,
    stock: String,
    pings: HashMap<String, String>, // 目标 -> "P:23ms"
    sun: String,                    // "6:02~19:23" (由 IP 定位经纬度计算)
}

impl Default for NetSnapshot {
    fn default() -> Self {
        Self {
            weather: "Wait...".to_string(),
            ip: "IP:Wait".to_string(),
            http_text: String::new(),
            stock: String::new(),
            pings: HashMap::new(),
            sun: "SUN:--".to_string(),
        }
    }
}

// 渲染层持有的只读句柄
#[derive(Clone)]
pub struct NetHandle(Arc<RwLock<NetSnapshot>>);

impl NetHandle {
    pub fn weather(&self) -> String {
        self.0.read().map(|s| s.weather.clone()).unwrap_or_else(|_| "Wait...".into())
    }
    pub fn ip(&self) -> String {
        self.0.read().map(|s| s.ip.clone()).unwrap_or_else(|_| "IP:Wait".into())
    }
    pub fn http_text(&self) -> String {
        self.0.read().map(|s| s.http_text.clone()).unwrap_or_default()
    }
    pub fn stock(&self) -> String {
        self.0.read().map(|s| s.stock.clone()).unwrap_or_default()
    }
    pub fn ping(&self, target: &str) -> String {
        self.0.read()
            .ok()
            .and_then(|s| s.pings.get(target).cloned())
            .unwrap_or_else(|| "P:Wait".to_string())
    }
    pub fn sun(&self) -> String {
        self.0.read().map(|s| s.sun.clone()).unwrap_or_else(|_| "SUN:--".into())
    }
}

// 从 JSON 文本中提取数字字段 (轻量解析，无需完整反序列化)
fn extract_json_number(text: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\"", key);
    let after = text.split(&pattern).nth(1)?;
    let after_colon = after.split(':').nth(1)?;
    let num_str: String = after_colon
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num_str.parse().ok()
}

// ==========================================
// 🚀 启动后台代理：扫描 profile 只刷新真正用到的数据种类
// ==========================================
pub fn spawn_net_agent(args: Args, control: SharedControl) -> NetHandle {
    let snapshot = Arc::new(RwLock::new(NetSnapshot::default()));
    let handle = NetHandle(Arc::clone(&snapshot));

    // 扫描 profile 中出现的模块，确定需要刷新哪些网络数据 (省 API 配额)
    let mut want_weather = false;
    let mut want_ip = false;
    let mut want_http = false;
    let mut want_stock = false;
    let mut want_sun = false;
    let mut ping_targets: Vec<String> = Vec::new();

    for p_str in &args.profile {
        for token in p_str.split_whitespace() {
            let name_with_param = token.split('#').next().unwrap_or("");
            let (name, param) = match name_with_param.split_once(':') {
                Some((n, p)) => (n, p),
                None => (name_with_param, ""),
            };
            match name {
                "weather" => want_weather = true,
                "ip" => want_ip = true,
                "http_custom" => want_http = true,
                "stock" => want_stock = true,
                // sun 带手动经纬度参数时本地直算，无需代理；仅无参数时走 IP 定位
                "sun" => {
                    if param.is_empty() {
                        want_sun = true;
                    }
                }
                "ping" => {
                    let t = param.to_string();
                    if !ping_targets.contains(&t) {
                        ping_targets.push(t);
                    }
                }
                _ => {}
            }
        }
    }

    println!(
        "🛰️ [网络代理] 启动后台刷新 (weather={}, ip={}, http={}, stock={}, sun={}, ping×{})",
        want_weather, want_ip, want_http, want_stock, want_sun, ping_targets.len()
    );

    tokio::spawn(async move {
        let mut agent = NetAgent::new();
        let mut last_stock: Option<Instant> = None;
        let mut last_ping: Option<Instant> = None;
        let mut last_sun_day: Option<chrono::NaiveDate> = None;
        // 🚨 [v2.5.0] 公网 IP 变化提醒: 记录上一次的有效 IP
        let mut last_good_ip: Option<String> = None;

        loop {
            // 各数据源的节流策略在 agent 方法内部 (天气 30min 缓存 + 120s 失败退避、
            // IP 1h 缓存、HTTP 按用户配置)，这里放心高频调用，返回都是秒回
            if want_weather {
                let text = agent
                    .get_smart_weather(&args.weather_city, &args.weather_source, &args.seniverse_key)
                    .await;
                if let Ok(mut s) = snapshot.write() { s.weather = text; }
            }

            if want_ip {
                let text = agent.get_public_ip(&args.ip_url).await;

                // 🚨 [v2.5.0] 公网 IP 变化提醒 (仅在两次都是有效 IP 且不同时播报;
                // 开机首次获取只记录不播报)
                if args.alert_ip && !text.contains("Err") && !text.contains("Wait") {
                    if let Some(prev) = &last_good_ip {
                        if prev != &text {
                            println!("🚨 [告警] 公网 IP 变化: {} -> {}", prev, text);
                            if let Ok(mut st) = control.lock() {
                                st.pending_alerts.push(Alert {
                                    text: format!("NEW {}", text),
                                    blink: false,
                                    secs: 6,
                                });
                            }
                        }
                    }
                    last_good_ip = Some(text.clone());
                }

                if let Ok(mut s) = snapshot.write() { s.ip = text; }
            }

            if want_http {
                let text = agent
                    .get_http_text(&args.custom_http_url, "", args.http_length, args.http_cache_secs)
                    .await;
                if let Ok(mut s) = snapshot.write() { s.http_text = text; }
            }

            // 股票无内部缓存，代理层节流 30 秒
            if want_stock && last_stock.map_or(true, |t| t.elapsed() >= Duration::from_secs(30)) {
                let (text, _) = agent.get_stock_trend(&args.stock_url).await;
                if let Ok(mut s) = snapshot.write() { s.stock = text; }
                last_stock = Some(Instant::now());
            }

            // 延迟探测每 10 秒一轮
            if !ping_targets.is_empty()
                && last_ping.map_or(true, |t| t.elapsed() >= Duration::from_secs(10))
            {
                for target in &ping_targets {
                    let result = agent.get_tcp_ping(target).await;
                    if let Ok(mut s) = snapshot.write() {
                        s.pings.insert(target.clone(), result);
                    }
                }
                last_ping = Some(Instant::now());
            }

            // 🌅 日出日落: 拿到 IP 定位坐标后本地计算，跨天自动重算
            if want_sun {
                agent.ensure_location().await;
                if let Some((lat, lon)) = agent.coords {
                    let today = chrono::Local::now().date_naive();
                    if last_sun_day != Some(today) {
                        let text = crate::sun::today_string(lat, lon);
                        if let Ok(mut s) = snapshot.write() { s.sun = text; }
                        last_sun_day = Some(today);
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    handle
}

// ==========================================
// 🧠 内部代理：持有 HTTP 客户端与各类缓存/节流状态
// (以下方法整体从旧 SystemMonitor 迁移而来，逻辑不变)
// ==========================================
struct NetAgent {
    http_client: Client,

    cached_weather: String,
    last_weather_time: Instant,
    // 上次“尝试”请求天气的时间 (无论成败)，用于失败退避；None = 从未尝试过
    last_weather_attempt: Option<Instant>,

    cached_ip: String,
    last_ip_time: Instant,
    // 🌟 [修复] IP 查询失败退避 (以前断网时每 5 秒 tick 都发起 30 秒超时请求，
    // 串行拖慢 ping/日出日落等所有后台刷新)
    last_ip_attempt: Option<Instant>,

    http_cache_text: String,
    http_cache_time: Instant,

    // 天气 api 获取定位城市
    auto_location: String,
    // 🌟 [v2.4.0] IP 定位经纬度 (供日出日落计算)
    coords: Option<(f64, f64)>,
    // 定位失败的重试节流
    last_geo_attempt: Option<Instant>,

    last_stock_price: f64,
}

impl NetAgent {
    fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Athena-LED Router)")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            http_client: client,
            cached_weather: "Wait...".to_string(),
            last_weather_time: Instant::now(),
            last_weather_attempt: None,
            cached_ip: "IP:Err".to_string(),
            last_ip_time: Instant::now(),
            last_ip_attempt: None,
            http_cache_text: String::new(),
            http_cache_time: Instant::now(),
            auto_location: String::new(),
            coords: None,
            last_geo_attempt: None,
            last_stock_price: 0.0,
        }
    }

    // ==========================================
    // 🌍 IP 自动定位 (双源: 紫辰 主 / ip-api.com 备)
    // 同时提取城市名 (天气用) 与经纬度 (日出日落用)；
    // 失败后 10 分钟内不重试，防止无谓请求
    // ==========================================
    async fn ensure_location(&mut self) {
        // 城市和坐标都拿到了就不再请求
        if !self.auto_location.is_empty() && self.coords.is_some() {
            return;
        }
        if let Some(last) = self.last_geo_attempt {
            if last.elapsed() < Duration::from_secs(600) {
                return;
            }
        }
        self.last_geo_attempt = Some(Instant::now());

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Athena-LED/2.0")
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let geo_sources = [
            "http://app.zichen.zone/api/geoip/api.php",
            "http://ip-api.com/json/?lang=zh-CN&fields=city,lat,lon",
        ];
        for geo_url in geo_sources {
            match client.get(geo_url).send().await {
                Ok(resp) => {
                    if let Ok(text) = resp.text().await {
                        #[cfg(debug_assertions)]
                        println!("🌍 [定位调试] {} 原始返回: {}", geo_url, text);

                        // 提取 city (两个接口都是 JSON 且都有 "city" 字段)
                        if self.auto_location.is_empty() {
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

                        // 🌟 提取经纬度 (ip-api 一定有; 其他源有则取)
                        if self.coords.is_none() {
                            if let (Some(lat), Some(lon)) = (
                                extract_json_number(&text, "lat"),
                                extract_json_number(&text, "lon"),
                            ) {
                                if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
                                    self.coords = Some((lat, lon));
                                    println!("✅ [定位] 坐标: {:.2},{:.2}", lat, lon);
                                }
                            }
                        }
                    }
                }
                Err(e) => println!("❌ [定位失败] {} 请求报错: {}", geo_url, e),
            }
            // 城市和坐标都齐了就提前收工
            if !self.auto_location.is_empty() && self.coords.is_some() {
                break;
            }
        }
    }

    // --- 通用 HTTP 文本获取 ---
    async fn get_http_text(&mut self, url: &str, prefix: &str, max_len: usize, cache_secs: u64) -> String {
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
                        let truncated: String = clean_text.chars().take(max_len).collect();

                        let result = format!("{}{}", prefix, truncated);
                        self.http_cache_text = result.clone();
                        self.http_cache_time = Instant::now();
                        result
                    }
                    Err(_) => format!("{}Err", prefix),
                }
            }
            Err(_) => {
                // 请求失败时回退旧缓存
                if !self.http_cache_text.is_empty() {
                    self.http_cache_text.clone()
                } else {
                    format!("{}Wait", prefix)
                }
            }
        }
    }

    async fn get_public_ip(&mut self, ip_url: &str) -> String {
        // [缓存策略] IP 变化很少，缓存 60 分钟
        if self.last_ip_time.elapsed() < Duration::from_secs(3600) {
            if !self.cached_ip.contains("Err") {
                return self.cached_ip.clone();
            }
        }

        // 🌟 [修复] 失败退避: 距上次尝试不足 120 秒不再重复请求 (与天气策略一致)
        if let Some(last_try) = self.last_ip_attempt {
            if last_try.elapsed() < Duration::from_secs(120) {
                return self.cached_ip.clone();
            }
        }
        self.last_ip_attempt = Some(Instant::now());

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
            new_ip
        } else if !self.cached_ip.contains("Err") {
            // 本次查询失败但以前查到过：回退显示旧 IP
            self.cached_ip.clone()
        } else {
            new_ip
        }
    }

    async fn get_stock_trend(&mut self, url: &str) -> (String, u8) {
        if url.is_empty() { return (String::new(), 0); }

        match self.http_client.get(url).send().await {
            Ok(resp) => {
                if let Ok(json_val) = resp.json::<Value>().await {
                    // 尝试找 price/last/close 字段
                    let price_opt = json_val["price"].as_f64()
                        .or_else(|| json_val["price"].as_str().and_then(|s| s.parse::<f64>().ok()))
                        .or_else(|| json_val["last"].as_f64())
                        .or_else(|| json_val["close"].as_f64());

                    if let Some(current_price) = price_opt {
                        let mut flag = 2;

                        if self.last_stock_price > 0.0 {
                            if current_price > self.last_stock_price {
                                flag = 4; // 涨 -> 上箭头
                            } else if current_price < self.last_stock_price {
                                flag = 8; // 跌 -> 下箭头
                            }
                        }

                        self.last_stock_price = current_price;

                        let text = if current_price > 1000.0 {
                            format!("{:.0}", current_price)
                        } else {
                            format!("{:.2}", current_price)
                        };

                        return (text, flag);
                    }
                }
            }
            Err(_) => {}
        }
        ("Err".to_string(), 0)
    }

    // --- 网络延迟 (TCP 连接耗时近似) ---
    async fn get_tcp_ping(&self, target: &str) -> String {
        let target = target.trim();
        let addr = if target.is_empty() {
            "223.5.5.5:80".to_string()
        } else if target.contains(':') {
            target.to_string()
        } else {
            format!("{}:80", target)
        };

        let start = Instant::now();
        match tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect(&addr),
        ).await {
            Ok(Ok(_stream)) => format!("P:{}ms", start.elapsed().as_millis()),
            _ => "P:Err".to_string(),
        }
    }

    // 天气结果是否为有效数据: 所有失败分支都以 "W:" 开头 (W:Err/W:NoCity/W:GeoNet/W:NoKey...)，
    // 正常数据以天气图标开头 ("☀ 25℃ 20-30")；初始占位为 "Wait..."
    // 🌟 [修复] 以前只查 Err/Wait 关键词，W:NoCity 这类错误会被当有效数据缓存 30 分钟
    fn weather_is_good(text: &str) -> bool {
        !text.starts_with("W:") && !text.contains("Wait")
    }

    // --- [入口] 统一智能天气接口 ---
    async fn get_smart_weather(&mut self, location: &str, source: &str, key: &str) -> String {
        // 1. [缓存检查] 缓存有效且未过期 (30分钟)，直接返回
        let cache_good = Self::weather_is_good(&self.cached_weather);
        if cache_good && self.last_weather_time.elapsed() < Duration::from_secs(1800) {
            return self.cached_weather.clone();
        }

        // 2. 失败退避：距上次尝试不足 120 秒时不再重复请求，
        // 防止持续失败时被服务端风控/封禁 (“天气一天后消失”的元凶)
        if let Some(last_try) = self.last_weather_attempt {
            if last_try.elapsed() < Duration::from_secs(120) {
                return self.cached_weather.clone();
            }
        }
        self.last_weather_attempt = Some(Instant::now());

        // ==========================================
        // 🌟 IP 自动定位 (双源加固版, 与日出日落共享 ensure_location)
        // ==========================================
        let mut target_location = location.to_string();
        if target_location.to_lowercase() == "auto" || target_location.is_empty() {
            self.ensure_location().await;

            // 兜底策略
            target_location = if self.auto_location.is_empty() {
                #[cfg(debug_assertions)]
                println!("⚠️ [定位兜底] 启用默认城市: 北京");
                "北京".to_string()
            } else {
                self.auto_location.clone()
            };
        }

        // 默认数据源接管：用户没选或填了 auto，强制使用 uapis
        let target_source = if source.is_empty() || source == "auto" {
            "uapis"
        } else {
            source
        };

        let result = match target_source {
            "seniverse" => self.get_weather_from_seniverse(&target_location, key).await,
            "openmeteo" => self.get_weather_from_open_meteo(&target_location).await,
            "uapis" => self.get_weather_from_uapis(&target_location).await,
            "wttr" => self.get_weather_from_wttr(&target_location).await,
            _ => self.get_weather_from_uapis(&target_location).await,
        };

        // 3. [更新缓存]
        if Self::weather_is_good(&result) {
            self.cached_weather = result.clone();
            self.last_weather_time = Instant::now();
            result
        } else if cache_good {
            // 请求失败但手里有旧数据：回退显示旧数据 (宁可旧，不可无)
            println!("⚠️ [天气] 刷新失败 ({})，暂时沿用上次数据: {}", result, self.cached_weather);
            self.cached_weather.clone()
        } else {
            // 从未成功过 (刚开机就断网等)，只能如实返回错误提示
            result
        }
    }

    // --- [通道1] uapis.cn (适合国内，支持中文名) ---
    async fn get_weather_from_uapis(&self, city: &str) -> String {
        let url = format!("https://uapis.cn/api/v1/misc/weather?city={}&forecast=true", city);

        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(data) = resp.json::<WeatherResponse>().await {
                    let temp = data.temperature;
                    let max = data.temp_max.unwrap_or(temp);
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

    // --- [通道2] Wttr ---
    async fn get_weather_from_wttr(&self, city: &str) -> String {
        let url = format!("https://wttr.in/{}?format=j1", city);
        #[cfg(debug_assertions)]
        println!("DEBUG: Requesting Wttr: {}", url);

        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                // 检查 HTTP 状态码 (wttr 经常封 IP 返回 429 或 503)
                if !resp.status().is_success() {
                    #[cfg(debug_assertions)]
                    println!("DEBUG: Wttr failed status: {}", resp.status());
                    return format!("W:Err({})", resp.status().as_u16());
                }

                match resp.json::<WttrResult>().await {
                    Ok(json) => {
                        if let (Some(curr), Some(daily)) = (json.current_condition.first(), json.weather.first()) {
                            let temp = &curr.temp_C;
                            let max = &daily.maxtempC;
                            let min = &daily.mintempC;

                            let desc = curr.weatherDesc.first()
                                .map(|d| d.value.to_lowercase())
                                .unwrap_or_else(|| "unknown".to_string());

                            let icon = if desc.contains("rain") || desc.contains("shower") || desc.contains("drizzle") { "☂" }
                            else if desc.contains("snow") || desc.contains("ice") || desc.contains("hail") { "❄" }
                            else if desc.contains("thunder") { "⚡" }
                            else if desc.contains("cloud") || desc.contains("overcast") { "☁" }
                            else if desc.contains("mist") || desc.contains("fog") { "🌫" }
                            else { "☀" };

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

    // --- [通道3] 心知天气 ---
    async fn get_weather_from_seniverse(&self, location: &str, key: &str) -> String {
        // [安全] key 为空直接提示，不再依赖内置的公共测试 key
        if key.trim().is_empty() {
            return "W:NoKey".to_string();
        }
        let url = format!(
            "https://api.seniverse.com/v3/weather/daily.json?key={}&location={}&language=en&unit=c&start=0&days=1",
            key, location
        );

        match self.http_client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<SeniverseResponse>().await {
                    if let Some(daily) = json.results.get(0).and_then(|r| r.daily.get(0)) {
                        let max = daily.high.parse::<f64>().unwrap_or(0.0);
                        let min = daily.low.parse::<f64>().unwrap_or(0.0);
                        // 取平均值当作当前温度 (免费版日预报不返回实时温度)
                        let temp = (max + min) / 2.0;

                        // 图标代码: 0-3 晴, 4-9 云, 10-19 雨, 20-29 雪
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

    // --- [通道4] Open-Meteo ---
    async fn get_weather_from_open_meteo(&self, city: &str) -> String {
        // 清洗城市名：IP 定位返回的多是 "北京市"/"朝阳区" 这类带行政后缀的名字
        let clean_city = city
            .trim()
            .trim_end_matches('市')
            .trim_end_matches('区')
            .trim_end_matches('县');
        let clean_city = if clean_city.is_empty() { city.trim() } else { clean_city };

        // Step 1: 查坐标
        let geo_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh&format=json",
            clean_city
        );

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

        // Step 2: 查当前天气 + 当天最高/最低温 (一次请求全带回)
        let weather_url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true&daily=temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto",
            lat, lon
        );

        match self.http_client.get(&weather_url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() { return "W:ApiErr".to_string(); }
                match resp.json::<OmWeatherResponse>().await {
                    Ok(data) => {
                        let temp = data.current_weather.temperature;
                        let code = data.current_weather.weathercode;

                        // 图标转换 (WMO 天气代码)，未知代码兜底云朵
                        let icon = match code {
                            0 => "☀",
                            1 | 2 | 3 => "☁",
                            45 | 48 => "🌫",
                            51..=67 | 80..=82 => "☂",
                            71..=77 | 85..=86 => "❄",
                            95..=99 => "⚡",
                            _ => "☁",
                        };

                        // 返回格式与其他数据源统一: ☀ 26℃ 20-30
                        if let Some(daily) = &data.daily {
                            if let (Some(max), Some(min)) = (daily.temperature_2m_max.first(), daily.temperature_2m_min.first()) {
                                return format!("{} {:.0}℃ {:.0}-{:.0}", icon, temp, min, max);
                            }
                        }
                        // daily 缺失时退回旧格式: ☀ 26.5℃
                        return format!("{} {:.1}℃", icon, temp);
                    }
                    Err(_) => "W:JsonErr".to_string(),
                }
            }
            Err(_) => "W:NetErr".to_string(),
        }
    }
}
