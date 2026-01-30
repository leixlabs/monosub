#!/bin/bash
# 准备 Windows Lite 版本打包资源（在 Mac 下运行）
# 下载 Windows 版本的 Python、uv、FFmpeg

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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"
CORE_ENGINE_DIR="$PROJECT_ROOT/core-engine"

log_info "准备 Windows Lite 版本资源文件"
log_info "资源目录: $RESOURCES_DIR"

# 创建资源目录
mkdir -p "$RESOURCES_DIR"
mkdir -p "$RESOURCES_DIR/bin"

# ============================================
# 1. 下载 Windows FFmpeg
# ============================================
log_info "下载 Windows FFmpeg..."

download_ffmpeg_windows() {
    local ffmpeg_dir="$RESOURCES_DIR/bin"
    local target_triple="x86_64-pc-windows-msvc"

    local ffmpeg_target="$ffmpeg_dir/ffmpeg-${target_triple}.exe"
    local ffprobe_target="$ffmpeg_dir/ffprobe-${target_triple}.exe"

    # 检查是否已存在
    if [ -f "$ffmpeg_target" ] && [ -f "$ffprobe_target" ]; then
        log_success "Windows FFmpeg 已存在，跳过下载"
        return 0
    fi

    log_info "下载 FFmpeg for Windows (x64)..."
    log_warning "这可能需要几分钟，FFmpeg 包体积约 80MB..."

    # 使用 gyan.dev 的 FFmpeg essentials build
    local ffmpeg_url="https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    local tmp_dir=$(mktemp -d)

    if ! curl -L -o "$tmp_dir/ffmpeg.zip" "$ffmpeg_url"; then
        log_error "FFmpeg 下载失败"
        rm -rf "$tmp_dir"
        return 1
    fi

    log_info "解压 FFmpeg..."
    unzip -q "$tmp_dir/ffmpeg.zip" -d "$tmp_dir"

    # 查找并复制 ffmpeg.exe 和 ffprobe.exe
    local ffmpeg_bin=$(find "$tmp_dir" -name "ffmpeg.exe" | head -1)
    local ffprobe_bin=$(find "$tmp_dir" -name "ffprobe.exe" | head -1)

    if [ -n "$ffmpeg_bin" ] && [ -n "$ffprobe_bin" ]; then
        cp "$ffmpeg_bin" "$ffmpeg_target"
        cp "$ffprobe_bin" "$ffprobe_target"
        log_success "Windows FFmpeg 下载完成"
    else
        log_error "未找到 ffmpeg.exe 或 ffprobe.exe"
        rm -rf "$tmp_dir"
        return 1
    fi

    rm -rf "$tmp_dir"
}

download_ffmpeg_windows

# ============================================
# 2. 下载 Windows Python
# ============================================
log_info "下载 Windows Python..."

download_python_windows() {
    local python_dir="$RESOURCES_DIR/python-windows"

    # 检查是否已下载
    if [ -d "$python_dir" ] && [ -f "$python_dir/python.exe" ]; then
        log_success "Windows Python 已存在，跳过下载"
        return 0
    fi

    # 使用 python-build-standalone
    local python_version="3.11.9"
    local python_build="20240726"
    local python_url="https://github.com/indygreg/python-build-standalone/releases/download/${python_build}/cpython-${python_version}+${python_build}-x86_64-pc-windows-msvc-shared-install_only.tar.gz"

    log_info "下载 Python ${python_version} for Windows (x64)..."
    log_warning "这可能需要几分钟，Python 包体积约 60MB..."

    local tmp_dir=$(mktemp -d)

    if ! curl -L -o "$tmp_dir/python.tar.gz" "$python_url"; then
        log_error "Python 下载失败"
        rm -rf "$tmp_dir"
        return 1
    fi

    log_info "解压 Python..."
    mkdir -p "$python_dir"
    tar -xzf "$tmp_dir/python.tar.gz" -C "$python_dir" --strip-components=1

    rm -rf "$tmp_dir"

    # 验证 Python
    if [ -f "$python_dir/python.exe" ]; then
        local python_size=$(du -sh "$python_dir" | cut -f1)
        log_success "Windows Python 下载完成 (大小: $python_size)"
    else
        log_error "Python 下载失败"
        return 1
    fi
}

download_python_windows

# ============================================
# 3. 下载 Windows uv
# ============================================
log_info "下载 Windows uv..."

download_uv_windows() {
    local uv_bin="$RESOURCES_DIR/bin/uv-windows.exe"

    # 检查是否已下载
    if [ -f "$uv_bin" ]; then
        log_success "Windows uv 已存在，跳过下载"
        return 0
    fi

    log_info "下载 uv for Windows..."
    local uv_version="0.5.13"
    local uv_url="https://github.com/astral-sh/uv/releases/download/${uv_version}/uv-x86_64-pc-windows-msvc.zip"

    local tmp_dir=$(mktemp -d)

    if ! curl -L -o "$tmp_dir/uv.zip" "$uv_url"; then
        log_error "uv 下载失败"
        rm -rf "$tmp_dir"
        return 1
    fi

    unzip -q "$tmp_dir/uv.zip" -d "$tmp_dir"
    cp "$tmp_dir/uv.exe" "$uv_bin" 2>/dev/null || cp "$tmp_dir/"*"/uv.exe" "$uv_bin"
    rm -rf "$tmp_dir"

    log_success "Windows uv 下载完成"
}

download_uv_windows

# ============================================
# 4. 复制 Python 核心脚本（与 macOS 共用）
# ============================================
log_info "复制 Python 核心脚本..."

