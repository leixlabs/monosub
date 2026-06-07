# 日志系统快速入门

## 快速查看日志

### macOS
```bash
# 实时查看今天的日志
tail -f ~/Library/Logs/MonoSub/monosub.log.$(date +%Y-%m-%d)

# 查看所有日志文件
ls -lht ~/Library/Logs/MonoSub/

# 搜索错误
grep "ERROR" ~/Library/Logs/MonoSub/*.log
```

### 从终端启动应用（查看实时输出）
```bash
/Applications/MonoSub.app/Contents/MacOS/MonoSub
```

### 使用系统日志查看器
```bash
# 实时监控
log stream --predicate 'process == "MonoSub"' --level debug

# 或打开控制台应用
open -a Console
```

## 日志级别控制

```bash
# 启用详细日志（DEBUG 级别）
RUST_LOG=debug /Applications/MonoSub.app/Contents/MacOS/MonoSub

# 只显示警告和错误
RUST_LOG=warn /Applications/MonoSub.app/Contents/MacOS/MonoSub
```

## 关键功能

✅ **自动轮转**: 每天自动创建新日志文件
✅ **自动清理**: 保留最近 7 天的日志
✅ **结构化日志**: 包含时间戳、级别、线程ID、文件位置
✅ **大小控制**: 通过日志轮转和自动清理限制总大小

详细说明请参考 [LOGGING.md](LOGGING.md)
