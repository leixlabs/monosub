# 本机可运行的安装包（沙盒 / 非沙盒）

> 回答一个问题：**`release:mas` 出的 `.pkg` 装完为什么打不开，怎么才能本地跑起来 —— 而且要带沙盒。**
> 相关文档：[mac-app-store.md](./mas-app-store.md)（上架流程）。

---

## 1. 为什么 MAS 的 pkg 装了打不开

不是签名"没签好"，而是**签错用途**。App Store 分发包有两件东西让系统只允许它从商店启动：

| 元凶 | 具体是什么 | 后果 |
|------|-----------|------|
| 证书 | `3rd Party Mac Developer Application`（门户里叫 **Mac App Distribution**） | 这是商店分发专用身份，launchd 不放行本地双击 |
| `Contents/embedded.provisionprofile` | `distribution.provisionprofile` | 系统按"App Store 安装态"校验收据，本地没有收据 → 拒绝启动 |

双击后只得到一句 `The application "MonoSub" can't be opened.`，控制台里能看到
`amfid` / `taskgated` 拒绝的痕迹：

```bash
log show --last 5m --predicate 'process == "amfid"' --info | grep -i monosub
```

所以本地版必须：**换成开发证书 + 不塞分发 profile**。沙盒与这两件事无关，可以保留。

---

## 2. 两种模式

| | `bun run release:pkg` | `bun run release:pkg:sandbox` | `bun run release:mas` |
|---|---|---|---|
| 沙盒 | 关 | **开**（与 MAS 包逐条一致） | 开 |
| 证书 | Apple Development / Developer ID | 同左 | 3rd Party Mac Developer |
| embedded profile | 不塞，且主动删除残留 | 同左 | 塞 distribution |
| 本机双击启动 | ✅ | ✅ | ❌ |
| 产物 | `build/pkg/MonoSub-<ver>-local.pkg` | `build/pkg/MonoSub-<ver>-local-sandbox.pkg` | `src-tauri/target/mas/MonoSub.pkg` |
| 用途 | 自用、内测分发 | **沙盒自测 / 审核前预演** | 上传 App Store |

```bash
bun run release:pkg:sandbox              # 构建 + 签名 + 出 pkg
bun run release:pkg:sandbox -- --install # 出包后装到 /Applications 并启动
bun run release:pkg:sandbox -- --skip-build   # 只重签 + 出包，省一次 cargo build
```

> **不要用 `sudo` 跑。** codesign 要用的私钥在**你自己的登录钥匙串**里，root 的钥匙串
> 没有它，表现是签第一个 dylib 就失败（底层 `errSecInternalComponent`）。真正需要
> 管理员权限的只有安装那一步，`--install` 会用系统授权面板自己提权。
> 已经用 sudo 跑过一次的话，`dist/`、`build/pkg/`、`src-tauri/target/local*/` 会变成
> root 属主，脚本会给出对应的 `sudo chown -R $(id -u):$(id -g) <目录>` 修复命令。

沙盒版装的还是 `MonoSub.app`，容器同样是 `~/Library/Containers/leixlabs.monosub`——
跑起来的行为就是上架包的行为。

---

## 3. 沙盒版为什么不用 `application-identifier`

`src-tauri/entitlements/MonoSub.mas.entitlements` 里有两条：

```xml
<key>com.apple.application-identifier</key>
<key>com.apple.developer.team-identifier</key>
```

它们属于 **restricted entitlement**：只能由内置的 provisioning profile 授予。
MAS 版靠 distribution profile 提供；本地版刻意不塞 profile，因此也不能声明它们
（声明了反而会在 exec 阶段因"无效授权"被杀）。

于是沙盒版用 `src-tauri/entitlements/MonoSub.sandbox.local.entitlements`：
**去掉这两条，其余逐条与 MAS 版相同**（app-sandbox、network client/server、
user-selected read-write、bookmarks.app-scope）。

对本项目没有实际损失：没有 iCloud / App Group / Keychain access group 需求，
`application-identifier` 只用于商店校验和 receipt 绑定。

配置在 `src-tauri/tauri.sandbox-local.conf.json`（与 `tauri.mas.conf.json` 同构，
只换了 entitlements 路径）。

---

## 4. 签名顺序（脚本内部，不要手工改）

```
动态库（46 个 ggml/llama dylib）
  → 随包可执行（audiocpp_cli / llama-server / ffmpeg / ffprobe / yt-dlp / monosub-cli，
    沙盒模式下带 com.apple.security.inherit）
  → app 本体（用沙盒 entitlements 封签）
  → codesign --verify --deep --strict
  → productbuild --component → .pkg（有 installer 证书就签）
```

脚本会硬拦三种不该出现的情况：

1. 沙盒版签完却没有 `app-sandbox`
2. 非沙盒版签完却带上了 `app-sandbox`
3. 签完仍残留 `Contents/embedded.provisionprofile`

---

## 5. 出包的三个隐形坑（都吃过一次）

这三件事的共同点是：**退出码全是 0，屏幕上写着"安装完成"，实际什么都没装上**。
单看终端日志完全看不出来，只能查 `/var/log/install.log`。

### 5.1 包版本 = App 版本 → Installer 静默跳过 payload

