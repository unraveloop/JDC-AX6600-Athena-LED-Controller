## 项目说明

本项目是 [NONGFAH/luci-app-athena-led](https://github.com/NONGFAH/luci-app-athena-led) 的 fork 版本，专门为适配 [athena-led](https://github.com/haipengno1/athena-led) 的 Rust 版本进行了修改。

# luci-app-athena-led

OpenWrt LuCI 界面的京东云 AX6600 LED 屏幕控制插件。

## 功能特点

- LED 屏幕亮度控制
- 多种显示模式选择：
  - 日期显示
  - 时间显示（普通/闪烁）
  - 温度显示（支持多个温度传感器）
  - 自定义文本显示
  - 远程文本显示（通过 HTTP/GET）
- 侧边 LED 状态指示灯控制
- 支持显示轮播和时间间隔设置

## 依赖说明

- lua
- luci-base

## 安装

### 从源码编译

1. 将代码克隆到 OpenWrt 的 package 目录下
2. 在 menuconfig 中选择 LuCI -> Applications -> luci-app-athena-led
3. 编译并安装

注意：插件会自动从 [athena-led releases](https://github.com/haipengno1/athena-led/releases) 下载对应架构的二进制文件

## 使用说明

1. 基础设置
   - 启用/禁用插件
   - 设置显示亮度（数值越大越亮）
   - 设置刷新时间间隔

2. 显示模式
   - 选择需要显示的内容类型
   - 可以设置多个显示内容轮播显示

3. 自定义显示
   - 在自定义文本模式下可输入自定义内容
   - 支持通过 HTTP/GET 请求获取远程文本内容

[推荐固件下载地址](https://github.com/VIKINGYFY/OpenWRT-CI/releases)