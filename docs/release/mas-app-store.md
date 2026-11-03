# MonoSub 上架 Mac App Store 全流程

> 面向本仓库当前状态（v2.0.2，`leixlabs.monosub`，Team `RF4CPKUXQ7`）。
> 目标：产出可上传的 `.pkg` 并走完审核。
> 与 DMG 直发不同，App Store 版本**强制沙盒**，这一步决定了改造量。

---

## 0. 先看这 6 件事，决定这个项目能不能一次过

| # | 事项 | 现状 | 结论 |
|---|------|------|------|
| 1 | `com.apple.security.app-sandbox` | 无 entitlements 文件 | **必须加**，已补 `src-tauri/entitlements/` |
| 2 | 随包 6 个可执行（audiocpp_cli / llama-server / ffmpeg / ffprobe / yt-dlp / monosub-cli） | 无签名（ad-hoc） | 必须逐个签 + `inherit`，脚本已处理 |
| 3 | 字幕默认写回**源文件所在目录** | `transcribe.rs:950 / 1643` 用 `output.parent()` | ⚠️ 沙盒下**会失败**，必须改 |
| 4 | 自定义模型目录存明文路径 | `model_dir.rs` → `settings.json` | ⚠️ 重启后失效，需 security-scoped bookmark |
| 5 | `macOSPrivateApi: true` + `transparent: true` | 已开启 | ⚠️ 中等风险（Guideline 2.5.1 私有 API） |
| 6 | 运行时下载 GGUF 模型 | 有 | ✅ 可接受（模型是**数据**，不是可执行代码）；App 无自动更新器，符合要求 |

真正可能卡住审核的是 **URL 导入（yt-dlp 抓 B 站字幕/音频）**：
Guideline 5.2.3 对"从第三方平台下载媒体"的判定很严。若被拒，App Store 版应只保留"抓取平台**官方字幕**"，
砍掉"下载音频再本地转写"的兜底（或在审核备注里明确说明用途），代码侧可用一个 `mas` cargo feature 隔离。

---

## 1. 账号后台准备（约 20 分钟，一次性）

### 1.1 确认 Team ID

```bash
security find-identity -v -p codesigning
# "Apple Development: leixlabs@gmail.com (66KANS54LB)"    ← 括号里是证书 ID，不是 Team ID
```

**Team ID 要另取**——证书 subject 里的 OU 字段才是 Team ID：

```bash
security find-certificate -a -c "Apple Development" -p \
  | openssl x509 -noout -subject -nameopt sep_multiline | grep OU=
# OU=RF4CPKUXQ7      ← 这才是 leixlabs@gmail.com 的 Team ID
```

或直接在 developer.apple.com/account → **Membership details** 页面读 Team ID，
以那里为准（本机测得为 `RF4CPKUXQ7`）。

> 本机现有三张开发证书分属三个**不同**的开发者账号：
> `seven4x@qq.com` → Team `K8BQ5976GL`、`kamalei.zhang@gmail.com` → Team `8KUB9SSHZ5`、
> `leixlabs@gmail.com` → Team `RF4CPKUXQ7`。多张证书共存不会互相干扰——
> 打包脚本用 `MAS_SIGN_IDENTITY` 显式指定身份。

### 1.2 接受协议

App Store Connect → **Business**（协议、税务和银行业务）→ 接受 **Free Apps** 协议。
没接受这一条，后面连"构建版本"都选不到。

### 1.3 注册 App ID

developer.apple.com → Certificates, Identifiers & Profiles → **Identifiers** → `+`
→ **App IDs** → **App** → Bundle ID 选 *Explicit*，填 `leixlabs.monosub`
（必须与 `tauri.conf.json` 的 `identifier` 逐字符一致，否则签名能过、启动即崩。）

### 1.4 创建两张证书

**Certificates** → `+`：

| 用途 | 门户里勾选的类型 | 签发后 CN 实际长这样 |
|------|-----------------|---------------------|
| 签 App 本体 | **Mac App Distribution** | `3rd Party Mac Developer Application: Lei Zhang (RF4CPKUXQ7)` |
| 签 .pkg 容器 | **Mac Installer Distribution** | `3rd Party Mac Developer Installer: Lei Zhang (RF4CPKUXQ7)` |

