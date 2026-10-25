# Scripts 说明文档

本文档介绍了 `scripts` 目录下的各种自动化工具集合：构建、本地开发环境配置、应用签名，
以及**把产物发布到 GitHub Releases**。

**构建（release）和发布（publish）是两件事、两个脚本。** 构建只在本机产出可发布的文件，
发布只把 `dist/` 里已有的文件传上 GitHub。理由见下一节。

## 目录结构

- `build/`：**macOS** 构建资源准备（audio.cpp CLI、Silero VAD、FFmpeg）与核心 dmg 打包。
- `release/`：**macOS** 发布构建 —— 跑构建、收集并校验 DMG，产物落到 `dist/`。不联网。
- `publish/`：把 `dist/` 里的产物发到 GitHub Releases。**不构建任何东西。**
- `windows/`：**Windows / Microsoft Store** 全套脚本（PowerShell），与 macOS 完全分开维护（见下）。
- `setup/`：搭建或清理开发环境，一键安装工具等。
- `utils/`：版本号读写与自增、i18n 键检查、清理等（两个平台共用）。

> macOS 与 Windows 的构建/发布脚本不共用文件：两边的工具链、产物格式（DMG vs MSIX）
> 和签名模型都不同，混在一个目录里改一处就可能悄悄改坏另一平台。macOS 侧是 bash，
> Windows 侧是 PowerShell —— 两个平台的脚本语言也刻意不统一，理由见第 4 节。
> 唯一的接口是 `dist/`：两个平台的产物都落在那里，`publish` 只认文件，不认平台。

---

## 一条铁律：构建与发布分开

| | 谁做 | 做什么 | 不做什么 | 失败了怎么办 |
| --- | --- | --- | --- | --- |
| **release** | `scripts/release/release.sh`（macOS）<br>`scripts/windows/build-msix.ps1`（Windows） | 版本对齐、编译、打包、签名校验、产物进 `dist/` | 不调 `gh`、不联网、不改版本号 | 修好再重跑，GitHub 上不会留下半成品 |
| **publish** | `scripts/publish/publish.sh`（macOS）<br>`scripts/windows/publish-msix.ps1`（Windows） | 生成 release notes、创建/更新 release、上传资产、成功后自增版本号 | 不编译、不打包、不签名 | 直接重跑，不用再等十几分钟构建 |

拆开的好处是每一步都能单独重跑：DMG 构建要十几分钟，上传只是一次 HTTP；构建失败时
不该在 GitHub 上留下一个只有 tag 没有资产的 release，上传失败时也不该逼你重新构建。

**版本自增放在 publish 成功之后**（`scripts/utils/bump-version.sh`），不是 release 里。
原因是 tag 必须严格等于打进 App 的版本号：App 的「检查更新」拿自身 `CARGO_PKG_VERSION`
跟 release tag 比。构建前自增就会让 DMG 文件名比 tag 大一号（线上真出过
tag `v2.0.0` / 产物 `MonoSub-2.0.1.dmg` 这种事）。

---

## 常用命令卡片

### 1. 📦 macOS：构建发布产物（release）

```bash
# 方式一：使用 package.json 的快捷命令
bun run release:dmg

# 方式二：直接执行脚本
./scripts/release/release.sh

# 已经构建过，只想核对 dist/ 里的产物
./scripts/release/release.sh --skip-build
```

它读取版本号 → 调用 `build/build-macos-app.sh`（准备资源 → 前端构建 → Tauri 打包 →
签名/分发校验）→ 把 DMG 收集到 `dist/`，最后打印「下一步发布用什么命令」。
**这一步不发任何东西到 GitHub。**

自 v2.0 起，推理引擎为随包分发的 `audiocpp_cli` 单文件二进制 + 内置 Silero VAD 模型，
不再有 Python/uv 运行时，因此也**不再区分 Lite / Full 构建模式**（旧参数 `--type=`
会被忽略并提示）。

`prepare-resources.sh` 是构建前唯一的资源准备入口，它会清理已废弃的 Python/uv 目录、
准备 FFmpeg，再调用 `fetch-audiocpp.sh` 拉取并校验 `audiocpp_cli` 与
`silero_vad_16k.safetensors`。脚本可重复运行。

✨ *DMG 落在根目录的 `dist/` 下，文件名形如 `MonoSub-2.0.0.dmg`。*

### 2. 🚀 macOS：发布到 GitHub（publish）

```bash
bun run publish:macos
# 等价于 ./scripts/publish/publish.sh
```

默认把 `dist/` 里**当前版本**的所有产物（`*.dmg`，以及同时存在的 `*.msix`）一起上传。
`*.signed.msix` 会被跳过：那是只用于本机安装验证的副本。

| 参数 | 作用 |
| --- | --- |
| `--version 2.0.0` | 明确指定发布哪个已构建版本（默认取 `tauri.conf.json`） |
| `--recreate` | 已有同名 release：删除后重建（换产物时用） |
| `--add-assets` | 已有同名 release：只补资产，不改 notes、不自增版本 |
| `--skip-bump` | 发布成功后不自增版本号 |
| `--draft` | 建为草稿（草稿不自增版本号） |
| `--asset <path>` | 只上传指定文件（可重复；给定时不再扫描 `dist/`） |
| `--notes-file <path>` | 用文件内容作为 release notes（默认自动生成） |

> **注意**：依赖官方的 `gh` CLI，并已执行 `gh auth login`。目标仓库默认
> `leixlabs/monosub`，可用环境变量 `RELEASE_REPO` 覆盖。

默认行为是「先构建再发布」变成两步之后，一次完整发版是：

