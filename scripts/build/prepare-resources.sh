#!/bin/bash
# Usage: ./scripts/build/prepare-resources.sh --type=[lite|full]
# Orchestrate all prerequisite scripts before the main Tauri compilation

set -e

BLUE='\033[0;34m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }

BUILD_TYPE="lite"
for arg in "$@"; do
    case $arg in
        --type=*) BUILD_TYPE="${arg#*=}"; shift ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"

log_info "1. 解析并准备环境依赖... (模式: $BUILD_TYPE)"

# Lite builds don't use python-venv; remove it to prevent Tauri from trying
# to bundle it (it contains absolute symlinks that Tauri's bundler can't follow)
if [ "$BUILD_TYPE" = "lite" ] && [ -d "$RESOURCES_DIR/python-venv" ]; then
    log_info "Lite 模式: 清除 python-venv 目录 (避免 Tauri 打包绝对符号链接)"
    rm -rf "$RESOURCES_DIR/python-venv"
fi

bash "$SCRIPT_DIR/prepare-python-env.sh" --type="$BUILD_TYPE"

log_info "2. 检测 FFmpeg 二进制环境..."
bash "$SCRIPT_DIR/prepare-ffmpeg.sh"

log_info "3. 处理应用核心逻辑源码..."
bash "$SCRIPT_DIR/prepare-engine.sh" --type="$BUILD_TYPE"

log_info "4. 生成执行启动器..."
bash "$SCRIPT_DIR/generate-launcher.sh" --type="$BUILD_TYPE"

log_info "✅ 资源整理准备完毕！进入打包环..."
exit 0
