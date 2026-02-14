local sys = require "luci.sys"

m = Map("athena_led", translate("Athena LED Controller"), translate("JDCloud AX6600 LED Screen Ctrl"))

-- 0. 状态显示区域
m:section(SimpleSection).template = "athena_led/athena_led_status"

-- ============================================================
-- 板块 1: 基础设置 (General Settings)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("General Settings"))
s.anonymous = true
s.addremove = false

-- 启用
o = s:option(Flag, "enabled", translate("Enabled"))
o.rmempty = false

-- 亮度
o = s:option(ListValue, "light_level", translate("Brightness Level"))
o.default = "5"
for i = 0, 7 do
    o:value(tostring(i), tostring(i))
end
o.description = translate("Adjust brightness (0-7).")

-- 轮播间隔
o = s:option(Value, "duration", translate("Loop Interval (s)"))
o.datatype = "uinteger"
o.default = "5"
o.description = translate("Time in seconds to display each module.")

-- 显示顺序
o = s:option(DynamicList, "display_order", translate("Display Order & Modules"))
o.description = translate("Add modules and drag to reorder.")
o:value("date", translate("Date (MM-DD)"))
o:value("time", translate("Time (HH:MM)"))
o:value("timeBlink", translate("Time (Blink)"))
o:value("uptime", translate("System Uptime"))
o:value("weather", translate("Weather"))
o:value("cpu", translate("CPU Load"))
o:value("mem", translate("RAM Usage"))
o:value("temp", translate("Temperatures"))
o:value("ip", translate("WAN IP"))
o:value("dev", translate("Online Devices (ARP)"))
o:value("netspeed_down", translate("Realtime Speed (RX)"))
o:value("netspeed_up", translate("Realtime Speed (TX)"))
o:value("traffic_down", translate("Total Traffic (RX)"))
o:value("traffic_up", translate("Total Traffic (TX)"))
o:value("banner", translate("Custom Text"))
o:value("http_custom", translate("HTTP Request Result"))
o.default = {"banner", "timeBlink", "weather", "cpu", "mem"}


-- ============================================================
-- 板块 2: 网络设置 (Network Settings)
-- 重新调用 m:section 指向同一个 'general' 配置，但生成新的视觉板块
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Network Settings"))
s.anonymous = true
s.addremove = false

-- 网口选择
o = s:option(Value, "net_interface", translate("Network Interface"))
o.description = translate("Interface for traffic monitoring (e.g. br-lan).")
o.default = "br-lan"
for _, dev in ipairs(sys.net.devices()) do
    if dev ~= "lo" then o:value(dev) end
end

-- WAN IP 接口
o = s:option(Value, "wan_ip_custom_url", translate("WAN IP API"))
o.description = translate("Select a preset or enter custom URL.")
o:value("http://checkip.amazonaws.com", "Amazon AWS (Recommended)")
o:value("http://members.3322.org/dyndns/getip", "3322.org")
o:value("http://ifconfig.me/ip", "ifconfig.me")
o:value("http://ipv4.icanhazip.com", "icanhazip.com")
o.default = "http://checkip.amazonaws.com"


-- ============================================================
-- 板块 3: 传感器与天气 (Sensor & Weather)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Sensor & Weather"))
s.anonymous = true
s.addremove = false

-- 温度传感器
o = s:option(MultiValue, "temp_sensors", translate("Temperature Sensors"))
o.widget = "checkbox"
o.default = "4"
o:value("0", translate("nss-top"))
o:value("1", translate("nss"))
o:value("2", translate("wcss-phya0"))
o:value("3", translate("wcss-phya1"))
o:value("4", translate("cpu"))
o:value("5", translate("lpass"))
o:value("6", translate("ddrss"))
o.description = translate("Select sensors to cycle through.")

-- 天气源
o = s:option(ListValue, "weather_source", translate("Weather Source"))
o:value("wttr", "Wttr.in (Simple)")
o:value("openmeteo", "Open-Meteo")
o:value("seniverse", "Seniverse (Key Required)")
o:value("uapis", "Uapis.cn")
o.default = "wttr"

-- 城市
o = s:option(Value, "weather_city", translate("City Name"))
o.default = "Shenzhen"
o.description = translate("Pinyin or English.")

-- 天气格式
o = s:option(ListValue, "weather_format", translate("Weather Format"))
o.default = "simple"
o:value("simple", translate("Simple (Icon + Temp)")) 
o:value("full", translate("Full (Original)"))
o.description = translate("Simple mode keeps only the first number to fit screen.")

-- API Key
o = s:option(Value, "seniverse_key", translate("Seniverse API Key"))
-- 这里依赖的是 ListValue (单选)，所以 depends 是有效的
o:depends("weather_source", "seniverse") 


-- ============================================================
-- 板块 4: 自定义内容 (Custom Content)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Custom Content"))
s.anonymous = true
s.addremove = false

-- [关键修改] 去掉了 depends，确保输入框永远显示
-- 自定义文本
o = s:option(Value, "custom_content", translate("Custom Text"))
o.placeholder = "Roc-Gateway"
o.description = translate("Effective only when 'Custom Text' is added to Display Order.")

-- HTTP 请求
o = s:option(Value, "http_url", translate("HTTP Request URL"))
o.placeholder = "http://192.168.1.1/api/status"
o.description = translate("Effective only when 'HTTP Request Result' is added to Display Order.")

-- [新增] HTTP 截断长度设置
o = s:option(Value, "http_length", translate("HTTP Max Length"))
o.datatype = "uinteger" -- 限制只能填正整数
o.default = "15"        -- 默认 15 个字
o.description = translate("Max characters to display (defaults to 15). Set higher for longer text.")


-- ============================================================
-- 板块 5: 定时休眠 (Scheduled Sleep)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Scheduled Sleep"))
s.anonymous = true
s.addremove = false

-- 总开关
o = s:option(Flag, "enable_sleep", translate("Enable Scheduled Sleep"))
o.default = "0"
o.rmempty = false

-- 开始时间
o = s:option(Value, "off_time", translate("Screen Off Time"))
o.placeholder = "23:00"
-- 这里依赖的是 Flag (0/1)，所以 depends 是有效的
o:depends("enable_sleep", "1")
o.description = translate("HH:MM format (e.g. 23:00).")

-- 结束时间
o = s:option(Value, "on_time", translate("Screen On Time"))
o.placeholder = "07:00"
o:depends("enable_sleep", "1")
o.description = translate("HH:MM format (e.g. 07:00).")


-- ============================================================
-- 板块 6: 服务控制 (Service Control)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Service Control"))
s.anonymous = true
s.addremove = false

-- 重启按钮
btn_restart = s:option(Button, "_restart", translate("Restart Service"))
btn_restart.inputstyle = "apply"
btn_restart.description = translate("Force restart the process immediately.")
function btn_restart.write(self, section)
    luci.sys.call("/etc/init.d/athena_led restart >/dev/null 2>&1")
    luci.http.redirect(luci.dispatcher.build_url("admin", "services", "athena_led", "settings"))
end

-- 停止按钮
btn_stop = s:option(Button, "_stop", translate("Stop Service"))
btn_stop.inputstyle = "remove"
btn_stop.description = translate("Stop the process (Will restart on reboot if Enabled is checked).")
function btn_stop.write(self, section)
    luci.sys.call("/etc/init.d/athena_led stop >/dev/null 2>&1")
    luci.http.redirect(luci.dispatcher.build_url("admin", "services", "athena_led", "settings"))
end

return m