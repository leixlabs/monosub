#!/bin/bash
# 准备 Lite 版本打包资源（Python + uv + 源码）
# 首次运行时自动安装依赖到用户缓存目录

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

log_info "准备 Lite 版本资源文件（Python + uv + 源码）"
log_info "资源目录: $RESOURCES_DIR"

# 创建资源目录
mkdir -p "$RESOURCES_DIR"
mkdir -p "$RESOURCES_DIR/bin"

# ============================================
# 1. 下载/复制 FFmpeg 二进制文件
# ============================================
log_info "准备 FFmpeg 二进制文件..."

download_ffmpeg() {
    local arch=$(uname -m)
    local ffmpeg_dir="$RESOURCES_DIR/bin"

    # Tauri 需要带架构后缀的二进制文件
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
# 2. 下载独立 Python 解释器
# ============================================
log_info "准备独立 Python 解释器..."

download_python() {
    local arch=$(uname -m)
    local python_dir="$RESOURCES_DIR/python"

    # 检查是否已下载
    if [ -d "$python_dir" ] && [ -f "$python_dir/bin/python3" ]; then
        log_success "Python 已存在，跳过下载"
        return 0
    fi

    # 使用 python-build-standalone
    # https://github.com/indygreg/python-build-standalone/releases
    local python_version="3.11.9"
    local python_build="20240726"

    if [ "$arch" = "arm64" ]; then
        local python_url="https://github.com/indygreg/python-build-standalone/releases/download/${python_build}/cpython-${python_version}+${python_build}-aarch64-apple-darwin-install_only.tar.gz"
    else
        local python_url="https://github.com/indygreg/python-build-standalone/releases/download/${python_build}/cpython-${python_version}+${python_build}-x86_64-apple-darwin-install_only.tar.gz"
    fi

    log_info "下载 Python ${python_version} for macOS ($arch)..."
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
    if [ -f "$python_dir/bin/python3" ]; then
        local python_size=$(du -sh "$python_dir" | cut -f1)
        log_success "Python 下载完成 (大小: $python_size)"
    else
        log_error "Python 下载失败"
        return 1
    fi
}

download_python

# ============================================
# 3. 下载 uv 工具
# ============================================
log_info "准备 uv 工具..."

download_uv() {
    local arch=$(uname -m)
    local uv_bin="$RESOURCES_DIR/bin/uv"

    ensure_uv_permissions() {
        # Tauri build copies resources into target and overwrites on next build.
        # If uv is readonly (e.g. 555), subsequent copy fails with EACCES.
        chmod u+w "$uv_bin" 2>/dev/null || true
        chmod 755 "$uv_bin" 2>/dev/null || chmod +x "$uv_bin"
    }

    # 检查是否已下载
    if [ -f "$uv_bin" ]; then
        ensure_uv_permissions
        log_success "uv 已存在，跳过下载"
        return 0
    fi

    # 优先从系统复制
    if command -v uv &> /dev/null; then
        log_info "从系统路径复制 uv..."
        cp "$(which uv)" "$uv_bin"
        ensure_uv_permissions
        log_success "uv 复制完成"
        return 0
    fi

    # 下载最新版本
    log_info "下载 uv..."
    local uv_version="0.5.13"  # 使用较新的固定版本

    if [ "$arch" = "arm64" ]; then
        local uv_url="https://github.com/astral-sh/uv/releases/download/${uv_version}/uv-aarch64-apple-darwin.tar.gz"
    else
        local uv_url="https://github.com/astral-sh/uv/releases/download/${uv_version}/uv-x86_64-apple-darwin.tar.gz"
    fi

    local tmp_dir=$(mktemp -d)

    if ! curl -L -o "$tmp_dir/uv.tar.gz" "$uv_url"; then
        log_error "uv 下载失败"
        rm -rf "$tmp_dir"
        return 1
    fi

    tar -xzf "$tmp_dir/uv.tar.gz" -C "$tmp_dir"
    cp "$tmp_dir/uv-"*"/uv" "$uv_bin" 2>/dev/null || cp "$tmp_dir/uv" "$uv_bin"
    ensure_uv_permissions
    rm -rf "$tmp_dir"

    log_success "uv 下载完成"
}

download_uv

# ============================================
# 4. 复制 Python 核心脚本
# ============================================
log_info "复制 Python 核心脚本..."

copy_python_scripts() {
    local scripts_target="$RESOURCES_DIR/core-engine"

    # 删除旧的脚本
    if [ -d "$scripts_target" ]; then
        rm -rf "$scripts_target"
    fi

    mkdir -p "$scripts_target"

    # 复制必要的 Python 文件
    log_info "复制 main.py 和 monosub_core/..."
    cp "$CORE_ENGINE_DIR/main.py" "$scripts_target/"
    cp -R "$CORE_ENGINE_DIR/monosub_core" "$scripts_target/"

    # 复制 pyproject.toml（uv 需要它来安装依赖）
    if [ -f "$CORE_ENGINE_DIR/pyproject.toml" ]; then
        cp "$CORE_ENGINE_DIR/pyproject.toml" "$scripts_target/"
    fi

    log_success "Python 脚本复制完成"
}

copy_python_scripts

# ============================================
# 5. 创建智能启动脚本
# ============================================
log_info "创建智能启动脚本..."

create_launcher() {
    local arch=$(uname -m)
    local target_triple=""
    if [ "$arch" = "arm64" ]; then
        target_triple="aarch64-apple-darwin"
    else
        target_triple="x86_64-apple-darwin"
    fi

    local launcher="$RESOURCES_DIR/bin/monosub-engine-$target_triple"
    local template_file="$SCRIPT_DIR/launcher-template.sh"

    # Check if template file exists
    if [ ! -f "$template_file" ]; then
        log_error "启动脚本模板不存在: $template_file"
        exit 1
    fi

    # Copy template to launcher
    cp "$template_file" "$launcher"
    chmod +x "$launcher"

    log_success "启动脚本创建完成: monosub-engine-$target_triple (Lite 版本)"
}

create_launcher

# ============================================
# 6. 创建资源清单
# ============================================
log_info "创建资源清单..."

cat > "$RESOURCES_DIR/MANIFEST.txt" << EOF
# MonoSub 资源清单 (Lite 版本 - Python + uv 动态依赖)
# 生成时间: $(date)

## 二进制文件
bin/ffmpeg-*               # FFmpeg 音视频处理
bin/ffprobe-*              # FFmpeg 媒体信息探测
bin/uv                     # Python 包管理器
bin/monosub-engine-*       # 启动脚本（智能依赖安装）

## Python 独立版本
python/                    # 独立 Python 3.11 (~60MB)
├── bin/python3            # Python 解释器
├── lib/                   # 标准库
└── ...

## Python 核心引擎脚本
core-engine/
├── main.py                # 入口脚本
├── monosub_core/          # 核心模块
└── pyproject.toml         # 依赖配置 (uv 读取)

## 版本信息
- 打包方式: Lite（动态依赖安装）
- 优点: 包体积小 (~120MB)，依赖一次性安装
- 缺点: 首次运行需联网安装依赖（~500MB）
- 依赖缓存: ~/Library/Caches/MonoSub/python-deps/
EOF

log_success "资源清单创建完成"

# ============================================
# 7. 显示摘要
# ============================================
echo ""
log_success "========================================="
log_success "Lite 版本资源准备完成!"
log_success "========================================="
echo ""
log_info "资源目录: $RESOURCES_DIR"
echo ""
log_info "包含的内容:"
log_info "  - FFmpeg/FFprobe 二进制文件"
log_info "  - 独立 Python 解释器 (python/)"
log_info "  - uv 包管理器"
log_info "  - Python 核心脚本 (core-engine/)"
log_info "  - 智能启动脚本（首次运行自动安装依赖）"
echo ""

# 显示大小统计
log_info "大小统计:"
if command -v du &> /dev/null; then
    du -sh "$RESOURCES_DIR/python" 2>/dev/null | awk '{print "  - Python: " $1}'
    du -sh "$RESOURCES_DIR/core-engine" 2>/dev/null | awk '{print "  - Core engine: " $1}'
    du -sh "$RESOURCES_DIR/bin" 2>/dev/null | awk '{print "  - Binaries: " $1}'
    du -sh "$RESOURCES_DIR" | awk '{print "  - 总计: " $1}'
fi

echo ""
log_info "下一步: 运行 'bun run build:dmg:lite' 进行打包"
