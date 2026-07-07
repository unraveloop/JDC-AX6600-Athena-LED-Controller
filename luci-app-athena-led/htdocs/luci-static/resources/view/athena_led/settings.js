'use strict';
'require view';
'require form';
'require fs';
'require ui';
'require poll';
'require network';

/*
 * Athena LED Controller — JS 版设置界面 (v2.3.0)
 * 🌟 [架构迁移] 从 Lua CBI 迁移到 LuCI JS 架构:
 *   - 不再依赖 luci-compat / luci-lua-runtime，彻底解决 QWRT 等新固件上
 *     "装完 Lua 插件整个管理页崩溃" 的兼容性问题
 *   - 旧 Lua 界面文件保留在 luasrc/ 目录中未安装，如需回退见 Makefile 注释
 */

// 读取服务运行状态: 优先读程序自己写的 PID 文件，再校验 /proc/<pid> 存活
function getServiceStatus() {
	return fs.read('/var/run/athena-led.pid').then(function(pid) {
		pid = (pid || '').trim();
		if (!pid || !/^\d+$/.test(pid))
			return { running: false };
		return fs.stat('/proc/' + pid + '/stat').then(
			function() { return { running: true, pid: pid }; },
			function() { return { running: false }; }
		);
	}).catch(function() {
		return { running: false };
	});
}

function callInitAction(action) {
	return fs.exec('/etc/init.d/athena_led', [action]).then(function(res) {
		if (res.code !== 0)
			throw new Error(_('Command failed with code %d').format(res.code));
		ui.addNotification(null, E('p', _('Service action "%s" executed.').format(action)), 'info');
	}).catch(function(e) {
		ui.addNotification(null, E('p', e.message), 'error');
	});
}

// 主菜单：显示模块选项 (与后端 scheduler.rs 的模块名一一对应)
function addModuleOptions(o) {
	// 1. 组合类
	o.value('time_group', _('🕒 Time & Date'));
	o.value('weather', _('⛅ Local Weather'));

	// 2. 系统核心
	o.value('cpu', _('💻 CPU Load'));
	o.value('mem', _('💾 RAM Usage'));
	o.value('load', _('⚙️ System Load'));
	o.value('temp_single', _('🌡️ Single Temp'));
	o.value('uptime', _('⏱️ System Uptime'));

	// 3. 网络与流量 (网卡可选)
	o.value('traffic_split', _('🌐 Realtime Speed (DL/UL)'));
	o.value('netspeed_down', _('⬇️ Download Speed'));
	o.value('netspeed_up', _('⬆️ Upload Speed'));
	o.value('traffic_down', _('📥 Total Downloaded'));
	o.value('traffic_up', _('📤 Total Uploaded'));
	o.value('traffic_total', _('📊 Total Traffic (DL+UL)'));

	// 4. 接口与拓展
	o.value('nic', _('🔌 NIC Status'));
	o.value('ip', _('🌍 WAN IP'));
	o.value('dev', _('📱 Online Devices'));
	o.value('banner', _('📝 Custom Text'));
	o.value('http_custom', _('🔗 HTTP API'));
	o.value('stock', _('📈 Stock Trend'));

	// 🌟 5. [v2.3.0 新增] 实用工具模块
	o.value('countdown', _('📆 Countdown (D-Day)'));
	o.value('ping', _('🛰️ Network Latency'));
	o.value('conn', _('🔗 Connection Count'));

	// 🌟 6. [v2.4.0 新增]
	o.value('lunar', _('🏮 Lunar Date'));
	o.value('sun', _('🌅 Sunrise / Sunset'));
	o.value('mqtt', _('📨 MQTT Message'));

	// 7. 动画播放
	o.value('anim', _('🎬 Animation (.bin)'));
}

