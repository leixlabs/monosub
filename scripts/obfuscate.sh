#!/bin/bash
# PyArmor 混淆 core-engine Python 代码
# 用法: bash scripts/obfuscate.sh <core-engine目录路径> [python解释器路径]
# 例如: bash scripts/obfuscate.sh src-tauri/resources/core-engine src-tauri/resources/python/bin/python3

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

TARGET_DIR="$1"
PYTHON_BIN="$2"

if [ -z "$TARGET_DIR" ]; then
    log_error "请指定要混淆的 core-engine 目录路径"
    echo "用法: bash scripts/obfuscate.sh <core-engine目录路径> [python解释器路径]"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "目录不存在: $TARGET_DIR"
    exit 1
fi

if [ ! -f "$TARGET_DIR/main.py" ]; then
    log_error "未找到 main.py: $TARGET_DIR/main.py"
    exit 1
fi

# 确定 Python 解释器
if [ -z "$PYTHON_BIN" ]; then
    PYTHON_BIN="python3"
fi

if [ ! -x "$PYTHON_BIN" ] && ! command -v "$PYTHON_BIN" &> /dev/null; then
    log_error "Python 解释器不可用: $PYTHON_BIN"
    exit 1
fi

PYTHON_VERSION=$("$PYTHON_BIN" --version 2>&1)
log_info "使用 Python 解释器: $PYTHON_BIN ($PYTHON_VERSION)"

# 确保 pyarmor 已安装到该 Python 环境
if ! "$PYTHON_BIN" -m pyarmor.cli --version &> /dev/null; then
    log_info "未找到 pyarmor，正在安装到目标 Python 环境..."
    "$PYTHON_BIN" -m pip install pyarmor
fi

log_info "开始混淆 Python 代码: $TARGET_DIR"

# 使用目标 Python 解释器运行 PyArmor，确保生成的 runtime .so 匹配目标 Python 版本
"$PYTHON_BIN" -m pyarmor.cli gen --output "$TARGET_DIR/dist" \
    "$TARGET_DIR/main.py" \
    "$TARGET_DIR/monosub_core/"

# 用混淆后的文件替换原始文件
log_info "替换原始文件为混淆后的文件..."
cp -f "$TARGET_DIR/dist/main.py" "$TARGET_DIR/main.py"
rm -rf "$TARGET_DIR/monosub_core"
cp -R "$TARGET_DIR/dist/monosub_core" "$TARGET_DIR/monosub_core"

# 复制 PyArmor 运行时库
log_info "复制 PyArmor 运行时库..."
for runtime_dir in "$TARGET_DIR"/dist/pyarmor_runtime_*; do
    if [ -d "$runtime_dir" ]; then
        cp -R "$runtime_dir" "$TARGET_DIR/"
    fi
done

# 清理临时目录
rm -rf "$TARGET_DIR/dist"

log_success "Python 代码混淆完成"
