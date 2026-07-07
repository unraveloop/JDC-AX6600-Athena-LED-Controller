// ==========================================
// 🎛️ control.rs — 运行时控制接口 (v2.4.0 新增)
// 监听 127.0.0.1:<port> (默认关闭)，行协议，可用 busybox nc 直接调用:
//
//   echo "next"           | nc 127.0.0.1 8377   # 切下一频道 (= 短按按键)
//   echo "home"           | nc 127.0.0.1 8377   # 回到频道 1 (= 双击按键)
//   echo "off" / "wake"   | nc 127.0.0.1 8377   # 息屏 / 亮屏
//   echo "toggle"         | nc 127.0.0.1 8377   # 息屏/亮屏切换 (= 长按按键)
//   echo "light 2"        | nc 127.0.0.1 8377   # 临时锁定亮度 (light auto 恢复)
//   echo "show 10 HELLO"  | nc 127.0.0.1 8377   # 插播文本 10 秒 (自动化/HA 通知)
//   echo "ping"           | nc 127.0.0.1 8377   # 存活探测 -> PONG
//
// 仅绑定回环地址，不对外网开放。也是按键双击、未来 HA 集成的共享地基。
// ==========================================
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// ==========================================
// 🚨 [v2.5.0] 通用系统告警 (温度/断网/新设备/IP变化 共用一套插播机制)
// ==========================================
#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub text: String,
    // true = 紧急 (闪烁 + 四状态灯全亮)；false = 通知 (静态/滚动显示)
    pub blink: bool,
    pub secs: u64,
}

// 调度器在每个模块边界消费的共享控制状态
#[derive(Default)]
pub struct ControlState {
    // 双击按键 / home 指令: 回到频道 1
    pub go_home: bool,
    // 亮度临时锁定 (优先级高于夜间亮度)，None = 自动
    pub light_override: Option<u8>,
    // 待插播的文本: (内容, 秒数)
    pub pending_show: Option<(String, u64)>,
    // 🌟 [v2.5.0] 系统告警队列 (net_agent 等后台任务写入，调度器边界消费)
    pub pending_alerts: Vec<Alert>,
}

pub type SharedControl = Arc<Mutex<ControlState>>;

pub fn new_shared() -> SharedControl {
    Arc::new(Mutex::new(ControlState::default()))
}

// 处理单条指令，返回响应文本
fn handle_command(
    line: &str,
    tx: &tokio::sync::watch::Sender<i32>,
    state: &SharedControl,
) -> String {
    let line = line.trim();
    let mut parts = line.splitn(3, ' ');
    let cmd = parts.next().unwrap_or("").to_lowercase();

    match cmd.as_str() {
        "ping" => "PONG".to_string(),

        "next" => {
            let current = *tx.borrow();
            let _ = tx.send(if current < 0 { 1 } else { current + 1 });
            "OK".to_string()
        }

        "home" => {
            if let Ok(mut st) = state.lock() { st.go_home = true; }
            let current = *tx.borrow();
            let _ = tx.send(if current < 0 { 1 } else { current + 1 });
            "OK".to_string()
        }

        "off" | "sleep" => {
            let _ = tx.send(-1);
            "OK".to_string()
        }

        "wake" => {
            let _ = tx.send(1);
            "OK".to_string()
        }

        "toggle" => {
            let current = *tx.borrow();
            let _ = tx.send(if current < 0 { 1 } else { -1 });
            "OK".to_string()
        }

        "light" => {
            match parts.next() {
                Some("auto") => {
                    if let Ok(mut st) = state.lock() { st.light_override = None; }
                    "OK".to_string()
                }
                Some(v) => match v.parse::<u8>() {
                    Ok(level) if level <= 7 => {
                        if let Ok(mut st) = state.lock() { st.light_override = Some(level); }
                        "OK".to_string()
                    }
                    _ => "ERR light 参数需为 0-7 或 auto".to_string(),
                },
                None => "ERR 用法: light <0-7|auto>".to_string(),
            }
        }

        "show" => {
            // show <秒数> <文本...>
            let secs = parts.next().and_then(|s| s.parse::<u64>().ok());
            let text = parts.next().unwrap_or("").trim();
            match (secs, text.is_empty()) {
                (Some(secs), false) if secs >= 1 && secs <= 300 => {
                    if let Ok(mut st) = state.lock() {
                        st.pending_show = Some((text.to_string(), secs));
                    }
                    // 顺手打断当前模块，让插播尽快上屏 (调度器识别 pending_show 不换台)
                    let current = *tx.borrow();
                    if current > 0 { let _ = tx.send(current + 1); }
                    "OK".to_string()
                }
                _ => "ERR 用法: show <1-300秒> <文本>".to_string(),
            }
        }

        "" => "ERR 空指令".to_string(),
        other => format!("ERR 未知指令: {} (可用: next/home/off/wake/toggle/light/show/ping)", other),
    }
}

