# scripts/

发布一条 MonoSub 版本的全部自动化：拉取运行时二进制、构建产物、校验、上传到
GitHub Releases。

这里是两条互不相干的发布线：GitHub Releases（DMG / MSIX，`publish/`）和
Mac App Store（`.pkg`，`release/mas.mjs`）。后者强制沙盒，产物也不进 `dist/`，
SOP 见[上架 Mac App Store](#上架-mac-app-store)。

## 一种语言，两个平台

这里所有脚本都是 Node（ESM，`>=20`，`package.json` 本来就要求）。不再有
bash / PowerShell 的分裂。

以前是 macOS 一套 bash、Windows 一套 PowerShell。那带来每个流程的两份拷贝、
一个让 bash 入口把控制权交给 PowerShell 的桥，以及一类只存在于接缝处的 bug：
传给 PowerShell 的 POSIX 路径、bash 执行不了的 `.cmd` shim、被 `-Command` 吃掉
的 `-File`。Node 在两个平台上都已经是这个项目的硬性依赖，所以脚本改为在内部
分支：

```js
const platform = hostPlatform();   // 'macos' | 'windows'，其余直接 die
if (platform === 'macos')   ...
else if (platform === 'windows') ...
```

两个平台真的需要不同逻辑时，各自一个函数放在同一个文件里；需要相同逻辑时
（校验和、资源清单、release notes、版本自增）就是共享的，只调一次。

## 目录结构

```
scripts/
├── lib/                    共享代码 + 几个独立小工具
│   ├── log.mjs             日志（只在 TTY 上着色）
│   ├── platform.mjs        宿主平台、target triple、.exe 后缀
│   ├── paths.mjs           项目根、dist/、resources/、暂存目录、tauri CLI
│   ├── shell.mjs           run / capture / which，处理 Windows 的 .cmd
│   ├── fs.mjs              拷贝、chmod、UTF-8（无 BOM）写入、临时目录
│   ├── net.mjs             带重试的下载、SHA-256 校验
│   ├── archive.mjs         zip 读取/解包（纯 JS）+ tar.gz
│   ├── version.mjs         版本读取 / 同步 / 自增
│   ├── xml.mjs             AppxManifest 的最小化编辑
│   ├── ffmpeg.mjs          随包 ffmpeg 的许可证校验（必须 LGPL）
│   ├── bump-version.mjs    独立工具：自增版本号
│   ├── check-i18n-keys.mjs 独立工具：i18n key 检查
│   └── clean-temp-files.mjs 独立工具：清理历史遗留
│
├── release/                产出可发布的产物 —— 只在本机跑，不联网
│   ├── release.mjs         入口：读版本 → 打包 → 核对 dist/
│   ├── package.mjs         构建产物（macOS → DMG，Windows → MSIX）
│   ├── prepare-resources.mjs 拉取随包的运行时二进制
│   ├── verify.mjs          校验产物（DMG / MSIX）
│   ├── bundle-ffmpeg.mjs   macOS 专用：重链 ffmpeg 及其 dylib
│   ├── build-ffmpeg-lgpl-macos.mjs 从源码编 LGPL-only ffmpeg（商店版必须）
│   ├── polish-dmg.mjs      DMG 窗口几何修正（create-dmg 会覆盖掉配置）
│   ├── installer.mjs       Windows 专用：本地用的 NSIS .exe
│   ├── local-pkg.mjs       本机可启动的 .pkg（--sandbox 时行为等同 MAS 包）
│   ├── mas.mjs             Mac App Store：构建 → 逐层签名 → .pkg
│   └── reset-sandbox.sh    清容器/授权，反复验证「只问一次」
│
└── publish/
    └── publish.mjs         dist/ → GitHub Releases —— 从不构建
```

`scripts/build/` 曾经和 `scripts/release/` 分开，实际上两者只是同一个流程的两段
（打包 / 核对产物），所以合成一个 `release/`。真正独立的是 `publish/`：它只上传，
不构建。（仓库根的 `build/` 是**输出目录**，装本机安装包，和这里说的旧 `scripts/build/`
不是一回事。）

## 两步（GitHub Releases）

发布故意是两个命令，因为构建要十几分钟，上传只是一次 HTTP 调用：

```bash
node scripts/release/release.mjs      # 1. 构建 + 打包 + 校验 → dist/
node scripts/publish/publish.mjs      # 2. 把 dist/ 传上 GitHub Releases
```

构建失败不该在 GitHub 上留下半成品 release；上传失败（网络、权限、release 已存在）
也不该逼你重新构建。两步都能单独重跑，中间只靠 `dist/` 衔接。

两步都不自增版本号。release tag 必须等于打进 App 的版本号 —— App 的「检查更新」
拿自己的 `CARGO_PKG_VERSION` 跟 tag 比 —— 所以自增发生在发布**成功之后**
（`publish.mjs` 调 `lib/bump-version.mjs`，可用 `--skip-bump` 关掉）。
在构建前自增，就是当初 `tag v2.0.0` 配 `MonoSub-2.0.1.dmg` 的成因。

## 命令一览

`package.json` 里的封装：

```
bun run release           # 读版本 → 打包 → 核对 dist/
bun run release:package   # 只打包（不核对 dist/）
bun run release:resources # 只拉运行时二进制
bun run release:verify    # 校验一个已有产物
bun run publish           # node scripts/publish/publish.mjs
bun run publish:macos     # ... --platform macos
bun run publish:msix      # ... --platform windows

bun run release:mas         # Mac App Store：构建 → 逐层签名 → .pkg
bun run release:pkg         # 本机可启动的 .pkg（非沙盒）
bun run release:pkg:sandbox # 同上但开沙盒，装着 MAS 那套 entitlements
bun run release:pkg:install # release:pkg + 装到 /Applications 并启动
bun run mas:test            # 重签 .app 直接打开，跳过出包（最快的沙盒冒烟）
```

每个脚本都认 `-h` / `--help`：

| 脚本 | 参数 |
|------|------|
| `release/release.mjs` | `--skip-build`（`--skip-package` 是旧名）只核对 `dist/` 已有产物 |
| `release/package.mjs` | `--skip-resources` 复用 `resources/`；`--sign` 额外产出 dev-signed 副本（仅 Windows） |
| `release/prepare-resources.mjs` | 无参数，幂等 |
| `release/verify.mjs` | `<path-to-dmg-or-msix>`，失败即非零退出 |
| `release/mas.mjs` | `--dev` `--no-pkg` `--skip-build` `--open`；env `MAS_SIGN_IDENTITY` `MAS_INSTALLER_IDENTITY` `MAS_PROFILE` `MAS_BUILD_NUMBER`；构建号自动 +1，记在仓库根 `.mas-build-number` |
| `release/local-pkg.mjs` | `--sandbox` `--skip-build` `--no-pkg` `--no-pkg-sign` `--ad-hoc` `--identity` `--notarize` `--install` `--open`；env `LOCAL_SIGN_IDENTITY` `LOCAL_INSTALLER_IDENTITY` `LOCAL_BUILD_NUMBER` `NOTARY_PROFILE` |
| `release/reset-sandbox.sh` | `--grants`（默认）`--full` `--dry-run` `--yes` |
| `publish/publish.mjs` | `--version` `--platform` `--recreate` `--add-assets` `--skip-bump` `--draft` `--asset` `--notes-file` |
| `lib/bump-version.mjs` | 无参数；发布成功后由 `publish.mjs` 调用 |

产物命名（两边都不一样，别凭印象）：

- macOS → `dist/MonoSub-<version>.dmg`
- Windows → `dist/monosub_<version>.0_x64.msix`
  （`*.signed.msix` 只是本地 `Add-AppxPackage` 用的副本，不算发布产物）
- 本机安装包 → `build/pkg/MonoSub-<version>-local[-sandbox].pkg`
  （放 `build/` 而不是 `dist/`：`tauri build` 前的 `bun run build` 会清空 `dist/`，
  两个模式先后跑一次就会互相抹掉对方的 pkg）
- Mac App Store → `src-tauri/target/mas/MonoSub.pkg`（`--dev` 时在 `target/mas/dev/`）

## 上架 Mac App Store

GitHub Releases 那条线到 `dist/` 就结束了。商店是另一条：产物是 `.pkg`，走 App Store
Connect，`publish.mjs` 永远不碰它 —— 别把 `.pkg` 塞进 Releases。

麻烦在于商店版**强制沙盒**，而沙盒失败几乎不会在 `tauri dev` 或 DMG 版里出现。所以这条
线要出三次包，各有各的用途。

### 三条产物线，别搞混

| 命令 | 沙盒 | 证书 | embedded profile | 本机双击 | 产物 | 用途 |
|------|------|------|------------------|----------|------|------|
| `bun run release` | 关 | ad-hoc（`signingIdentity: "-"`） | 无 | ✅ | `dist/MonoSub-<ver>.dmg` | GitHub Releases |
| `bun run release:pkg` | 关 | Apple Development / Developer ID | 主动删掉 | ✅ | `build/pkg/…-local.pkg` | 自用、内测 |
| `bun run release:pkg:sandbox` | **开**，与 MAS 逐条一致 | 同上 | 主动删掉 | ✅ | `build/pkg/…-local-sandbox.pkg` | **上传前自测** |
| `bun run release:mas` | 开 | 3rd Party Mac Developer | 塞 distribution | ❌ | `target/mas/MonoSub.pkg` | 上传 App Store |

中间那条存在的唯一理由：MAS 包在本机装不上。`3rd Party Mac Developer Application` 证书
加上 `Contents/embedded.provisionprofile`，系统只放行"由 App Store 安装"的那一份，双击
只会得到 `The application "MonoSub" can't be opened.`。`--sandbox` 用开发证书签、不塞
profile，但套的是**同一份 entitlements**（只去掉 profile 才授予的两条 restricted：
`com.apple.application-identifier` / `com.apple.developer.team-identifier`），容器还是
`~/Library/Containers/leixlabs.monosub` —— 跑起来的行为就是上架包的行为。

> `release:mas --no-pkg` 之后直接 `open` 那个 `.app` 也能冒烟（`bun run mas:test` 就是
> 这条路径），但「装到 /Applications」这一段只有 `release:pkg:sandbox` 覆盖得到。

### 一次性准备

点对点的后台操作（截图位置、协议名称）见
[docs/release/mas-app-store.md](../../docs/release/mas-app-store.md)，这里只列会和脚本
对上的四样东西：

| 东西 | 要求 |
|------|------|
| App ID | `leixlabs.monosub`，与 `tauri.conf.json` 的 `identifier` 逐字符一致 |
| 签 app 的证书 | 门户里勾 **Mac App Distribution**，签发后 CN 落成 `3rd Party Mac Developer Application: …` |
| 签 pkg 的证书 | 门户里勾 **Mac Installer Distribution**，CN 落成 `3rd Party Mac Developer Installer: …` |
| provisioning profile | Distribution → **Mac App Store Connect** → 存为 `src-tauri/mas/distribution.provisionprofile`（已在 `.gitignore`） |

证书名一律用 `security find-identity -v` 打印的**原字符串**，门户里的新名字不作数 ——
这是这条线最容易踩的坑。`mas.mjs` 会用 `security cms -D` 读 profile 里的 Team ID，
再按 team 过滤证书（本机三张开发证书分属三个账号，不按 team 过滤就会签成别人的身份）。

### 1. 本地沙盒自测（上传前必做）

```bash
bun run release:pkg:sandbox -- --install
```

构建 → 逐层签名 → 出 pkg → 弹一次管理员授权装到 `/Applications` → 启动。别用 `sudo`：
签名私钥在你的登录钥匙串里，root 读不到，表现是第一个 dylib 就 `errSecInternalComponent`。

先确认它**真的**在沙盒里跑，而不是又出了个非沙盒包：

```bash
codesign -d --entitlements :- /Applications/MonoSub.app | grep app-sandbox
ls -d ~/Library/Containers/leixlabs.monosub     # 容器出现 = 进程确实被关进盒子了
```

另开一个终端盯拒绝日志，自测全程开着：

```bash
log stream --predicate 'process == "sandboxd"' --info | grep leixlabs.monosub
```

按下面这张表过一遍 —— 每一行都对应一条 entitlement，缺了才在这里暴露：

| 动作 | 验证什么 |
|------|----------|
| 拖入 `.mp4` → 字幕写回源文件目录 | `files.user-selected.read-write`；沙盒只给文件不给父目录，靠任务前的目录面板拿一次授权 |
| 模型下载 / 删除 / 重装 | `network.client`（reqwest 直连） |
| 指定自定义模型目录 → **退出重启** → 还能读到 | `files.bookmarks.app-scope`，这是最容易漏的一条 |
| 翻译一条 | 6 个随包工具的 `inherit`；`llama-server` 走 `$TMPDIR/monosub-llama-*.sock`，**不该**需要 `network.server` |
| URL 导入：平台字幕命中 + 音频下载兜底 | yt-dlp 在容器内的网络与写权限 |
| 「在访达中显示」 | 曾死在这里：沙盒里 `/usr/bin/open` 拿不到 LaunchServices 授权，现走 `NSWorkspace` |
| 通知、托盘菜单、窗口透明 | 与沙盒无关，但和审核同一批看 |

验收标准只有一条：**日志里没有任何 `deny`**。有 deny 就别往下走，上传后一样会被拒。

反复验证「授权一次 → 以后不问」，用脚本，别手工删容器：

```bash
scripts/release/reset-sandbox.sh --dry-run   # 只打印现状：modelsDir + 每条 dirGrants
scripts/release/reset-sandbox.sh             # 只清目录授权，设置/模型目录/日志都留着
scripts/release/reset-sandbox.sh --full      # 删整个容器 = 全新安装（bookmark 全失效）
```

三类"授权"分属三个系统，清错地方等于白忙：

| 东西 | 存在哪 | 谁清 |
|------|--------|------|
| 目录授权（保存位置 / 模型目录） | `settings.json` 的 `dirGrants`（security-scoped bookmark） | 脚本默认模式 |
| 通知、麦克风等隐私项 | TCC 数据库 | 脚本顺手 `tccutil reset All` |
| 沙盒容器本身 | `~/Library/Containers/leixlabs.monosub` | 只有 `--full`；容器 UUID 会变，bookmark 一起失效 |

### 2. 出正式包

```bash
security find-identity -v -p codesigning   # app 证书在这里
security find-identity -v -p basic         # Installer 证书不在 codesigning 策略里，得查全量

MAS_SIGN_IDENTITY="3rd Party Mac Developer Application: <名字> (<TEAM>)" \
MAS_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: <名字> (<TEAM>)" \
MAS_PROFILE=src-tauri/mas/distribution.provisionprofile \
bun run release:mas
```

签名顺序由内向外，**不能改**：46 个 dylib → 6 个随包工具（`Contents/Resources/resources/bin/*`，
带 `inherit`）→ 拷 `embedded.provisionprofile` 与 `PrivacyInfo.xcprivacy` → 签 `.app` 本体
（封签）→ `codesign --verify --deep --strict` → `productbuild` 出 pkg。

脚本会硬拦四种"上传后才被退回"的情况：app 上没有 `app-sandbox`（ITMS-90296）、
entitlements 与 profile 的 `application-identifier` 不一致（ITMS-90886 —— 手工调用
`codesign` 不会像 Xcode 那样自动注入）、任一 helper 缺 `inherit`、profile 的 App ID 与
`identifier` 不一致。

**构建号不用手填。** Tauri 把 `CFBundleVersion` 写成和营销版本一样（`2.0.2`），而
App Store Connect 要求每次上传的构建号递增，否则判"构建已存在"。脚本在仓库根维护
`.mas-build-number`：读到 N 就取 N+1，**先写回文件再封签**——这次构建失败也只是跳过
一个号，不会撞号。

```bash
cat .mas-build-number        # 上一个已上传的构建号
```

这个文件是本地状态（已 gitignore，不随仓库分发），初值为 `4`。`--dev` 不消耗计数；
偶尔要指定（比如接别人的号）仍然可以用 `MAS_BUILD_NUMBER=9`，指定值会写回文件成为
新基准。**注意**：脚本只能保证"本机不重复"，真正上传到 App Store Connect 的那一步在
Transporter 里，所以别在两台机器上同时出包。

### 3. 上传

**Transporter**（App Store 免费下载）拖入 `.pkg` → 交付，最省事。命令行等价物：

```bash
xcrun altool --upload-app -f src-tauri/target/mas/MonoSub.pkg -t macos \
  -u "<Apple ID>" -p "<App 专用密码>"
```

等 10–60 分钟处理完，回 App Store Connect → 该版本 → **构建版本** → 选中它 → 提交审核。
审核备注模板（解释随包的命令行工具、运行期下载的是模型**数据**而不是可执行代码）在
`docs/release/mas-app-store.md` §8，直接贴。

### 4. 被退回时先查这几个码

| 码 | 含义 | 先看哪里 |
|----|------|----------|
| ITMS-90296 | App sandbox not enabled | `--config` 是否把 entitlements 覆盖掉了 |
| ITMS-90886 | application-identifier 缺失 / 不一致 | `MAS_PROFILE` 与 entitlements 里的 team、bundle |
| ITMS-90238 / 90362 | 无效签名 / 嵌套代码未签 | 别手工 `--deep`，用脚本重签 |
| ITMS-91053 | 缺 required reason API 声明 | `src-tauri/PrivacyInfo.xcprivacy` |
| ITMS-90095 / 90207 | 缺分类 / 图标不符 | `Info.mas.plist`、1024×1024 无透明图标 |
| "无效的二进制文件" | 主可执行仍是 ad-hoc | `codesign -dv --verbose=2` 看 `Authority`（`-dv` 不打这一行） |

提交前最后一眼，三条都是"能编译、能跑、但会被卡"：

- **ffmpeg 必须是 LGPL 构建。** `brew` 那份带 `--enable-gpl`，会把整个二进制拖进 GPL，
  闭源 + 商店 EULA 下没法交代（§6 要 Corresponding Source，§10 禁止附加限制）。用
  `release/build-ffmpeg-lgpl-macos.mjs` 产的那份，`lib/ffmpeg.mjs` 在打包前会校验。
- **「从浏览器读取 cookies」在沙盒里必然失败**（读不到 Chrome 目录，也读不到登录钥匙串），
  商店版的 URL 导入只能留 cookies.txt。
- **商店版不能绕过 App Store 更新。** 确认「检查更新」不会把用户导去 GitHub 下 DMG。

## Windows 只走 CUDA

CPU 推理太慢，不能出货，所以 Windows 包拉的是两个引擎的 **CUDA** 构建，里面
没有 CPU 版本：

| 引擎 | 包 | 落位 |
|------|-----|------|
| audio.cpp v0.9.1 | `...-windows-x64-cuda13.3.zip` + 同名 `cudart` 包 | `bin\audiocpp_cli\` |
| llama.cpp b11696 | `...-win-cuda-13.4-x64.zip` + 同名 `cudart` 包 | `bin\llama-server\` |

每个引擎放**各自的目录**：两者都带一个 `ggml-cuda.dll`，却是针对不同 CUDA
toolkit 编的，平铺在一个 `bin\` 里时后装的会静默覆盖先装的。Windows 解析进程
导入时先查可执行文件所在目录，所以 `.exe` 挨着自己的 DLL 就够了 —— 不需要 PATH
或 manifest。`engine::cli::find_executable` 会再探一层 `<dir>/<binary-name>/`，
`Backend::platform_default()` 在 Windows 上返回 `Cuda`，都是同一件事的两半。

覆盖范围是 **Turing（RTX 20 系）→ Blackwell（RTX 50 系）**：

| 构建 | 架构列表 | 覆盖 |
|------|----------|------|
| audio.cpp cuda13.3 | `75;80;86;89;90;120` | Turing → Blackwell |
| llama.cpp cuda13.4 | ggml 默认（CTK 13）：`75/80/86/89/90` + `120` | 同上 |

**Pascal（GTX 10 系）及更老是有意不支持的，这不是运行时限制而是编译器限制**：
CUDA 13.0 Release Notes §2.5.2 明确 removed offline compilation and library support
for Maxwell / Pascal / Volta，所以 13.x 的 fatbin 里不可能有 sm_61 的内核——在
GTX 1080 上会直接报 `no kernel image is available for execution on the device`，
不是"慢一点"。CUDA 12.4 曾经作为默认试过（`61-virtual;70;75;80;86;89;90`），
能覆盖 Pascal→Hopper，但代价是 50 系没有原生 sm_120 内核、只能跑 compute_90 PTX
由驱动 JIT，而且 fatbin 大 1.3 GB。权衡后选择放弃 8 年前的卡，换 50 系原生内核。

另一个理由在驱动：CUDA 13 随 **R580 分支**发布，而那正是最后一个支持 7.5 之前 GPU
的分支。

机器上没有 NVIDIA 显卡时，`prepare-resources.mjs` 只警告不中止。

macOS 不受影响：那边 audio.cpp 是静态链接，llama 的 dylib 本来就平铺在 `bin/`，
没有冲突，后端仍是 `metal`。

## 加一个平台

1. 给需要的脚本加分支 —— `platform.mjs` 已经会拒绝未知宿主，三个 release 脚本
   遇到不认识的平台会 `die` 并给出明确信息，而不是跑错的那半边。
2. `prepare-resources.mjs` 末尾那段共享尾巴（Silero VAD、yt-dlp 许可证、
   `MANIFEST.txt`）保持共享。
3. 在 `release.mjs` 和 `publish.mjs` 里加上该平台的产物扩展名。

## 约定

- **一切都 pin。** 每个下载都带 SHA-256。空 pin 表示「还没算」，会跳过校验，
  而不是让一个本该可行的构建失败。
- **幂等。** 产物已匹配时重跑是空操作，所以构建可以每次都调拉取脚本。
- **失败要响。** 缺 DLL、校验和不匹配都中止；绝不让一个只在用户运行时才坏的
  包发出去。
- **不要手改 `resources/MANIFEST.txt`** —— 它是生成的。
- **签名顺序固定：由内向外。** dylib → 随包工具 → app 本体。手工 `--deep` 只能保证
  "有签名"，不能让 6 个工具拿到 `inherit`，商店校验一定会打回。
- **`.pkg` 只给商店。** 它落在 `target/mas/`，不进 `dist/`、不进 Releases —— 那个目录
  是 DMG / MSIX 的通道。
