# MonoSub · Microsoft Store 上架 Tech Spec（MSIX / winapp CLI 路径）

| 项 | 值 |
| --- | --- |
| 状态 | 待执行（Windows 环境） |
| 作者 | 开发机（macOS）起草 |
| 目标平台 | Windows 11 x64 |
| 目标商店 | Microsoft Store（收费应用） |
| 应用 | MonoSub（Tauri 2.12 / React+Vite+TS / Rust） |
| 产品类型 | MSIX or PWA app（Partner Center 已建，草稿状态） |
| 关联文档 | `cli-design.md`、`model-selection-design.md`（资源二进制命名与运行约定） |

---

## 1. 背景与目标

MonoSub 是"本地语音转文章"桌面应用（Tauri 2.12）。目标是以**收费应用**形式上架 Microsoft Store，让 Windows 用户通过商店购买、安装并自动更新。

本 Spec 只覆盖 **MSIX 打包 → 商店提交** 的完整落地路径，供在 Windows 11 机器上按序执行。macOS 侧无法交叉编译 Windows 目标（MSVC 链接 + WebView2 SDK 依赖），因此**打包必须在 Windows 上完成**。

## 2. 关键决策记录（ADR）

| 决策点 | 结论 | 理由 |
| --- | --- | --- |
| 产品类型 | **MSIX or PWA app**（已建） | 免自购代码签名证书（商店重签）、免自托管安装包、更新自动分发 |
| 打包工具 | **winapp CLI**（微软官方） | Tauri 不原生生成 MSIX（官方文档明确只生成 EXE/MSI）；`winapp` 是微软官方 MSIX 打包 CLI |
| 打包机 | Windows 11 x64（本机 / VM / GitHub Actions windows runner 三选一） | winapp CLI 要求 Windows 11；Tauri 不支持 macOS → Windows 交叉编译 |
| 运行时二进制 | 需为 `x86_64-pc-windows-msvc` 准备 Windows 版（见 §4，**当前为阻塞项**） | 应用运行时按 `-<target-triple>` 约定查找 ffmpeg / audiocpp_cli / ggml |
| 商店内更新 | 走商店版本提交（新版本号 → 新提交 → 认证） | Store 自动向用户分发更新；**不用** Tauri updater |

### 2.1 为什么不用 Tauri 官方 EXE/MSI 路径（备用）

Tauri 官方 Store 指南默认走 **EXE or MSI app**，但该路径要求：自购代码签名证书、自托管安装包 URL、offlineInstaller WebView2 模式、静默安装参数 `/S`。成本与维护量高于 MSIX 路径。若 MSIX 路径受阻，备用方案见附录 A。

## 3. 前置条件（Windows 11 环境）

在 Windows 机器上以管理员 PowerShell 执行：

```powershell
# 1) 包管理器
winget --version

# 2) Git
winget install Git.Git --source winget

# 3) Node.js（项目 engines 要求 >= 20）
winget install OpenJS.NodeJS.LTS --source winget

# 4) bun（tauri.conf.json 的 beforeBuildCommand 是 `bun run build`）
npm install -g bun

# 5) Rust 工具链（MSVC 目标）
winget install Rustlang.Rustup --source winget
rustup default stable-msvc

# 6) Visual Studio Build Tools（Tauri 在 Windows 编译必需：MSVC + Windows SDK）
#    安装 "使用 C++ 的桌面开发" 工作负载 + Windows 11 SDK
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 7) winapp CLI（MSIX 打包）
winget install microsoft.winappcli --source winget
winapp --help
```

> 若在受限网络安装 NSIS/WiX 失败：本节后续打包使用 `--no-bundle`（见 §5.3），不依赖 NSIS/WiX 下载。

## 4. 运行时二进制准备（关键阻塞项 · 先做）

### 4.1 现状

`src-tauri/resources/` 当前只包含 **macOS ARM64** 二进制（由 `scripts/build/prepare-resources.sh` 生成）：

```
bin/audiocpp_cli-aarch64-apple-darwin
bin/ffmpeg-aarch64-apple-darwin
bin/ffprobe-aarch64-apple-darwin
bin/lib/*.dylib            # ffmpeg 动态库
bin/libggml*.dylib         # ggml 动态库
bin/libllama*.dylib
models/  models.json  licenses/  MANIFEST.txt
```

代码在 `engine/cli.rs` 与 `commands/video.rs` 中按 **`<名字>-<target-triple>`** 约定解析二进制；Windows x64 的目标三元组为 **`x86_64-pc-windows-msvc`**（Windows 下还会追加 `.exe` 后缀候选）。

