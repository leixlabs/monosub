# Scripts 说明文档

本文档介绍了 `scripts` 目录下的各种自动化工具集合，主要包含了构建、本地开发环境配置、应用签名与公证以及发布到 GitHub Releases 等。

## 目录结构

- `build/`：构建资源准备（audio.cpp CLI、Silero VAD、FFmpeg）与核心 dmg 打包。
- `release/`：代码签名（Mac）、推平 GitHub 发布流等。
- `setup/`：搭建或清理开发环境，一键安装工具等。
- `utils/`：通用的常量检测、版本计算助手等。

---

## 常用命令卡片

### 1. 📦 构建打包 macOS 应用（DMG）

自 v2.0 起，推理引擎为随包分发的 `audiocpp_cli` 单文件二进制 + 内置 Silero VAD 模型，
不再有 Python/uv 运行时，因此也**不再区分 Lite / Full 构建模式**。

可以在项目的最根目录下通过 `bun` / `npm` 运行，或者直接调用脚本文件：

```bash
# 方式一：使用 package.json 的快捷命令（推荐）
bun run build:dmg

# 方式二：直接执行脚本
./scripts/build/build-macos-app.sh
```

构建流程：准备资源 → 前端构建 → Tauri 打包 → 签名/公证 → 收集 DMG。

`prepare-resources.sh` 是构建前唯一的资源准备入口，它会清理已废弃的
Python/uv 目录、准备 FFmpeg，再调用 `fetch-audiocpp.sh` 拉取并校验
`audiocpp_cli` 与 `silero_vad_16k.safetensors`。脚本可重复运行。

✨ *构建成功后的 DMG 文件会被输出到根目录的 `dist/` 文件夹下。*

---

### 2. 🚀 发布 GitHub Release

若打算将打包好的代码直接作为新 Release 一键发布至 Github 仓库：

```bash
# 构建并发布
./scripts/release/release.sh

# 如果你已经执行过上面构建的内容拿到了文件，跳过重新构建，仅做信息提取与上传：
./scripts/release/release.sh --skip-package
```
> **注意**：
> 这个脚本依赖官方的 `gh` CLI 工具。请确保你本地装有 Github CLI，以及具备了相应的提交权限并且事先走过 `gh auth login` 进行了鉴权。

---

## 额外进阶

* 如果打包或交叉编译的过程中发生报错、依赖缓存不对，可以使用通用的清理工具：
```bash
bun run clean        # (或者直接 rm -rf dist/* target/*)
./scripts/utils/clean-temp-files.sh
```

* 如果在新设的系统环境做开发、跑不动项目代码的时候，请执行：
```bash
./scripts/setup/verify-setup.sh
```
该脚本会智能识别你是否漏装了某些必要依赖环境（例如 Rust，FFmpeg）。
