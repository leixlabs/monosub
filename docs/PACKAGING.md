# MonoSub 打包说明

本文档说明如何为 MonoSub 创建可分发的 DMG 安装包。

## 打包架构

MonoSub 是一个混合应用，包含以下组件：

1. **前端**: React + TypeScript (打包到应用内)
2. **Tauri 后端**: Rust (编译为原生二进制)
3. **FFmpeg**: 视频/音频处理工具 (打包到应用资源)
4. **Python 引擎**: FunASR 语音识别核心 (源代码打包，依赖需用户安装)

## 完整打包流程

### 1. 准备依赖

```bash
# 安装 FFmpeg (如果尚未安装)
brew install ffmpeg

# 安装 create-dmg (可选，用于创建更美观的 DMG)
brew install create-dmg
```

### 2. 执行打包

```bash
# 完整打包流程 (推荐)
bun run build:dmg

# 或分步执行
./scripts/build-dmg.sh prepare   # 准备资源
./scripts/build-dmg.sh build     # 完整构建
```

### 3. 输出文件

打包完成后，DMG 文件位于:
```
dist/MonoSub-{version}-universal.dmg
```

## 资源打包详情

### 自动打包的资源

以下资源会自动打包到 DMG 中：

- ✅ **FFmpeg/ffprobe**: 二进制文件位于 `MonoSub.app/Contents/Resources/bin/`
- ✅ **Python 源代码**: core-engine 位于 `MonoSub.app/Contents/Resources/core-engine/`
- ✅ **前端资源**: 编译后的网页应用

### 需要用户安装的依赖

由于 Python 依赖包较大且依赖系统环境，用户需要在首次运行时安装：

- Python 3.9+
- FunASR
- ModelScope
- PyTorch
- 其他 Python 依赖

## 首次运行配置

用户首次运行应用时需要：

### 选项 1: 手动安装 (推荐)

```bash
# 1. 安装 Python (如果没有)
brew install python@3.11

# 2. 运行依赖安装脚本
# 脚本位于应用内或可从 GitHub 下载
./install-deps.sh
```

### 选项 2: 使用应用内安装器

应用会检测依赖缺失并提示用户安装 (功能待实现)。

## 开发环境 vs 生产环境

### 开发环境

```bash
# 使用虚拟环境中的 Python
core-engine/.venv/bin/python

# FFmpeg 从系统路径查找
which ffmpeg
```

### 生产环境 (打包后)

```bash
# Python 从系统安装
/usr/local/bin/python3

# FFmpeg 从应用资源目录
MonoSub.app/Contents/Resources/bin/ffmpeg

# Python 源代码从应用资源目录
MonoSub.app/Contents/Resources/core-engine/main.py
```

## 故障排除

### 问题: 选择文件后没有显示列表

**原因**: Python 或 FFmpeg 依赖缺失

**解决方案**:
1. 检查 Python 是否安装: `python3 --version`
2. 检查 FFmpeg 是否可用: `ffmpeg -version`
3. 运行依赖安装脚本: `./install-deps.sh`

### 问题: Python 依赖包未找到

**原因**: FunASR 等依赖未安装

**解决方案**:
```bash
# 安装所有依赖
pip3 install -r core-engine/requirements.txt --user
```

### 问题: FFmpeg 命令失败

**原因**: FFmpeg 二进制未正确打包或权限问题

**解决方案**:
1. 检查资源目录: `ls MonoSub.app/Contents/Resources/bin/`
2. 检查权限: `chmod +x MonoSub.app/Contents/Resources/bin/*`
3. 重新打包: `bun run build:dmg`

## 签名和公证 (可选)

为了在 macOS 上顺利分发，建议进行代码签名和公证：

```bash
# 设置环境变量
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name"
export APPLE_ID="your@email.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="XXXXXXXXXX"

# 构建并签名
bun run build:dmg
```

## 持续改进计划

未来版本可能的改进：

1. ✅ 将 Python 依赖打包为独立可执行文件 (PyInstaller)
2. ✅ 实现应用内依赖检测和安装向导
3. ✅ 支持增量更新，减少下载大小
4. ✅ 提供 Windows 和 Linux 构建脚本

## 参考资料

- [Tauri 打包文档](https://tauri.app/v1/guides/building/)
- [macOS 应用分发指南](https://developer.apple.com/documentation/xcode/distributing-your-app-for-beta-testing-and-releases)
- [PyInstaller 文档](https://pyinstaller.org/en/stable/)
