// ==========================================
// 📨 mqtt.rs — MQTT 集成 (v2.4.0 新增)
// 订阅指定 topic，把收到的消息推上屏 (配合 "mqtt" 显示模块)。
// 典型用法: Home Assistant 自动化 -> mqtt.publish -> 路由器屏幕显示。
// broker 未配置时完全不启动，零开销。
// ==========================================
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::{Arc, RwLock};
use std::time::Duration;

// 渲染层持有的只读句柄 (与 net_agent 的快照同款模式)
#[derive(Clone)]
pub struct MqttHandle(Arc<RwLock<String>>);

impl MqttHandle {
    pub fn text(&self) -> String {
        self.0.read().map(|s| s.clone()).unwrap_or_default()
    }
}

/// 启动 MQTT 客户端 (broker 为空 = 功能关闭，返回空句柄)
pub fn spawn_mqtt(broker: &str, topic: &str, user: &str, pass: &str) -> MqttHandle {
    let shared = Arc::new(RwLock::new(String::new()));
    let handle = MqttHandle(Arc::clone(&shared));

    let broker = broker.trim().to_string();
    let topic = topic.trim().to_string();
    if broker.is_empty() || topic.is_empty() {
        return handle;
    }

    // 解析 host[:port]，默认 1883
    let (host, port) = match broker.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(1883)),
        None => (broker.clone(), 1883),
    };

    let user = user.trim().to_string();
    let pass = pass.to_string();

    tokio::spawn(async move {
        println!("📨 [MQTT] 连接 {}:{} 订阅 '{}'", host, port, topic);

        let client_id = format!("athena-led-{}", std::process::id());
        let mut options = MqttOptions::new(client_id, host, port);
        options.set_keep_alive(Duration::from_secs(30));
        if !user.is_empty() {
            options.set_credentials(user, pass);
        }

        let (client, mut eventloop) = AsyncClient::new(options, 16);

        loop {
            match eventloop.poll().await {
                // 🌟 每次 (重) 连接成功都重新订阅 — broker 重启后自动恢复
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    println!("✅ [MQTT] 已连接，订阅 {}", topic);
                    let _ = client.subscribe(topic.clone(), QoS::AtMostOnce).await;
                }
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    let payload = String::from_utf8_lossy(&publish.payload);
                    // 截断到 30 字符防止撑爆滚动 (中文按字符数安全截断)
                    let text: String = payload.trim().chars().take(30).collect();
                    println!("📨 [MQTT] 收到消息: {}", text);
                    if let Ok(mut s) = shared.write() {
                        *s = text;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    // 断线退避重连 (eventloop.poll 会自动重连，这里只做节流)
                    println!("⚠️ [MQTT] 连接异常: {:?}，5 秒后重试", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    handle
}
