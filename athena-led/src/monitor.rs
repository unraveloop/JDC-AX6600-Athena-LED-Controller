// ==========================================
// 📊 monitor.rs — 本地系统监控器
// 负责: CPU/内存/负载/温度/网速/流量/设备数/连接数/倒数日 等
// 纯本地数据 (读 /proc、/sys)，全部瞬时返回，渲染循环可放心同步调用。
// 🌟 网络类数据 (天气/IP/股票/HTTP/延迟) 已迁往 net_agent.rs 后台刷新
// ==========================================
use chrono::{Datelike, Local, NaiveDate};
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

pub struct SystemMonitor {
    net_interface: String,

    // 🌟 独立记忆每个网卡的 (rx_bytes, tx_bytes, last_time)
    net_speed_cache: HashMap<String, (u64, u64, std::time::Instant)>,

    // CPU 记录
    last_cpu_total: u64,
    last_cpu_idle: u64,

    // 4 盏 LED 用的独立状态引擎变量
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
    pub fn new(net_dev: String) -> Self {
        Self {
            net_interface: net_dev,
            net_speed_cache: HashMap::new(),

            last_cpu_total: 0,
            last_cpu_idle: 0,

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

            led_last_cpu_total: 0,
            led_last_cpu_idle: 0,
            led_cpu_usage: 0.0,
        }
    }

