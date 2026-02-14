# JDCloud Athena AX6600 LED Controller (Enhanced)

[English](#english) | [简体中文](#简体中文)

---
<a name="简体中文"></a>
## 🇨🇳 简体中文介绍

**适用于京东云无线宝 AX6600 (雅典娜) 的高性能 LED 控制器。**

本项目 Fork 自 [haipengno1/athena-led](https://github.com/haipengno1/athena-led)（源自 NONGFAH）。我们将 Rust 核心程序与 LuCI 界面整合，并新增了大量系统监控功能。

### ✨ 功能列表

**新增监控功能**
* **实时网速**: 区分显示上传和下载速度
* **在线设备**: 基于 ARP 表显示当前局域网设备数。
* **WAN IP**: 显示当前 IP 地址。
* **系统状态**: 实时 CPU 负载、内存占用率、系统运行时间。
* **定时休眠**: 使用非corntab方法实现LED定时休眠。
* **天气显示**: 内置当地天气获取功能。

**基础显示与控制**
* **时间日期**: 支持多种格式及闪烁特效。
* **硬件信息**: 显示系统温度传感器数据。
* **自定义文本**: 支持自定义静态文字，或通过 HTTP/GET 获取远程内容（修复中文显示 Bug）。
* **硬件控制**: 支持亮度调节及侧边灯控制。
* **LuCI 管理**: 原生 OpenWrt 网页配置界面。

### 📥 安装说明

1.  前往 [Releases](../../releases) 下载最新的 `.ipk` 安装包。
2.  上传至路由器并安装。
3.  进入 **服务 -> Athena LED** 进行配置。

---


<a name="english"></a>
## 🇬🇧 English Description

**A high-performance LED matrix controller for the JDCloud AX6600 (Athena) router.**

This project is a fork of [haipengno1/athena-led](https://github.com/haipengno1/athena-led) (based on `athena-led` by NONGFAH). It integrates the Rust backend with the LuCI interface and adds significant system monitoring capabilities.

### ✨ Features

**System & Network Monitoring (New)**
* **Real-time Traffic**: Separate Upload / Download speed display.
* **Device Count**: Shows online devices based on ARP table.
* **WAN IP**: Displays current public IP address.
* **System Status**: CPU Load, RAM Usage, and System Uptime.
* **Zero-Load Sleep**: Completely suspends the process during sleep hours (0% CPU usage).
* **Weather**: Integrated local weather display.

**Basic Display & Control**
* **Time & Date**: Supports blinking effects and various formats.
* **Hardware Info**: System temperature monitoring.
* **Custom Text**: Supports static text and remote HTTP/GET content.
* **Control**: Adjustable brightness and side LED control.
* **LuCI Web UI**: Full configuration via OpenWrt web admin.

### 📥 Installation

1.  Download the `.ipk` file from [Releases](../../releases).
2.  Install via `opkg install` or upload to your router.
3.  Configure at **Services -> Athena LED**.

---


## 🏗️ Development & Building / 开发与构建

If you are a developer or want to compile from source, please refer to the specific documentation below:
如果您是开发者或希望从源码编译，请参阅以下详细文档：

* **Rust Core Binary**: [athena-led/README.md](athena-led/README.md)
    * *How to cross-compile the backend binary using Docker/Cargo.*
    * *如何使用 Docker/Cargo 交叉编译后端二进制文件。*

* **LuCI Interface**: [luci-app-athena-led/README.md](luci-app-athena-led/README.md)
    * *How to build the IPK package using OpenWrt SDK.*
    * *如何使用 OpenWrt SDK 编译 IPK 安装包。*


## 📜 Credits / 致谢

This project is built upon the excellent work of the following authors. We deeply appreciate their contributions to the community.
本项目基于以下作者的优秀工作，我们深表感谢。

### 1. Original Creator (NONGFAH)
* **Core Logic**: [NONGFAH/athena-led](https://github.com/NONGFAH/athena-led)
    * *The original concept and implementation.* (原创概念与实现)
* **LuCI App**: [NONGFAH/luci-app-athena-led](https://github.com/NONGFAH/luci-app-athena-led)
    * *The original LuCI interface framework.* (LuCI 界面框架雏形)

### 2. Rust Port & Refactor (haipengno1)
* **Core Logic**: [haipengno1/athena-led](https://github.com/haipengno1/athena-led)
    * *Ported the core logic to Rust for better performance.* (将核心重写为 Rust)
* **LuCI App**: [haipengno1/luci-app-athena-led](https://github.com/haipengno1/luci-app-athena-led)
    * *Adapted the LuCI app for the Rust version.* (适配 Rust 版本的界面)

### 3. Extended Version (Yi Liu & Team)
* **Current Fork**: This repository integrates and enhances the above projects.
    * *Implemented Real-time Network Speed (Up/Down), ARP Device Count, WAN IP, System Load (CPU/RAM), Weather Integration, Zero-Load Precision Sleep, and Stability Fixes.*
    * *实现了实时网速(上下行)、在线设备数、WAN IP、系统负载(CPU/内存)、天气集成、零负载精准休眠及稳定性修复。*

## 📄 License

Licensed under the **Apache License 2.0**.