copy_python_scripts() {
    local scripts_target="$RESOURCES_DIR/core-engine"

    # 如果已经存在（macOS 版本已准备），跳过
    if [ -d "$scripts_target" ] && [ -f "$scripts_target/main.py" ]; then
        log_success "Python 脚本已存在，跳过复制"
        return 0
    fi

    mkdir -p "$scripts_target"

    log_info "复制 main.py 和 monosub_core/..."
    cp "$CORE_ENGINE_DIR/main.py" "$scripts_target/"
    cp -R "$CORE_ENGINE_DIR/monosub_core" "$scripts_target/"

    if [ -f "$CORE_ENGINE_DIR/requirements.txt" ]; then
        cp "$CORE_ENGINE_DIR/requirements.txt" "$scripts_target/"
    fi

    if [ -f "$CORE_ENGINE_DIR/pyproject.toml" ]; then
        cp "$CORE_ENGINE_DIR/pyproject.toml" "$scripts_target/"
    fi

    log_success "Python 脚本复制完成"
}

copy_python_scripts

# ============================================
# 5. 创建 Windows 启动脚本
# ============================================
log_info "创建 Windows 启动脚本..."

create_launcher_windows() {
    local launcher="$RESOURCES_DIR/bin/monosub-engine-x86_64-pc-windows-msvc.bat"

    cat > "$launcher" << 'EOF'
@echo off
REM MonoSub Python Engine Launcher (Windows Lite version)
REM Auto-installs dependencies on first run using uv

setlocal

REM Get the directory of this script
set SCRIPT_DIR=%~dp0
set SCRIPT_DIR=%SCRIPT_DIR:~0,-1%

REM Find Resources directory
for %%I in ("%SCRIPT_DIR%") do set PARENT_DIR=%%~dpI
set RESOURCES_DIR=%PARENT_DIR%Resources

REM Paths
set PYTHON_BIN=%RESOURCES_DIR%\python-windows\python.exe
set UV_BIN=%RESOURCES_DIR%\bin\uv-windows.exe
set MAIN_PY=%RESOURCES_DIR%\core-engine\main.py
set REQUIREMENTS=%RESOURCES_DIR%\core-engine\requirements.txt

REM Cache directory for dependencies
set CACHE_DIR=%LOCALAPPDATA%\MonoSub\python-deps
set DEPS_INSTALLED_MARKER=%CACHE_DIR%\.installed

REM Check if Python exists
if not exist "%PYTHON_BIN%" (
    echo ERROR: Python not found: %PYTHON_BIN% 1>&2
    exit /b 1
)

if not exist "%MAIN_PY%" (
    echo ERROR: main.py not found: %MAIN_PY% 1>&2
    exit /b 1
)

REM Check if dependencies are installed
if not exist "%DEPS_INSTALLED_MARKER%" (
    echo MSG:首次运行，正在安装 AI 依赖... 1>&2
    echo MSG:这可能需要 3-5 分钟，请耐心等待... 1>&2
    echo PROGRESS:0 1>&2

    REM Create cache directory
    if not exist "%CACHE_DIR%" mkdir "%CACHE_DIR%"

    REM Install dependencies using uv
    if exist "%UV_BIN%" (
        echo MSG:使用 uv 安装依赖到缓存目录... 1>&2
        "%UV_BIN%" pip install -r "%REQUIREMENTS%" --target "%CACHE_DIR%" --python "%PYTHON_BIN%" 2>&1
    ) else (
        echo MSG:使用 pip 安装依赖... 1>&2
        "%PYTHON_BIN%" -m pip install -r "%REQUIREMENTS%" --target "%CACHE_DIR%" 2>&1
    )

    REM Mark as installed
    type nul > "%DEPS_INSTALLED_MARKER%"
    echo MSG:依赖安装完成！ 1>&2
    echo PROGRESS:5 1>&2
)

REM Set PYTHONPATH to include cached dependencies
set PYTHONPATH=%CACHE_DIR%;%PYTHONPATH%

REM Run the Python script with all arguments
"%PYTHON_BIN%" "%MAIN_PY%" %*
EOF

    log_success "Windows 启动脚本创建完成: monosub-engine-x86_64-pc-windows-msvc.bat"
}

create_launcher_windows

# ============================================
# 6. 更新资源清单
# ============================================
log_info "更新资源清单..."

cat >> "$RESOURCES_DIR/MANIFEST.txt" << EOF

## Windows 资源（交叉编译）
bin/ffmpeg-x86_64-pc-windows-msvc.exe
bin/ffprobe-x86_64-pc-windows-msvc.exe
bin/uv-windows.exe
bin/monosub-engine-x86_64-pc-windows-msvc.bat
python-windows/                # Windows Python 3.11
EOF

log_success "资源清单更新完成"

# ============================================
# 7. 显示摘要
# ============================================
echo ""
log_success "========================================="
log_success "Windows Lite 版本资源准备完成!"
log_success "========================================="
echo ""
log_info "资源目录: $RESOURCES_DIR"
echo ""
log_info "包含的内容:"
log_info "  - Windows FFmpeg/FFprobe"
log_info "  - Windows Python 解释器"
log_info "  - Windows uv 工具"
log_info "  - Windows 启动脚本 (.bat)"
echo ""

# 显示大小统计
log_info "大小统计:"
if command -v du &> /dev/null; then
    du -sh "$RESOURCES_DIR/python-windows" 2>/dev/null | awk '{print "  - Windows Python: " $1}'
    du -sh "$RESOURCES_DIR" | awk '{print "  - 总计: " $1}'
fi

echo ""
log_info "下一步: 运行 'bun run build:windows:lite' 进行 Windows 交叉编译"
