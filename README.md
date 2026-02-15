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

请根据您的 OpenWrt 系统版本选择对应的安装方式，无需自行编译。

#### 🅰️ 方案一：OpenWrt 23.05 及旧版 (使用 `.ipk`)
适用于大多数目前的稳定版固件。

1.  前往 **[Releases (发行版)](../../releases)** 页面下载最新的 `luci-app-athena-led_*.ipk` 文件。
2.  上传至路由器 `/tmp/` 目录。
3.  执行安装命令：
    ```bash
    opkg install /tmp/luci-app-athena-led_*.ipk
    ```

#### 🅱️ 方案二：OpenWrt 24.x / Snapshot (使用 `.apk`)
适用于最新使用 `apk` 包管理器的固件。

1.  前往 **[Releases (发行版)](../../releases)** 页面下载最新的 `luci-app-athena-led_*.apk` 文件。
2.  上传至路由器 `/tmp/` 目录。
3.  执行安装命令 (**必须添加 `--allow-untrusted` 参数**)：
    ```bash
    apk add --allow-untrusted /tmp/luci-app-athena-led_*.apk
    ```

🎉 **配置**：安装完成后刷新网页，进入 **服务 (Services) -> Athena LED** 进行配置。

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

Please choose the appropriate installation method based on your OpenWrt version. No compilation is required.

#### 🅰️ Option 1: OpenWrt 23.05 & Older (Use `.ipk`)
For current stable releases using `opkg`.

1.  Go to the **[Releases](../../releases)** page and download the latest `luci-app-athena-led_*.ipk` file.
2.  Upload it to your router's `/tmp/` directory.
3.  Run the installation command:
    ```bash
    opkg install /tmp/luci-app-athena-led_*.ipk
    ```

#### 🅱️ Option 2: OpenWrt 24.x / Snapshot (Use `.apk`)
For the latest development snapshots using the new `apk` package manager.

1.  Go to the **[Releases](../../releases)** page and download the latest `luci-app-athena-led_*.apk` file.
2.  Upload it to your router's `/tmp/` directory.
3.  Run the installation command (**Must include `--allow-untrusted` flag**):
    ```bash
    apk add --allow-untrusted /tmp/luci-app-athena-led_*.apk
    ```

🎉 **Configuration**: After installation, refresh the web interface and go to **Services -> Athena LED** to configure.

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
