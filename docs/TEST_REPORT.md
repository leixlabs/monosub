# MonoSub 打包测试报告

**测试日期**: 2026-01-29
**测试环境**: macOS 14.x (Apple Silicon)
**测试文件**: `/Users/lei/Movies/Video_Workspace/00_Inbox/stripe-收款.mp4` (368MB, 471秒)

---

## 📋 测试流程

### 1. 构建流程 ✅

```bash
# 更新依赖
cd core-engine && uv sync --group dev

# 构建 Python 可执行文件
cd .. && ./scripts/prepare-resources.sh

# 构建完整 DMG
bun run build:dmg
```

**结果**:
- ✅ Python 可执行文件成功创建 (214MB)
- ✅ DMG 文件成功创建 (216MB)
- ✅ 位置: `/Users/lei/dev/personal/monosub/src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/MonoSub_0.1.0_aarch64.dmg`

### 2. 安装测试 ✅

```bash
# 挂载 DMG
hdiutil attach MonoSub_0.1.0_aarch64.dmg

# 启动应用
open /Volumes/MonoSub/MonoSub.app
```

**结果**:
- ✅ App 成功启动
- ✅ UI 正常显示
- ✅ 文件选择和元数据提取正常工作

### 3. 转录功能测试 ❌

**测试步骤**:
1. 启动 MonoSub.app
2. 添加视频文件 `stripe-收款.mp4`
3. 点击开始转录

**观察到的错误** (来自 `/Users/lei/Library/Logs/MonoSub/monosub.log.2026-01-29`):

#### 错误 #1: FunASR 模块未找到
```
ModuleNotFoundError: No module named 'funasr'
```

#### 错误 #2: FFmpeg 未找到
```
Audio extraction failed: [Errno 2] No such file or directory: 'ffmpeg'
```

#### 错误 #3: PyTorch 动态库缺失
```
OSError: dlopen(.../_MEI0CS1Np/torch/lib/libtorch_global_deps.dylib, 0x000A):
tried: '.../libtorch_global_deps.dylib' (no such file)
```

#### 错误 #4: charset_normalizer 缺失
```
RequestsDependencyWarning: Unable to find acceptable character detection
dependency (chardet or charset_normalizer).
```

---

## 🐛 根本原因分析

### 问题 1: PyInstaller 未正确收集依赖

**症状**: `ModuleNotFoundError: No module named 'funasr'`

**原因**: 尽管使用了 `--collect-all funasr`，但 PyInstaller 默认的 hooks 不够完善，导致某些模块（特别是动态导入的模块）未被收集。

**证据**: 查看 PyInstaller 构建日志:
```
WARNING: Failed to collect submodules for 'funasr.models.sense_voice.whisper_lib'
because importing 'funasr.models.sense_voice.whisper_lib' raised:
ModuleNotFoundError: No module named 'tiktoken'
```

### 问题 2: PyTorch 动态库未打包

**症状**: `OSError: dlopen(.../libtorch_global_deps.dylib, ...) (no such file)`

**原因**: 我们创建的 [hook-torch.py](../core-engine/hooks/hook-torch.py) 虽然设计正确，但在收集动态库时可能存在问题。

**修复方案**: 需要改进 hook 来确保所有 `.dylib` 文件都被正确收集。

### 问题 3: FFmpeg PATH 未设置

**症状**: `FileNotFoundError: [Errno 2] No such file or directory: 'ffmpeg'`

**原因**: 虽然创建了 [runtime_hook_ffmpeg.py](../core-engine/hooks/runtime_hook_ffmpeg.py)，但检查日志发现:
- 环境变量 `MONOSUB_FFMPEG_PATH` 被正确设置（由 Tauri Rust 代码设置）
- 但是 runtime hook 可能没有被执行，或者 PATH 修改在 FunASR 内部调用 ffmpeg 之前未生效

**修复方案**: 需要确保 runtime hook 在所有导入之前执行，或者修改 FunASR 的调用方式。

### 问题 4: 循环导入问题

**症状**: 测试运行时出现
```
AttributeError: partially initialized module 'torch' has no attribute 'autograd'
(most likely due to a circular import)
```

**原因**: PyInstaller 的导入机制与普通 Python 解释器不同，可能导致 torch 的初始化顺序问题。

---

## 🔧 待修复问题清单

| # | 问题 | 严重性 | 修复优先级 |
|---|------|--------|----------|
| 1 | PyTorch 动态库未打包 | 🔴 Critical | P0 |
| 2 | FFmpeg PATH 未正确设置 | 🔴 Critical | P0 |
| 3 | FunASR 模块收集不完整 | 🔴 Critical | P0 |
| 4 | torch 循环导入 | 🟡 High | P1 |
| 5 | charset_normalizer 缺失 | 🟢 Low | P2 |

---