`productbuild --component` 把包版本取自 App 的 `CFBundleShortVersionString`（`2.0.2`）。
上一次安装留下了同版本收据（`pkgutil --pkg-info leixlabs.monosub` 会显示 `version: 2.0.2`），
于是 Installer 判定"已安装"，整包跳过。

修法：改用 `pkgbuild --version <编译时刻>`，版本单调递增，永远算升级。

### 5.2 Relocation —— 装到了构建目录自己身上

```
PackageKit: Applications/MonoSub.app relocated to
  Users/lei/.../src-tauri/target/local-sandbox/MonoSub.app
```

PackageKit 会按 bundle id 在磁盘上找"另一份同名 bundle"（就是仓库里刚构建的那份），
把安装目标改成它，于是 payload 覆盖了暂存目录，`/Applications` 依然为空。
线索来自 `PackageInfo` 里的 `<relocate>` / `<upgrade-bundle>` / `<strict-identifier>`。

修法：`pkgutil --expand` → 删掉这几个元素 → `pkgutil --flatten` → `productsign` 重签。
没有命令行开关可以关掉它，只能在包内容上动手。

### 5.3 `codesign -dv` 不打 `Authority`

用 `codesign -dv` 检查"装进去的是不是本次签的身份"会恒为假：默认详细级别只输出
`Executable` / `Identifier` / `TeamIdentifier`。必须 `codesign -dv --verbose=2`
（或 `-dvv`）才会打印 `Authority=` 行。

---

## 6. 沙盒自测清单

沙盒版的价值就在这里——以下每条都是"非沙盒下正常、沙盒下失败"的候选：

```bash
log stream --predicate 'process == "sandboxd"' --info | grep leixlabs.monosub
```

1. 拖入 `.mp4` → 字幕写回**源文件目录**（沙盒下这是最可能失败的一条）
2. 模型下载 / 删除 / 重装（network.client + 现有容器权限）
3. 指定自定义模型目录 → **退出重启** → 还能读到（bookmarks.app-scope）
4. 翻译：`llama-server` 子进程 + `$TMPDIR/monosub-llama-*.sock`（helper inherit；**不需要** network.server）
5. URL 导入：平台字幕命中、音频下载兜底两条路
6. 通知、托盘、窗口透明

彻底重来（清空容器授权与缓存）：

```bash
rm -rf ~/Library/Containers/leixlabs.monosub
```

---

## 7. 发给别人 / 真正分发

当前账号（Team `RF4CPKUXQ7`）只有 Development / Mac Development / Mac App Distribution /
Mac Installer Distribution，**没有 Developer ID**。所以：

- 本机、同 Team 的开发机：直接双击 ✅
- 其他任何人的机器：会被 Gatekeeper 拦，只能 `xattr -dr com.apple.quarantine <pkg>` 或右键打开 ⚠️

要免掉这一步，去 developer.apple.com 另签两张：

| 证书 | 用途 | 脚本读取 |
|------|------|---------|
| **Developer ID Application** | 签 App | 自动优先选它，并开启 Hardened Runtime + 安全时间戳 |
| **Developer ID Installer** | 签 pkg | 自动优先选它 |

然后：

```bash
NOTARY_PROFILE=<keychain-profile> bun run release:pkg:sandbox --notarize
# 先存凭据（一次即可）：
xcrun notarytool store-credentials <keychain-profile> \
  --apple-id you@example.com --team-id RF4CPKUXQ7 --password <App 专用密码>
```

拿到 Developer ID 后，`--timestamp` / Hardened Runtime / `stapler staple` 全部自动打开，
脚本里对应分支已就绪。

---

## 8. 排查表

| 现象 | 原因 | 处理 |
|------|------|------|
| 提示"安装完成"，但 `/Applications` 里没有 App | 同版本收据让 Installer 跳过 payload；或发生了 relocation | 看 `/var/log/install.log` 里的 `relocated to` / `PackageKit:` 行；本节 5.1 / 5.2 |
| `codesign exited with code 1`，第一个 dylib 就挂 | 用 `sudo` 跑了脚本，root 钥匙串没有私钥 | 去掉 sudo；只让 `--install` 弹授权 |
| `签名失败 … errSecInternalComponent` | login 钥匙串锁着 / 私钥 ACL 未授权 | `security unlock-keychain`，或在「钥匙串访问」里给 codesign 授权 |
| `暂存目录被 root 占用` | 之前 sudo 跑过，`target/local*` 属主变成 root | `sudo chown -R $(id -u):$(id -g) src-tauri/target/local-sandbox` |
| 双击 App 毫无反应 / "can't be opened" | 仍带 distribution profile 或分发证书 | `codesign -dv --entitlements :- MonoSub.app`、`ls Contents/embedded.provisionprofile` |
| 装完打开就闪退 | 沙盒缺权限（`sandboxd` deny） | 上面第 5 节日志；对照 MAS entitlements 是否少项 |
| 安装器提示"未签名" | 没有 installer 类证书 | 本机安装可无视；或 `--no-pkg-sign` |
| 换机器后打不开 | 开发证书不受信 / 未公证 | `xattr -dr com.apple.quarantine <pkg>`；长期解需 Developer ID |
| 模型目录每次重启失效 | 缺 `files.bookmarks.app-scope` | 检查 entitlements 文件 |
