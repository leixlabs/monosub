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
| `audiocpp_cli-x86_64-pc-windows-msvc.exe` | 自行构建（audiocpp_cli 的 MSVC 目标） | 版本/能力与 macOS 版一致；**入口程序** |
| `ffmpeg-x86_64-pc-windows-msvc.exe` | BtbN 或 gyan.dev 的 Windows 官方构建 | 与 macOS 版行为对齐（libx264、libopus、libmp3lame 等） |
| `ffprobe-x86_64-pc-windows-msvc.exe` | 同上 | — |
| `lib/*.dll`（如 libavcodec.dll 等） | 随 ffmpeg 构建 | 若 ffmpeg 为静态构建则不需要 |
| `libggml*.dll`、llama 相关 DLL | audiocpp_cli 构建产物 | 若 audiocpp_cli **动态链接** ggml/llama 则必须；静态链接可省略 |
| `models/**`、`models.json`、`licenses/**` | 原样保留（gguf 模型跨平台） | 无需改动 |
| `MANIFEST.txt` | 用 Windows 侧等效脚本重新生成 | 由 `prepare-resources.sh` 生成，勿手工编辑 |

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
| Package name | `monosub`（默认即可） |
| Publisher name | 你的发布者名（如 `leix` / 公司名；**不能等于** "MonoSub"） |
| Version | `2.0.0.0`（四段式，与 tauri.conf.json 的 2.0.0 对应） |
| Entry point | `monosub.exe`（GUI 主程序；项目 `default-run = "monosub"` 已保证主程序名） |
| Setup SDKs | **Do not setup SDKs**（Tauri 用 Rust windows crate，不需要 C++ SDK 管理） |

产出：根目录 `Package.appxmanifest` + `Assets/`（商店图标）。可打开 manifest 核对 DisplayName / Publisher / Capabilities。

> 双二进制说明：项目含 `monosub`（GUI）与 `monosub-cli` 两个 binary，商店包只装 GUI，入口 `monosub.exe` 即可。

### 5.3 构建 release 可执行文件

```powershell
# 只构建 exe，跳过 NSIS/MSI 安装器（商店包不需要，且省去 NSIS 下载）
npm run tauri -- build --no-bundle
# 产物：src-tauri\target\release\monosub.exe
```

### 5.4 组装打包目录 dist

```powershell
if not exist dist mkdir dist
copy /Y src-tauri\target\release\monosub.exe dist\
xcopy /E /I /Y src-tauri\resources dist\resources
dir /s dist
```

> `resources/**/*` 是运行时资源（音频处理二进制 + 模型），**必须**进包，否则应用无法工作。

### 5.5 生成开发证书并打包 MSIX

```powershell
# 生成与 manifest Publisher 匹配的自签名证书（首次）
winapp cert generate --if-exists skip

# 打包（签名用的 devcert 仅供本地验证；商店提交后由微软重签，无需正式证书）
winapp pack .\dist --cert .\devcert.pfx
```

产物：`monosub_2.0.0.0_x64.msix`（当前目录）。

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
| 入口程序 | monosub.exe |
| Windows x64 目标三元组 | x86_64-pc-windows-msvc |
| 资源二进制后缀规则 | `<名字>-x86_64-pc-windows-msvc.exe` |
| 包版本（四段式） | 2.0.0.0（与 tauri.conf.json 的 2.0.0 对应） |
| 标识符 | dev.leix.monosub |
| 默认 Publisher（标识符第二段） | leix（≠ MonoSub，符合官方命名规则） |
