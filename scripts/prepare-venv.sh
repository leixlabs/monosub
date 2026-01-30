#!/bin/bash
# 准备完整 Python venv 打包（Full 版本）
# 这个脚本会打包完整的 Python 虚拟环境，而不是使用 PyInstaller

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

log_info "准备 Full 版本资源文件（完整 Python venv）"
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
# 2. 准备 Python venv
# ============================================
log_info "准备完整 Python 虚拟环境..."

prepare_venv() {
    local venv_source="$CORE_ENGINE_DIR/.venv"
    local venv_target="$RESOURCES_DIR/python-venv"

    # 检查源 venv 是否存在
    if [ ! -d "$venv_source" ]; then
        log_error "Python 虚拟环境未找到: $venv_source"
        log_info "请先运行: cd $CORE_ENGINE_DIR && uv sync"
        return 1
    fi

    # 确保所有依赖都已安装
    log_info "确保所有依赖都已安装..."
    cd "$CORE_ENGINE_DIR"
    uv sync
    cd "$PROJECT_ROOT"

    # 删除旧的 venv 目标
    if [ -d "$venv_target" ]; then
        log_info "删除旧的 venv: $venv_target"
        rm -rf "$venv_target"
    fi

    # 复制完整的 venv
    log_info "复制 Python 虚拟环境..."
    log_warning "这可能需要 1-2 分钟，venv 大小约 400-600MB..."

    cp -R "$venv_source" "$venv_target"

    # 验证关键文件存在
    local python_bin="$venv_target/bin/python"
    if [ ! -f "$python_bin" ]; then
        log_error "Python 可执行文件未找到: $python_bin"
        return 1
    fi

    # 测试 Python 环境
    log_info "测试 Python 环境..."
    if "$python_bin" -c "import funasr; import torch; print('OK')" &> /dev/null; then
        log_success "Python 环境测试通过"
    else
        log_warning "Python 环境测试失败，但继续打包"
    fi

    local venv_size=$(du -sh "$venv_target" | cut -f1)
    log_success "Python venv 复制完成 (大小: $venv_size)"
}

prepare_venv

# ============================================
# 3. 复制 Python 脚本
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

    # 复制 requirements.txt（用于参考）
    if [ -f "$CORE_ENGINE_DIR/requirements.txt" ]; then
        cp "$CORE_ENGINE_DIR/requirements.txt" "$scripts_target/"
    fi

    log_success "Python 脚本复制完成"
}

copy_python_scripts

# ============================================
# 4. 创建启动脚本
# ============================================
log_info "创建 Python 启动脚本..."

create_launcher() {
    local arch=$(uname -m)
    local target_triple=""
    if [ "$arch" = "arm64" ]; then
        target_triple="aarch64-apple-darwin"
    else
        target_triple="x86_64-apple-darwin"
    fi

    # 使用与 PyInstaller 版本相同的名称，这样不需要修改 tauri.conf.json
    local launcher="$RESOURCES_DIR/bin/monosub-engine-$target_triple"

    cat > "$launcher" << 'EOF'
#!/bin/bash
# MonoSub Python Engine Launcher (Full venv version)
# This script runs the Python engine using the bundled venv

# Get the directory of this script (Contents/MacOS/)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# In macOS app bundle:
# - This script is in: Contents/MacOS/
# - Resources are in: Contents/Resources/resources/ (array syntax preserves "resources/" prefix)
APP_CONTENTS="$(dirname "$SCRIPT_DIR")"
RESOURCES_DIR="$APP_CONTENTS/Resources/resources"

# Python venv paths
VENV_PYTHON="$RESOURCES_DIR/python-venv/bin/python"
MAIN_PY="$RESOURCES_DIR/core-engine/main.py"

# Check if files exist
if [ ! -f "$VENV_PYTHON" ]; then
    echo "ERROR: Python not found: $VENV_PYTHON" >&2
    echo "DEBUG: SCRIPT_DIR=$SCRIPT_DIR" >&2
    echo "DEBUG: RESOURCES_DIR=$RESOURCES_DIR" >&2
    exit 1
fi

if [ ! -f "$MAIN_PY" ]; then
    echo "ERROR: main.py not found: $MAIN_PY" >&2
    exit 1
fi

# Run Python with all arguments
exec "$VENV_PYTHON" "$MAIN_PY" "$@"
EOF

    chmod +x "$launcher"
    log_success "启动脚本创建完成: monosub-engine-$target_triple (Full venv 版本)"
}

create_launcher

# ============================================
# 5. 创建资源清单文件
# ============================================
log_info "创建资源清单..."

cat > "$RESOURCES_DIR/MANIFEST.txt" << EOF
# MonoSub 资源清单 (Full 版本 - 完整 Python venv)
# 生成时间: $(date)

## 二进制文件
bin/ffmpeg
bin/ffprobe
bin/monosub-engine (启动脚本，调用 venv Python)

## Python 虚拟环境
python-venv/           # 完整的 Python 虚拟环境 (~400-600MB)
├── bin/python         # Python 解释器
├── lib/               # Python 库和依赖
└── ...

## Python 核心引擎脚本
core-engine/
├── main.py            # 入口脚本
└── monosub_core/      # 核心模块

## 版本信息
- 打包方式: 完整 Python venv
- 优点: 100% 兼容性，稳定可靠
- 缺点: 包体积较大 (~600-800MB)
EOF

log_success "资源清单创建完成"

# ============================================
# 6. 显示摘要
# ============================================
echo ""
log_success "========================================="
log_success "Full 版本资源准备完成!"
log_success "========================================="
echo ""
log_info "资源目录: $RESOURCES_DIR"
echo ""
log_info "包含的内容:"
log_info "  - FFmpeg/FFprobe 二进制文件"
log_info "  - 完整 Python 虚拟环境 (python-venv/)"
log_info "  - Python 核心脚本 (core-engine/)"
log_info "  - 启动脚本 (monosub-engine-venv)"
echo ""

# 显示大小统计
log_info "大小统计:"
if command -v du &> /dev/null; then
    du -sh "$RESOURCES_DIR/python-venv" 2>/dev/null | awk '{print "  - Python venv: " $1}'
    du -sh "$RESOURCES_DIR/core-engine" 2>/dev/null | awk '{print "  - Core engine: " $1}'
    du -sh "$RESOURCES_DIR/bin" 2>/dev/null | awk '{print "  - Binaries: " $1}'
    du -sh "$RESOURCES_DIR" | awk '{print "  - 总计: " $1}'
fi

echo ""
log_info "下一步: 运行 'bun run build:dmg:full' 进行完整打包"