### 4.2 必须准备的 Windows 文件

放入 `src-tauri/resources/bin/`（与 macOS 文件并列，互不冲突）：

| 期望文件名 | 来源 | 说明 |
| --- | --- | --- |
| `audiocpp_cli-x86_64-pc-windows-msvc.exe` | audio.cpp 官方 `bin-windows-x64-cpu-portable` 发布包 | 版本/能力与 macOS 版一致；**识别引擎** |
| `ffmpeg-x86_64-pc-windows-msvc.exe` | gyan.dev `essentials_build`（静态） | 与 macOS 版行为对齐（libx264、libopus、libmp3lame 等） |
| `ffprobe-x86_64-pc-windows-msvc.exe` | 同上 | — |
| `llama-server-x86_64-pc-windows-msvc.exe` | llama.cpp 官方 `bin-win-cpu-x64` 发布包 | **翻译引擎**；它的 DLL 必须平铺在 `bin/` 下（`@loader_path` 是它唯一的 RPATH） |
| `yt-dlp-x86_64-pc-windows-msvc.exe` | yt-dlp 官方单文件发布 | URL 导入 |
| `*.dll` | 上述三个发布包自带 | 平铺在 `bin/`：`llama-server` 是 `llama-server-impl.dll` 的薄壳，audiocpp 的 portable 构建还需要 MSVC 运行时 |
| `models/**`、`models.json`、`licenses/**` | 原样保留（gguf 模型跨平台） | 无需改动 |
| `MANIFEST.txt` | 用 Windows 侧等效脚本重新生成 | 由 `scripts/windows/prepare-resources.ps1` 生成，勿手工编辑 |

> 这一整张表都由 `scripts/windows/prepare-resources.ps1` 自动完成（含 SHA256 校验），
> 手工放置只用于调试。实测确认的发布包形状：audiocpp 的 portable 包**不含 ggml DLL**
> （静态链接，只需 MSVC 运行时那 9 个）；llama.cpp 的 `bin-win-cpu-x64` 包**含 29 个 DLL**
> （`ggml-cpu-*.dll` 按 CPU 微架构各一份，`llama-server-impl.dll` 是真正干活的那个）。
> 两者合起来正好 38 个 DLL —— 这也是 `check-msix-package.ps1` 要求「至少 10 个」的
> 依据：少一个家族就说明某个发布包没装进来。

### 4.3 验证运行时二进制

```powershell
# 环境变量覆盖（代码中已支持，用于冒烟测试）
$env:MONOSUB_AUDIOCPP_PATH = "D:\monosub\src-tauri\resources\bin\audiocpp_cli-x86_64-pc-windows-msvc.exe"
$env:MONOSUB_FFMPEG_PATH   = "D:\monosub\src-tauri\resources\bin\ffmpeg-x86_64-pc-windows-msvc.exe"

# 直接执行各二进制自检（如 --help / --version）
audiocpp_cli-x86_64-pc-windows-msvc.exe --help
ffmpeg-x86_64-pc-windows-msvc.exe -version
```

> **阻塞判定**：此步未就绪前不要进入打包。Windows 包内缺二进制 = 认证/真机运行直接崩溃。

## 5. 打包步骤（Windows 上按序执行）

工作目录：项目根（如 `D:\monosub`）。

### 5.1 拉代码、装依赖

```powershell
git clone <repo-url> monosub
cd monosub
bun install
```

### 5.2 初始化 MSIX 清单（winapp init）

```powershell
winapp init
```

按提示填写：

| 提示项 | 值 |
| --- | --- |
| Package name | `BuyOnceLabs.monosub`（Partner Center「Identity details」的 identity Name，**不是**文件名，也不能等于 "MonoSub"） |
| Publisher name | `CN=3AFDD6AB-7CB4-4064-8C0B-7F48F2BC7DFA`（发布者 **ID**；不是账户名或显示名） |
| Publisher display name | `BuyOnce Labs`（发布者**显示名**，与上一项是两个不同的值） |
| Version | `2.0.0.0`（四段式，与 tauri.conf.json 的 2.0.0 对应） |
| Entry point | `monosub.exe`（GUI 主程序；项目 `default-run = "monosub"` 已保证主程序名） |
| Setup SDKs | **Do not setup SDKs**（Tauri 用 Rust windows crate，不需要 C++ SDK 管理） |