## ✅ 已验证的功能

| 功能 | 状态 | 备注 |
|------|------|------|
| App 启动 | ✅ | 正常 |
| UI 显示 | ✅ | 正常 |
| 文件选择 | ✅ | 正常 |
| 视频元数据提取 | ✅ | FFmpeg/FFprobe 工作正常 |
| 缩略图生成 | ✅ | 正常 |
| **转录功能** | ❌ | **Python 可执行文件启动失败** |

---

## 📊 详细日志分析

### 成功的部分 (Tauri/Rust 层)

```
✅ App 启动
✅ 视频元数据提取成功: duration=471.871s, size=385MB, resolution=3548x2160
✅ 缩略图生成成功
✅ Sidecar 进程启动
✅ FFmpeg/FFprobe 路径正确传递
   MONOSUB_FFMPEG_PATH=/Applications/MonoSub.app/Contents/MacOS/ffmpeg
   MONOSUB_FFPROBE_PATH=/Applications/MonoSub.app/Contents/MacOS/ffprobe
```

### 失败的部分 (Python/FunASR 层)

```
❌ Python 可执行文件初始化失败
   - funasr 模块未找到
   - torch 动态库缺失
   - charset_normalizer 缺失

❌ 运行时 FFmpeg 调用失败
   - FunASR 内部使用裸 'ffmpeg' 命令
   - PATH 环境变量未包含 FFmpeg 目录
   - runtime_hook_ffmpeg.py 可能未执行或未生效
```

---

## 🚀 下一步修复计划

### 方案 A: 改进 PyInstaller Hooks (推荐)

1. **加强 hook-torch.py**
   - 使用更精确的方式收集 `.dylib` 文件
   - 添加调试日志来验证收集了哪些文件
   - 确保 `libtorch_global_deps.dylib` 被正确收集

2. **改进 runtime_hook_ffmpeg.py**
   - 添加调试输出到 stderr
   - 验证 PATH 修改是否生效
   - 考虑直接 patch FunASR 的 subprocess 调用

3. **添加专门的 FunASR hook**
   - 创建 `hook-funasr.py` 来确保所有子模块被收集
   - 处理动态导入的模块（如 tiktoken）

### 方案 B: 使用 Python 虚拟环境 (备选)

如果 PyInstaller 打包持续存在问题，考虑：
- 打包完整的 Python 虚拟环境（而非单一可执行文件）
- 使用 Tauri sidecar 调用 venv 中的 Python
- 权衡：包大小会更大（~500MB → ~1GB），但兼容性更好

---

## 🔍 调试建议

### 1. 验证 PyInstaller 收集了什么

```bash
# 运行打包的可执行文件，检查临时目录
cd core-engine/dist
./monosub-engine --help &

# 快速查看临时目录
ls -la /var/folders/*/T/_MEI*/torch/lib/

# 检查是否包含 libtorch_global_deps.dylib
find /var/folders/*/T/_MEI* -name "libtorch_global_deps.dylib"
```

### 2. 测试 runtime hook

在 `runtime_hook_ffmpeg.py` 中添加:
```python
import sys
print(f"DEBUG [runtime_hook]: sys.frozen = {getattr(sys, 'frozen', False)}", flush=True)
print(f"DEBUG [runtime_hook]: MONOSUB_FFMPEG_PATH = {os.environ.get('MONOSUB_FFMPEG_PATH')}", flush=True)
print(f"DEBUG [runtime_hook]: PATH before = {os.environ.get('PATH')}", flush=True)
# ... 修改 PATH ...
print(f"DEBUG [runtime_hook]: PATH after = {os.environ.get('PATH')}", flush=True)
```

### 3. 使用开发模式测试

```bash
# 强制使用开发模式（跳过 PyInstaller 可执行文件）
# 修改 transcribe.rs:88 临时返回 true
fn is_dev_mode() -> bool {
    true  // 强制使用开发模式
}
```

然后重新编译并测试，验证 Python 脚本本身是否正常工作。

---

## 📝 测试总结

**好消息**:
- ✅ 构建流程完整且自动化
- ✅ Tauri 应用本身工作正常
- ✅ FFmpeg 集成正确（Rust 层面）
- ✅ 我们的修复方向是正确的（已识别所有关键问题）

**坏消息**:
- ❌ PyInstaller 打包仍然存在严重问题
- ❌ 当前版本的 app 无法完成转录
- ❌ 需要进一步改进 PyInstaller hooks

**建议**:
1. **短期**: 使用开发模式 (Python venv) 进行功能测试和演示
2. **中期**: 继续改进 PyInstaller hooks，逐步解决打包问题
3. **长期**: 考虑 方案B（打包完整 venv）作为备选方案

---

**测试完成时间**: 2026-01-29 23:20
**测试者**: Claude Code (Automated Test)
