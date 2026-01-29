# core-engine 目录说明

## ❓ 为什么不能删除 core-engine 目录

### 1. **开发模式需要**

在开发时（`bun run dev`），应用会直接运行 Python 源代码：

```rust
// src-tauri/src/commands/transcribe.rs
async fn run_dev_mode(...) {
    // 直接运行 core-engine/main.py
    Command::new(&venv_python)
        .arg(&main_py)  // core-engine/main.py
        .spawn()
}
```

### 2. **PyInstaller 构建源**

每次构建时，PyInstaller 需要从 `core-engine/` 读取源代码：

```bash
# scripts/prepare-resources.sh
cd "$core_engine_dir"  # core-engine/
pyinstaller main.py    # 从这里读取源代码和依赖
```

### 3. **依赖管理**

Python 虚拟环境位于 `core-engine/.venv/`：

```
core-engine/
├── .venv/              ← Python 虚拟环境
│   ├── bin/python
│   └── lib/...
├── main.py             ← 入口文件
└── monosub_core/       ← 核心逻辑
```

## ✅ 可以清理的临时文件

### 自动清理

```bash
# 清理所有临时文件
bun run clean:temp
```

这会删除：
- ✅ `core-engine/build/` - PyInstaller 临时文件
- ✅ `core-engine/dist/` - PyInstaller 输出（已复制到 resources）
- ✅ `core-engine/*.spec` - PyInstaller 配置文件
- ✅ `core-engine/__pycache__/` - Python 缓存
- ✅ `src-tauri/resources/core-engine/` - 旧的复制的源代码（已废弃）

### 手动清理

如果需要完全清理：

```bash
# 清理所有构建产物（包括 DMG）
bun run clean

# 仅清理临时文件
bun run clean:temp
```

## 📂 目录结构说明

### 必须保留

```
core-engine/
├── .venv/              ← 虚拟环境（开发需要）
├── main.py             ← Python 入口（开发和构建需要）
├── monosub_core/       ← 核心逻辑（开发和构建需要）
├── pyproject.toml      ← 依赖定义
└── uv.lock             ← 锁文件
```

### 临时文件（可删除）

```
core-engine/
├── build/              ← PyInstaller 临时文件
├── dist/               ← PyInstaller 输出
├── *.spec              ← PyInstaller 配置
└── __pycache__/        ← Python 缓存
```

### 打包后的文件

```
src-tauri/resources/bin/
├── ffmpeg-aarch64-apple-darwin
├── ffprobe-aarch64-apple-darwin
└── monosub-engine-aarch64-apple-darwin  ← 包含所有 Python 代码
```

打包后的 `monosub-engine` 是独立可执行文件，包含：
- Python 解释器
- core-engine 的所有代码
- 所有 Python 依赖（FunASR, PyTorch 等）

## 🔄 工作流程

### 开发模式

```
bun run dev
  ↓
检测到 core-engine/.venv/
  ↓
直接运行 core-engine/main.py
```

### 生产模式（打包后）

```
用户运行 MonoSub.app
  ↓
使用 monosub-engine sidecar
  ↓
无需 core-engine 源代码
```

## 📊 磁盘空间占用

| 目录 | 大小 | 说明 |
|------|------|------|
| `core-engine/.venv/` | ~500MB | Python 虚拟环境 |
| `core-engine/monosub_core/` | ~50KB | 源代码 |
| `monosub-engine` | ~8MB | 打包的可执行文件 |

**注意**：虽然 `.venv` 很大，但它是开发必需的，不能删除。

## 🎯 总结

| 问题 | 答案 |
|------|------|
| 可以删除 core-engine 吗？ | ❌ 不能 |
| 开发需要 core-engine 吗？ | ✅ 需要 |
| 构建需要 core-engine 吗？ | ✅ 需要 |
| 用户需要 core-engine 吗？ | ❌ 不需要（已打包） |
| 可以清理临时文件吗？ | ✅ 可以（`bun run clean:temp`） |

---

**结论**：`core-engine/` 目录是项目的核心部分，必须保留。但可以定期清理临时构建文件。
