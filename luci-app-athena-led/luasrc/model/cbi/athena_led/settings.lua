local sys = require "luci.sys"
local http = require "luci.http"         -- 🌟 防呆引入
local dispatcher = require "luci.dispatcher" -- 🌟 防呆引入
local sys = require "luci.sys"

m = Map("athena_led", translate("Athena LED Controller"), translate("JDCloud AX6600 LED Screen Controller (v2.0.0 Multi-Profile Edition)"))

-- 0. 状态显示区域
m:section(SimpleSection).template = "athena_led/athena_led_status"

-- ============================================================
-- 板块 1: 基础设置与模式切换
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("General Settings"))
s.anonymous = true
s.addremove = false

o = s:option(Flag, "enabled", translate("Enabled"))
o.rmempty = false

o = s:option(ListValue, "light_level", translate("Brightness Level"))
o.default = "5"
for i = 0, 7 do o:value(tostring(i), tostring(i)) end

-- 物理按键 GPIO 绑定
o = s:option(Value, "button_gpio", translate("Physical Button GPIO"))
o.default = "71"
o.datatype = "uinteger"
o.description = translate("GPIO pin number for the physical screen button. Default is 71 for JDCloud AX6600. Run 'find_button' in SSH to detect if unsure.")

-- 🌟 核心模式切换开关
o = s:option(ListValue, "profile_mode", translate("Button & Profile Mode"))
o:value("single", translate("Single Profile (Button skips to next module)"))
o:value("multi", translate("Multi Profile (Button switches to next channel)"))
o.default = "multi"
o.description = translate("<b>Single Profile:</b> All modules loop continuously. Button skips current module.<br/><b>Multi Profile:</b> Modules are grouped into channels. Button switches channels.")


-- ============================================================
-- 板块 2A: 单 Profile 模式表格 (极简拖拽)
-- ============================================================
s1 = m:section(TypedSection, "single_module", translate("Single Profile Layout"), translate("Used ONLY when <b>Single Profile</b> mode is selected. Drag to reorder."))
s1.template = "cbi/tblsection"
s1.anonymous = true
s1.addremove = true
s1.sortable = true  -- 开启拖拽排序



-- ==========================================
-- 🌟 1. 主菜单：极限瘦身版
-- ==========================================
local function add_module_options(opt)
    -- 1. 组合类
    opt:value("time_group", translate("🕒 Time & Date"))  -- 🌟 把 7 个选项合并成了这 1 个！
    opt:value("weather", translate("⛅ Local Weather"))
    
    -- 2. 系统核心
    opt:value("cpu", translate("💻 CPU Load"))
    opt:value("mem", translate("💾 RAM Usage"))
    opt:value("load", translate("⚙️ System Load"))
    opt:value("temp_single", translate("🌡️ Single Temp")) -- 🌟 包含二级菜单
    opt:value("uptime", translate("⏱️ System Uptime"))

    -- 3. 网络与流量 (网卡可选)
    opt:value("traffic_split", translate("🌐 Realtime Speed (DL/UL)"))
    opt:value("netspeed_down", translate("⬇️ Download Speed"))
    opt:value("netspeed_up", translate("⬆️ Upload Speed"))
    opt:value("traffic_down", translate("📥 Total Downloaded"))
    opt:value("traffic_up", translate("📤 Total Uploaded"))
    opt:value("traffic_total", translate("📊 Total Traffic (DL+UL)"))
    
    -- 4. 接口与拓展
    opt:value("nic", translate("🔌 NIC Status"))
    opt:value("ip", translate("🌍 WAN IP"))
    opt:value("dev", translate("📱 Online Devices"))
    opt:value("banner", translate("📝 Custom Text"))
    opt:value("http_custom", translate("🔗 HTTP API"))
    opt:value("stock", translate("📈 Stock Trend"))
end

-- ==========================================
-- 🌟 2. 二级参数菜单：加入时间格式选项
-- ==========================================
local function add_module_params(section)
    -- [新增] 时间与日期专属下拉框
    local o_time = section:option(ListValue, "param_time", translate("Display Format"))
    o_time:depends("module", "time_group")
    o_time.default = "timeBlink"
    o_time:value("timeBlink", translate("⌚ Time (Blink)"))
    o_time:value("time_sec", translate("⌚ Time (HH:MM:SS)"))
    o_time:value("time", translate("⌚ Time (Static)"))
    o_time:value("date", translate("📅 Date (MM-DD)"))
    o_time:value("date_Y", translate("📅 Date (YYYY.MM.DD)"))
    o_time:value("weekday", translate("🗓️ Week & Time (Cycle)"))
    o_time:value("week_only", translate("🗓️ Day of Week"))

    -- [保留] 单体温度专属下拉框
    local o_temp = section:option(ListValue, "param_temp", translate("Target Sensor"))
    o_temp:depends("module", "temp_single")
    o_temp.default = "4"
    o_temp:value("0", translate("NSS-Top"))
    o_temp:value("1", translate("NSS"))
    o_temp:value("2", translate("Wi-Fi PHY0"))
    o_temp:value("3", translate("Wi-Fi PHY1"))
    o_temp:value("4", translate("CPU"))
    o_temp:value("5", translate("LPASS"))
    o_temp:value("6", translate("DDR"))

    -- [保留] 独立网卡专属下拉框
    local o_net = section:option(Value, "param_net", translate("Target Interface"))
    o_net:depends("module", "netspeed_down")
    o_net:depends("module", "netspeed_up")
    o_net:depends("module", "traffic_down")
    o_net:depends("module", "traffic_up")
    o_net:depends("module", "traffic_total")
    o_net:depends("module", "traffic_split")
    o_net.placeholder = "br-lan"
    for _, dev in ipairs(sys.net.devices()) do
        if dev ~= "lo" then o_net:value(dev) end
    end
