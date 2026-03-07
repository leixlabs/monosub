#!/bin/bash
# Usage: ./scripts/build/prepare-python-env.sh --type=[lite|full]
# 准备 Python 环境（Lite 版动态下载所需依赖，Full版直接复制虚拟环境）

set -e

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

BUILD_TYPE="lite"
for arg in "$@"; do
    case $arg in
        --type=*)
            BUILD_TYPE="${arg#*=}"
            shift
            ;;
    esac
done

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"
CORE_ENGINE_DIR="$PROJECT_ROOT/core-engine"
ARCH=$(uname -m)

mkdir -p "$RESOURCES_DIR/bin"

download_python_lite() {
    local python_dir="$RESOURCES_DIR/python"
    if [ -d "$python_dir" ] && [ -f "$python_dir/bin/python3" ]; then
        log_success "独立 Python 解释器已存在"
        return 0
    fi
    local python_version="3.11.9"
    local python_build="20240726"
    if [ "$ARCH" = "arm64" ]; then
        local url="https://github.com/indygreg/python-build-standalone/releases/download/${python_build}/cpython-${python_version}+${python_build}-aarch64-apple-darwin-install_only.tar.gz"
    else
        local url="https://github.com/indygreg/python-build-standalone/releases/download/${python_build}/cpython-${python_version}+${python_build}-x86_64-apple-darwin-install_only.tar.gz"
    fi
    log_info "下载独立 Python 解释器 ($ARCH)..."
    local tmp_dir=$(mktemp -d)
    if ! curl -L -o "$tmp_dir/python.tar.gz" "$url"; then
        log_error "Python 下载失败"
        rm -rf "$tmp_dir"
        exit 1
    fi
    mkdir -p "$python_dir"
    tar -xzf "$tmp_dir/python.tar.gz" -C "$python_dir" --strip-components=1
    rm -rf "$tmp_dir"
    log_success "独立 Python 解释器下载完成"
}

download_uv_lite() {
    local uv_bin="$RESOURCES_DIR/bin/uv"
    ensure_uv() { chmod u+w "$uv_bin" 2>/dev/null || true; chmod 755 "$uv_bin" 2>/dev/null || chmod +x "$uv_bin"; }
    if [ -f "$uv_bin" ]; then
        ensure_uv
        log_success "uv 工具已存在"
        return 0
    fi
    if command -v uv &> /dev/null; then
        cp "$(which uv)" "$uv_bin"
        ensure_uv
        log_success "从系统复制 uv 完成"
        return 0
    fi
    log_info "下载 uv ($ARCH)..."
    local uv_version="0.5.13"
    if [ "$ARCH" = "arm64" ]; then
        local url="https://github.com/astral-sh/uv/releases/download/${uv_version}/uv-aarch64-apple-darwin.tar.gz"
    else
        local url="https://github.com/astral-sh/uv/releases/download/${uv_version}/uv-x86_64-apple-darwin.tar.gz"
    fi
    local tmp_dir=$(mktemp -d)
    if ! curl -L -o "$tmp_dir/uv.tar.gz" "$url"; then
        log_error "uv 下载失败"
        rm -rf "$tmp_dir"
        exit 1
    fi
    tar -xzf "$tmp_dir/uv.tar.gz" -C "$tmp_dir"
    cp "$tmp_dir/uv-"*"/uv" "$uv_bin" 2>/dev/null || cp "$tmp_dir/uv" "$uv_bin"
    ensure_uv
    rm -rf "$tmp_dir"
    log_success "uv 下载完成"
}

fix_python_dylib() {
    local venv_target="$1"
    local python_bin="$venv_target/bin/python"
    local python_version="3.11"
    local libpython_name="libpython${python_version}.dylib"
    local lib_dir="$venv_target/lib"

    if [ -f "$lib_dir/$libpython_name" ]; then
        log_success "libpython 已存在于 venv/lib"
    else
        local current_python=$(which python3.11 || which python3)
        local py_prefix=$("$current_python" -c "import sys; print(sys.prefix)" 2>/dev/null || true)
        local paths=(
            "$py_prefix/lib/$libpython_name"
            "$HOME/.local/share/uv/python/cpython-3.11.*/lib/$libpython_name"
            "$RESOURCES_DIR/python/lib/$libpython_name"
            "/opt/homebrew/opt/python@${python_version}/Frameworks/Python.framework/Versions/${python_version}/lib/$libpython_name"
            "/Library/Frameworks/Python.framework/Versions/${python_version}/lib/$libpython_name"
        )
        local found=""
        for p in "${paths[@]}"; do
            for exp in $p; do
                if [ -f "$exp" ]; then
                    found="$exp"; break 2
                fi
            done
        done
        if [ -n "$found" ]; then
            mkdir -p "$lib_dir" && cp "$found" "$lib_dir/"
            log_success "复制 libpython 完成: $found"
        else
            log_error "未找到 libpython3.11.dylib"
            exit 1
        fi
    fi
    if [ -f "$python_bin" ]; then
        install_name_tool -add_rpath "@executable_path/../lib" "$python_bin" 2>/dev/null || true
        log_success "Python 动态库修复完成"
    fi
}

prepare_venv_full() {
    local venv_src="$CORE_ENGINE_DIR/.venv"
    local venv_target="$RESOURCES_DIR/python-venv"
    if [ ! -d "$venv_src" ]; then
        log_error "虚拟环境不存在: $venv_src, 请先运行 uv sync"
        exit 1
    fi
    cd "$CORE_ENGINE_DIR" && uv sync && cd "$PROJECT_ROOT"
    rm -rf "$venv_target"
    log_info "开始完整复制虚拟环境 (~400-600MB)..."
    cp -R "$venv_src" "$venv_target"
    if [ ! -f "$venv_target/bin/python" ]; then
        log_error "Python 可执行文件复制失败"
        exit 1
    fi
    fix_python_dylib "$venv_target"
    log_success "Python 全量环境准备完成"
}

if [ "$BUILD_TYPE" = "lite" ]; then
    log_info "准备 Lite 版本 Python 动态构建依赖"
    download_python_lite
    download_uv_lite
elif [ "$BUILD_TYPE" = "full" ]; then
    log_info "准备 Full 版本 Python 内嵌执行环境"
    prepare_venv_full
else
    log_error "未知的 BUILD_TYPE: $BUILD_TYPE"
    exit 1
fi
