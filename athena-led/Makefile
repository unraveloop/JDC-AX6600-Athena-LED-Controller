# 🌟 [包拆分] athena-led 核心包 (OpenWrt 打包用，与同目录的 Rust 工程共存)
# 负责: 预编译二进制 / init 脚本 / UCI 配置 / 预置动画 / find_button 探测脚本
# 界面部分见 luci-app-athena-led (纯 UI，PKGARCH=all，依赖本包)

include $(TOPDIR)/rules.mk

PKG_NAME:=athena-led
PKG_VERSION:=2.3.1
PKG_RELEASE:=1

PKG_MAINTAINER:=unraveloop <https://github.com/unraveloop>
PKG_LICENSE:=Apache-2.0
PKG_LICENSE_FILES:=LICENSE

PKG_SOURCE:=athena-led-aarch64-unknown-linux-musl-v$(PKG_VERSION).tar.gz
PKG_SOURCE_URL:=https://github.com/unraveloop/JDC-AX6600-Athena-LED-Controller/releases/download/v$(PKG_VERSION)/
PKG_HASH:=skip

include $(INCLUDE_DIR)/package.mk

define Package/$(PKG_NAME)
	SECTION:=utils
	CATEGORY:=Utilities
	TITLE:=JDCloud AX6600 (Athena) LED matrix screen daemon
	URL:=https://github.com/unraveloop/JDC-AX6600-Athena-LED-Controller
	DEPENDS:=@(aarch64||arm)
endef

define Package/$(PKG_NAME)/description
Rust daemon driving the JDCloud AX6600 (Athena) LED matrix screen.
v2.3.0: dual GPIO backend (character device /dev/gpiochipN with
sysfs fallback), works across QWRT / iStoreOS / official OpenWrt
kernels without hardcoded GPIO numbers.
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)
	tar xf $(DL_DIR)/$(PKG_SOURCE) -C $(PKG_BUILD_DIR)
endef

# 二进制为预编译产物，无需编译步骤
define Build/Compile
endef

define Package/$(PKG_NAME)/conffiles
/etc/config/athena_led
endef

define Package/$(PKG_NAME)/install
	# 1. 主程序
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/athena-led $(1)/usr/bin/

	# 2. 物理按键探测脚本 (去掉 .sh 后缀，提升命令逼格)
	$(INSTALL_BIN) ./files/find_button.sh $(1)/usr/bin/find_button

	# 3. init.d 守护脚本
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/athena_led.init $(1)/etc/init.d/athena_led

	# 4. UCI 配置文件
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./files/athena_led.config $(1)/etc/config/athena_led

	# 5. 预置动画 (.bin)
	$(INSTALL_DIR) $(1)/etc/athena_led/anim
	$(CP) ./files/anim/*.bin $(1)/etc/athena_led/anim/ 2>/dev/null || true
endef

define Package/$(PKG_NAME)/postinst
#!/bin/sh
if [ -z "$${IPKG_INSTROOT}" ]; then
	# apk / opkg 升级时的配置文件接管 (沿用旧版逻辑)
	if [ -f /etc/config/athena_led.apk-new ]; then
		mv /etc/config/athena_led /etc/config/athena_led.v1_bak 2>/dev/null
		mv /etc/config/athena_led.apk-new /etc/config/athena_led
	fi
	if [ -f /etc/config/athena_led-opkg ]; then
		mv /etc/config/athena_led /etc/config/athena_led.v1_bak 2>/dev/null
		mv /etc/config/athena_led-opkg /etc/config/athena_led
	fi
	/etc/init.d/athena_led enable 2>/dev/null
	/etc/init.d/athena_led restart >/dev/null 2>&1
fi
exit 0
endef

define Package/$(PKG_NAME)/prerm
#!/bin/sh
if [ -z "$${IPKG_INSTROOT}" ]; then
	/etc/init.d/athena_led stop >/dev/null 2>&1
	/etc/init.d/athena_led disable 2>/dev/null
fi
exit 0
endef

$(eval $(call BuildPackage,$(PKG_NAME)))
