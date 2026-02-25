include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-athena-led
PKG_VERSION:=2.0.2
PKG_RELEASE:=1

PKG_MAINTAINER:=unraveloop <https://github.com/unraveloop>
PKG_LICENSE:=Apache-2.0
PKG_LICENSE_FILES:=LICENSE

PKG_SOURCE:=athena-led-aarch64-unknown-linux-musl-v$(PKG_VERSION).tar.gz

PKG_SOURCE_URL:=https://github.com/unraveloop/JDC-AX6600-Athena-LED-Controller/releases/download/v$(PKG_VERSION)/

PKG_HASH:=skip

include $(INCLUDE_DIR)/package.mk

define Package/$(PKG_NAME)
	SECTION:=luci
	CATEGORY:=LuCI
	SUBMENU:=3. Applications
	TITLE:=LuCI Support for Athena LED (Enhanced)
	DEPENDS:=+lua +luci-base @(aarch64||arm)
	PKGARCH:=all
endef

define Package/$(PKG_NAME)/description
	Enhanced LuCI support for JDCloud AX6600 (Athena) LED Screen Control.
	
	v2.0.0 Epic Update:
	- Single/Multi-profile seamless switching via physical button.
	- Drag-and-drop table UI for easy module management.
	- Simple/Full dual-mode weather animations.
	- New modules: time_sec, weekday, nic, traffic_split, and more.
	- Hardware LED switches and HTTP caching.
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)
	$(CP) ./luasrc $(PKG_BUILD_DIR)/
	$(CP) ./root $(PKG_BUILD_DIR)/
	$(CP) ./po $(PKG_BUILD_DIR)/
endef

define Build/Compile
	[ -d ./po/zh_Hans ] && po2lmo ./po/zh_Hans/athena_led.po $(PKG_BUILD_DIR)/zh_Hans.lmo || true
endef

define Package/$(PKG_NAME)/install
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci
	$(CP) ./luasrc/* $(1)/usr/lib/lua/luci/
	
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./root/etc/init.d/athena_led $(1)/etc/init.d/
	
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./root/etc/config/athena_led $(1)/etc/config/
	
	$(INSTALL_DIR) $(1)/usr/bin
	tar xf $(DL_DIR)/$(PKG_SOURCE) -C $(PKG_BUILD_DIR)
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/athena-led $(1)/usr/bin/
	
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/i18n
	[ -f $(PKG_BUILD_DIR)/zh_Hans.lmo ] && $(INSTALL_DATA) $(PKG_BUILD_DIR)/zh_Hans.lmo $(1)/usr/lib/lua/luci/i18n/athena_led.zh-cn.lmo || true
endef

# =========================================================
# 🌟 安装后脚本 (Post-install)：完美解决跨版本配置覆盖与缓存清理
# =========================================================
define Package/$(PKG_NAME)/postinst
#!/bin/sh
[ -n "$${IPKG_INSTROOT}" ] || {
	# 1. 兼容新型 apk 包管理器
	if [ -f /etc/config/athena_led.apk-new ]; then
		mv /etc/config/athena_led /etc/config/athena_led.v1_bak 2>/dev/null
		mv /etc/config/athena_led.apk-new /etc/config/athena_led
	fi

	# 2. 兼容传统的 opkg 包管理器
	if [ -f /etc/config/athena_led-opkg ]; then
		mv /etc/config/athena_led /etc/config/athena_led.v1_bak 2>/dev/null
		mv /etc/config/athena_led-opkg /etc/config/athena_led
	fi

	# 3. 强制清理 LuCI 缓存，让新版界面立刻生效
	rm -rf /tmp/luci-indexcache /tmp/luci-modulecache/* 2>/dev/null
	
	# 4. 尝试重启服务以加载 v2.0.0 新配置
	/etc/init.d/athena_led restart >/dev/null 2>&1
}
exit 0
endef

$(eval $(call BuildPackage,$(PKG_NAME)))