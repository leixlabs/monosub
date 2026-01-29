# MonoSub Sidecar 打包方案

## 🎯 最终解决方案

使用 **PyInstaller + Tauri Sidecar** 将 Python 应用打包为独立可执行文件，无需用户安装任何依赖！

## ✅ 方案优势

- ✅ **零依赖**: 用户无需安装 Python 或任何依赖包
- ✅ **开箱即用**: 下载 DMG，拖入 Applications，立即使用
- ✅ **跨平台**: 可轻松适配 Windows 和 Linux
- ✅ **更安全**: Python 代码打包为二进制，不易被修改
- ✅ **更专业**: 符合桌面应用的标准打包方式

## 📦 打包架构

```
MonoSub.app/Contents/
├── MacOS/
│   └── MonoSub                    # Rust/Tauri 主程序
└── Resources/
    └── bin/
        ├── ffmpeg                 # FFmpeg 二进制 ✅
        ├── ffprobe                # FFprobe 二进制 ✅
        └── monosub-engine         # Python 打包的可执行文件 ✅
            (包含所有 Python 依赖：FunASR, PyTorch 等)
```

## 🚀 一键构建流程

### 自动构建（推荐）

```bash
# 一条命令完成所有构建
bun run build:dmg
```

这会自动完成：
1. 检测并复制 FFmpeg 二进制
2. 使用 PyInstaller 打包 Python 可执行文件
3. 构建前端
4. 构建 Tauri 应用
5. 创建 DMG 安装包

### 前置要求

```bash
# 1. 安装 FFmpeg
brew install ffmpeg

# 2. 初始化 Python 环境
cd core-engine
uv sync
cd ..
```

### 仅准备资源（可选）

```bash
# 如果只想准备资源文件而不构建整个应用
bun run prepare:resources
```

## 🔧 技术实现

### PyInstaller 打包

[core-engine/build-executable.sh](core-engine/build-executable.sh) 使用 PyInstaller 将 Python 应用打包：

```bash
pyinstaller \
    --onefile \
    --name monosub-engine \
    --hidden-import=funasr \
    --collect-all funasr \
    --collect-all modelscope \
    main.py
```

生成的可执行文件包含：
- Python 解释器
- 所有依赖库（FunASR, PyTorch, ModelScope 等）
- 您的 Python 源代码

### Tauri Sidecar 配置

[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) 配置:

```json
{
  "bundle": {
    "externalBin": [
      "resources/bin/monosub-engine"
    ]
  },
  "plugins": {
    "shell": {
      "sidecar": true,
      "scope": [
        {
          "name": "resources/bin/monosub-engine",
          "sidecar": true
        }
      ]
    }
  }
}
```

### Rust 代码使用 Sidecar

[src-tauri/src/commands/transcribe_sidecar.rs](src-tauri/src/commands/transcribe_sidecar.rs):

```rust
// 生产环境：使用 sidecar 运行打包的可执行文件
let sidecar_command = shell.sidecar("monosub-engine")?;
let (mut rx, _child) = sidecar_command
    .args(&[video_path, output_path, "--format", format])
    .spawn()?;

// 开发环境：直接运行 Python 脚本
let mut child = Command::new("python")
    .arg("core-engine/main.py")
    .spawn()?;
```

## 📋 构建步骤详解

### Step 1: 安装 PyInstaller 依赖

```bash
cd core-engine
source .venv/bin/activate
pip install pyinstaller
```

### Step 2: 构建 Python 可执行文件

```bash
./build-executable.sh
```

输出：
- `dist/monosub-engine` (约 200-500MB，包含所有依赖)

### Step 3: 准备资源

```bash
cd ..
./scripts/prepare-resources.sh
```

自动完成：
- ✅ 复制 FFmpeg/ffprobe 到 `src-tauri/resources/bin/`
- ✅ 复制 `monosub-engine` 到 `src-tauri/resources/bin/`

### Step 4: 构建 Tauri 应用

```bash
bun run build:dmg
```

## 🎯 用户体验

### 安装

1. 下载 `MonoSub-0.1.0-universal.dmg`
2. 拖动到 Applications 文件夹
3. 双击运行 - **无需任何配置！**

### 首次运行

- ❌ 无需安装 Python
- ❌ 无需安装依赖包
- ❌ 无需运行安装脚本
- ✅ 直接选择文件开始转录

## ⚖️ 文件大小权衡

### 打包后的大小

- DMG 总大小: ~300-600MB
  - Tauri 应用: ~10MB
  - FFmpeg: ~50MB
  - monosub-engine: ~200-500MB (包含 PyTorch)

### 优化建议

如果觉得太大，可以考虑：

1. **分离模型下载**: 首次运行时下载模型（而不是打包）
2. **精简依赖**: 使用 `--exclude-module` 排除不需要的包
3. **压缩**: DMG 本身就是压缩格式

## 🐛 故障排除

### PyInstaller 构建失败

```bash
# 清理缓存
rm -rf build dist *.spec

# 重新构建
./build-executable.sh
```

### 可执行文件无法运行

```bash
# 检查权限
chmod +x core-engine/dist/monosub-engine

# 测试运行
./core-engine/dist/monosub-engine --help
```

### Sidecar 无法启动

检查 [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json:37-40):
```json
"externalBin": [
  "resources/bin/monosub-engine"  // 确保路径正确
]
```

## 📊 对比：三种打包方案

| 方案 | 用户体验 | 包大小 | 维护成本 |
|------|---------|--------|---------|
| **方案1**: 要求用户安装 Python | ❌ 差 | 小 (~50MB) | 高（支持成本高） |
| **方案2**: 打包 Python 源码 | ⚠️ 中等 | 中 (~100MB) | 中（需安装依赖） |
| **方案3**: PyInstaller + Sidecar | ✅ 好 | 大 (~500MB) | 低（开箱即用） |

**推荐**: 方案3 (PyInstaller + Sidecar) - 虽然包较大，但用户体验最好。

## 🎉 总结

通过 PyInstaller + Tauri Sidecar 方案：

1. ✅ **Python 及所有依赖打包为独立可执行文件**
2. ✅ **FFmpeg 打包到应用资源**
3. ✅ **用户无需安装任何依赖**
4. ✅ **开发/生产环境自动切换**
5. ✅ **专业的桌面应用体验**

---

下一步：运行 `bun run build:dmg` 开始构建！
