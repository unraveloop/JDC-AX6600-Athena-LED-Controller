# 📦 分发指南:进 iStore 商店 / 自建 opkg 软件源

> v2.4.0 起项目已完成两项前置条件:包拆分(`athena-led` 核心 + `luci-app-athena-led` 纯 UI)
> 和 LuCI JS 化(不依赖 luci-compat)。以下是把插件送进用户"软件中心"的两条路。

---

## 方案 A:提交 iStore 商店(推荐,受众最大)

iStoreOS / iStore 的第三方软件仓库是 [linkease/istore](https://github.com/linkease/istore),
社区包放在 `appstore/` 目录,每个应用一个 Makefile(引用你的 GitHub Release 产物)。

**步骤:**

1. Fork [linkease/istore](https://github.com/linkease/istore)
2. 在 `appstore/` 下新建 `luci-app-athena-led/Makefile`,内容参考仓库里现有应用
   (如 `appstore/linkease/Makefile`),核心字段:
   - `PKG_VERSION` 跟随本仓库 Release 版本
   - 依赖声明 `+athena-led`(核心包也需要一并可下载,见下方注意事项)
3. 商店入口图标与介绍:iStore 应用需要在 Makefile 中声明
   `PKG_FLAGS`、图标 URL(112x112 png,可放本仓库 `docs/icon.png`)
4. 提 PR,等 linkease 维护者审核合并

**注意事项:**
- iStore 要求包能从公网稳定下载 —— 我们的 GitHub Release 满足,但国内
  裸连 GitHub 偏慢,可以考虑同时传一份到 ghproxy 可加速的 Release 资产即可
- 首次提交时在 PR 描述里附上恩山论坛帖链接,说明用户量,有助于过审

## 方案 B:自建 opkg 软件源(用户 opkg update 直接装)

原理:把 ipk + 索引文件(`Packages` / `Packages.gz`)托管到 GitHub Pages,
用户在 `/etc/opkg/customfeeds.conf` 加一行即可。

**1. 生成索引(可加进 build-chain.yml 的收尾步骤):**

```bash
# 在 SDK 构建完成后 (sdk/bin/packages/aarch64_cortex-a53/base/ 下已有 ipk)
cd sdk/bin/packages/aarch64_cortex-a53/base
# SDK 自带索引脚本
../../../../scripts/ipkg-make-index.sh . > Packages
gzip -k Packages
```

**2. 发布到 gh-pages 分支**(workflow 中用 `peaceiris/actions-gh-pages@v4`):

```yaml
- name: Deploy opkg feed
  uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: sdk/bin/packages/aarch64_cortex-a53/base
    destination_dir: feed
```

**3. 用户侧配置(写进 README):**

```sh
echo "src/gz athena_led https://unraveloop.github.io/JDC-AX6600-Athena-LED-Controller/feed" \
  >> /etc/opkg/customfeeds.conf
# 未签名源需要关闭校验 (或用 usign 给 feed 签名后分发公钥)
echo "option check_signature 0" >> /etc/opkg.conf   # 视固件默认配置而定
opkg update && opkg install athena-led luci-app-athena-led
```

**签名(可选但推荐):** 用 `usign` 生成密钥对,workflow 里对 `Packages` 签名生成
`Packages.sig`,公钥文件随 README 发布,用户放进 `/etc/opkg/keys/`。
密钥私钥存 GitHub Secrets(如 `USIGN_SECRET_KEY`)。

## 版本发布检查单

- [ ] `athena-led/Cargo.toml`、`athena-led/Makefile`、`luci-app-athena-led/Makefile`
      三处版本号一致
- [ ] `git tag vX.Y.Z` 与 Makefile `PKG_VERSION` 一致(CI 二进制下载 URL 依赖它)
- [ ] Release 发布后确认 build-chain 工作流产出:二进制 tar.gz + 2×ipk + 2×apk + sha256
- [ ] 若走自建 feed:确认 gh-pages 的 `Packages.gz` 已更新
