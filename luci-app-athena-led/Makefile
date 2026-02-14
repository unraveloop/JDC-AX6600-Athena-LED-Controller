include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-athena-led
PKG_VERSION:=0.4.0
PKG_RELEASE:=20241203

PKG_MAINTAINER:=Athena LED <https://github.com/haipengno1/athena-led>
PKG_LICENSE:=GPL-3.0-or-later
PKG_LICENSE_FILES:=LICENSE

PKG_SOURCE:=athena-led-$(ARCH)-musl-v$(PKG_VERSION).tar.gz
PKG_SOURCE_URL:=https://github.com/haipengno1/athena-led/releases/download/v$(PKG_VERSION)
PKG_HASH:=6e5929e516f713c011facbf5aa2236e2b89d1f5b8312ede83dc684830854bcf9

include $(INCLUDE_DIR)/package.mk

define Package/$(PKG_NAME)
  SECTION:=luci
  CATEGORY:=LuCI
  SUBMENU:=3. Applications
  TITLE:=LuCI Support for JDCloud AX6600 LED Screen Control
  DEPENDS:=+lua +luci-base @(aarch64||arm)
  PKGARCH:=all
endef

define Package/$(PKG_NAME)/description
  LuCI support for JDCloud AX6600 LED Screen Control.
  Features:
  - LED screen brightness control
  - Display mode selection (time, date, temperature, custom text)
  - Side LED status indicators
  - Remote text display via HTTP/GET
endef

define Build/Prepare
	mkdir -p $(PKG_BUILD_DIR)
	$(CP) ./luasrc $(PKG_BUILD_DIR)/
	$(CP) ./root $(PKG_BUILD_DIR)/
	$(CP) ./po $(PKG_BUILD_DIR)/
endef

define Build/Compile
	po2lmo ./po/zh_Hans/athena_led.po $(PKG_BUILD_DIR)/zh_Hans.lmo
endef

define Package/$(PKG_NAME)/install
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci
	$(CP) ./luasrc/* $(1)/usr/lib/lua/luci/
	
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./root/etc/init.d/athena_led $(1)/etc/init.d/
	
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./root/etc/config/athena_led $(1)/etc/config/
	
	$(INSTALL_DIR) $(1)/usr/sbin
	tar xf $(DL_DIR)/$(PKG_SOURCE) -C $(PKG_BUILD_DIR)
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/athena-led $(1)/usr/sbin/
	
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/i18n
	$(INSTALL_DATA) $(PKG_BUILD_DIR)/zh_Hans.lmo $(1)/usr/lib/lua/luci/i18n/athena_led.zh-cn.lmo
endef

$(eval $(call BuildPackage,$(PKG_NAME)))
