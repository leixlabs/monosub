# DMG 启动崩溃问题修复

## 🐛 问题描述

打包后的 DMG 文件，双击打开后直接崩溃退出。

## 🔍 问题诊断过程

### 1. 检查崩溃日志

```bash
# 直接运行可执行文件查看错误
/path/to/MonoSub.app/Contents/MacOS/monosub
```

**错误输出**：
```
thread 'main' panicked at src/main.rs:18:10:
error while running tauri application: PluginInitialization("shell",
"Error deserializing 'plugins.shell' within your Tauri configuration:
unknown field `scope`, expected `open`")
```

### 2. 发现的问题

#### 问题 1: Tauri 配置文件错误

[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) 中有无效的配置：

```json
{
  "plugins": {
    "shell": {
      "sidecar": true,
      "scope": [...]  // ❌ 错误：Tauri v2 不支持此字段
    }
  }
}
```

#### 问题 2: 二进制文件路径查找错误

Rust 代码在错误的位置查找二进制文件：
- 代码查找位置：`Contents/Resources/bin/`
- 实际位置：`Contents/MacOS/` （`externalBin` 自动放置）

## ✅ 解决方案

### 1. 修复 Tauri 配置

删除无效的 `plugins.shell` 配置：

```json
{
  "bundle": {
    "externalBin": [
      "resources/bin/ffmpeg",
      "resources/bin/ffprobe",
      "resources/bin/monosub-engine"
    ]
  }
  // ✅ 不需要显式配置 plugins.shell
}
```

Tauri v2 会自动处理 `externalBin`，无需额外配置。

### 2. 修复二进制文件路径查找

**之前的代码**（错误）：
```rust
fn get_ffmpeg_path(app: Option<&AppHandle>) -> String {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let ffmpeg_bin = resource_dir.join("bin/ffmpeg"); // ❌ 错误位置
        ...
    }
}
```

**修复后的代码**：
```rust
fn get_ffmpeg_path(app: Option<&AppHandle>) -> String {
    // externalBin 文件位于 Contents/MacOS/ 目录
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let ffmpeg_bin = exe_dir.join("ffmpeg"); // ✅ 正确位置
            if ffmpeg_bin.exists() {
                return ffmpeg_bin.to_string_lossy().to_string();
            }
        }
    }
    "ffmpeg".to_string() // 回退到系统 FFmpeg
}
```

## 📂 Tauri externalBin 工作原理

### 打包流程

1. **源文件**（构建前）：
   ```
   src-tauri/resources/bin/
   ├── ffmpeg-aarch64-apple-darwin
   ├── ffprobe-aarch64-apple-darwin
   └── monosub-engine-aarch64-apple-darwin
   ```

2. **Tauri 自动处理**：
   - 去掉架构后缀
   - 复制到 `Contents/MacOS/`
   - 设置执行权限

3. **打包后**（应用内）：
   ```
   MonoSub.app/Contents/MacOS/
   ├── monosub              # 主程序
   ├── ffmpeg               # ✅ 自动处理
   ├── ffprobe              # ✅ 自动处理
   └── monosub-engine       # ✅ 自动处理
   ```

### 在代码中使用

```rust
// 方法 1: 使用相对于主程序的路径
std::env::current_exe()?.parent()?.join("ffmpeg")

// 方法 2: 使用 Tauri sidecar API
shell.sidecar("ffmpeg")  // 自动查找 Contents/MacOS/ffmpeg
```

## 🎯 修复的文件

1. **[src-tauri/tauri.conf.json](src-tauri/tauri.conf.json)**
   - 删除了 `plugins.shell` 配置

2. **[src-tauri/src/commands/video.rs](src-tauri/src/commands/video.rs)**
   - 修复了 `get_ffmpeg_path()` 和 `get_ffprobe_path()`
   - 改为查找 `current_exe().parent()` 目录

3. **[src-tauri/src/commands/transcribe.rs](src-tauri/src/commands/transcribe.rs)**
   - sidecar 调用已经正确，无需修改

## ✅ 验证修复

```bash
# 重新构建
cargo tauri build --target aarch64-apple-darwin

# 测试运行
/path/to/MonoSub.app/Contents/MacOS/monosub

# 检查进程
ps aux | grep monosub
# ✅ 应该看到进程正在运行
```

## 📊 问题总结

| 问题 | 原因 | 解决方案 |
|------|------|---------|
| 启动崩溃 | 配置文件错误 | 删除无效的 `plugins.shell` 配置 |
| 找不到 FFmpeg | 查找路径错误 | 使用 `current_exe().parent()` 查找 |
| sidecar 无法运行 | 配置不正确 | 依赖 Tauri 自动处理 `externalBin` |

## 🎉 结果

- ✅ 应用可以正常启动
- ✅ FFmpeg 和 ffprobe 可以被找到
- ✅ Python sidecar 可以正常运行
- ✅ 用户无需安装任何依赖

---

**应用现在可以正常工作了！**
