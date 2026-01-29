# Tauri 架构后缀说明

## 🎯 问题

在构建时遇到错误：
```
resource path `resources/bin/ffmpeg-aarch64-apple-darwin` doesn't exist
```

## 📝 原因

Tauri v2 的 `externalBin` 配置会自动为不同架构添加后缀：

| 平台 | 架构后缀 |
|------|---------|
| macOS (Apple Silicon) | `-aarch64-apple-darwin` |
| macOS (Intel) | `-x86_64-apple-darwin` |
| Windows | `-x86_64-pc-windows-msvc.exe` |
| Linux | `-x86_64-unknown-linux-gnu` |

参考: https://v2.tauri.app/reference/config/#externalbinconfig

## ✅ 解决方案

### 1. 资源文件命名

所有二进制文件必须包含架构后缀：

```
src-tauri/resources/bin/
├── ffmpeg-aarch64-apple-darwin
├── ffprobe-aarch64-apple-darwin
└── monosub-engine-aarch64-apple-darwin
```

### 2. 自动检测和添加后缀

[scripts/prepare-resources.sh](scripts/prepare-resources.sh) 会自动检测当前架构并添加正确的后缀：

```bash
# 检测架构
local arch=$(uname -m)
local target_triple=""
if [ "$arch" = "arm64" ]; then
    target_triple="aarch64-apple-darwin"
else
    target_triple="x86_64-apple-darwin"
fi

# 复制文件时添加后缀
cp ffmpeg "$ffmpeg_dir/ffmpeg-$target_triple"
```

### 3. Tauri 配置

[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) 中的配置保持简单：

```json
{
  "bundle": {
    "externalBin": [
      "resources/bin/ffmpeg",
      "resources/bin/ffprobe",
      "resources/bin/monosub-engine"
    ]
  }
}
```

Tauri 会自动查找对应架构的文件，例如在 Apple Silicon 上会查找 `ffmpeg-aarch64-apple-darwin`。

## 🚀 使用流程

### 开发者

```bash
# 1. 准备资源（自动添加架构后缀）
bun run prepare:resources

# 2. 构建
bun run build:dmg
```

### 输出文件

```
src-tauri/resources/bin/
├── ffmpeg-aarch64-apple-darwin      # Apple Silicon
├── ffprobe-aarch64-apple-darwin     # Apple Silicon
└── monosub-engine-aarch64-apple-darwin  # Apple Silicon
```

## 📊 跨平台支持

如果要支持多个架构，可以准备多份带不同后缀的文件：

```
src-tauri/resources/bin/
├── ffmpeg-aarch64-apple-darwin      # Apple Silicon
├── ffmpeg-x86_64-apple-darwin       # Intel Mac
├── ffmpeg-x86_64-pc-windows-msvc.exe  # Windows
└── ffmpeg-x86_64-unknown-linux-gnu    # Linux
```

Tauri 会根据目标平台自动选择正确的文件。

## 🎉 总结

1. ✅ Tauri 自动为 `externalBin` 添加架构后缀
2. ✅ [scripts/prepare-resources.sh](scripts/prepare-resources.sh) 自动检测并添加后缀
3. ✅ 配置文件保持简洁，无需手动指定后缀
4. ✅ 支持跨平台构建

---

**现在构建应该可以正常工作了！** 🚀