// ==========================================
// 🌟 [v2.4.0 修复] 二级参数按模块类型联动
// 以前是一个混装全部选项的大杂烩下拉 (老 CBI 单列布局的历史包袱)；
// 现在拆成多个控件、共写同一个 UCI 字段 'param' (ucioption)，
// depends 联动后只显示与所选模块相关的选项，无参数模块不显示参数框。
// ==========================================
function addModuleParams(s, netDevices, animFiles) {
	var o;

	// 所有变体的公共属性
	function bind(opt) {
		opt.ucioption = 'param';  // 共写 UCI 'param' 字段
		opt.retain = true;        // 切换模块时不互相删除对方写的值
		opt.rmempty = true;
		opt.modalonly = true;     // 只在编辑弹窗里出现，表格列保持干净
		return opt;
	}

	// 🕒 时间格式 (time_group)
	o = bind(s.option(form.ListValue, 'param_time', _('Time Format')));
	o.depends('module', 'time_group');
	o.default = 'timeBlink';
	o.value('timeBlink', _('⌚ Blink colon (Default)'));
	o.value('time_sec', _('⌚ HH:MM:SS'));
	o.value('time', _('⌚ Static HH:MM'));
	o.value('date', _('📅 MM-DD'));
	o.value('date_Y', _('📅 YYYY.MM.DD'));
	o.value('weekday', _('🗓️ Week & Time (Cycle)'));
	o.value('week_only', _('🗓️ Day of Week'));

	// 🌡️ 温度传感器 (temp_single)
	o = bind(s.option(form.ListValue, 'param_temp', _('Sensor')));
	o.depends('module', 'temp_single');
	o.default = '4';
	o.value('4', _('🌡️ CPU'));
	o.value('0', _('🌡️ NSS-Top'));
	o.value('1', _('🌡️ NSS'));
	o.value('2', _('🌡️ Wi-Fi PHY0'));
	o.value('3', _('🌡️ Wi-Fi PHY1'));
	o.value('5', _('🌡️ LPASS'));
	o.value('6', _('🌡️ DDR'));

	// 🌐 网卡 (流量/网速类模块)
	o = bind(s.option(form.Value, 'param_net', _('Interface')));
	['traffic_split', 'netspeed_down', 'netspeed_up',
	 'traffic_down', 'traffic_up', 'traffic_total'].forEach(function(mod) {
		o.depends('module', mod);
	});
	o.placeholder = 'br-lan';
	netDevices.forEach(function(dev) {
		var name = dev.getName();
		if (/^(lo$|sit|gre|ifb|ip6|teql|erspan|miireg|phy)/.test(name))
			return;
		o.value(name);
	});

	// 🎬 动画文件 (anim)
	o = bind(s.option(form.ListValue, 'param_anim', _('Animation File')));
	o.depends('module', 'anim');
	animFiles.forEach(function(entry) {
		if (entry.name && entry.name.match(/\.bin$/))
			o.value(entry.name);
	});

	// 📆 倒数日目标 (countdown)
	o = bind(s.option(form.Value, 'param_countdown', _('Target Date')));
	o.depends('module', 'countdown');
	o.placeholder = '2027-06-07';
	o.value('2027-06-07', _('One-off: 2027-06-07'));
	o.value('01-01', _('Yearly: every Jan 1st'));
	o.description = _('YYYY-MM-DD for a one-off date, MM-DD to repeat every year.');

	// 🛰️ 延迟目标 (ping)
	o = bind(s.option(form.Value, 'param_ping', _('Ping Target')));
	o.depends('module', 'ping');
	o.placeholder = '223.5.5.5:80';
	o.value('223.5.5.5:80', 'AliDNS');
	o.value('114.114.114.114:80', '114DNS');
	o.description = _('host[:port], measured via TCP connect. Empty = AliDNS.');

	// 🌅 日出日落坐标 (sun)
	o = bind(s.option(form.Value, 'param_sun', _('Coordinates')));
	o.depends('module', 'sun');
	o.placeholder = _('empty = auto (IP geolocation)');
	o.value('39.90,116.40', _('Beijing'));
	o.value('31.23,121.47', _('Shanghai'));
	o.value('23.13,113.26', _('Guangzhou'));
	o.description = _('"lat,lon" or leave empty for IP-based location.');
}

