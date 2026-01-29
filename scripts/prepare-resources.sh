#!/bin/bash
# 准备打包所需的资源文件
# 包括 FFmpeg 二进制文件和 Python 环境

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

log_info "准备资源文件到: $RESOURCES_DIR"

# 创建资源目录
mkdir -p "$RESOURCES_DIR"
mkdir -p "$RESOURCES_DIR/bin"
mkdir -p "$RESOURCES_DIR/core-engine"

# ============================================
# 1. 下载/复制 FFmpeg 二进制文件
# ============================================
log_info "准备 FFmpeg 二进制文件..."

download_ffmpeg() {
    local arch=$(uname -m)
    local ffmpeg_dir="$RESOURCES_DIR/bin"

    # Tauri 需要带架构后缀的二进制文件
    # 参考: https://v2.tauri.app/reference/config/#externalbinconfig
    local target_triple=""
    if [ "$arch" = "arm64" ]; then
        target_triple="aarch64-apple-darwin"
    else
        target_triple="x86_64-apple-darwin"
    fi

    local ffmpeg_target="$ffmpeg_dir/ffmpeg-$target_triple"
    local ffprobe_target="$ffmpeg_dir/ffprobe-$target_triple"

    # 检查是否已存在
    if [ -f "$ffmpeg_target" ] && [ -f "$ffprobe_target" ]; then
        log_success "FFmpeg 已存在 ($target_triple)，跳过下载"
        return 0
    fi

    log_info "准备 FFmpeg for macOS ($arch / $target_triple)..."

    # 使用 Homebrew 的 FFmpeg
    if command -v brew &> /dev/null; then
        local brew_ffmpeg=$(brew --prefix ffmpeg 2>/dev/null || echo "")

        if [ -n "$brew_ffmpeg" ]; then
            log_info "从 Homebrew 复制 FFmpeg..."
            cp "$brew_ffmpeg/bin/ffmpeg" "$ffmpeg_target"
            cp "$brew_ffmpeg/bin/ffprobe" "$ffprobe_target"
            chmod +x "$ffmpeg_target" "$ffprobe_target"
            log_success "FFmpeg 复制完成: $target_triple"
            return 0
        fi
    fi

    # 从系统路径复制
    if command -v ffmpeg &> /dev/null && command -v ffprobe &> /dev/null; then
        log_info "从系统路径复制 FFmpeg..."
        cp "$(which ffmpeg)" "$ffmpeg_target"
        cp "$(which ffprobe)" "$ffprobe_target"
        chmod +x "$ffmpeg_target" "$ffprobe_target"
        log_success "FFmpeg 复制完成: $target_triple"
        return 0
    fi

    log_error "未找到 FFmpeg，请先安装: brew install ffmpeg"
    return 1
}

download_ffmpeg

# ============================================
# 2. 使用 PyInstaller 构建 Python 可执行文件
# ============================================
log_info "构建 Python 独立可执行文件..."

build_python_executable() {
    local core_engine_dir="$PROJECT_ROOT/core-engine"
    local bin_dir="$RESOURCES_DIR/bin"

    # 获取架构后缀
    local arch=$(uname -m)
    local target_triple=""
    if [ "$arch" = "arm64" ]; then
        target_triple="aarch64-apple-darwin"
    else
        target_triple="x86_64-apple-darwin"
    fi

    local engine_target="$bin_dir/monosub-engine-$target_triple"

    # 强制每次都重新打包，删除旧的可执行文件
    if [ -f "$engine_target" ]; then
        log_info "删除旧的 Python 可执行文件: $engine_target"
        rm -f "$engine_target"
    fi

    log_info "检查 Python 虚拟环境..."
    if [ ! -d "$core_engine_dir/.venv" ]; then
        log_error "Python 虚拟环境未找到"
        log_info "请先运行: cd $core_engine_dir && uv sync"
        return 1
    fi

    # 激活虚拟环境
    source "$core_engine_dir/.venv/bin/activate"

    # 检查并安装 PyInstaller
    if ! command -v pyinstaller &> /dev/null; then
        log_info "安装 PyInstaller..."
        pip install pyinstaller
    fi

    log_info "使用 PyInstaller 打包 Python 应用..."
    log_warning "这可能需要 5-10 分钟，请耐心等待..."

    cd "$core_engine_dir"

    # 清理旧的构建产物
    rm -rf build dist *.spec

    # 使用 PyInstaller 打包
    pyinstaller \
        --onefile \
        --name monosub-engine \
        --hidden-import=funasr \
        --hidden-import=funasr.models \
        --hidden-import=funasr.utils \
        --hidden-import=modelscope \
        --hidden-import=modelscope.hub \
        --hidden-import=modelscope.models \
        --hidden-import=torch \
        --hidden-import=torchaudio \
        --hidden-import=ffmpeg \
        --collect-all funasr \
        --collect-all modelscope \
        --clean \
        main.py

    # 检查构建结果
    if [ -f "dist/monosub-engine" ]; then
        log_info "测试可执行文件..."
        ./dist/monosub-engine --help || log_warning "测试运行失败，但二进制已创建"

        # 复制到资源目录，添加架构后缀
        cp "dist/monosub-engine" "$engine_target"
        chmod +x "$engine_target"

        local size=$(du -h "$engine_target" | cut -f1)
        log_success "Python 可执行文件构建完成: $target_triple (大小: $size)"

        # 清理构建临时文件
        log_info "清理临时文件..."
        rm -rf build *.spec
    else
        log_error "构建失败，未找到 dist/monosub-engine"
        cd "$PROJECT_ROOT"
        return 1
    fi

    # 停用虚拟环境
    deactivate

    cd "$PROJECT_ROOT"
}

build_python_executable

# ============================================
# 3. 创建资源清单文件
# ============================================
log_info "创建资源清单..."

cat > "$RESOURCES_DIR/MANIFEST.txt" << EOF
# MonoSub 资源清单
# 生成时间: $(date)

## 二进制文件
bin/ffmpeg
bin/ffprobe

## Python 核心引擎
core-engine/main.py
core-engine/monosub_core/
core-engine/requirements.txt

## 说明
这些资源文件会被打包到应用程序中
运行时会被复制到应用数据目录
EOF

log_success "资源清单创建完成"

# ============================================
# 4. 显示摘要
# ============================================
echo ""
log_success "========================================="
log_success "资源准备完成!"
log_success "========================================="
echo ""
log_info "资源目录: $RESOURCES_DIR"
echo ""
log_info "包含的文件:"
tree -L 2 "$RESOURCES_DIR" 2>/dev/null || find "$RESOURCES_DIR" -type f | head -20
echo ""
log_info "下一步: 运行 'bun run build:dmg' 进行完整打包"
