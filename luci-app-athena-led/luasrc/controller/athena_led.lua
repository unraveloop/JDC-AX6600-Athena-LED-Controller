module("luci.controller.athena_led", package.seeall)

local sys = require "luci.sys"
local http = require "luci.http"

function index()
    -- 如果配置文件不存在，就不显示菜单
    local f = io.open("/etc/config/athena_led", "r")
    if not f then
        return
    end
    f:close()

    -- 1. 主菜单入口
    -- 我把它改到了 "Services" (服务) 下，这样更符合 OpenWrt 插件规范
    -- firstchild() 表示点击这个菜单时，自动跳到第一个子菜单(Settings)
    entry({"admin", "services", "athena_led"}, firstchild(), _("Athena LED"), 60).dependent = false

    -- 2. 设置页面 (Settings)
    -- 指向 model/cbi/athena_led/settings.lua
    entry({"admin", "services", "athena_led", "settings"}, cbi("athena_led/settings"), _("Base Setting"), 1)

    -- 3. 隐藏的状态查询接口 (Status API)
    -- 前端可以通过 AJAX 请求这个地址来获取运行状态
    entry({"admin", "services", "athena_led", "status"}, call("act_status")).leaf = true
end

function act_status()
    local e = {}
    e.running = false

    -- 🌟 [改进] 优先读程序自己写的 PID 文件并校验进程存活。
    -- pgrep -f 在进程崩溃循环 (procd respawn) 时会误报“运行中”，只作兜底
    local pf = io.open("/var/run/athena-led.pid", "r")
    if pf then
        local pid = pf:read("*l")
        pf:close()
        if pid and pid:match("^%d+$") then
            local cf = io.open("/proc/" .. pid .. "/cmdline", "r")
            if cf then
                local cmd = cf:read("*a") or ""
                cf:close()
                if cmd:find("athena%-led") then
                    e.running = true
                    e.pid = pid
                end
            end
        end
    end

    -- 兜底: 老版本核心程序没有写 PID 文件
    if not e.running then
        local pid = sys.exec("pgrep -f /usr/bin/athena-led | head -n 1")
        if pid and pid ~= "" then
            e.running = true
            e.pid = string.gsub(pid, "\n", "")
        end
    end

    http.prepare_content("application/json")
    http.write_json(e)
end