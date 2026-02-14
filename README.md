# OpenWrt Athena LED Controller (Enhanced)

[English](#english) | [简体中文](#简体中文)


---

<a name="简体中文"></a>
## 🇨🇳 简体中文介绍

**适用于京东云无线宝 AX6600 (雅典娜) 的终极 LED 点阵屏控制器。**

本项目基于 `haipengno1` 和 `NONGFAH` 的作品进行了深度开发。我们将核心程序与 LuCI 界面整合，并实现了一些新功能。

### ✨ 核心功能
* **网络监控**: 实时上下行网速、WAN IP 显示、ARP 在线设备数。
* **系统状态**: CPU/内存占用率、系统运行时间、温度监控。
* **极致休眠**: **零负载精准休眠** (休眠期间 CPU 0% 占用)。
* **天气集成**: 内置当地天气显示。
* **稳定性**: 修复了网速显示异常及中文字符导致的崩溃问题。

### 📥 安装方法 (推荐)

对于大多数用户，直接下载我们提供的 `.ipk` 安装包即可，无需自行编译。

1.  前往 **[Releases (发行版)](../../releases)** 页面。
2.  下载最新的 `luci-app-athena-led_*.ipk` 文件。
3.  上传至路由器 (例如 `/tmp/` 目录) 并执行安装命令：
    ```bash
    opkg install /tmp/luci-app-athena-led_*.ipk
    ```
4.  安装完成后，进入 **服务 -> Athena LED** 进行配置。

### 🏗️ 开发者 / 固件编译
如果您是固件开发者，或者希望从源码编译：
* **Rust 核心**: 请参阅 [athena-led/README.md](athena-led/README.md)
* **LuCI 界面**: 请参阅 [luci-app-athena-led/README.md](luci-app-athena-led/README.md)


---

<a name="english"></a>
## 🇬🇧 English Description

**The ultimate LED matrix controller for JDCloud AX6600 (Athena), featuring a comprehensive LuCI interface and extensive system monitoring.**

This project is a heavily modified fork based on `haipengno1` and `NONGFAH`. We have integrated the backend and frontend into a single repository and added significant new features.

### ✨ Key Features
* **Network**: Real-time Upload/Download speed, WAN IP, ARP Device Count.
* **System**: CPU/RAM usage, Uptime, Temperature.
* **Sleep Mode**: **Zero-Load Precision Sleep** (0% CPU usage during sleep).
* **Weather**: Local weather integration.
* **Stability**: Fixed traffic speed bugs and UTF-8 text crashes.

### 📥 Installation (Recommended)

For most users, you simply need to install the pre-compiled `.ipk` package.

1.  Go to the **[Releases](../../releases)** page.
2.  Download the latest file named `luci-app-athena-led_*.ipk`.
3.  Upload it to your router (e.g., to `/tmp/`) and install:
    ```bash
    opkg install /tmp/luci-app-athena-led_*.ipk
    ```
4.  Configure via **Services -> Athena LED**.

### 🏗️ For Developers / Custom Firmware
If you are building your own OpenWrt firmware or want to modify the source:
* **Rust Core**: See [athena-led/README.md](athena-led/README.md)
* **LuCI App**: See [luci-app-athena-led/README.md](luci-app-athena-led/README.md)


---

## 📜 Credits / 致谢

* **Core Logic**: Based on [NONGFAH/athena-led](https://github.com/NONGFAH/athena-led).
* **LuCI Base**: Based on [haipengno1/luci-app-athena-led](https://github.com/haipengno1/luci-app-athena-led).
* **Enhanced Features**: Implemented by **unraveloop** & Team (Network/System monitors, Weather, Precision Sleep, etc.).

## 📄 License

Licensed under the **Apache License 2.0**.
