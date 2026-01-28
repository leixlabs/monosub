# 包管理器配置

本项目使用现代化的包管理工具，提供更快的安装速度和更好的开发体验。

## 前端：Bun

### 为什么选择 Bun？
- ⚡ **极快速度** - 比 npm/yarn 快 10-100 倍
- 🔋 **内置功能** - 包管理、打包、测试一体化
- 🎯 **Node.js 兼容** - 完全兼容 npm 生态系统
- 💾 **节省空间** - 全局缓存，节省磁盘空间

### 安装 Bun

```bash
# macOS/Linux
curl -fsSL https://bun.sh/install | bash

# 或使用 Homebrew
brew install oven-sh/bun/bun

# 验证安装
bun --version
```

### 常用命令

```bash
# 安装依赖
bun install

# 运行开发服务器
bun run dev

# 构建应用
bun run build

# 类型检查
bun run check

# 代码检查
bun run lint
```

## Python：uv

### 为什么选择 uv？
- 🚀 **超快速度** - 用 Rust 编写，比 pip 快 10-100 倍
- 📦 **现代化** - 使用 `pyproject.toml` 配置
- 🔒 **可靠锁定** - 自动生成 `uv.lock` 确保依赖一致
- 🎯 **简单易用** - 自动管理虚拟环境
- 🌐 **兼容性好** - 完全兼容 pip 和 PyPI

### 安装 uv

```bash
# macOS/Linux
curl -LsSf https://astral.sh/uv/install.sh | sh

# 或使用 Homebrew
brew install uv

# 验证安装
uv --version
```

### 常用命令

```bash
# 同步依赖（自动创建虚拟环境）
cd core-engine
uv sync

# 运行 Python 脚本（在虚拟环境中）
uv run python main.py

# 添加新依赖
uv add package-name

# 移除依赖
uv remove package-name

# 重新安装所有依赖
uv sync --reinstall

# 激活虚拟环境（可选）
source .venv/bin/activate
```

## 项目配置文件

### 前端 (package.json)

```json
{
  "name": "voice-to-text",
  "scripts": {
    "dev": "bunx tauri dev",
    "build": "bun run tsc -b && vite build && bunx tauri build"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "zustand": "^5.0.3"
  }
}
```

### Python (pyproject.toml)

```toml
[project]
name = "voice-to-subtitle-engine"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = [
    "funasr>=1.0.0",
    "modelscope>=1.9.0",
    "torch>=2.0.0",
]

[tool.uv]
dev-dependencies = [
    "pytest>=7.0.0",
    "black>=23.0.0",
]
```

## 完整安装流程

### 1. 安装工具

```bash
# 安装 Bun
curl -fsSL https://bun.sh/install | bash

# 安装 uv
curl -LsSf https://astral.sh/uv/install.sh | sh

# 安装 FFmpeg
brew install ffmpeg
```

### 2. 安装项目依赖

```bash
# 前端依赖
bun install

# Python 依赖
./scripts/setup-python.sh
# 或手动
cd core-engine && uv sync
```

### 3. 启动开发

```bash
# 启动 Tauri 开发服务器
bun run dev
```

## 虚拟环境位置

### Bun
Bun 将依赖安装到：
- 全局缓存：`~/.bun/install/cache`
- 项目依赖：`node_modules/`

### uv
uv 创建的虚拟环境位于：
- `core-engine/.venv/` (推荐)
- 或使用传统的 `core-engine/venv/`

## 对比传统工具

| 功能 | npm | Bun | pip | uv |
|------|-----|-----|-----|-----|
| 安装速度 | ⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| 磁盘占用 | 高 | 低 | 中 | 低 |
| 依赖锁定 | package-lock.json | bun.lockb | - | uv.lock |
| 自动虚拟环境 | ❌ | ❌ | ❌ | ✅ |
| 语言 | JavaScript | Zig | Python | Rust |

## 迁移指南

### 从 npm 迁移到 Bun

```bash
# 删除旧文件
rm -rf node_modules package-lock.json

# 使用 Bun 安装
bun install
```

### 从 pip + venv 迁移到 uv

```bash
# 删除旧虚拟环境
rm -rf core-engine/venv

# 创建 pyproject.toml（已完成）
# 使用 uv 安装
cd core-engine
uv sync
```

## 故障排除

### Bun 相关

**Q: Bun 命令未找到**
```bash
# 检查 PATH
echo $PATH | grep .bun

# 手动添加到 shell 配置
echo 'export PATH="$HOME/.bun/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

**Q: Bun 安装失败**
```bash
# 清理缓存
rm -rf ~/.bun/install/cache
bun install
```

### uv 相关

**Q: uv 命令未找到**
```bash
# 检查 PATH
echo $PATH | grep .cargo

# 手动添加到 shell 配置
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

**Q: uv sync 失败**
```bash
# 清理并重新安装
rm -rf .venv uv.lock
uv sync --reinstall
```

## 参考资源

- [Bun 官方文档](https://bun.sh/docs)
- [uv 官方文档](https://docs.astral.sh/uv/)
- [pyproject.toml 规范](https://packaging.python.org/en/latest/specifications/pyproject-toml/)
