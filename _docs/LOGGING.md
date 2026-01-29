# 日志系统说明

MonoSub 已配置持久化日志系统，支持日志轮转和大小限制。

## 日志存储位置

日志文件会根据操作系统存储在不同位置：

### macOS
```bash
~/Library/Logs/MonoSub/
```

### Windows
```
%APPDATA%/MonoSub/logs/
```

### Linux
```
~/.local/share/MonoSub/logs/
```

## 日志文件格式

- 文件名格式：`monosub.log.YYYY-MM-DD`（每天轮转一次）
- 编码：UTF-8
- 包含内容：时间戳、日志级别、线程ID、文件位置、日志消息

### 示例日志
```
2026-01-29T10:30:45.123456Z  INFO monosub::logging:52: Logging initialized. Log directory: "/Users/lei/Library/Logs/MonoSub"
2026-01-29T10:31:12.456789Z  INFO monosub::commands::transcribe:55 task_id="abc123" video_path="/path/to/video.mp4": Starting transcription task
2026-01-29T10:31:15.789012Z  INFO monosub::commands::transcribe:78 task_id="abc123" is_dev_mode=true: Running transcription
```

## 日志轮转策略

- **轮转周期**：每天午夜自动轮转
- **保留数量**：最近 7 天的日志文件
- **自动清理**：启动时自动删除超过 7 天的旧日志

## 日志级别

可以通过环境变量 `RUST_LOG` 控制日志级别：

```bash
# 设置为 DEBUG 级别（更详细）
export RUST_LOG=debug
/Applications/MonoSub.app/Contents/MacOS/MonoSub

# 设置为 INFO 级别（默认）
export RUST_LOG=info
/Applications/MonoSub.app/Contents/MacOS/MonoSub

# 设置为 WARN 级别（只显示警告和错误）
export RUST_LOG=warn
/Applications/MonoSub.app/Contents/MacOS/MonoSub
```

可用级别（从详细到简略）：
- `trace` - 最详细（包含所有调试信息）
- `debug` - 调试信息
- `info` - 一般信息（默认）
- `warn` - 警告信息
- `error` - 错误信息

## 查看日志的方法

### 方法 1: 直接查看日志文件

```bash
# macOS - 查看今天的日志
tail -f ~/Library/Logs/MonoSub/monosub.log.$(date +%Y-%m-%d)

# macOS - 查看所有日志文件
ls -lht ~/Library/Logs/MonoSub/

# macOS - 搜索特定关键词
grep "ERROR" ~/Library/Logs/MonoSub/*.log
```

### 方法 2: 使用系统日志查看器

**macOS Console.app**:
```bash
# 打开控制台应用
open -a Console

# 或者使用 log stream 命令实时查看
log stream --predicate 'process == "MonoSub"' --level debug
```

### 方法 3: 从终端启动应用查看实时输出

```bash
# 直接运行应用，可以看到 stdout/stderr 实时输出
/Applications/MonoSub.app/Contents/MacOS/MonoSub
```

## 日志内容说明

### 应用启动
```
INFO monosub::logging: Logging initialized
INFO monosub::main: MonoSub application starting...
```

### 转录任务
```
INFO monosub::commands::transcribe: Starting transcription task
INFO monosub::commands::transcribe: Running transcription
DEBUG monosub::commands::transcribe: Python process spawned successfully
DEBUG monosub::commands::transcribe: Transcription progress
INFO monosub::commands::transcribe: Transcription completed
```

### 错误日志
```
ERROR monosub::commands::transcribe: Failed to spawn Python process
ERROR monosub::commands::video: ffprobe command failed
```

## 故障排查

### 如果日志文件未生成

1. 检查日志目录权限：
```bash
# macOS
ls -la ~/Library/Logs/MonoSub/
```

2. 手动创建日志目录：
```bash
# macOS
mkdir -p ~/Library/Logs/MonoSub
```

3. 从终端启动应用查看错误信息：
```bash
/Applications/MonoSub.app/Contents/MacOS/MonoSub
```

### 如果需要更详细的日志

```bash
# 设置环境变量后启动
RUST_LOG=debug /Applications/MonoSub.app/Contents/MacOS/MonoSub
```

### 如果日志文件过多占用空间

日志会自动保留最近 7 天的文件。如需手动清理：

```bash
# macOS - 删除所有日志
rm ~/Library/Logs/MonoSub/*.log.*

# macOS - 只保留最近 3 天
find ~/Library/Logs/MonoSub -name "*.log.*" -mtime +3 -delete
```

## 开发环境

在开发模式下（`bun run dev`），日志同时输出到：
1. 文件：`~/Library/Logs/MonoSub/monosub.log.YYYY-MM-DD`
2. 控制台：Tauri 开发服务器的 stdout

在生产环境下（打包的 DMG），日志只输出到文件。
