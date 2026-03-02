# Scripts 说明文档

本文档介绍了 `scripts` 目录下的各种自动化工具集合，主要包含了构建、本地开发环境配置、应用签名与公证以及发布到 GitHub Releases 等。

## 目录结构

我们在重构后主要划分为四大领域的单一职责脚本：

- `build/`：涵盖构建（下载 Python 二进制、FFmpeg 等）、混淆代码与核心 dmg 打包等操作。
- `release/`：涵盖代码签名（Mac）、推平 GitHub 发布流等。
- `setup/`：搭建或清理开发环境，一键安装工具等。
- `utils/`：通用的常量检测、版本计算助手等。

---

## 常用命令卡片

### 1. 📦 构建打包 macOS 应用（DMG）

我们支持两种架构模式的构建：
* **Lite（默认/推荐）**：带轻量的独立 Python，代码压缩体积约 100MB。第一次在用户端运行的时候走 `uv` 动态拉取。
* **Full（稳定）**：把环境直接整个复制打包进入客户端里，体积约 500-800M。

#### 命令行执行构建：
可以在项目的最根目录下通过 `npm` / `bun` 运行，或者直接调用脚本文件：

**构建 Lite 版本（轻便）**
```bash
# 方式一：使用 package.json 的快捷命令（推荐）
bun run build:dmg:lite
# (等同于 bun run build:dmg)

# 方式二：直接执行脚本
./scripts/build/build-macos-app.sh lite
```

**构建 Full 版本（全量）**
```bash
# 方式一：
bun run build:dmg:full

# 方式二：直接执行脚本
./scripts/build/build-macos-app.sh full
```

✨ *构建成功后的 DMG 文件会被输出到根目录的 `dist/` 文件夹下。*

---

### 2. 🚀 发布 GitHub Release

若打算将打包好的代码直接作为新 Release 一键发布至 Github 仓库：

```bash
# 发布 Lite 版本（默认）
./scripts/release/release.sh

# 发布 Full 版本
./scripts/release/release.sh --type=full

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
该脚本会智能识别你是否漏装了某些必要依赖环境（例如 Rust，uv, FFmpeg）。
