
# Athena LED Core (Rust Backend)

[English](#english) | [简体中文](#简体中文)

---
<a name="简体中文"></a>

## 🇨🇳 简体中文说明

**京东云 AX6600 LED 屏的核心驱动程序 (Rust 实现)。**

本项目是 [haipengno1/athena-led](https://github.com/haipengno1/athena-led) 的增强版 Fork。该程序负责与底层 LED 驱动通信，并实现了各种高级监控逻辑。

### 🚀 核心功能增强

相比原版，本核心程序增加了以下底层支持：

* **精准休眠**: 新增 `--sleep-start` 和 `--sleep-end` 参数，支持计算唤醒时间并挂起进程（0% CPU 占用）。
* **稳定性修复**: 新增 `--http-length` 参数。
* **高级监控**: 内置了 ARP 设备数统计、WAN IP 获取、上下行网速计算、CPU/内存负载及天气获取等的底层实现。

### 📥 下载二进制文件

请前往项目主页的 **[Releases (发行版)](../../releases)** 页面下载编译好的二进制文件或 IPK 安装包。

### 🛠️ 源码编译说明

**目标架构**: `aarch64-unknown-linux-musl`

#### 方法 1: 使用 Docker (推荐)

使用项目自带的脚本进行容器化编译（无需配置本地环境）：

```bash
./scripts/aarch64-unknown-linux-musl-build.sh

```

编译完成的文件位于：`output/aarch64-unknown-linux-musl/athena-led`

#### 方法 2: 使用 Cargo Cross

如果你本地安装了 Rust 和 cross 工具链：

```bash
cross build --target aarch64-unknown-linux-musl --release

```

编译完成的文件位于：`target/aarch64-unknown-linux-musl/release/athena-led`

### ⚙️ 命令行参数说明

该程序通常由 LuCI 界面自动调用，但你也可以手动运行进行调试：

```bash
athena-led [选项]

```

| 参数 (Flag) | 默认值 | 说明 | 对应 Rust 字段 |
| --- | --- | --- | --- |
| **基础设置** |  |  |  |
| `--seconds <NUM>` | `5` | 每个模块显示的持续时间 (秒) | `seconds` |
| `--light-level <NUM>` | `5` | 屏幕亮度等级 (0-7) | `light_level` |
| `--night-start <HH:MM>` | `""` | 夜间低亮度时段开始 (空 = 关闭定时亮度) | `night_start` |
| `--night-end <HH:MM>` | `""` | 夜间低亮度时段结束 (支持跨午夜) | `night_end` |
| `--night-level <NUM>` | `1` | 夜间亮度等级 (0-7) | `night_level` |
| `--display-order <STR>` | *(见下方)* | **关键参数**：模块显示顺序 (空格分隔) | `display_order` |
| **硬件兼容** |  |  |  |
| `--gpio-backend <STR>` | `auto` | GPIO 后端。`auto`=优先 `/dev/gpiochipN` 字符设备、失败回退 sysfs；`cdev`/`sysfs`=强制指定 | `gpio_backend` |
| `--gpio-base <STR>` | `auto` | GPIO 基址 (仅 sysfs 后端用)。`auto`=自动探测 gpiochip base (兼容 QWRT/iStoreOS 等固件)；也可强制指定 `512`(内核6.1+) / `432`(内核5.x) / `0`(老内核) | `gpio_base` |
| `--button-gpio <NUM>` | `71` | 物理按键的 TLMM 引脚偏移 | `button_gpio` |
| **新功能模块 (v2.3.0)** |  |  |  |
| `countdown:<日期>` | - | 倒数日模块。`countdown:2027-06-07`(一次性) 或 `countdown:01-01`(每年循环)，显示 `D-123` | - |
| `ping:<host[:port]>` | - | 网络延迟模块 (TCP 连接耗时)。留空默认阿里 DNS，显示 `P:23ms` | - |
| `conn` | - | 连接数模块 (nf_conntrack)，显示 `CT:1234` | - |
| **新功能模块 (v2.4.0)** |  |  |  |
| `lunar` | - | 农历日期，显示 `L:5.7`(五月初七)，闰月 `L:R6.15` | - |
| `sun[:lat,lon]` | - | 日出日落，显示 `6:02~19:23`。参数留空用 IP 定位 | - |
| `mqtt` | - | 显示 MQTT 订阅收到的最新消息 (需配置 broker) | - |
| **自动化 (v2.4.0)** |  |  |  |
| `--temp-alert <°C>` | `0` | 温度告警阈值，超过时插播闪烁警示 (0=关，3°C 滞回) | `temp_alert` |
| `--temp-alert-sensor <ID>` | `4` | 告警监控的 thermal_zone 编号 | `temp_alert_sensor` |
| `--alert-wan` | 关 | 断网闪烁 `NET DOWN` / 恢复显示 `NET OK` (v2.5.0) | `alert_wan` |
| `--alert-newdev` | 关 | 陌生设备接入时显示 `NEW xx:xx:xx` (v2.5.0) | `alert_newdev` |
| `--alert-ip` | 关 | 公网 IP 变化时上屏新地址 (v2.5.0, 需启用 ip 模块) | `alert_ip` |
| `--control-port <PORT>` | `0` | 运行时控制接口 (仅监听 127.0.0.1)。用法: `echo "show 10 HI" \| nc 127.0.0.1 端口`，指令: next / home / off / wake / toggle / light 0-7 / show 秒 文本 | `control_port` |
| `--mqtt-broker <host[:port]>` | `""` | MQTT broker 地址 (空 = 关闭) | `mqtt_broker` |
| `--mqtt-topic <topic>` | `athena-led/display` | 订阅主题，收到的消息经 `mqtt` 模块上屏 | `mqtt_topic` |
| **网络与系统** |  |  |  |
| `--net-interface <STR>` | `br-lan` | 用于检测网速的网络接口名称 | `net_interface` |
| `--ip-url <URL>` | *(见代码)* | 用于查询 WAN IP 的 API 地址 | `ip_url` |
| `--temp-flag <ID>` | `4` | 温度传感器 ID  | `temp_flag` |
| **休眠模式** |  |  |  |
| `--sleep-start <TIME>` | `""` | 开始休眠时间 (格式 HH:MM，如 23:00) | `sleep_start` |
| `--sleep-end <TIME>` | `""` | 唤醒时间 (格式 HH:MM，如 07:00) | `sleep_end` |
| **天气设置** |  |  |  |
| `--weather-source <STR>` | `uapis` | 天气数据源 (如 `seniverse`, `uapis`) | `weather_source` |
| `--weather-city <STR>` | `Beijing` | 城市名称 (拼音) | `weather_city` |
| `--seniverse-key <STR>` | *(测试Key)* | 心知天气 API 密钥 | `seniverse_key` |
| `--weather-format <STR>` | `simple` | 天气显示格式 | `weather_format` |
| **自定义内容** |  |  |  |
| `--custom-text <STR>` | `""` | 自定义静态文本内容 | `custom_text` |
| `--custom-http-url <URL>` | `""` | 自定义 HTTP 文本获取地址 | `custom_http_url` |
| `--http-length <NUM>` | `15` | HTTP 文本截断长度 (防中文崩溃) | `http_length` |
| `--stock-url <URL>` | `""` | 股票/基金信息获取地址 | `stock_url` |

---

### 2. 可用的模块关键字 (`--display-order`)


| 关键字 (Keyword) | 功能描述 |
| --- | --- |
| `date` | 当前日期 (MM-DD) |
| `time` | 当前时间 (HH:MM) |
| `timeBlink` | 带闪烁冒号的时间 (HH:MM) |
| `weather` | 当地天气信息 |
| `stock` | 股票/基金信息 |
| `uptime` | 系统运行时间 |
| `netspeed_down` | 实时**下载**速度 |
| `netspeed_up` | 实时**上传**速度 |
| `cpu` | CPU 占用率 |
| `mem` | 内存占用率 (推测代码中有) |
| `wan_ip` | 公网 IP 地址 (推测代码中有) |
| `arp` | 在线设备数 (推测代码中有) |
| `temp` | 系统温度 |
| `custom_text` | 自定义静态文本 |
| `http_custom` | 自定义 HTTP 文本 |



<a name="english"></a>
## 🇬🇧 English Description

**The Rust-based core driver for the JDCloud AX6600 LED Matrix.**

This is a heavily enhanced fork of [haipengno1/athena-led](https://github.com/haipengno1/athena-led). It handles the low-level communication with the LED driver and implements advanced monitoring logic.

### 🚀 New Features in This Core

Compared to the original version, this core binary includes:

* **Precision Sleep**: Zero-load sleep logic (suspend process) controlled by `--sleep-start` and `--sleep-end`.
* **Stability Fixes**: Added `--http-length` to prevent panics when truncating multi-byte characters (e.g., Chinese).
* **Advanced Monitors**: Native implementations for ARP device counting, WAN IP, Network Speed (Up/Down), CPU/RAM load, and Weather.

### 📥 Download

Please download the compiled binary (or the full IPK package) from the **[Project Releases](../../releases)** page.

### 🛠️ Build from Source

**Target Architecture**: `aarch64-unknown-linux-musl`

#### Method 1: Docker (Recommended)

Use the included script to build in a clean container environment without setting up local tools:

```bash
./scripts/aarch64-unknown-linux-musl-build.sh

```

The binary will be output to: `output/aarch64-unknown-linux-musl/athena-led`

#### Method 2: Cargo Cross

If you have the Rust toolchain and `cross` installed locally:

```bash
cross build --target aarch64-unknown-linux-musl --release

```

The binary will be in: `target/aarch64-unknown-linux-musl/release/athena-led`

### ⚙️ CLI Usage

```bash
athena-led [OPTIONS]

```

#### 1. General & Display

| Option | Default | Description |
| --- | --- | --- |
| `--seconds <U64>` | `5` | Display duration for each module (seconds). |
| `--light-level <U8>` | `5` | Brightness level (0-7). |
| `--display-order <STR>` | *(See below)* | **Key Parameter!** Space-separated list of modules to display. |
| **Default Order**: |  | `"date timeBlink weather stock uptime netspeed_down netspeed_up cpu"` |

#### 2. Sleep Mode

| Option | Default | Description |
| --- | --- | --- |
| `--sleep-start <HH:MM>` | `""` | Start time for zero-load sleep (e.g., `23:00`). |
| `--sleep-end <HH:MM>` | `""` | Wake up time (e.g., `07:00`). |

#### 3. Network & System

| Option | Default | Description |
| --- | --- | --- |
| `--net-interface <STR>` | `br-lan` | Network interface for traffic monitoring. |
| `--ip-url <URL>` | *(See code)* | API URL to fetch Public/WAN IP. |
| `--temp-flag <ID>` | `4` | Temperature sensor ID (4=CPU). |

#### 4. Weather, Custom Content & Stock

| Option | Default | Description |
| --- | --- | --- |
| `--weather-source <STR>` | `uapis` | Weather provider (`seniverse` or `uapis`). |
| `--weather-city <STR>` | `Beijing` | City name for weather. |
| `--seniverse-key <STR>` | *(Test Key)* | API Key for Seniverse (XinZhi) weather. |
| `--weather-format <STR>` | `simple` | Weather display format. |
| `--custom-text <STR>` | `""` | Static custom text content. |
| `--custom-http-url <URL>` | `""` | URL to fetch dynamic text content. |
| `--http-length <NUM>` | `15` | Max characters for HTTP text (prevents crash). |
| `--stock-url <URL>` | `""` | URL for stock market data. |

---

### 📋 Available Keywords for `--display-order`

Use these keywords to customize your display loop:

| Keyword | Description |
| --- | --- |
| `time` | Current time (HH:MM). |
| `timeBlink` | Time with blinking colon (HH:MM). |
| `date` | Current date (MM-DD). |
| `weather` | Local weather info. |
| `netspeed_up` | Real-time Upload speed. |
| `netspeed_down` | Real-time Download speed. |
| `cpu` | CPU usage percentage. |
| `mem` | RAM usage percentage. |
| `uptime` | System uptime. |
| `wan_ip` | Public WAN IP address. |
| `arp` | Online device count (ARP). |
| `temp` | System temperature. |
| `stock` | Stock market info. |
| `custom_text` | Static custom text. |
| `http_custom` | Dynamic HTTP text. |


## 📄 License

Apache License 2.0