⚠️ **这是最容易踩的坑**：门户里显示的是新名字（Mac App Distribution），签出来的证书
却仍是旧 CN（`3rd Party Mac Developer Application`）。签名时用的字符串必须是
`security find-identity -v -p codesigning` 打印出来的那个，不是门户里的名字。

门户里还有一项泛用的 `Apple Distribution`（iOS/macOS 通用），也能签 macOS；
但 Mac App Store 这条线用 **Mac App Distribution** 最对口。

> 证书需要 CSR：门户会要求上传 `.certSigningRequest`。可以用 Xcode 的
> Manage Certificates 代劳（私钥直接进钥匙串），也可以用 openssl 生成后手工导入
> （注意 macOS 不支持 OpenSSL 3 默认的 PBES2 加密，导出 p12 时要加 `-legacy`，
> 且口令不能为空，否则 `security import` 会报 MAC verification failed）。

### 1.5 生成 provisioning profile

**Profiles** → `+` → Distribution 里选 **Mac App Store Connect**
（自测选 Development 里的 **macOS App Development**）
→ App ID 下拉里选 `MonoSub` → 勾上 Mac App Distribution 证书 → 命名 → Generate → Download：

```
src-tauri/mas/distribution.provisionprofile   # 已在 .gitignore 里
```

本仓库现有的一份：`MonoSub MAS` / `App Store` / `RF4CPKUXQ7.leixlabs.monosub` /
2027-11-02 到期。

### 1.6 在 App Store Connect 建 App 记录

我的 App → `+` → 新 App：

- 平台：**macOS**
- 名称：`MonoSub`（被占用就换，如 `MonoSub - 本地字幕`）
- 主要语言：简体中文
- 套装 ID：`leixlabs.monosub`
- SKU：自定义，如 `monosub-mac`

---

## 2. 工程侧：已经替你加好的文件

| 文件 | 作用 |
|------|------|
| `src-tauri/entitlements/MonoSub.mas.entitlements` | App 沙盒：network client/server、user-selected 读写、bookmark |
| `src-tauri/entitlements/MonoSubHelper.mas.entitlements` | 6 个命令行工具：`app-sandbox` + `inherit` |
| `src-tauri/Info.mas.plist` | 分类 `productivity`、出口合规 `false`、版权 |
| `src-tauri/PrivacyInfo.xcprivacy` | 隐私清单（避免 ITMS-91053） |
| `src-tauri/tauri.mas.conf.json` | 只出 `.app`、挂 entitlements、`minimumSystemVersion 12.0` |
| `scripts/release/mas.mjs` | 构建 → 由内向外签名 → 校验 → 出 `.pkg` |
| `package.json` | 新增 `bun run release:mas` |

`bun tauri build`（不带 `--config`）仍然是原来的 DMG 流程，两者互不影响。

---

## 3. 沙盒适配：已完成 / 待处理

### 3.1 输出目录与中间文件 ✅ 已完成

沙盒里"用户拖进来的文件"只给**该文件**的读写权限，不给**所在目录**的写权限。
最初的表现比预想更早：ffmpeg 连 `<stem>_audio.wav` 都建不出来
（`Error opening output file … Operation not permitted`）。

现在的做法（`src-tauri/src/paths.rs` + `bookmark.rs` + `commands/grants.rs`）：

| 场景 | 行为 |
|---|---|
| 中间文件（16k wav、缩略图） | 沙盒下一律落进程 scratch 区（`$TMPDIR/monosub/<路径哈希>_…`），DMG 版仍写在源文件旁 |
| 保存位置 | 沙盒版**保留"同目录"**，但那只是意愿：选文件只给出该文件的权限，父目录仍然只读，所以写入前需要该目录的授权。`app_sandboxed` 让前端知道自己在沙盒里（`lib/grants.ts::bootstrapSandboxFlag`），DMG 版完全不受影响 |
| 任务开始前 | `ensureOutputAccess(源目录列表)`：**先弹应用内说明对话框**（`FolderGrantDialog`，讲清为什么需要授权、只问一次），用户点「授权」后再弹系统目录面板（`defaultPath` 指向该目录），`authorize_dir` 存 bookmark。授权后仍按"同目录"写 `<源目录>/<stem>.srt` |
| 用户在面板里选了别的目录 | 视为"字幕放这里"，切到 `custom` 模式，后续文件不再逐个询问（面板只弹一次） |
| 用户取消 | 不派发任务，行内显示 `errors.outputFolderRequired`——绝不在没有权限时开跑 |
| 输出目录不可写（兜底） | bookmark 失效等情况仍返回 `OUTPUT_NOT_WRITABLE:<目录>`，绝不静默改存到别处；前端重授权后重试该文件 |
| 下次启动 | `main.rs` setup 调 `activate_grants()` 重放所有 bookmark，**不再询问**；启动前那次探写也会因此通过 |

