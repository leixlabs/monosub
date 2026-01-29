# MonoSub 快速开始

## 🎯 开发者 - 首次构建

### 1. 安装依赖

```bash
# 安装系统依赖
brew install ffmpeg

# 安装前端依赖
bun install

# 初始化 Python 环境
cd core-engine
uv sync
cd ..
```

### 2. 构建 DMG

```bash
# 一键构建
bun run build:dmg
```

等待 10-15 分钟（首次构建较慢），完成后 DMG 位于 `dist/` 目录。

### 3. 测试应用

```bash
# 打开 DMG 并安装
open dist/MonoSub-*.dmg

# 或直接运行应用
open /Applications/MonoSub.app
```

---

## 🎉 用户 - 安装使用

### 1. 下载安装

1. 下载 `MonoSub-{version}-universal.dmg`
2. 双击打开 DMG
3. 拖动 `MonoSub.app` 到 `Applications` 文件夹
4. 完成！

### 2. 首次运行

```bash
# 在 Applications 中找到 MonoSub
# 双击运行

# 如果遇到安全警告
# 前往：系统偏好设置 > 安全性与隐私 > 点击"仍要打开"
```

### 3. 使用应用

1. 点击"选择文件"选择视频/音频文件
2. 选择输出格式（SRT/VTT/TXT）
3. 点击"开始转录"
4. 等待处理完成
5. 字幕文件自动保存到源文件目录

---

## 🛠️ 开发者 - 日常开发

### 启动开发服务器

```bash
bun run dev
```

应用会在开发模式启动：
- 前端热重载
- 使用开发环境的 Python
- 实时日志输出

### 重新构建

```bash
# 清理旧构建
bun run clean

# 重新构建
bun run build:dmg
```

### 仅更新某个部分

```bash
# 仅构建前端
bun run build:frontend

# 仅准备资源（FFmpeg + Python）
bun run prepare:resources

# 仅构建 Tauri
./scripts/build-dmg.sh tauri

# 仅创建 DMG
./scripts/build-dmg.sh dmg
```

---

## 📁 项目结构

```
monosub/
├── src/                    # 前端代码 (React + TypeScript)
├── src-tauri/             # Tauri 后端 (Rust)
│   ├── src/               # Rust 源代码
│   └── resources/         # 打包资源（构建时生成）
│       └── bin/
│           ├── ffmpeg
│           ├── ffprobe
│           └── monosub-engine
├── core-engine/           # Python 核心引擎
│   ├── main.py           # Python 入口
│   ├── monosub_core/     # 核心逻辑
│   └── .venv/            # Python 虚拟环境
├── scripts/              # 构建脚本
│   ├── build-dmg.sh      # 主构建脚本
│   └── prepare-resources.sh  # 资源准备脚本
└── dist/                 # 输出目录（构建时生成）
    └── MonoSub-*.dmg
```

---

## 🔍 常见问题

### Q: 首次构建很慢？

**A:** 正常！PyInstaller 打包 Python 需要 5-10 分钟。后续构建会快很多（如果 `monosub-engine` 已存在会跳过）。

### Q: 如何强制重新构建 Python 可执行文件？

**A:**
```bash
rm -rf src-tauri/resources/bin/monosub-engine
bun run build:dmg
```

### Q: 构建失败怎么办？

**A:** 查看详细错误日志，常见问题：
- FFmpeg 未安装：`brew install ffmpeg`
- Python 环境问题：`cd core-engine && uv sync`
- 磁盘空间不足：清理旧构建 `bun run clean`

### Q: DMG 文件太大？

**A:** 这是正常的！包含了：
- PyTorch (~200MB)
- FunASR 模型和依赖 (~100-200MB)
- FFmpeg (~50MB)

这样用户才能开箱即用，无需安装任何依赖。

### Q: 如何减小 DMG 大小？

**A:** 可以尝试：
1. 排除不需要的模块（修改 `scripts/prepare-resources.sh` 中的 PyInstaller 参数）
2. 使用模型懒加载（首次运行时下载）
3. 分离不同架构（Intel vs Apple Silicon）

---

## 📚 更多文档

- [BUILD.md](BUILD.md) - 详细构建指南
- [PACKAGING_SIDECAR.md](PACKAGING_SIDECAR.md) - Sidecar 架构说明
- [INSTALL.md](INSTALL.md) - 用户安装指南

---

**开始构建**: `bun run build:dmg` 🚀