return view.extend({
	load: function() {
		return Promise.all([
			network.getDevices().catch(function() { return []; }),
			fs.list('/etc/athena_led/anim').catch(function() { return []; })
		]);
	},

	render: function(data) {
		var netDevices = data[0] || [];
		var animFiles = data[1] || [];
		var m, s, o;

		m = new form.Map('athena_led', _('Athena LED Controller'),
			_('JDCloud AX6600 LED Screen Controller (v2.3.0 — dual GPIO backend, split packages, JS UI)'));

		// ============================================================
		// 板块 1: 基础设置
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('General Settings'));
		s.addremove = false;

		o = s.option(form.Flag, 'enabled', _('Enabled'));
		o.rmempty = false;

		o = s.option(form.ListValue, 'light_level', _('Brightness Level'));
		o.default = '5';
		for (var i = 0; i <= 7; i++)
			o.value(String(i), String(i));

		// 🌟 [v2.3.1] 定时亮度: 夜间自动降低亮度 (不熄屏)
		o = s.option(form.Flag, 'night_light_enable', _('Night Brightness'));
		o.default = '0';
		o.rmempty = false;
		o.description = _('Automatically dim the screen during the configured period (screen stays on).');

		o = s.option(form.ListValue, 'night_light_level', _('Night Brightness Level'));
		o.default = '1';
		for (var j = 0; j <= 7; j++)
			o.value(String(j), String(j));
		o.depends('night_light_enable', '1');

		o = s.option(form.Value, 'night_start', _('Dim Start Time'));
		o.placeholder = '22:00';
		o.depends('night_light_enable', '1');
		o.description = _('HH:MM format, supports crossing midnight.');

		o = s.option(form.Value, 'night_end', _('Dim End Time'));
		o.placeholder = '07:00';
		o.depends('night_light_enable', '1');

		o = s.option(form.Value, 'button_gpio', _('Physical Button GPIO'));
		o.default = '71';
		o.datatype = 'uinteger';
		o.description = _('TLMM pin offset of the physical screen button. Default is 71 for JDCloud AX6600. Run "find_button" in SSH to detect if unsure.');

		// 🌟 GPIO 后端 (v2.3.0 新增)
		o = s.option(form.ListValue, 'gpio_backend', _('GPIO Backend'));
		o.default = 'auto';
		o.value('auto', _('Auto (cdev first, sysfs fallback)'));
		o.value('cdev', _('Character device (/dev/gpiochipN)'));
		o.value('sysfs', _('Legacy sysfs'));
		o.description = _('Keep "auto" unless the screen stays dark.');

		// 🌟 GPIO 基址 (兼容 QWRT / iStoreOS 等不同内核的固件, 仅 sysfs 后端使用)
		o = s.option(form.Value, 'gpio_base', _('GPIO Base Address'));
		o.default = 'auto';
		o.value('auto', _('Auto Detect (Recommended)'));
		o.value('512', '512 (Kernel 6.1+)');
		o.value('432', '432 (Kernel 5.x)');
		o.value('0', '0 (Legacy Kernel)');
		o.description = _('Base number of the main gpiochip, used by the sysfs backend only. Check with: ls /sys/class/gpio/');
		o.depends('gpio_backend', 'auto');
		o.depends('gpio_backend', 'sysfs');

		o = s.option(form.ListValue, 'profile_mode', _('Button & Profile Mode'));
		o.value('single', _('Single Profile (Button skips to next module)'));
		o.value('multi', _('Multi Profile (Button switches to next channel)'));
		o.default = 'multi';
		o.description = _('Single Profile: all modules loop continuously, button skips current module. Multi Profile: modules are grouped into channels, button switches channels.');

		// ============================================================
		// 板块 2A: 单 Profile 模式表格
		// ============================================================
		var s1 = m.section(form.GridSection, 'single_module', _('Single Profile Layout'),
			_('Used ONLY when Single Profile mode is selected. Drag to reorder.'));
		s1.anonymous = true;
		s1.addremove = true;
		s1.sortable = true;
		s1.nodescriptions = true;

		o = s1.option(form.ListValue, 'module', _('Display Module'));
		addModuleOptions(o);

		addModuleParams(s1, netDevices, animFiles);

		o = s1.option(form.Value, 'duration', _('Duration (s)'));
		o.datatype = 'uinteger';
		o.default = '5';

		// ============================================================
		// 板块 2B: 多 Profile 模式表格
		// ============================================================
		var s2 = m.section(form.GridSection, 'multi_module', _('Multi Profile Layout'),
			_('Used ONLY when Multi Profile mode is selected. Group modules by assigning them to the same channel.'));
		s2.anonymous = true;
		s2.addremove = true;
		s2.sortable = true;
		s2.nodescriptions = true;

		o = s2.option(form.ListValue, 'channel', _('Channel ID'));
		for (var c = 1; c <= 8; c++)
			o.value(String(c), _('Channel ') + c);
		o.default = '1';

		o = s2.option(form.ListValue, 'module', _('Display Module'));
		addModuleOptions(o);

		addModuleParams(s2, netDevices, animFiles);

		o = s2.option(form.Value, 'duration', _('Duration (s)'));
		o.datatype = 'uinteger';
		o.default = '5';

		// ============================================================
		// 板块 3: 网络设置
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Network Settings'));
		s.addremove = false;

		o = s.option(form.Value, 'net_interface', _('Network Interface'));
		o.description = _('Interface for traffic monitoring (e.g. br-lan).');
		o.default = 'br-lan';
		netDevices.forEach(function(dev) {
			var name = dev.getName();
			if (name !== 'lo')
				o.value(name);
		});

		o = s.option(form.Value, 'wan_ip_custom_url', _('WAN IP API'));
		o.description = _('Select a preset or enter custom URL.');
		o.value('http://checkip.amazonaws.com', 'Amazon AWS (Recommended)');
		o.value('http://members.3322.org/dyndns/getip', '3322.org');
		o.value('http://ifconfig.me/ip', 'ifconfig.me');
		o.value('http://ipv4.icanhazip.com', 'icanhazip.com');
		o.default = 'http://checkip.amazonaws.com';

		// ============================================================
		// 板块 4: 传感器与天气
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Sensor & Weather'));
		s.addremove = false;

		o = s.option(form.ListValue, 'weather_source', _('Weather Source'));
		o.value('uapis', 'Uapis.cn (Recommended)');
		o.value('wttr', 'Wttr.in (Simple)');
		o.value('openmeteo', 'Open-Meteo');
		o.value('seniverse', 'Seniverse (Key Required)');
		o.default = 'uapis';

		o = s.option(form.Value, 'weather_city', _('City Name'));
		o.default = 'auto';
		o.description = _('City name (Chinese/Pinyin/English) or "auto" for IP-based location. Open-Meteo needs a real city NAME, not a numeric code.');

		o = s.option(form.ListValue, 'weather_format', _('Weather Format'));
		o.default = 'simple';
		o.value('simple', _('Simple (Static Icon + Temp)'));
		o.value('full', _('Full (Rolling Text)'));
		o.description = _('Simple mode prevents scrolling and locks position. Full mode rolls long text.');

		o = s.option(form.Value, 'seniverse_key', _('Seniverse API Key'));
		o.description = _('Apply for a free key at seniverse.com. No built-in key anymore.');
		o.depends('weather_source', 'seniverse');

		// 🌟 [v2.4.0] 温度告警
		o = s.option(form.Value, 'temp_alert', _('Temp Alert Threshold (°C)'));
		o.datatype = 'uinteger';
		o.default = '0';
		o.description = _('Blink a warning on screen when the sensor exceeds this temperature. 0 = disabled.');

		o = s.option(form.ListValue, 'temp_alert_sensor', _('Temp Alert Sensor'));
		o.default = '4';
		o.value('4', _('🌡️ CPU'));
		o.value('0', _('🌡️ NSS-Top'));
		o.value('1', _('🌡️ NSS'));
		o.value('2', _('🌡️ Wi-Fi PHY0'));
		o.value('3', _('🌡️ Wi-Fi PHY1'));
		o.value('5', _('🌡️ LPASS'));
		o.value('6', _('🌡️ DDR'));
		o.depends({ 'temp_alert': '0', '!reverse': true });

		// ============================================================
		// 板块 5: 自定义内容与拓展 API
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Custom Content & APIs'));
		s.addremove = false;

		o = s.option(form.Value, 'custom_content', _('Custom Text (banner)'));
		o.placeholder = 'Roc-Gateway';
		o.description = _('Shown when "banner" is in the profile.');

		o = s.option(form.Value, 'stock_url', _('Stock API URL (stock)'));
		o.placeholder = 'https://your-api.com/stock';
		o.description = _('Shown when "stock" is in the profile.');

		o = s.option(form.Value, 'http_url', _('HTTP Request URL (http_custom)'));
		o.placeholder = 'http://192.168.1.1/api/status';
		o.description = _('Shown when "http_custom" is in the profile.');

		o = s.option(form.Value, 'http_cache_secs', _('HTTP Cache Time (s)'));
		o.datatype = 'uinteger';
		o.default = '60';
		o.description = _('Cache API results to prevent rate limiting.');

		o = s.option(form.Value, 'http_length', _('HTTP Max Length'));
		o.datatype = 'uinteger';
		o.default = '15';
		o.description = _('Max characters to display (defaults to 15).');

		// ============================================================
		// 板块 6: 硬件 LED 状态指示灯开关
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Hardware LED Switches'));
		s.addremove = false;
		s.description = _('Check the box to Turn OFF (Disable) specific status LEDs on the router.');

		o = s.option(form.Flag, 'disable_led_clock', _('Disable Clock LED (CPU/Mem)'));
		o.rmempty = false;
		o = s.option(form.Flag, 'disable_led_medal', _('Disable Medal LED (Internet)'));
		o.rmempty = false;
		o = s.option(form.Flag, 'disable_led_up', _('Disable Upload LED (Arrow Up)'));
		o.rmempty = false;
		o = s.option(form.Flag, 'disable_led_down', _('Disable Download LED (Arrow Down)'));
		o.rmempty = false;

		// ============================================================
		// 板块 7: 定时休眠
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Scheduled Sleep'));
		s.addremove = false;

		o = s.option(form.Flag, 'enable_sleep', _('Enable Scheduled Sleep'));
		o.default = '0';
		o.rmempty = false;

		o = s.option(form.Value, 'off_time', _('Screen Off Time'));
		o.placeholder = '23:00';
		o.depends('enable_sleep', '1');
		o.description = _('HH:MM format (e.g. 23:00).');

		o = s.option(form.Value, 'on_time', _('Screen On Time'));
		o.placeholder = '07:00';
		o.depends('enable_sleep', '1');
		o.description = _('HH:MM format (e.g. 07:00).');

		// ============================================================
		// 板块 8: 自动化与集成 (v2.4.0 新增)
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Automation & Integration'));
		s.addremove = false;

		o = s.option(form.Value, 'control_port', _('Control Port'));
		o.datatype = 'port';
		o.default = '0';
		o.description = _('Runtime control interface on 127.0.0.1 (0 = disabled). Usage: echo "show 10 HELLO" | nc 127.0.0.1 PORT — commands: next / home / off / wake / toggle / light 0-7 / show SECS TEXT.');

		o = s.option(form.Value, 'mqtt_broker', _('MQTT Broker'));
		o.placeholder = '192.168.1.10:1883';
		o.description = _('host[:port]. Leave empty to disable. Received messages show via the "MQTT Message" module.');

		o = s.option(form.Value, 'mqtt_topic', _('MQTT Topic'));
		o.default = 'athena-led/display';
		o.depends({ 'mqtt_broker': '', '!reverse': true });

		o = s.option(form.Value, 'mqtt_user', _('MQTT Username'));
		o.depends({ 'mqtt_broker': '', '!reverse': true });

		o = s.option(form.Value, 'mqtt_pass', _('MQTT Password'));
		o.password = true;
		o.depends({ 'mqtt_broker': '', '!reverse': true });

		// ============================================================
		// 板块 9: 服务控制
		// ============================================================
		s = m.section(form.NamedSection, 'general', 'settings', _('Service Control'));
		s.addremove = false;

		o = s.option(form.Button, '_restart', _('Restart Service'));
		o.inputstyle = 'apply';
		o.inputtitle = _('Restart');
		o.onclick = function() { return callInitAction('restart'); };

		o = s.option(form.Button, '_stop', _('Stop Service'));
		o.inputstyle = 'remove';
		o.inputtitle = _('Stop');
		o.onclick = function() { return callInitAction('stop'); };

		// ============================================================
		// 渲染 + 顶部运行状态轮询
		// ============================================================
		return m.render().then(function(mapEl) {
			var statusText = E('em', {}, _('Collecting data...'));
			var statusBox = E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, _('Running Status')),
				E('div', { 'class': 'cbi-section-descr' }, [ statusText ])
			]);

			poll.add(function() {
				return getServiceStatus().then(function(st) {
					if (st.running) {
						statusText.innerHTML = '';
						statusText.appendChild(E('span', { 'style': 'color:green;font-weight:bold' }, _('RUNNING')));
						statusText.appendChild(document.createTextNode(' | PID: ' + st.pid));
					} else {
						statusText.innerHTML = '';
						statusText.appendChild(E('span', { 'style': 'color:red;font-weight:bold' }, _('NOT RUNNING')));
					}
				});
			}, 5);

			return E('div', {}, [ statusBox, mapEl ]);
		});
	}
});