DMG 版不受影响：`paths::is_sandboxed()` 为假时 `authorize_dir` 直接返回，什么都不存。

### 3.2 自定义模型目录 ✅ 已完成

沙盒下"没授权的目录"和"空目录"对文件 API 完全一样（`exists()` 连 EPERM 都吞成 false），
所以判定不靠探测：容器之外的模型目录必须有覆盖它的 `dirGrant`，否则
`list_models` / `start_transcription` 返回 `MODELS_DIR_NOT_ACCESSIBLE:<path>`。
设置页把它显示为一句人话 + **「授予访问权限」按钮**（弹目录面板 → `authorize_dir` → 重新加载），
而不是一个点了也不会成功的「重试」。

`set_models_dir` 在验证目录的同时生成 bookmark 并写进 `settings.json` 的 `dirGrants`，
启动时统一重放，因此重启后模型目录仍然可读。授权彻底失效时该目录会被
`activate_grants` 记一条 warn，写入路径再按 3.1 的规则请求重新授权。

### 3.3 `--cookies-from-browser` ⬜ 待处理

`site/ytdlp.rs:199` 会传 `--cookies-from-browser chrome`。沙盒禁止读
`~/Library/Application Support/Google/Chrome` 与登录钥匙串，这条路径在 MAS 版必然失败。

改法：MAS 版隐藏"从浏览器读取"选项，只保留 cookies.txt（用户手动选择文件 → 有授权）。

### 3.4 `open_directory`（"在访达中显示"）✅ 已完成

实测确认失败：沙盒里 `/usr/bin/open` 拿不到 LaunchServices 的授权，「打开模型目录」直接报
`The file couldn't be opened.`。已改为 macOS 走 `NSWorkspace.openURL`（`reveal` 走
`activateFileViewerSelectingURLs`），容器内路径可以正常打开。非 macOS 仍走原来的 argv 分支。

### 3.5 首次启动的路径迁移 ⬜ 待处理

容器路径是 `~/Library/Containers/leixlabs.monosub/Data/...`，与 DMG 版的
`~/Library/Application Support/leixlabs.monosub` 不同。老用户从 DMG 版迁过来时需要提示
"模型需重新下载 / 重新指定目录"，否则会看到"模型已安装但找不到文件"。

---

## 4. 私有 API 风险（先试，被拒再改）

`tauri.conf.json` 里 `app.macOSPrivateApi: true` + 窗口 `transparent: true`，
Tauri 靠私有 API 实现透明标题栏。Guideline 2.5.1 禁止私有 API，虽多数情况检测不到，
但这是最容易"审核通过一次、后续版本被卡"的点。

被拒时的关法：**直接改 `tauri.conf.json` 本体**（不要塞进 `tauri.mas.conf.json`——
`windows` 是数组，合并行为会把整段替换，容易变成两个窗口）：

```jsonc
"app": { "macOSPrivateApi": false,
  "windows": [{ "...": "...", "transparent": false,
                "titleBarStyle": "Visible", "hiddenTitle": false }] }
```

代价是失去无边框毛玻璃观感，前端需要补一层顶部留白。

---

## 5. 本地沙盒自测（务必先做，再上传）

**不需要 dev 包。** 用分发证书签出来的包本机可以直接跑，而且照样会进沙盒——
实测已确认：`open` 之后进程起来，容器 `~/Library/Containers/leixlabs.monosub`
被创建（`Data/Library/Logs/MonoSub` 也有日志）。

```bash
bun run release:mas              # 出 target/mas/MonoSub.pkg 与 MonoSub.app
open src-tauri/target/mas/MonoSub.app
```

盯沙盒拒绝日志（另开一个终端）：

