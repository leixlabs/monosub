# PyInstaller Packaging Fixes

## 🐛 Issues Fixed

This document explains the critical bugs that were preventing the packaged MonoSub application from working and how they were resolved.

---

## Issue #1: PyTorch Dynamic Libraries Not Bundled

### Problem
```
OSError: dlopen(/var/folders/.../T/_MEI.../torch/lib/libtorch_global_deps.dylib, 0x000A):
tried: '...' (no such file)
```

**Root Cause**: PyInstaller was not collecting PyTorch's native dynamic libraries (`.dylib` files on macOS). PyTorch has many native dependencies that need special handling during packaging.

### Solution

Created custom PyInstaller hooks to properly bundle PyTorch libraries:

**1. Created [core-engine/hooks/hook-torch.py](../core-engine/hooks/hook-torch.py)**
```python
# Collects all .dylib files from torch/lib directory
# Preserves directory structure for proper loading
# Includes all torch submodules and metadata
```

**2. Created [core-engine/hooks/hook-torchaudio.py](../core-engine/hooks/hook-torchaudio.py)**
```python
# Collects torchaudio dynamic libraries
# Includes backend and transform modules
```

**3. Updated [prepare-resources.sh](../scripts/prepare-resources.sh)** to use these hooks:
```bash
--additional-hooks-dir=hooks \
--collect-submodules torch \
--collect-submodules torchaudio \
--copy-metadata torch \
--copy-metadata torchaudio \
```

---

## Issue #2: FFmpeg Not Found by FunASR

### Problem
```
FileNotFoundError: [Errno 2] No such file or directory: 'ffmpeg'
```

**Root Cause**: FunASR internally calls `ffmpeg` as a subprocess, but couldn't find it in PATH because:
1. The bundled executable runs in an isolated environment
2. Environment variables `MONOSUB_FFMPEG_PATH` were set but not added to PATH

### Solution

Created a runtime hook that runs when the bundled executable starts:

**Created [core-engine/hooks/runtime_hook_ffmpeg.py](../core-engine/hooks/runtime_hook_ffmpeg.py)**
```python
# Runs before any imports
# Reads MONOSUB_FFMPEG_PATH environment variable (set by Tauri)
# Extracts directory and adds it to PATH
# Now FunASR's subprocess calls can find ffmpeg
```

**Updated [prepare-resources.sh](../scripts/prepare-resources.sh)** to include the runtime hook:
```bash
--runtime-hook=hooks/runtime_hook_ffmpeg.py \
```

---

## Issue #3: Missing Dependencies

### Problem
```
WARNING: Unable to find acceptable character detection dependency
(chardet or charset_normalizer)
```

**Root Cause**: The `requests` library requires charset detection, but neither `chardet` nor `charset-normalizer` were explicitly listed as dependencies.

### Solution

**Updated [core-engine/pyproject.toml](../core-engine/pyproject.toml)**
```toml
dependencies = [
    "funasr>=1.0.0",
    "modelscope>=1.9.0",
    "ffmpeg-python>=0.2.0",
    "torch>=2.0.0",
    "torchaudio>=2.0.0",
    "charset-normalizer>=3.0.0",  # Added
]
```

**Updated PyInstaller command** to exclude unnecessary module:
```bash
--exclude-module=torchcodec \  # Not needed, causes import errors
```

---

## 📦 File Structure

```
core-engine/
├── hooks/                              # PyInstaller hooks (NEW)
│   ├── hook-torch.py                  # PyTorch bundling hook
│   ├── hook-torchaudio.py             # TorchAudio bundling hook
│   └── runtime_hook_ffmpeg.py         # FFmpeg PATH setup (runtime)
├── monosub_core/
│   ├── transcriber.py                 # Uses FunASR + PyTorch
│   └── audio_extractor.py             # Uses FFmpeg subprocess
├── main.py                            # Entry point
└── pyproject.toml                     # Dependencies (updated)
```

---

## 🔧 How PyInstaller Hooks Work

### Build-time Hooks (`hook-*.py`)
- Run **during packaging** (when you run PyInstaller)
- Tell PyInstaller what files to collect
- Example: "Include all .dylib files from torch/lib/"