end

o = s1:option(ListValue, "module", translate("Display Module"))
add_module_options(o)

add_module_params(s1)

o = s1:option(Value, "duration", translate("Duration (s)"))
o.datatype = "uinteger"
o.default = "5"


-- ============================================================
-- 板块 2B: 多 Profile 模式表格 (高级通道隔离)
-- ============================================================
s2 = m:section(TypedSection, "multi_module", translate("Multi Profile Layout"), translate("Used ONLY when <b>Multi Profile</b> mode is selected. Group modules by assigning them to the same channel."))
s2.template = "cbi/tblsection"
s2.anonymous = true
s2.addremove = true
s2.sortable = true


o = s2:option(ListValue, "channel", translate("Channel ID"))
for i = 1, 8 do o:value(tostring(i), translate("Channel ") .. i) end
o.default = "1"

o = s2:option(ListValue, "module", translate("Display Module"))
add_module_options(o)

add_module_params(s2)

o = s2:option(Value, "duration", translate("Duration (s)"))
o.datatype = "uinteger"
o.default = "5"


-- ============================================================
-- 板块 2: 网络设置 (Network Settings)
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

-- ============================================================
-- ⚠️ [V2.0 遗留兼容] 以下全局温度传感器多选框已在前端隐藏
-- V2.0 起已由 temp_single 模块的动态二级菜单完全接管。
-- 保留代码注释，仅供向下兼容及未来调试参考。
-- ============================================================
--[[
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
]]--

-- 天气源
o = s:option(ListValue, "weather_source", translate("Weather Source"))
o:value("wttr", "Wttr.in (Simple)")
o:value("openmeteo", "Open-Meteo")
o:value("seniverse", "Seniverse (Key Required)")
o:value("uapis", "Uapis.cn")
o.default = "uapis"

-- 城市
o = s:option(Value, "weather_city", translate("City Name"))
o.default = "auto"
o.description = translate("Pinyin, English, or 'auto'.")

-- 天气格式
o = s:option(ListValue, "weather_format", translate("Weather Format"))
o.default = "simple"
o:value("simple", translate("Simple (Static Icon + Temp)")) 
o:value("full", translate("Full (Rolling Text)"))
o.description = translate("Simple mode prevents scrolling and locks position. Full mode rolls long text.")

-- API Key
o = s:option(Value, "seniverse_key", translate("Seniverse API Key"))
o:depends("weather_source", "seniverse") 


-- ============================================================
-- 板块 4: 自定义内容与拓展 API (Custom Content & APIs)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Custom Content & APIs"))
s.anonymous = true
s.addremove = false

-- 自定义文本
o = s:option(Value, "custom_content", translate("Custom Text (banner)"))
o.placeholder = "Roc-Gateway"
o.description = translate("Shown when 'banner' is in the profile.")

-- 股票 API
o = s:option(Value, "stock_url", translate("Stock API URL (stock)"))
o.placeholder = "https://your-api.com/stock"
o.description = translate("Shown when 'stock' is in the profile.")

-- HTTP 请求
o = s:option(Value, "http_url", translate("HTTP Request URL (http_custom)"))
o.placeholder = "http://192.168.1.1/api/status"
o.description = translate("Shown when 'http_custom' is in the profile.")

-- HTTP 缓存时间
o = s:option(Value, "http_cache_secs", translate("HTTP Cache Time (s)"))
o.datatype = "uinteger"
o.default = "60"
o.description = translate("Cache API results to prevent rate limiting.")

-- HTTP 截断长度设置
o = s:option(Value, "http_length", translate("HTTP Max Length"))
o.datatype = "uinteger"
o.default = "15"
o.description = translate("Max characters to display (defaults to 15).")


-- ============================================================
-- 板块 5: 硬件 LED 状态指示灯开关 (Hardware LED Switch)
-- ============================================================
s = m:section(NamedSection, "general", "settings", translate("Hardware LED Switches"))
s.anonymous = true
s.addremove = false
s.description = translate("Check the box to <b>Turn OFF (Disable)</b> specific status LEDs on the router.")

o = s:option(Flag, "disable_led_clock", translate("Disable Clock LED (CPU/Mem)"))
o.rmempty = false
o = s:option(Flag, "disable_led_medal", translate("Disable Medal LED (Internet)"))
o.rmempty = false
o = s:option(Flag, "disable_led_up", translate("Disable Upload LED (Arrow Up)"))
o.rmempty = false
o = s:option(Flag, "disable_led_down", translate("Disable Download LED (Arrow Down)"))
o.rmempty = false


-- ============================================================
-- 板块 6: 定时休眠 (Scheduled Sleep)
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
o:depends("enable_sleep", "1")
o.description = translate("HH:MM format (e.g. 23:00).")

-- 结束时间
o = s:option(Value, "on_time", translate("Screen On Time"))
o.placeholder = "07:00"
o:depends("enable_sleep", "1")
o.description = translate("HH:MM format (e.g. 07:00).")


-- ============================================================
-- 板块 7: 服务控制 (Service Control)
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
    luci.http.redirect(luci.dispatcher.build_url("admin", "services", "athena_led"))
end

-- 停止按钮
btn_stop = s:option(Button, "_stop", translate("Stop Service"))
btn_stop.inputstyle = "remove"
btn_stop.description = translate("Stop the process (Will restart on reboot if Enabled is checked).")
function btn_stop.write(self, section)
    luci.sys.call("/etc/init.d/athena_led stop >/dev/null 2>&1")
    luci.http.redirect(luci.dispatcher.build_url("admin", "services", "athena_led"))
end

return m