产出：根目录 `Package.appxmanifest` + `Assets/`（商店图标）。可打开 manifest 核对 DisplayName / Publisher / Capabilities。

> **三个身份值必须逐字等于 Partner Center → 产品 →「Identity details」页**，任一处不符都会被上传校验打回：
>
> | 清单位置 | 值 | 不匹配时的报错 |
> | --- | --- | --- |
> | `Package/Identity/Name` | `BuyOnceLabs.monosub` | 必须与 Partner Center identity Name 一致 |
> | `Package/Identity/Publisher` | `CN=3AFDD6AB-7CB4-4064-8C0B-7F48F2BC7DFA` | 必须与 Partner Center publisher ID 一致 |
> | `Package/Properties/PublisherDisplayName` | `BuyOnce Labs` | 必须与 Partner Center publisher display name 一致 |
>
> 包族名（PFN）由前两项推导，不写进清单：`BuyOnceLabs.monosub_3jbgsq2x4ngec`。
> `scripts/release/package.mjs` 已把这三个值写成默认值，`verify.mjs` 会重算 PFN 并断言一致——本地打包就能拦住这类错误。

> 双二进制说明：项目含 `monosub`（GUI）与 `monosub-cli` 两个 binary。**两个都进包**：
> GUI 是入口程序（`monosub.exe`），CLI 放在
> `resources\bin\monosub-cli.exe`，并通过
> `windows.appExecutionAlias` 暴露为命令 `monosub-cli.exe`——见 §5.2b。
>
> **已变更（2026-10-26）**：CLI 落地名去掉了 `-x86_64-pc-windows-msvc` 后缀
> （`engine::cli::monosub_cli_file_name()` 与 `scripts/lib/platform.mjs` 的 `cliName`
> 均已同步）。它是唯一由用户直接敲的二进制，
> 便携/自编译场景把 `resources\bin` 加进 PATH 后，带三元组后缀的文件对 `monosub-cli`
> 这个查询完全无效。本文下面出现的 `monosub-cli-<triple>.exe` 均按 `monosub-cli.exe` 理解。

### 5.2b 命令行工具与 App Execution Alias

`monosub-cli` 也进包，位置与 macOS 对称：

```
resources\bin\monosub-cli-x86_64-pc-windows-msvc.exe
```

它必须能在终端里直接敲 `monosub-cli`，而这**不能靠改 PATH 实现**。MSIX 安装在

```
C:\Program Files\WindowsApps\BuyOnceLabs.monosub_<版本>_x64__<发布者哈希>\
```

路径里带版本号，所以任何把安装目录写进用户 PATH 的做法**下次更新就失效**。正确机制是
App Execution Alias：

```xml
<uap3:Extension Category="windows.appExecutionAlias"
                Executable="resources\bin\monosub-cli-x86_64-pc-windows-msvc.exe"
                EntryPoint="Windows.FullTrustApplication">
  <uap3:AppExecutionAlias>
    <desktop:ExecutionAlias Alias="monosub-cli.exe" />
  </uap3:AppExecutionAlias>
</uap3:Extension>
```

要点（每一条踩错都会静默失效）：