```bash
log stream --predicate 'process == "sandboxd"' --info | grep -i monosub
```

> 想知道为什么 `--dev` 会失败：`macOS App Development` profile 要求先在门户
> **Devices** 里注册这台 Mac，否则卡在 "No Devices are available"。为一个冒烟测试
> 去注册设备不值得，所以 `--dev` 只是可选路径；真要用，就得先注册设备、再用
> Mac Development 证书签（Xcode 自动生成的那张 `Lei Zhang (lei's Mac mini)`
> 私钥不在本机，不能用）。

### 重置授权再验证

反复验证「授权一次 → 之后不再问」，用仓库里的脚本，别手工删容器：

```bash
scripts/release/reset-sandbox.sh --dry-run   # 先看会做什么
scripts/release/reset-sandbox.sh             # 只清目录授权（保留设置/模型目录/日志）
scripts/release/reset-sandbox.sh --full      # 删整个容器 = 重装（bookmark 全失效）
```

三类"授权"分属不同系统，改错地方等于白忙：

| 东西 | 存在哪 | 怎么清 | 清掉之后 |
|---|---|---|---|
| 目录授权（保存位置/模型目录） | `settings.json` 的 `dirGrants`（security-scoped bookmark） | 脚本默认模式 | 下次写入时重新弹面板 |
| 通知/麦克风等隐私项 | TCC 数据库 | `tccutil reset All leixlabs.monosub` | 系统重新询问 |
| 沙盒容器本身 | `~/Library/Containers/leixlabs.monosub` | `--full` | 等于首次安装，所有 bookmark 失效（容器 UUID 变了） |

`--dry-run` 会打印当前的 `modelsDir` 与每条 `dirGrants`，跑之前先确认状态。

自测清单（每一项都可能暴露一条缺失 entitlement）：

1. 拖入 `.mp4` / `.mov` / `.m4a` → 生成字幕（看是否 `EPERM`）
2. 模型下载 + 删除 + 重装
3. 指定自定义模型目录 → **退出重启** → 还能否读到模型
4. 翻译（llama-server 子进程 + `$TMPDIR` 下的 Unix socket；不该出现 bind 失败，也不该需要 network.server）
5. URL 导入（官方字幕命中 / 音频下载兜底两条路）
6. 通知、托盘菜单、窗口透明
7. `sandboxd` 日志里没有任何 `deny` 才继续

---

## 6. 出正式包

```bash
MAS_SIGN_IDENTITY="3rd Party Mac Developer Application: Lei Zhang (RF4CPKUXQ7)" \
MAS_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Lei Zhang (RF4CPKUXQ7)" \
MAS_PROFILE=src-tauri/mas/distribution.provisionprofile \
bun run release:mas
```

产物：`src-tauri/target/mas/MonoSub.pkg`（当前 113 MB）

> ⚠️ 这个 pkg **只能上传给 App Store，装到本机双击是打不开的**：Mac App Distribution
> 证书 + 内置 distribution profile 让它只在"由商店安装"时被放行。
> 想在本机跑、而且要带沙盒（验证同一套权限）：`bun run release:pkg:sandbox`，
> 见 [local-pkg.md](./local-pkg.md)。

脚本内部顺序（**不能改**，改了就过不了校验）：
动态库 → 命令行工具（带 inherit）→ 拷 `embedded.provisionprofile` 与 `PrivacyInfo.xcprivacy`
→ 签 `.app` 本体（封签）→ `codesign --verify --deep --strict` → `productbuild` 出 pkg。

---

## 7. 上传

**方式 A（推荐）**：App Store 里的免费 App「Transporter」→ 登录 → 拖入 `.pkg` → 交付。

**方式 B（命令行）**：

```bash
xcrun altool --upload-app -f src-tauri/target/mas/MonoSub.pkg -t macos \
  -u "leixlabs@gmail.com" -p "<App 专用密码>"
```

App 专用密码在 appleid.apple.com → 登录与安全 → App 专用密码 生成。

上传后等 10–60 分钟处理完成，回 App Store Connect → 该版本 → **构建版本** → 选中它。

### 常见上传错误