```bash
./scripts/release/release.sh        # 1. 构建 + 本地校验，十几分钟
./scripts/publish/publish.sh        # 2. 上传，几秒
```

### 3. 🪟 Windows：构建 MSIX（Microsoft Store）

规范见 `docs/tech-spec/microsoft-store-publish-spec.md`。**必须在 Windows 11 x64 上运行**：
Tauri 不支持从 macOS 交叉编译 Windows 目标（MSVC 链接 + WebView2 SDK）。

Windows 侧脚本是 **PowerShell**（`.ps1`），不是 bash：这条链路只能在 Windows 上跑，
它驱动的每个工具（winapp、`tauri.cmd`、cargo、System32 的 `curl.exe`）都是原生 Windows
进程，用同一个运行时就不必在 bash 与 PowerShell 之间来回转义路径。Git Bash 不是必需的。

```powershell
# 1) 准备 Windows 运行时二进制（audiocpp_cli / llama-server / ffmpeg / yt-dlp + DLL）
powershell -File scripts\windows\prepare-resources.ps1

# 2) 构建并打包 MSIX（默认不签名，产出可直接提交 Partner Center）
powershell -File scripts\windows\build-msix.ps1

# 本地安装验证：额外生成一份开发证书签名的副本
powershell -File scripts\windows\build-msix.ps1 -Sign
```

也可以走 `bun`：`bun run build:msix`（参数同样会透传，例如
`bun run build:msix --sign`、`bun run build:msix --skip-resources`）。
这一步就是 Windows 侧的 release：产物进 `dist\`，不碰 GitHub。

产物在 `dist\`：

| 文件 | 用途 |
| --- | --- |
| `monosub_<version>.0_x64.msix` | 上传 Partner Center / GitHub。**商店会重新签名**，自签名包会被拒绝 |
| `monosub_<version>.0_x64.signed.msix` | 仅用于本机 `Add-AppxPackage` 验证，不会被发布 |

`check-msix-package.ps1` 会在构建末尾自动跑一遍规范 §5.7 的检查清单（包内二进制、DLL、
四段式版本号、x64 架构、WebView2 依赖、入口点、`runFullTrust`，以及 `monosub-cli` 的
App Execution Alias 是否真的挂在 `<Application>\<Extensions>` 下），缺任何一项都会让构建失败。

### 4. 🚀 Windows：发布 MSIX 到 GitHub（publish）

```powershell
# 上传 dist\ 里的 MSIX，并在 release notes 里补一张 Windows 表
powershell -File scripts\windows\publish-msix.ps1

# 显式指定发布哪个已构建版本（tauri.conf.json 已被上次发布自增过时用这个）
powershell -File scripts\windows\publish-msix.ps1 -Version 2.0.0

# 顺手把 tag 重写到当前 HEAD（见下面的警告）
powershell -File scripts\windows\publish-msix.ps1 -Tag v2.0.0 -UpdateTag
```

也可以走 `bun run publish:msix`。这个脚本**不构建**：`dist\` 里没有对应版本的 MSIX 时，
它会直接失败并告诉你先跑 `build-msix.ps1`。目标 release 必须已经存在（通常由 macOS 侧
的 `publish.sh` 创建），因为在这里另建一个会盖掉同一版本下的 DMG。

`-UpdateTag` 会把 tag 重写到 HEAD 并强制推送 —— 这是对已发布 ref 的历史改写，
只有在 release 说明里写着 "built from <sha>"、而产物实际来自更新的提交时才需要。
不加这个参数时脚本只上传资产、不动 tag。

> 注意：`gh release create` 创建的 release 与 git tag 是两回事。移动 tag 不会移动
> release 页面，给 release 加资产也不会改变 tag。

### 5. 两个平台发同一个版本

构建阶段不动版本号，所以两个平台的产物可以同版本先各建各的；版本号是**发布成功之后**
才自增的，因此 Windows 的构建要在 macOS 的 publish 之前完成：

```bash
# 1) macOS 构建（产物 MonoSub-2.0.0.dmg）
./scripts/release/release.sh
```

```powershell
# 2) Windows 构建（产物 monosub_2.0.0.0_x64.msix），此时 tauri.conf.json 还是 2.0.0
powershell -File scripts\windows\build-msix.ps1
```

```bash
# 3) macOS 发布：创建 release v2.0.0 并上传 DMG，随后版本号变成 2.0.1
#    （如果这时 dist/ 里已经有 MSIX，它会被一起传上去，第 4 步就可以跳过）
./scripts/publish/publish.sh
```

```powershell
# 4) Windows 补资产：产物是 2.0.0，用 -Version 明确指定，别用配置里的 2.0.1
powershell -File scripts\windows\publish-msix.ps1 -Version 2.0.0
```

`publish` 认的是**产物版本**，不是配置版本 —— 配置文件在上一轮发布结束时已经被改成
下一版了。发错了版本也不需要重新构建：`--recreate`（macOS）或 `-Version`（Windows）
可以重来。

---

## 额外进阶

* 如果打包或交叉编译的过程中发生报错、依赖缓存不对，可以使用通用的清理工具：
```bash
bun run clean        # (或者直接 rm -rf dist/* target/*)
./scripts/utils/clean-temp-files.sh
```

* 版本号三处（`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`package.json`）
  的读写与自增都在 `scripts/utils/` 里：`version-utils.sh` 提供函数，
  `bump-version.sh` 是命令行入口（由 publish 在发版成功后调用）。

* 如果在新设的系统环境做开发、跑不动项目代码的时候，请执行：
```bash
./scripts/setup/verify-setup.sh
```
该脚本会智能识别你是否漏装了某些必要依赖环境（例如 Rust，FFmpeg）。
