# MonoSub 构建指南

## 🚀 一键构建

```bash
# 一条命令完成所有构建
bun run build:dmg
```

这会自动完成：
1. ✅ 准备 FFmpeg 二进制
2. ✅ 使用 PyInstaller 打包 Python 可执行文件
3. ✅ 构建前端
4. ✅ 构建 Tauri 应用
5. ✅ 创建 DMG 安装包

输出文件：`dist/MonoSub-{version}-universal.dmg`

## 📋 前置要求

### 1. 安装基础依赖

```bash
# macOS
brew install ffmpeg
```

### 2. 初始化 Python 环境

```bash
cd core-engine
uv sync  # 或者 python -m venv .venv && source .venv/bin/activate && pip install -r requirements.txt
cd ..
```

## 🔧 构建流程详解

### 自动流程（推荐）

```bash
bun run build:dmg
```

### 手动分步构建

如果需要调试或分步执行：

```bash
# 1. 准备资源（FFmpeg + Python 可执行文件）
bun run prepare:resources

# 2. 构建前端
bun run build:frontend

# 3. 构建 Tauri 应用
./scripts/build-dmg.sh tauri

# 4. 创建 DMG
./scripts/build-dmg.sh dmg
```

## 📦 打包架构

### 最终 DMG 结构

```
MonoSub.app/
├── Contents/
│   ├── MacOS/
│   │   └── MonoSub                    # Tauri 主程序
│   └── Resources/
│       ├── bin/
│       │   ├── ffmpeg                 # FFmpeg 二进制
│       │   ├── ffprobe                # FFprobe 二进制
│       │   └── monosub-engine         # Python 可执行文件（包含所有依赖）
│       └── dist/                      # 前端资源
```

### monosub-engine 包含

- Python 3.x 解释器
- FunASR
- ModelScope
- PyTorch
- TorchAudio
- 所有其他依赖

## ⏱️ 构建时间

首次构建：
- PyInstaller 打包: ~5-10 分钟
- Tauri 编译: ~3-5 分钟
- 总计: ~10-15 分钟

后续构建（如果 monosub-engine 已存在）：
- ~3-5 分钟

## 📊 文件大小

- FFmpeg: ~50MB
- monosub-engine: ~200-500MB (包含 PyTorch)
- Tauri 应用: ~10MB
- 前端资源: ~5MB
- **总 DMG 大小: ~300-600MB**

## 🎯 用户安装体验

用户只需：
1. 下载 DMG
2. 拖到 Applications
3. 双击运行

**无需安装 Python 或任何依赖！**

## 🐛 故障排除

### PyInstaller 构建失败

```bash
# 重新构建（删除旧文件）
rm -rf src-tauri/resources/bin/monosub-engine
rm -rf core-engine/dist core-engine/build

# 重新运行
bun run build:dmg
```

### FFmpeg 未找到

```bash
# 安装 FFmpeg
brew install ffmpeg

# 重新准备资源
bun run prepare:resources
```

### 虚拟环境问题

```bash
# 重新创建虚拟环境
cd core-engine
rm -rf .venv
uv sync
cd ..
```

### 清理所有构建产物

```bash
# 使用内置清理命令
bun run clean

# 或手动清理
rm -rf dist
rm -rf src-tauri/target
rm -rf src-tauri/resources
rm -rf core-engine/dist
rm -rf core-engine/build
```

## 📝 开发 vs 生产

### 开发模式

```bash
bun run dev
```

- 使用开发环境的 Python 虚拟环境
- 直接运行 `core-engine/main.py`
- 热重载前端

### 生产模式（DMG）

```bash
bun run build:dmg
```

- 使用 PyInstaller 打包的 monosub-engine
- 优化的前端构建
- 代码签名（如果配置）

## 🔐 代码签名和公证（可选）

如需分发到其他用户：

```bash
# 设置环境变量
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name"
export APPLE_ID="your@email.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="XXXXXXXXXX"

# 构建并签名
bun run build:dmg
```

## 📚 相关命令

```bash
# 开发
bun run dev                    # 启动开发服务器
bun run build:frontend         # 仅构建前端

# 构建
bun run build:dmg              # 完整构建 DMG
bun run prepare:resources      # 仅准备资源
bun run build:dmg-only         # 仅创建 DMG（需要先构建）

# 维护
bun run clean                  # 清理构建产物
bun run check                  # TypeScript 类型检查
bun run lint                   # ESLint 检查
```

## 🎉 成功标志

构建成功后，您应该看到：

```
[SUCCESS] =========================================
[SUCCESS] 构建完成!
[SUCCESS] DMG 文件: /path/to/dist/MonoSub-0.1.0-universal.dmg
[SUCCESS] =========================================
[INFO] DMG 大小: 456M
```

---

**现在就试试**: `bun run build:dmg`