| 要点 | 原因 |
|---|---|
| `Extension` 必须在 `<Application>` 的 **`<Extensions>`** 里 | 直接挂在 `<Application>` 下 schema 校验就失败（`C00CE014`，makeappx 实测）；挂在别处则运行时不报错、只是完全不生效 |
| `Executable` 是**包内相对路径**，允许子目录 | 别名指向 `resources\bin\` 下的真实二进制 |
| `Alias` 必须以 `.exe` 结尾 | 否则 shell 不解析 |
| 用户看到的两个 `DisplayName` 是两个元素 | Start 菜单/磁贴读 `<Application>\<VisualElements DisplayName>`，设置→应用读 `<Properties>\<DisplayName>`；生成器两个都取小写包名，只改一个就会在 Start 菜单里显示 `monosub`（实测，旧 bash 版只改了 Properties 那个） |
| `EntryPoint="Windows.FullTrustApplication"` | 缺了它激活时**不转发命令行参数** |
| `<Package>` 上声明 `xmlns:uap3` / `xmlns:desktop`，并加进 `IgnorableNamespaces` | 缺命名空间清单校验直接失败 |
| 别名目录 `%LOCALAPPDATA%\Microsoft\WindowsApps` 默认就在 PATH 上 | 所以用户**不需要执行任何命令**；Settings 在 Windows 上也就不显示安装命令 |

`scripts/windows/build-msix.ps1` 在打包前调用
`scripts/windows/add-execution-alias.ps1` 声明它（XML DOM，幂等，失败即中止）。
`scripts/windows/check-msix-package.ps1` 会验证文件在包内、别名在清单里，并且
**确实是 `<Application>\<Extensions>` 的子元素**（用真实 XML 解析，不是字符串匹配）。

> **实测记录 1**：`add-execution-alias.ps1` 第一版在声明 `uap3` 之后才建
> `XmlNamespaceManager`，`SelectNodes('uap3:…')` 直接抛
> 「未定义命名空间前缀 uap3」。脚本中止且**没有写出半成品清单**——这是刻意的：
> 一个半改的清单能顺利打包、直到认证阶段才失败，那是最糟的失败点。
>
> **实测记录 2**：第二版把 `uap3:Extension` 直接 `AppendChild` 到 `<Application>`。
> 这在 XML 上合法、在 DOM 上"成功"，但 `CT_Application` 的子元素是
> `VisualElements` / `ApplicationContentUriRules` / `Extensions` 三者之一，
> Extension 不是它的兄弟。用 SDK 的 makeappx 校验：
>
> ```
> <Application><uap3:Extension .../></Application>
>   → error C00CE014: The app manifest must be valid as per schema
> <Application><Extensions><uap3:Extension .../></Extensions></Application>
>   → 打包通过
> ```
>
> 所以脚本现在先找 `<Extensions>`、没有就建一个再 append；检查脚本也改为
> 只认 `./Extensions/Extension[@Category="windows.appExecutionAlias"]`——
> 只认 `./Extension` 会漏掉正确形状，只认 `//Extension` 又会放过错误形状。
>
> **实测记录 3**：检查脚本原先从 bash 里 `powershell -Command` 内嵌一段 XPath，
> 用 `\"` 转义引号。PowerShell 的转义符是反引号不是反斜杠，内嵌片段收到的是字面
> `\"`，**每次**解析失败并落到 `catch` 分支输出 `unparseable`，于是这一项恒判失败。
> 这也是整条 Windows 链路改成 PowerShell 的原因之一：不再跨语言转义。

### 5.2c 为什么 Windows 侧脚本是 PowerShell

`scripts/windows/` 下全部是 `.ps1`。这条链路只能在 Windows 上跑，而它驱动的每个工具
（`winapp`、`node_modules\.bin\tauri.cmd`、`cargo`、System32 的 `curl.exe`）都是原生
Windows 进程；bash 版本为了 PowerShell 能力（XML DOM、证书、zip）反过来要 shell out，
每一条缝都是一次转义或路径约定的机会。

`package.json` 的 `build:msix` 因此是
`powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/windows/build-msix.ps1`。
直接写 `./scripts/windows/build-msix.ps1` 不行：bun 的 shell 不读 shebang，
`CreateProcess` 对 `.ps1` 失败且拿不到错误信息，只会打印
`bun: unknown error:` 并以 1 退出（实测）。同理，`bash ./x.sh` 这种写法在
Windows 上虽然能跑，却把 Git Bash 变成了隐藏依赖。

`-Sign` / `-SkipResources` / `-Tag` 是真正的 PowerShell 参数（`[switch]` / `[string]`），
从 `bun run build:msix --sign` 或直接在 PowerShell 里都能用；bash 版本只能用
`--sign` 这类手写字符串解析。

两个实测细节：PowerShell 对参数名**大小写不敏感、也不在乎前面几个 `-`**，所以
`--sign` / `-Sign` / `-sign` 都能绑定 `$Sign`；但名字**中间**的连字符不会被折叠，
`--skip-resources` 会报 `NamedParameterNotFound`，因此带连字符的那几个必须显式写
`[Alias('skip-resources')]`。反过来，`[Alias('sign')]` 这种只差大小写的别名本身
就是错误（`ParameterNameConflictsWithAlias`）。

### 5.3 构建 release 可执行文件

整条流程已脚本化，下面 5.3–5.5 是它内部做的事；日常只需要一条命令：

```powershell
powershell -File scripts\windows\build-msix.ps1
# 或：bun run build:msix
```

手工等价步骤：

```powershell
# 只构建 exe，跳过 NSIS/MSI 安装器（商店包不需要，且省去 NSIS 下载）
node_modules\.bin\tauri.cmd build --no-bundle
# 产物：src-tauri\target\release\monosub.exe
```

> 用 `tauri.cmd` 而不是不带扩展名的 `tauri`：后者是 POSIX shell 脚本，
> Windows 无法直接执行，正是本次改写要去掉的隐藏依赖。

