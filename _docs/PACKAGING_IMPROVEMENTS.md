# MonoSub 打包优化总结

## 🎯 问题分析

您遇到的问题是：**打包后的 DMG 无法使用，选择文件后中间没有展示 list**

根本原因：
1. ❌ Python 环境和 core-engine 未打包到 DMG
2. ❌ FFmpeg/ffprobe 二进制未打包
3. ❌ Rust 代码使用的是开发环境的硬编码路径
4. ❌ 缺少依赖检测和错误提示机制

## ✅ 已实施的解决方案

### 1. 创建资源准备脚本

**文件**: [scripts/prepare-resources.sh](scripts/prepare-resources.sh)

功能：
- 自动从系统复制 FFmpeg/ffprobe 到 `src-tauri/resources/bin/`
- 复制 Python 源代码到 `src-tauri/resources/core-engine/`
- 生成资源清单文件

使用：
```bash
./scripts/prepare-resources.sh
```

### 2. 更新 Tauri 配置

**文件**: [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json:24-34)

变更：
```json
{
  "bundle": {
    "resources": {
      "resources/*": "./"
    },
    "externalBin": [
      "resources/bin/ffmpeg",
      "resources/bin/ffprobe"
    ]
  }
}
```

这确保资源文件会被打包到 `MonoSub.app/Contents/Resources/`

### 3. 修改 Rust 代码使用动态路径

**文件**:
- [src-tauri/src/commands/transcribe.rs](src-tauri/src/commands/transcribe.rs:33-56)
- [src-tauri/src/commands/video.rs](src-tauri/src/commands/video.rs:10-41)

关键改进：

```rust
// 自动检测环境并使用正确的路径
fn get_python_path(app: &AppHandle) -> Result<String, String> {
    // 生产: 使用系统 Python
    // 开发: 使用虚拟环境 Python
}

fn get_core_engine_path(app: &AppHandle) -> Result<String, String> {
    // 生产: 使用 app.path().resource_dir()
    // 开发: 使用相对路径
}

fn get_ffmpeg_path(app: Option<&AppHandle>) -> String {
    // 生产: 使用打包的二进制
    // 开发: 使用系统 FFmpeg
}
```

### 4. 增强构建脚本

**文件**: [scripts/build-dmg.sh](scripts/build-dmg.sh:257-268)

新增功能：
- 在构建前自动运行 `prepare_resources()`
- 添加 `prepare` 命令单独准备资源
- 改进错误处理和日志输出

使用：
```bash
# 完整构建 (推荐)
bun run build:dmg

# 或分步执行
./scripts/build-dmg.sh prepare   # 准备资源
./scripts/build-dmg.sh frontend  # 构建前端
./scripts/build-dmg.sh tauri     # 构建 Tauri
./scripts/build-dmg.sh dmg       # 创建 DMG
```

### 5. 创建依赖安装脚本

**文件**: [scripts/install-deps.sh](scripts/install-deps.sh)

功能：
- 检测 Python 和 pip 是否安装
- 自动安装所有 Python 依赖
- 用户友好的错误提示

用户使用：
```bash
./install-deps.sh
```

### 6. 完善文档

新增文档：
- ✅ [PACKAGING.md](PACKAGING.md) - 开发者打包指南
- ✅ [INSTALL.md](INSTALL.md) - 用户安装指南
- ✅ 本文档 - 改进总结

## 📦 打包后的应用结构

```
MonoSub.app/
├── Contents/
│   ├── MacOS/
│   │   └── MonoSub              # 主程序
│   ├── Resources/
│   │   ├── bin/
│   │   │   ├── ffmpeg           # FFmpeg 二进制 ✅
│   │   │   └── ffprobe          # FFprobe 二进制 ✅
│   │   ├── core-engine/
│   │   │   ├── main.py          # Python 入口 ✅
│   │   │   ├── monosub_core/    # Python 源代码 ✅
│   │   │   └── requirements.txt # Python 依赖列表 ✅
│   │   └── dist/                # 前端资源 ✅
│   └── Info.plist
```

## 🚀 使用流程

### 开发者 - 构建 DMG

```bash
# 1. 准备环境
brew install ffmpeg create-dmg

# 2. 完整构建
bun run build:dmg

# 3. 输出文件
ls -lh dist/MonoSub-*.dmg
```

### 用户 - 安装应用

```bash
# 1. 安装应用
# 将 MonoSub.app 拖到 Applications 文件夹

# 2. 安装 Python (如果没有)
brew install python@3.11

# 3. 安装 Python 依赖
pip3 install funasr modelscope ffmpeg-python torch torchaudio --user

# 4. 运行应用
open /Applications/MonoSub.app
```

## 🔧 工作原理

### 开发环境
```
src-tauri/  → 相对路径查找 ../core-engine/
系统 ffmpeg → which ffmpeg
虚拟环境   → core-engine/.venv/bin/python
```

### 生产环境 (打包后)
```
Resources/core-engine/ → app.path().resource_dir()
Resources/bin/ffmpeg   → app.path().resource_dir()
系统 Python            → which python3
```

## ⚠️ 注意事项

### Python 依赖为什么不打包？

原因：
1. Python 依赖包非常大 (PyTorch ~2GB)
2. 依赖系统特定的编译版本
3. 用户可能需要使用 GPU 加速
4. 打包为单个可执行文件会导致启动缓慢

解决方案：
- ✅ 用户首次运行时安装依赖
- ✅ 提供自动化安装脚本
- 🔄 未来: 实现应用内依赖安装向导

### 验证打包是否成功

```bash
# 检查资源文件
tree /Applications/MonoSub.app/Contents/Resources/

# 应该看到:
# Resources/
# ├── bin/
# │   ├── ffmpeg
# │   └── ffprobe
# └── core-engine/
#     ├── main.py
#     ├── monosub_core/
#     └── requirements.txt
```

## 🐛 故障排除

### 问题 1: 准备资源失败

```bash
# 检查 FFmpeg 是否安装
which ffmpeg

# 如果没有，安装
brew install ffmpeg

# 重新运行
./scripts/prepare-resources.sh
```

### 问题 2: 构建失败

```bash
# 清理并重新构建
./scripts/build-dmg.sh clean
bun run build:dmg
```

### 问题 3: 打包后应用无法运行

```bash
# 检查代码签名 (macOS 安全限制)
codesign -dv /Applications/MonoSub.app

# 移除隔离属性
xattr -cr /Applications/MonoSub.app
```

## 📝 TODO / 未来改进

- [ ] 实现应用内依赖检测和安装向导
- [ ] 使用 PyInstaller 打包 Python 为独立可执行文件
- [ ] 支持 Windows 和 Linux 构建
- [ ] 实现增量更新机制
- [ ] 添加自动签名和公证流程

## 🎉 总结

通过这些改进：

1. ✅ **FFmpeg** 现在会被打包到 DMG
2. ✅ **Python 源代码** 会被打包到 DMG
3. ✅ **路径自动检测** 开发和生产环境自动切换
4. ✅ **错误提示** 依赖缺失时会显示友好的错误信息
5. ✅ **文档完善** 提供完整的安装和故障排除指南

**选择文件后无法显示列表的问题应该已经解决！**

---

如有问题，请查看 [INSTALL.md](INSTALL.md) 或 [PACKAGING.md](PACKAGING.md)
