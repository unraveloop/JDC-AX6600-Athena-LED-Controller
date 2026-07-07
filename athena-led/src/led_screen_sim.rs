// ==========================================
// 🪟 led_screen_sim.rs — Windows 本地调试用“终端虚拟屏幕”
// 通过 #[cfg(not(unix))] + #[path] 在非 Linux 平台顶替 led_screen.rs，
// 对外 API 与真实驱动完全一致
// ==========================================
use anyhow::Result;

pub struct LedScreen {}

// 模拟 GPIO 基址探测 (Windows 下无实际意义，返回主流内核默认值)
pub fn detect_gpio_base() -> u64 {
    println!("🔍 [虚拟屏幕] 模拟 GPIO base 探测 => 512");
    512
}

impl LedScreen {
    // 模拟屏幕初始化 (参数: gpio_backend, gpio_base)
    pub fn new(_: &str, _: &str) -> Result<Self> {
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