/// 启动控制服务 (port = 0 表示关闭)
pub fn spawn_control_server(
    port: u16,
    tx: tokio::sync::watch::Sender<i32>,
    state: SharedControl,
) {
    if port == 0 {
        return;
    }

    tokio::spawn(async move {
        let addr = format!("127.0.0.1:{}", port);
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => {
                println!("🎛️ [控制接口] 已监听 {} (echo \"ping\" | nc 127.0.0.1 {})", addr, port);
                l
            }
            Err(e) => {
                println!("⚠️ [控制接口] 绑定 {} 失败: {}", addr, e);
                return;
            }
        };

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => continue,
            };
            let tx = tx.clone();
            let state = Arc::clone(&state);

            // 每个连接一个轻量任务: 逐行处理，出错即断开
            tokio::spawn(async move {
                let (read_half, mut write_half) = stream.into_split();
                let mut lines = BufReader::new(read_half).lines();
                // 5 秒空闲超时，防止连接悬挂
                while let Ok(Ok(Some(line))) = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    lines.next_line(),
                ).await {
                    let resp = handle_command(&line, &tx, &state);
                    if write_half.write_all(format!("{}\n", resp).as_bytes()).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
}

// ==========================================
// 🧪 单元测试: 指令解析与状态写入
// ==========================================
#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tokio::sync::watch::Sender<i32>, tokio::sync::watch::Receiver<i32>, SharedControl) {
        let (tx, rx) = tokio::sync::watch::channel(1i32);
        (tx, rx, new_shared())
    }

    #[test]
    fn cmd_next_increments() {
        let (tx, rx, st) = setup();
        assert_eq!(handle_command("next", &tx, &st), "OK");
        assert_eq!(*rx.borrow(), 2);
    }

    #[test]
    fn cmd_next_wakes_from_sleep() {
        let (tx, rx, st) = setup();
        let _ = tx.send(-1);
        assert_eq!(handle_command("next", &tx, &st), "OK");
        assert_eq!(*rx.borrow(), 1);
    }

    #[test]
    fn cmd_home_sets_flag() {
        let (tx, _rx, st) = setup();
        handle_command("home", &tx, &st);
        assert!(st.lock().unwrap().go_home);
    }

    #[test]
    fn cmd_light() {
        let (tx, _rx, st) = setup();
        assert_eq!(handle_command("light 3", &tx, &st), "OK");
        assert_eq!(st.lock().unwrap().light_override, Some(3));
        assert_eq!(handle_command("light auto", &tx, &st), "OK");
        assert_eq!(st.lock().unwrap().light_override, None);
        assert!(handle_command("light 9", &tx, &st).starts_with("ERR"));
    }

    #[test]
    fn cmd_show() {
        let (tx, _rx, st) = setup();
        assert_eq!(handle_command("show 10 HELLO WORLD", &tx, &st), "OK");
        assert_eq!(
            st.lock().unwrap().pending_show,
            Some(("HELLO WORLD".to_string(), 10))
        );
        assert!(handle_command("show abc", &tx, &st).starts_with("ERR"));
        assert!(handle_command("show 0 X", &tx, &st).starts_with("ERR"));
    }

    #[test]
    fn cmd_toggle_and_unknown() {
        let (tx, rx, st) = setup();
        handle_command("toggle", &tx, &st);
        assert_eq!(*rx.borrow(), -1);
        handle_command("toggle", &tx, &st);
        assert_eq!(*rx.borrow(), 1);
        assert!(handle_command("foobar", &tx, &st).starts_with("ERR"));
        assert_eq!(handle_command("ping", &tx, &st), "PONG");
    }
}