    // ==========================================
    // [灯光大脑 1] 零流量底层路由探测 (用于奖牌灯)
    // ==========================================
    fn check_default_route(&self) -> bool {
        // IPv4: Destination 为 00000000 代表存在默认网关
        if let Ok(content) = fs::read_to_string("/proc/net/route") {
            for line in content.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 && parts[1] == "00000000" { return true; }
            }
        }
        // 🌟 IPv6: 目的前缀 ::/0 (全零 + 前缀长度 00) 即默认路由
        // 修复 IPv6-only 拨号环境下奖牌灯误判“断网”的问题
        if let Ok(content) = fs::read_to_string("/proc/net/ipv6_route") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // 格式: dest(32hex) plen(2hex) src srclen nexthop metric refcnt use flags ifname
                if parts.len() >= 10
                    && parts[0] == "00000000000000000000000000000000"
                    && parts[1] == "00"
                    && parts[9] != "lo"
                {
                    return true;
                }
            }
        }
        false
    }

    // ==========================================
    // [灯光大脑 2] 综合输出当前 100ms 的 4 灯状态 (状态隔离版)
    // ==========================================
    pub fn get_global_led_flag(&mut self) -> u8 {
        let mut flag = 0;
        let now = Instant::now();

        // 🔄 每 0.25 秒统一刷新一次底层硬件数据 (网络 + CPU)
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

            // 2. 🌟 独立计算 CPU (不和屏幕显示的 CPU 抢夺数据)
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

    // --- 天气图标动画帧 (供 scheduler 的天气模块使用) ---
    pub fn get_animated_icon(&self, static_icon: &str, frame_toggle: bool) -> String {
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

            // 6. 雾 🌫 -> 保持静态
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
                // parts[1]..parts[4] = user, nice, system, idle; parts[5].. = iowait, irq, softirq
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

    // ==========================================
    // [终极网速修复] 完美适配所有网卡动态切换
    // ==========================================
    fn read_net_bytes_for(&self, target_iface: &str) -> (u64, u64) {
        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines() {
                if let Some((name, data)) = line.split_once(':') {
                    // 🌟 [修复] 网卡名必须精确匹配！
                    // 以前用 contains 子串匹配：选 "lan1" 会命中 "wlan1"、
                    // 选 "eth0" 会命中 "veth0xxx"，导致网速读到别的网卡上
                    if name.trim() != target_iface {
                        continue;
                    }
                    let parts: Vec<&str> = data.split_whitespace().collect();
                    if parts.len() >= 9 {
                        let rx = parts[0].parse::<u64>().unwrap_or(0);
                        let tx = parts[8].parse::<u64>().unwrap_or(0);
                        return (rx, tx);
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

        // 🌟 去字典里拿这个专属网卡的数据。如果没拿过（第一次），就存入当前数据
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

    // --- 获取上下行同显实时网速 (updl) ---
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

    // --- 物理网口链路状态 (nic) 自适应版 ---
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
            // 💡 使用 carrier 判断底层物理连接，比 operstate 准确
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
                "10000" => result.push('T'), // Ten Gigabit (10G)
                _ => result.push('?'),       // 链路通了但速度未知
            }
        }

        if result.is_empty() { "NIC:Err".to_string() } else { result }
    }

    // 获取 CPU 占用率
    pub fn get_cpu_usage_string(&mut self) -> String {
        let (curr_total, curr_idle) = self.read_cpu_stats();
        let diff_total = curr_total.saturating_sub(self.last_cpu_total);
        let diff_idle = curr_idle.saturating_sub(self.last_cpu_idle);

        self.last_cpu_total = curr_total;
        self.last_cpu_idle = curr_idle;

        if diff_total == 0 { return "CPU:-".to_string(); }

        let usage = 100.0 * (1.0 - (diff_idle as f64 / diff_total as f64));
        format!("C:{:.0}%", usage)
    }

    // --- 内存监控 (读取 /proc/meminfo) ---
    pub fn get_mem_string(&self) -> String {
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
            format!("M:{:.0}%", usage_percent)
        } else {
            "M:Err".to_string()
        }
    }

    // --- 负载监控 (读取 /proc/loadavg) ---
    pub fn get_load_string(&self) -> String {
        let content = fs::read_to_string("/proc/loadavg").unwrap_or_default();
        let parts: Vec<&str> = content.split_whitespace().collect();
        if !parts.is_empty() {
            format!("L:{}", parts[0])
        } else {
            "L:Err".to_string()
        }
    }

    pub fn get_uptime_string(&self) -> String {
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
    pub fn get_temps_by_ids(&self, ids: &str) -> String {
        let mut results = Vec::new();

        // 1. 分割 ID 字符串 (支持空格或逗号分隔)
        let id_list: Vec<&str> = ids.split(|c| c == ' ' || c == ',')
                                    .filter(|s| !s.is_empty())
                                    .collect();

        for id in id_list {
            let type_path = format!("/sys/class/thermal/thermal_zone{}/type", id);
            let temp_path = format!("/sys/class/thermal/thermal_zone{}/temp", id);

            // 2. 读取名字 (用于显示标签，如 "cpu", "nss")
            if let Ok(type_name_raw) = fs::read_to_string(&type_path) {
                let label = type_name_raw.trim().to_lowercase().replace("-thermal", "");

                // 3. 读取温度
                if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                    if let Ok(raw_temp) = temp_str.trim().parse::<f64>() {
                        // 标准化：OpenWrt 通常是毫摄氏度 (55000 -> 55)
                        let val = if raw_temp > 1000.0 { raw_temp / 1000.0 } else { raw_temp };
                        results.push(format!("{}:{:.0}℃", label, val));
                    }
                }
            }
        }

        if results.is_empty() {
            "Temp:--".to_string()
        } else {
            results.join(" ")
        }
    }

    // ==========================================
    // 极简而精准的单体温度探针
    // ==========================================
    pub fn get_single_temp(&self, sensor_id: &str) -> String {
        let path = format!("/sys/class/thermal/thermal_zone{}/temp", sensor_id);

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(temp_millideg) = content.trim().parse::<f64>() {
                let temp_c = temp_millideg / 1000.0;

                // 自动分配前缀 (严格控制在 1~2 个字符，防止撑爆屏幕)
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
                // 确保有完整的 6 列，并且 MAC 地址不是全零
                if parts.len() >= 4 && parts[3] != "00:00:00:00:00:00" {
                    // Flags 字段 (parts[2]) 包含 "0x2" 代表可达
                    if parts[2].contains("0x2") {
                        count += 1;
                    }
                }
            }
            return format!("DEV:{}", count);
        }
        "DEV:Err".to_string()
    }

    // ==========================================
    // 🌟 [新功能模块] 倒数日 (countdown)
    // param: "YYYY-MM-DD" (一次性) 或 "MM-DD" (每年循环，如生日/节日)
    // 显示: D-123 (还有123天) / D-DAY (就是今天) / D+5 (已过5天)
    // ==========================================
    pub fn get_countdown(&self, param: &str) -> String {
        let param = param.trim();
        if param.is_empty() {
            return "NO DATE".to_string();
        }

        let today = Local::now().date_naive();

        let target = if param.len() <= 5 {
            // "MM-DD" 格式: 自动找最近的下一次 (今年已过就算明年的)
            NaiveDate::parse_from_str(&format!("{}-{}", today.year(), param), "%Y-%m-%d")
                .ok()
                .map(|d| {
                    if d < today {
                        d.with_year(today.year() + 1).unwrap_or(d)
                    } else {
                        d
                    }
                })
        } else {
            // "YYYY-MM-DD" 完整日期
            NaiveDate::parse_from_str(param, "%Y-%m-%d").ok()
        };

        match target {
            Some(date) => {
                let days = (date - today).num_days();
                if days == 0 {
                    "D-DAY".to_string()
                } else if days > 0 {
                    format!("D-{}", days)
                } else {
                    format!("D+{}", -days)
                }
            }
            None => "D:Err".to_string(),
        }
    }

    // ==========================================
    // 🌟 [新功能模块] 连接数 (conn)
    // 读取内核 conntrack 计数，反映 NAT 连接压力 (BT/PCDN 一眼看穿)
    // 显示: CT:1234
    // ==========================================
    pub fn get_conntrack(&self) -> String {
        for path in [
            "/proc/sys/net/netfilter/nf_conntrack_count",
            "/proc/sys/net/ipv4/netfilter/ip_conntrack_count",
        ] {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(count) = content.trim().parse::<u64>() {
                    return format!("CT:{}", count);
                }
            }
        }
        "CT:Err".to_string()
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
