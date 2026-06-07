# MonoSub 打包方案 - 最终版

## ✅ 完成的优化

您最初的问题：**"打包的 DMG 不能使用，选择文件后中间没有展示 list"**

**根本原因**：Python 环境和依赖未打包到应用中。

**最终解决方案**：使用 **PyInstaller + Tauri Sidecar** 架构。

---

## 🎯 方案特点

### ✅ 用户体验

- **零配置**：用户无需安装 Python 或任何依赖
- **开箱即用**：下载 DMG → 拖入 Applications → 立即使用
- **专业品质**：符合 macOS 应用标准

### ✅ 开发体验

- **一键构建**：`bun run build:dmg` 完成所有工作
- **自动化**：所有脚本集中在 `scripts/` 目录
- **智能切换**：自动识别开发/生产环境

---

## 📦 打包架构

```
MonoSub.app/Contents/Resources/bin/
├── ffmpeg           ← FFmpeg 二进制 (从 Homebrew 复制)
├── ffprobe          ← FFprobe 二进制 (从 Homebrew 复制)
└── monosub-engine   ← Python + 所有依赖打包的可执行文件
    ├── Python 3.x 解释器
    ├── FunASR
    ├── ModelScope
    ├── PyTorch
    ├── TorchAudio
    └── 所有其他依赖
```

---

## 🚀 使用方法

### 首次构建

```bash
# 1. 安装系统依赖
brew install ffmpeg

# 2. 安装前端依赖
bun install

# 3. 初始化 Python 环境
cd core-engine && uv sync && cd ..

# 4. 一键构建
bun run build:dmg
```

### 日常开发

```bash
# 开发模式
bun run dev

# 重新构建 DMG
bun run build:dmg
```

---

## 🔧 关键文件说明

### 1. 构建脚本

- **[scripts/build-dmg.sh](scripts/build-dmg.sh)** - 主构建脚本
  - 调用 `prepare_resources()`
  - 构建前端和 Tauri
  - 创建 DMG

- **[scripts/prepare-resources.sh](scripts/prepare-resources.sh)** - 资源准备脚本
  - 复制 FFmpeg 二进制
  - 使用 PyInstaller 打包 Python
  - 生成资源清单

### 2. 配置文件

- **[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)** - Tauri 配置
  ```json
  {
    "bundle": {
      "externalBin": [
        "resources/bin/monosub-engine"
      ]
    },
    "plugins": {
      "shell": {
        "sidecar": true
      }
    }
  }
  ```

### 3. 核心代码

- **[src-tauri/src/commands/transcribe.rs](src-tauri/src/commands/transcribe.rs)** - Sidecar 实现
  - 生产环境：使用 `shell.sidecar("monosub-engine")`
  - 开发环境：使用 `Command::new("python")` 直接运行

- **[src-tauri/src/commands/video.rs](src-tauri/src/commands/video.rs)** - 视频处理
  - 智能查找 FFmpeg：优先使用打包的，回退到系统版本

---

## 📋 完整改动列表

### 新增文件

- ✅ `scripts/prepare-resources.sh` - 资源准备脚本
- ✅ `scripts/install-deps.sh` - 依赖安装脚本（已废弃，用户无需此脚本）
- ✅ `src-tauri/src/commands/transcribe.rs` - 新的 Sidecar 实现
- ✅ `BUILD.md` - 构建指南
- ✅ `QUICKSTART.md` - 快速开始
- ✅ `PACKAGING_SIDECAR.md` - Sidecar 架构说明
- ✅ `INSTALL.md` - 用户安装指南
- ✅ `PACKAGING_IMPROVEMENTS.md` - 改进总结
- ✅ `PACKAGING_FINAL.md` - 本文档

### 修改文件

- ✅ `scripts/build-dmg.sh` - 添加 `prepare_resources()` 调用
- ✅ `src-tauri/tauri.conf.json` - 配置 sidecar 和 externalBin
- ✅ `src-tauri/src/commands/video.rs` - 智能 FFmpeg 路径查找
- ✅ `src-tauri/Cargo.toml` - 添加 features 配置
- ✅ `package.json` - 添加 `prepare:resources` 命令

