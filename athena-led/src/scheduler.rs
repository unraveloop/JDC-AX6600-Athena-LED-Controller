// ==========================================
// 🎛️ scheduler.rs — 智能轮播调度引擎
// 负责: Profile 解析 / 模块渲染 / 按键打断 / 定时休眠
// 数据来自 monitor.rs，硬件输出走 led_screen
// ==========================================
use crate::led_screen;
use crate::monitor::SystemMonitor;
use crate::net_agent::NetHandle;
use crate::Args;
use anyhow::Result;
use chrono::{Local, NaiveTime};
use std::time::{Duration, Instant};

// ==========================================
// [智能调度引擎] 专属配置结构 (V2.0 动态参数版)
// ==========================================
#[derive(Debug, Clone)]
struct ModuleConfig {
    name: String,
    param: String, // 🌟 冒号后面的二级参数 (如 "wan", "time_sec", "4", "2027-06-07")
    duration: u64,
}

#[derive(Debug, Clone)]
struct ProfileConfig {
    modules: Vec<ModuleConfig>,
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
        None => return 60, // 极罕见的夏令时跳跃导致时间不存在，兜底睡 60 秒后重试
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
        now >= start || now < end
    }
}

// 🌟 [新增] 定时亮度: 在夜间时段内使用低亮度档 (复用 is_sleep_time 的跨午夜区间判断)
fn current_light_level(args: &Args) -> u8 {
    if is_sleep_time(&args.night_start, &args.night_end) {
        args.night_level.min(7)
    } else {
        args.light_level
    }
}

