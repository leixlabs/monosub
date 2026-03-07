#!/bin/bash
# Usage: ./scripts/build/prepare-engine.sh --type=[lite|full]
# 复制并处理（混淆）Python核心逻辑代码

set -e

BLUE='\033[0;34m'
GREEN='\033[0;32m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }

BUILD_TYPE="lite"
for arg in "$@"; do
    case $arg in
        --type=*)
            BUILD_TYPE="${arg#*=}"
            shift
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"
CORE_ENGINE_DIR="$PROJECT_ROOT/core-engine"
TARGET_DIR="$RESOURCES_DIR/core-engine"

rm -rf "$TARGET_DIR"
mkdir -p "$TARGET_DIR"

log_info "复制 core-engine 项目代码..."
cp "$CORE_ENGINE_DIR/main.py" "$TARGET_DIR/"
cp -R "$CORE_ENGINE_DIR/monosub_core" "$TARGET_DIR/"

if [ -f "$CORE_ENGINE_DIR/requirements.txt" ]; then
    cp "$CORE_ENGINE_DIR/requirements.txt" "$TARGET_DIR/"
fi

if [ "$BUILD_TYPE" = "lite" ] && [ -f "$CORE_ENGINE_DIR/pyproject.toml" ]; then
    # Lite 模式需要基于 pyproject 安装包
    cp "$CORE_ENGINE_DIR/pyproject.toml" "$TARGET_DIR/"
fi
log_success "核心代码复制完成"

log_info "执行代码混淆流程..."
OBFUSCATE_SCRIPT="$SCRIPT_DIR/obfuscate.sh"

if [ "$BUILD_TYPE" = "full" ]; then
    PYTHON_BIN="$RESOURCES_DIR/python-venv/bin/python3"
else
    PYTHON_BIN="$RESOURCES_DIR/python/bin/python3"
fi

if [ -x "$OBFUSCATE_SCRIPT" ]; then
    bash "$OBFUSCATE_SCRIPT" "$TARGET_DIR" "$PYTHON_BIN"
    log_success "混淆完成"
else
    log_info "跳过代码混淆，未找到脚本或无权限"
fi