### 删除文件

- ✅ `core-engine/build-executable.sh` - 不再需要（逻辑集成到 prepare-resources.sh）

---

## 📊 构建时间和文件大小

### 构建时间

- **首次构建**: ~10-15 分钟
  - PyInstaller 打包: ~5-10 分钟
  - Tauri 编译: ~3-5 分钟

- **后续构建**: ~3-5 分钟
  - 如果 `monosub-engine` 已存在会跳过 PyInstaller

### 文件大小

- **DMG 总大小**: ~300-600MB
  - `monosub-engine`: ~200-500MB (含 PyTorch)
  - `ffmpeg`: ~50MB
  - `ffprobe`: ~20MB
  - Tauri 应用: ~10MB
  - 前端资源: ~5MB

---

## 🎯 用户安装流程

### 1. 下载

用户从 GitHub Releases 下载 `MonoSub-0.1.0-universal.dmg`

### 2. 安装

1. 双击 DMG
2. 拖动 `MonoSub.app` 到 `Applications`
3. 完成

### 3. 首次运行

```bash
# 在 Applications 中找到并双击 MonoSub
# 如果遇到安全警告，前往系统偏好设置允许
```

### 4. 使用

1. 选择视频/音频文件
2. 选择输出格式
3. 开始转录
4. 完成

**无需任何配置或依赖安装！**

---

## 🔍 开发 vs 生产

### 开发模式

```bash
bun run dev
```

- ✅ 使用 `core-engine/.venv/bin/python`
- ✅ 直接运行 `core-engine/main.py`
- ✅ 系统 FFmpeg (`which ffmpeg`)
- ✅ 前端热重载

### 生产模式（打包后）

```bash
open /Applications/MonoSub.app
```

- ✅ 使用 `monosub-engine` sidecar
- ✅ 打包的 FFmpeg 二进制
- ✅ 优化的前端构建
- ✅ 代码签名（如配置）

**Rust 代码自动检测并切换！**

---

## 🎉 问题解决

### 原问题

> "使用 bun run build:dmg 打包的内容发现并不能使用，怀疑是打包的问题，是不是需要一些技术手段将 python ffmpeg 等依赖资源也打包到 dmg 中"

### 解决方案总结

1. ✅ **FFmpeg 打包** - 自动从 Homebrew 复制到应用资源
2. ✅ **Python 打包** - 使用 PyInstaller 打包为独立可执行文件
3. ✅ **依赖打包** - 所有 Python 依赖（FunASR, PyTorch 等）都在 `monosub-engine` 中
4. ✅ **路径智能** - Rust 代码自动检测开发/生产环境
5. ✅ **一键构建** - `bun run build:dmg` 自动完成所有工作

### 用户体验提升

| 之前 | 现在 |
|------|------|
| ❌ 需要用户安装 Python | ✅ 内置 Python |
| ❌ 需要用户安装依赖包 | ✅ 内置所有依赖 |
| ❌ 需要运行安装脚本 | ✅ 开箱即用 |
| ❌ 选择文件后无响应 | ✅ 正常工作 |

---

## 📚 参考文档

| 文档 | 用途 |
|------|------|
| [QUICKSTART.md](QUICKSTART.md) | 快速开始（推荐首先阅读） |
| [BUILD.md](BUILD.md) | 详细构建说明 |
| [PACKAGING_SIDECAR.md](PACKAGING_SIDECAR.md) | Sidecar 架构解释 |
| [INSTALL.md](INSTALL.md) | 用户安装指南 |

---

## 🚀 立即开始

```bash
# 确保已安装 FFmpeg
brew install ffmpeg

# 初始化 Python 环境（仅首次）
cd core-engine && uv sync && cd ..

# 一键构建
bun run build:dmg

# 构建完成后
open dist/MonoSub-*.dmg
```

---

**恭喜！您的应用现在可以完美打包并分发给用户了！** 🎉