### 5.4 组装打包目录

脚本用的是 `build\msix\app\`，不是 `dist\`：`dist\` 是 Vite 的前端产物目录，也是
发布产物（DMG / MSIX）的收集目录，把包内布局写进去会互相覆盖。

```powershell
# 包内布局 = AppPaths::resolve_resources_root 期望的形状：
#   <install>\monosub.exe + <install>\resources\
```

> `resources/**/*` 是运行时资源（音频处理二进制 + 模型），**必须**进包，否则应用无法工作。

### 5.5 生成开发证书并打包 MSIX

```powershell
# 生成与 manifest Publisher 匹配的自签名证书（首次）
winapp cert generate --manifest build\msix\app\Package.appxmanifest `
    --output build\msix\devcert.pfx --if-exists skip --export-cer

# 打包（签名用的 devcert 仅供本地验证；商店提交后由微软重签，无需正式证书）
winapp package build\msix\app `
    --manifest build\msix\app\Package.appxmanifest `
    --output dist\monosub_2.0.0.0_x64.msix --no-sign
```

产物：`dist\monosub_2.0.0.0_x64.msix`。

> **版本号并非固定为 2.0.0.0**：本文所有 `2.0.0.0` 都是首次上架时的实际值。四段式规则是
> `<tauri.conf.json 的 x.y.z>.0` —— 例如 2.0.1 → `2.0.1.0`，文件名为
> `dist\monosub_2.0.1.0_x64.msix`。`scripts/release/package.mjs` 会按当前版本自动命名并写
> manifest，手打命令时才需要自己替换。

### 5.6 本机安装验证

```powershell
# 信任开发证书（仅一次，管理员）
winapp cert install .\devcert.pfx

# 若之前用 winapp run 注册过开发包，先注销
winapp unregister

# 安装 MSIX
Add-AppxPackage .\monosub_2.0.0.0_x64.msix

# 从开始菜单启动 MonoSub，冒烟测试核心链路：
#   导入音频 → 转写 → 字幕 → 翻译 → 导出文章
```

### 5.7 产物检查清单

- [ ] 文件名：`monosub_2.0.0.0_x64.msix`
- [ ] 架构：x64（与资源二进制一致）
- [ ] 版本号：2.0.0.0 与 tauri.conf.json 一致
- [ ] 包内包含：`monosub.exe` + `resources/`（含 Windows 二进制与模型）
- [ ] 本机安装后核心功能通过冒烟测试
- [ ] 卸载干净（`Remove-AppxPackage` 后无残留）

## 6. Partner Center 提交配置（6 个区块）

登录 Partner Center → 应用和游戏 → `monosub` → 开始提交。

| 区块 | 必填/建议内容 |
| --- | --- |
| **Pricing and availability** | 收费价格（选价格档位）；市场范围（全球或指定）；免费试用（可选）；发布可见性与时间 |
| **Properties** | 类别与子类别（建议 **Productivity**，按实际内容调整）；系统要求（Windows 10/11、x64、建议内存）；支持网站/邮箱 |
| **Age ratings** | 完成 **IARC 问卷**（注意：问卷近期有更新，逐项确认没有 incomplete） |
| **Packages** | 上传 `monosub_2.0.0.0_x64.msix` |
| **Store listings** | 应用描述；截图（≥1，建议 4+）；商店 Logo/图标；**隐私政策 URL（必填）**；网站/支持邮箱 |
| **Submission options** | 可选：认证说明（如测试账号）、发布计划（立即/定时） |

全部无"不完整"后点 **提交进行认证**（≤3 个工作日）。通过后平均 15 分钟内商店可见。

> **隐私政策**：无页面时建议直接托管到项目现有的 GitHub Pages（`pages-deployment.yaml` 已存在），把 URL 填进商店一览。

## 7. 税务与收款（上线前完成）

- 账户设置 → 付款和税务配置文件：
  - 非美国开发者在线填写 **IRS W-8BEN**（中国开发者）；
  - 绑定银行收款账户（按微软支持的地区与币种）。
- 建议按中美税收协定申报，可降低预扣税率；具体请咨询税务顾问。
- 收款为按月结算；**首次提现前务必完成税务+银行验证**，否则付款状态为"需要操作"。

## 8. 更新与维护