| 错误码 | 含义 | 处理 |
|--------|------|------|
| ITMS-90296 | App sandbox not enabled | entitlements 没生效，检查 `bundle.macOS.entitlements` 是否被 `--config` 覆盖 |
| ITMS-90238 / 90362 | 无效签名 / 嵌套代码未签 | 用脚本重签，别手工 `--deep` |
| ITMS-91053 | 缺 required reason API 声明 | 已用 `PrivacyInfo.xcprivacy` 覆盖；不够就按日志补 reason code |
| ITMS-90095 / 90207 | Info.plist 缺分类 / 图标不符 | `LSApplicationCategoryType` 已加；图标需 1024×1024 无透明 |
| "无效的二进制文件" | 主可执行仍是 ad-hoc | `codesign -dv --verbose=4 MonoSub.app` 看 Authority |

---

## 8. App Store Connect 要填的东西

- 描述 / 关键词 / 支持网址 / **隐私政策网址**（必须有，可放到 landing_page）
- 截图：1280×800、1440×900、2560×1600、2880×1800 任一组（至少 1 张）
- 分级：4+（无用户内容）
- 价格：免费
- **App 隐私**：选"不收集数据"（本 app 全本地推理，确实不收集）
- 出口合规：`Info.mas.plist` 已声明 `ITSAppUsesNonExemptEncryption = false`

### 审核备注模板（英文，直接贴）

```
MonoSub converts local video/audio files into subtitles entirely on-device.
No account, no server, no analytics.

1. Speech recognition and translation run as bundled command-line tools
   (audio.cpp / llama.cpp) inside the app sandbox; no network inference.
2. AI models are NOT bundled (size). The user downloads model *data files*
   (GGUF weights) from within the app. These are data, not executable code;
   no new functionality is downloaded.
3. URL import only fetches a video's own subtitle track via yt-dlp.
   Optional audio download is used only when the uploader provides no subtitle,
   to transcribe it locally for the user's own accessibility use.
4. Output files are written to ~/Movies/MonoSub by default; the user may grant
   access to another folder in Settings.
```

---

## 9. 时间预期

| 阶段 | 耗时 |
|------|------|
| 后台准备（证书/profile/App 记录） | 20–30 分钟 |
| 沙盒适配（3.1–3.3） | 0.5–1.5 天（取决于是否引入 bookmark 封装） |
| 本地沙盒自测 | 半天 |
| 首次审核 | 1–3 天（被拒重提常见 1–2 轮） |

---

## 10. 建议：双轨发布

App Store 版受沙盒与 5.2.3 限制，功能会略弱（默认输出目录、无浏览器 cookie、可能砍下载兜底）。
**不要放弃官网 DMG + 公证**那条线——它没有沙盒限制，可以做自动更新、可以随意读目录。

```
bun tauri build                      # 现有 DMG 流程
xcrun notarytool submit MonoSub.dmg --keychain-profile <profile> --wait
xcrun stapler staple MonoSub.dmg
```

两个渠道共用一个代码库，靠 `mas` feature / 运行时沙盒检测分流即可。

---

## 附录 A：Xcode 到底要不要？

**不需要。** App Store Connect 是一个网站（appstoreconnect.apple.com），不是 Xcode 的功能。

| 你想做的事 | 需要 Xcode 吗 | 实际用什么 |
|---|---|---|
| 建 App 记录、填元数据、提交审核 | ❌ | 浏览器打开 appstoreconnect.apple.com |
| 建证书 / App ID / provisioning profile | ❌ | 浏览器打开 developer.apple.com/account |
| 上传 `.pkg` | ❌ | Transporter app（App Store 免费下载）或 `xcrun altool` |
| 管理本机证书（可选、更方便） | 可选 | Xcode → Settings → Accounts → Manage Certificates |
| 编译 / 调试本项目 | ❌ 做不到 | `bun tauri dev` + VS Code / RustRover |

关于「用 Xcode 打开这个项目」：仓库里**不存在** `.xcodeproj` / `.xcworkspace`
（`src-tauri/gen` 下只有 `schemas`）。Tauri 走 cargo 构建，Xcode 无法编译它——
`open -a Xcode src-tauri` 只会把目录当普通文件夹打开，没有 target、没有补全、没有产物。

唯一需要 Xcode 组件的地方是 Command Line Tools 提供的 `codesign` / `productbuild` / `xcrun`，
本机已安装（`xcode-select -p` 可确认），不需要打开 Xcode 本体。
