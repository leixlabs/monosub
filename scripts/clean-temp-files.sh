#!/bin/bash
# 清理临时构建文件，但保留源代码

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🧹 清理临时文件..."

# 1. 清理 PyInstaller 临时文件
echo "清理 PyInstaller 临时文件..."
rm -rf "$PROJECT_ROOT/core-engine/build"
rm -rf "$PROJECT_ROOT/core-engine/dist"
rm -f "$PROJECT_ROOT/core-engine"/*.spec
rm -rf "$PROJECT_ROOT/core-engine/__pycache__"
rm -rf "$PROJECT_ROOT/core-engine/.pytest_cache"
rm -rf "$PROJECT_ROOT/core-engine/.ruff_cache"

# 2. 清理 resources 中的 Python 源代码（已不需要）
echo "清理 resources 中的旧 Python 源代码..."
rm -rf "$PROJECT_ROOT/src-tauri/resources/core-engine"

# 3. 清理旧的无架构后缀的二进制文件
echo "清理旧的二进制文件..."
rm -f "$PROJECT_ROOT/src-tauri/resources/bin/ffmpeg"
rm -f "$PROJECT_ROOT/src-tauri/resources/bin/ffprobe"
rm -f "$PROJECT_ROOT/src-tauri/resources/bin/monosub-engine"

echo "✅ 清理完成！"
echo ""
echo "保留的文件："
echo "  ✅ core-engine/ (源代码 - 开发和构建需要)"
echo "  ✅ core-engine/.venv/ (虚拟环境 - 开发需要)"
echo "  ✅ src-tauri/resources/bin/*-aarch64-apple-darwin (带架构后缀的二进制)"