1. 版本递增：同步修改 `tauri.conf.json` 的 `version` 与 `Package.appxmanifest` 的 `Version`（Windows 要求更高版本号才能覆盖安装）。
2. 重新执行 §5 打包（资源二进制有变则先更新 §4）。
3. Partner Center 新建提交 → 上传新 .msix → 认证。
4. 商店自动向已购用户分发更新。

## 9. 风险与应对

| 风险 | 现象 | 应对 |
| --- | --- | --- |
| Windows 运行时二进制缺失/不匹配 | 启动后转写失败或崩溃 | §4 前置准备 + §5.6 真机冒烟测试 |
| IARC 问卷未完成 | 提交被挡（"不完整"） | 提交前逐项检查 Age ratings |
| 缺少隐私政策 URL | 认证被拒 | 先上线隐私政策页（GitHub Pages）再提交 |
| MSIX 包内资源路径错误 | 资源找不到 | 核对 §5.4 dist 目录结构，与本地验证 |
| 版本号冲突 | 安装/更新失败 | 每次提交递增版本，manifest 与 tauri.conf 同步 |
| 商店重签后包体变化 | — | 正常现象，无需操作 |
| EXE/MSI 路径被误用 | 要求填 Package URL | 确认产品类型是 MSIX or PWA app（当前已是） |

## 10. 验收标准（Definition of Done）

- [ ] Windows 11 上产出 `monosub_<version>_x64.msix` 且本机安装、核心链路冒烟通过
- [ ] Partner Center 6 区块全部无"不完整"，提交认证
- [ ] 认证通过，商店页面可见、可购买
- [ ] 税务（W-8BEN）与收款银行已验证
- [ ] 隐私政策 URL 可访问

## 附录 A：备用路径（Tauri 官方 EXE/MSI）

仅当 MSIX 路径受阻时启用，需产品类型改为 **EXE or MSI app**：

1. 新增 `src-tauri/tauri.microsoftstore.conf.json`：WebView2 用 `offlineInstaller` 模式。
2. `npm run tauri -- bundle -- --config src-tauri/tauri.microsoftstore.conf.json` 生成 `-setup.exe`。
3. 自购代码签名证书并签名（商店不会重签 EXE/MSI）。
4. 安装包自托管到 HTTPS，填版本化 URL；静默参数：NSIS 用 `/S`、MSI 用 `/quiet`。
5. 商店只提供下载，**不推送更新**（需应用自更新）。

## 附录 B：参考链接

- Tauri 官方 Microsoft Store 指南：https://v2.tauri.app/zh-cn/distribute/microsoft-store/
- 微软 winapp CLI × Tauri：https://learn.microsoft.com/windows/apps/dev-tools/winapp-cli/guides/tauri
- Microsoft Store 入门（提交与认证）：https://learn.microsoft.com/zh-cn/windows/apps/publish/get-started
- 开发者账户注册（免费新流程）：https://storedeveloper.microsoft.com/
- 付款/税务配置：https://learn.microsoft.com/zh-cn/windows/apps/publish/faq/manage-your-account

## 附录 C：命名对照速查

| 含义 | 值 |
| --- | --- |
| 产品名（Partner Center / manifest DisplayName） | monosub（显示名 MonoSub） |
| 包身份名（`Package/Identity/Name`） | BuyOnceLabs.monosub |
| 发布者 ID（`Package/Identity/Publisher`） | CN=3AFDD6AB-7CB4-4064-8C0B-7F48F2BC7DFA |
| 发布者显示名（`PublisherDisplayName`） | BuyOnce Labs |
| 包族名（PFN，推导值） | BuyOnceLabs.monosub_3jbgsq2x4ngec |
| Store ID | 9P6WTCQRQ1VW |
| Store 页面 | https://apps.microsoft.com/detail/9P6WTCQRQ1VW |
| Store 协议链接 | `ms-windows-store://pdp/?productid=9P6WTCQRQ1VW` |
| MSA app ID | 28043347-260a-4443-a3d9-356dc783adfb |
| 入口程序 | monosub.exe |
| 命令行入口别名 | monosub-cli.exe → `resources\bin\monosub-cli.exe` |
| Windows x64 目标三元组 | x86_64-pc-windows-msvc |
| 资源二进制后缀规则 | `<名字>-x86_64-pc-windows-msvc.exe` |
| 包版本（四段式） | 2.0.0.0（与 tauri.conf.json 的 2.0.0 对应） |
| 产物文件名 | `dist\monosub_2.0.0.0_x64.msix`（商店不校验文件名） |
| tauri 标识符（`leixlabs.monosub`，决定配置目录，与商店身份无关） | leixlabs.monosub |