### Runtime Hooks (`runtime_hook_*.py`)
- Run **when the bundled executable starts**
- Before any of your code imports
- Example: "Add FFmpeg directory to PATH"

---

## 🚀 Building the Fixed Version

### Step 1: Install dependencies
```bash
cd core-engine
uv sync --group dev  # Installs PyInstaller + all dependencies
```

### Step 2: Run the build script
```bash
cd ..
./scripts/prepare-resources.sh
```

This will:
1. ✅ Use custom PyInstaller hooks to bundle PyTorch properly
2. ✅ Include runtime hook to setup FFmpeg PATH
3. ✅ Collect all necessary dependencies
4. ✅ Create `monosub-engine` executable in `core-engine/dist/`
5. ✅ Copy to `src-tauri/resources/bin/monosub-engine-{arch}`

### Step 3: Build the full application
```bash
bun run build:dmg
```

---

## 🧪 Testing the Fix

### Test the Python executable directly
```bash
cd core-engine

# Set FFmpeg paths (simulating Tauri environment)
export MONOSUB_FFMPEG_PATH=/opt/homebrew/bin/ffmpeg
export MONOSUB_FFPROBE_PATH=/opt/homebrew/bin/ffprobe

# Test the bundled executable
./dist/monosub-engine --help
./dist/monosub-engine <video.mp4> <output.srt>
```

### Expected output
- ✅ No PyTorch import errors
- ✅ No FFmpeg not found errors
- ✅ Successful transcription

---

## 📊 Before vs After

| Issue | Before | After |
|-------|--------|-------|
| PyTorch loading | ❌ `OSError: libtorch_global_deps.dylib not found` | ✅ Loads successfully |
| FFmpeg access | ❌ `FileNotFoundError: 'ffmpeg'` | ✅ Found in PATH |
| Dependencies | ⚠️ Missing charset_normalizer | ✅ All deps included |
| Package size | ~200MB (incomplete) | ~500MB (complete) |

---

## 🎯 Key Learnings

### 1. PyTorch Requires Special Handling
PyTorch has **hundreds of dynamic libraries** that must be bundled. Default PyInstaller hooks are insufficient.

### 2. Environment Variables ≠ PATH
Setting `MONOSUB_FFMPEG_PATH` doesn't help if subprocess calls use bare `ffmpeg` command. Must add to PATH.

### 3. Runtime Hooks Are Critical
For environment setup that needs to happen **before imports**, runtime hooks are the only solution.

### 4. Test in Isolation
Always test the bundled executable **outside** of the development environment to catch these issues early.

---

## 🔍 Debugging Tips

### Check what PyInstaller bundled
```bash
cd core-engine/dist
./monosub-engine  # Will extract to /var/folders/.../T/_MEI.../

# While it's running, inspect the temp directory:
ls -la /var/folders/*/T/_MEI*/torch/lib/
```

### Enable PyInstaller debug mode
```bash
pyinstaller --debug=all ...  # Verbose output
```

### Test FFmpeg PATH
```python
import os
import subprocess

print("PATH:", os.environ.get('PATH'))
print("MONOSUB_FFMPEG_PATH:", os.environ.get('MONOSUB_FFMPEG_PATH'))

# Try calling ffmpeg
subprocess.run(['ffmpeg', '-version'])
```

---

## ✅ Checklist for Future Packaging

- [ ] Run `uv sync --group dev` to update dependencies
- [ ] Test in a clean environment (no system Python, no Homebrew FFmpeg)
- [ ] Check PyInstaller warnings during build
- [ ] Verify bundled executable size (should be ~500MB)
- [ ] Test with a video file end-to-end
- [ ] Check Tauri app logs for any startup errors

---

## 📚 References

- [PyInstaller Hooks](https://pyinstaller.org/en/stable/hooks.html)
- [PyTorch Packaging Issues](https://github.com/pyinstaller/pyinstaller/issues?q=pytorch)
- [Tauri Sidecar Configuration](https://v2.tauri.app/reference/config/#externalbinconfig)

---

**Last Updated**: 2026-01-29
**Tested On**: macOS 14.x (Apple Silicon)
**PyInstaller Version**: 6.0+