pub async fn process_loop(
    screen: &mut led_screen::LedScreen,
    args: &Args,
    monitor: &mut SystemMonitor,
    net: &NetHandle,
    rx: &mut tokio::sync::watch::Receiver<i32>,
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

            // 🌟 3. 用 ':' 切割主体，同时生成 name 和 param
            let (name, param) = match name_with_param.split_once(':') {
                Some((n, p)) => (n.to_string(), p.to_string()),
                None => (name_with_param.to_string(), String::new()),
            };

            modules.push(ModuleConfig { name, param, duration });
        }
        if !modules.is_empty() { profiles.push(ProfileConfig { modules }); }
    }

    let profiles_count = profiles.len();
    let mut current_profile_idx = 0;

    // 🌟 夜间被按键唤醒后的“临时免死金牌”时间
    let mut manual_wake_expire: Option<std::time::Instant> = None;

    // 🌟 [定时亮度] 当前已应用的亮度档 (main 启动时用的是 light_level)
    let mut applied_light = args.light_level;

    // --- 2. 状态机死循环 ---
    loop {
        // 🌟 [处理长按息屏] (由监听器发送 -1 触发)
        if *rx.borrow() < 0 {
            let _ = screen.write_data(b"        ", 0).await;
            screen.power(false, 0).unwrap_or_default();
            // 陷入沉睡，直到监听到大于 0 的短按唤醒信号
            let _ = rx.wait_for(|&val| val > 0).await;
            applied_light = current_light_level(args);
            screen.power(true, applied_light).unwrap_or_default();
            continue;
        }

        // 🌟 判断当前是否处于“临时唤醒”保护期
        let is_manual_awake = manual_wake_expire.map_or(false, |exp| exp > std::time::Instant::now());

        // 🌟 [处理夜间休眠] (仅在保护期外，且满足时间时才休眠)
        if !is_manual_awake && is_sleep_time(&args.sleep_start, &args.sleep_end) {
            let _ = screen.write_data(b"        ", 0).await;
            screen.power(false, 0).unwrap_or_default();
            let sleep_sec = get_seconds_until_wake(&args.sleep_end);

            tokio::select! {
                // 1. 正常睡到天亮自动醒
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(sleep_sec)) => {
                    applied_light = current_light_level(args);
                    screen.power(true, applied_light).unwrap_or_default();
                    continue;
                }
                // 2. 半夜被起夜的用户按了按钮
                Ok(_) = rx.changed() => {
                    // 赋予 60 秒免死金牌，这 60 秒内正常轮播配置
                    manual_wake_expire = Some(std::time::Instant::now() + std::time::Duration::from_secs(60));
                    applied_light = current_light_level(args);
                    screen.power(true, applied_light).unwrap_or_default();
                    continue;
                }
            }
        }

        let profile = &profiles[current_profile_idx];
        let mut module_idx = 0;

        // 🌟 [防御] 记录本轮开始时间：如果整轮瞬间跑完 (比如模块名全部拼错、
        // 动画文件全部缺失)，外层 loop 会变成 100% CPU 死循环，必须强制歇脚
        let pass_start = Instant::now();
        let mut switched_by_button = false;

        // --- 3. 模块级渲染与打断 ---
        while module_idx < profile.modules.len() {
            let module = &profile.modules[module_idx];
            let mut text_to_show = String::new();
            let mut module_interrupted = false;

            // 🌟 [定时亮度] 每个模块边界检查一次：跨入/离开夜间时段时自动调整
            let desired_light = current_light_level(args);
            if desired_light != applied_light {
                let _ = screen.power(true, desired_light);
                applied_light = desired_light;
                println!("💡 [亮度] 已切换到 {} 级", desired_light);
            }

            // 💡 全局灯光掩码过滤器
            let get_leds = |monitor: &mut SystemMonitor, args: &Args| -> u8 {
                let mut raw_flag = monitor.get_global_led_flag();
                if args.disable_led_clock { raw_flag &= !1; } // 1: 时钟
                if args.disable_led_medal { raw_flag &= !2; } // 2: 奖牌
                if args.disable_led_up    { raw_flag &= !4; } // 4: 上箭头
                if args.disable_led_down  { raw_flag &= !8; } // 8: 下箭头
                raw_flag
            };

            // 提取静态文本
            match module.name.as_str() {
                "uptime" => text_to_show = monitor.get_uptime_string(),
                "cpu" => text_to_show = monitor.get_cpu_usage_string(),
                "mem" => text_to_show = monitor.get_mem_string(),
                "load" => text_to_show = monitor.get_load_string(),

                "temp" => text_to_show = monitor.get_temps_by_ids(&args.temp_flag),
                // 单体温度专属通道：
                "temp_single" => {
                    let sensor_id = if module.param.is_empty() { "4" } else { &module.param };
                    text_to_show = monitor.get_single_temp(sensor_id);
                }

                // 🌟 [解耦] 网络类数据全部改读后台快照，渲染永不等网络
                "ip" => text_to_show = net.ip(),
                // ==========================================
                // 🌟 动态网口流量组
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

                // ==========================================
                // 🌟 [新功能模块] 倒数日 / 网络延迟 / 连接数
                // ==========================================
                "countdown" => text_to_show = monitor.get_countdown(&module.param),
                "ping" => text_to_show = net.ping(&module.param),
                "conn" => text_to_show = monitor.get_conntrack(),

                "banner" => {
                    if !args.custom_text.is_empty() { text_to_show = args.custom_text.clone(); }
                    else { text_to_show = "Welcome".to_string(); }
                }
                "http_custom" => {
                    text_to_show = net.http_text();
                }

                // ==========================================
                // 🌟 [向下兼容合并] 时间与日期组
                // ==========================================
                "time_group" | "timeBlink" | "time_sec" | "weekday" | "time" | "date" | "date_y" | "date_Y" | "week_only" => {
                    // 智能提取格式：新版 time_group 用冒号后的 param；旧版直传的用旧版名字
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

                // --- 动态模块: 天气动画 (智能双模版：静态防抖 + 循环滚动) ---
                "weather" => {
                    // 🌟 [解耦] 读后台快照 (后台代理已带 30min 缓存 + 失败退避 + 旧数据回退)
                    let full_text = net.weather();
                    let (static_icon, raw_rest) = match full_text.split_once(' ') {
                        Some((icon, rest)) => (icon, rest),
                        None => {
                            // 解析失败时，直接显示原文，防止极端乱码卡死
                            let _ = screen.write_data(full_text.as_bytes(), get_leds(monitor, args)).await;
                            // 🌟 [修复] 短暂停留 2 秒再切走：以前这里瞬间跳过，
                            // 导致天气失败时模块“一闪而过”，用户完全看不到发生了什么
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            module_idx += 1;
                            continue;
                        }
                    };

                    let clean_rest = raw_rest.trim();

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

                            // 强制静态锁死，彻底解决图标闪烁导致的左右横跳
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
                    text_to_show = net.stock();
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

                        tokio::select! {
                            // 🏃 赛道 1：播放动画，播满设定的时长后自然结束
                            _ = screen.play_animation(&file_name, duration_secs, get_leds(monitor, args)) => {}

                            // 🏃 赛道 2：按键狙击手！一旦按下，瞬间掐断动画
                            Ok(_) = rx.changed() => {
                                module_interrupted = true;
                            }
                        }
                    }
                }
                _ => {
                    module_idx += 1;
                    continue;
                }
            } // match 结束

            // === 统一中断接管 ===
            if module_interrupted {
                let new_val = *rx.borrow();
                if new_val < 0 { break; } // 长按息屏，回溯外层休眠

                if profiles_count == 1 {
                    module_idx += 1; // 只有1个配置：行为=切歌
                } else {
                    current_profile_idx = (current_profile_idx + 1) % profiles_count;
                    switched_by_button = true;
                    break; // 有多个配置：行为=换台
                }
                continue;
            }

            // === 静态模块的智能渲染层 ===
            if !text_to_show.is_empty() {
                // 🌟 文字内容刷新计时器
                let mut last_refresh_time = Instant::now();
                let module_start_time = Instant::now();
                while module_start_time.elapsed() < Duration::from_secs(module.duration) {
                    // 🌟 每隔 1 秒，重新抓取一次动态数据
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
                            // 🌟 [新功能模块] 连接数/延迟实时刷新 (延迟由后台代理每10秒探测)
                            "conn"          => text_to_show = monitor.get_conntrack(),
                            "ping"          => text_to_show = net.ping(&module.param),

                            // --- 🕒 时间组 (防止跨分) ---
                            "time"          => text_to_show = Local::now().format("%H:%M").to_string(),

                            _ => {} // 纯静态模块（如 Banner）不处理
                        }
                        last_refresh_time = Instant::now(); // 重置计时器
                    }

                    // 🌟 打断逻辑：画图和按键同时进行
                    tokio::select! {
                        // 🏃 赛道 1：执行画图（即使滚很久）并休眠 100ms
                        _res = async {
                            let _ = screen.write_data(text_to_show.as_bytes(), get_leds(monitor, args)).await;
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        } => {
                            // 一帧画完，继续下一轮循环
                        }

                        // 🏃 赛道 2：按键狙击手
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
                        switched_by_button = true;
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

        // 🚨 如果 while 自然结束（没被按钮打断），current_profile_idx 没变，
        // 它就会无限循环当前的 Profile 频道！

        // 🌟 [防御] 整轮 pass 在 300ms 内就结束了？说明该频道的模块全部异常
        // (名字拼错/文件缺失)。强制休息 500ms，防止路由器 CPU 被打满
        if !switched_by_button && pass_start.elapsed() < Duration::from_millis(300) {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}
