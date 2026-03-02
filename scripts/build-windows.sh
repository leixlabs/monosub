#!/bin/bash
# Windows 构建脚本（在 Mac 下运行交叉编译）

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
BUILD_DIR="$PROJECT_ROOT/src-tauri/target"
OUTPUT_DIR="$PROJECT_ROOT/dist"
VERSION_UTILS="$SCRIPT_DIR/version-utils.sh"
VERSION=""

if [ ! -f "$VERSION_UTILS" ]; then
    log_error "未找到版本工具脚本: $VERSION_UTILS"
    exit 1
fi
source "$VERSION_UTILS"

load_version_metadata() {
    sync_versions_from_tauri "$PROJECT_ROOT"
    VERSION=$(get_tauri_version "$PROJECT_ROOT")
}

auto_bump_version() {
    log_info "自动递增打包版本号..."
    local bump_result
    bump_result=$(bash "$SCRIPT_DIR/bump-version.sh")
    load_version_metadata
    log_success "版本号已更新: $bump_result"
}

load_version_metadata

log_info "开始 Windows 交叉编译..."

# 检查依赖
check_dependencies() {
    log_info "检查 Windows 交叉编译依赖..."

    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo 未安装"
        exit 1
    fi

    # 检查 Windows target
    if ! rustup target list --installed | grep -q "x86_64-pc-windows-msvc"; then
        log_error "Windows 编译目标未安装"
        log_info "请运行: rustup target add x86_64-pc-windows-msvc"
        exit 1
    fi

    # 检查 Bun 或 npm
    if command -v bun &> /dev/null; then
        PKG_MANAGER="bun"
    elif command -v npm &> /dev/null; then
        PKG_MANAGER="npm"
    else
        log_error "Node.js/npm 或 Bun 未安装"
        exit 1
    fi

    log_success "依赖检查完成"
}

# 准备资源文件
prepare_resources() {
    log_info "准备 Windows 资源文件..."
    cd "$PROJECT_ROOT"

    local build_type="${BUILD_TYPE:-lite}"

    if [ "$build_type" = "lite" ]; then
        log_info "使用 Lite 版本打包"
        if [ -f "$SCRIPT_DIR/prepare-lite-windows.sh" ]; then
            bash "$SCRIPT_DIR/prepare-lite-windows.sh"
            log_success "Windows Lite 资源准备完成"
        else
            log_error "未找到 prepare-lite-windows.sh"
            exit 1
        fi
    else
        log_error "Windows 交叉编译目前仅支持 Lite 版本"
        log_info "请使用: BUILD_TYPE=lite ./scripts/build-windows.sh"
        exit 1
    fi
}

# 构建前端
build_frontend() {
    log_info "构建前端..."
    cd "$PROJECT_ROOT"

    if [ "$PKG_MANAGER" = "bun" ]; then
        bun run build
    else
        npm run build
    fi

    log_success "前端构建完成"
}

# 构建 Windows 可执行文件
build_windows() {
    log_info "开始构建 Windows 可执行文件..."
    cd "$PROJECT_ROOT"

    log_info "使用 Tauri 交叉编译到 Windows (x86_64-pc-windows-msvc)..."
    log_warning "这可能需要 5-10 分钟..."

    # 使用 cargo tauri build 进行交叉编译
    cargo tauri build --target x86_64-pc-windows-msvc

    log_success "Windows 构建完成"
}

# 查找并移动构建产物
collect_artifacts() {
    log_info "收集构建产物..."

    mkdir -p "$OUTPUT_DIR"

    # 查找生成的 .exe 和 .msi 安装包
    local exe_path=$(find "$BUILD_DIR/x86_64-pc-windows-msvc/release/bundle" -name "*.exe" -type f | grep -v "unins" | head -1)
    local msi_path=$(find "$BUILD_DIR/x86_64-pc-windows-msvc/release/bundle" -name "*.msi" -type f | head -1)

    if [ -n "$exe_path" ]; then
        cp "$exe_path" "$OUTPUT_DIR/"
        log_success "已复制: $(basename "$exe_path")"
    else
        log_warning "未找到 .exe 安装包"
    fi

    if [ -n "$msi_path" ]; then
        cp "$msi_path" "$OUTPUT_DIR/"
        log_success "已复制: $(basename "$msi_path")"
    else
        log_warning "未找到 .msi 安装包"
    fi

    log_success "构建产物已收集到: $OUTPUT_DIR"
}

# 显示构建结果
show_results() {
    echo ""
    log_success "========================================="
    log_success "Windows 构建完成!"
    log_success "========================================="
    echo ""
    log_info "构建产物位置: $OUTPUT_DIR"
    echo ""

    if command -v du &> /dev/null; then
        log_info "文件列表:"
        ls -lh "$OUTPUT_DIR"/*.exe "$OUTPUT_DIR"/*.msi 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
    fi

    echo ""
    log_info "注意事项:"
    log_info "  1. 生成的安装包需要在 Windows 系统上运行测试"
    log_info "  2. Lite 版本首次运行时会自动下载依赖"
    log_info "  3. 安装包已包含 Python、uv、FFmpeg"
}

# 完整构建流程
full_build() {
    check_dependencies
    auto_bump_version
    log_info "当前打包版本: $VERSION"
    prepare_resources
    build_frontend
    build_windows
    collect_artifacts
    show_results
}

# 帮助信息
show_help() {
    echo "Windows 交叉编译脚本（在 Mac 下运行）"
    echo ""
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  build      完整构建流程 (默认)"
    echo "  prepare    仅准备 Windows 资源"
    echo "  help       显示此帮助信息"
    echo ""
    echo "环境变量:"
    echo "  BUILD_TYPE=lite    构建 Lite 版本（默认，目前仅支持 Lite）"
    echo ""
    echo "示例:"
    echo "  ./scripts/build-windows.sh build"
    echo "  BUILD_TYPE=lite ./scripts/build-windows.sh build"
}

# 主函数
main() {
    cd "$PROJECT_ROOT"

    case "${1:-build}" in
        build)
            full_build
            ;;
        prepare)
            check_dependencies
            prepare_resources
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            log_error "未知命令: $1"
            show_help
            exit 1
            ;;
    esac
}

# 执行主函数
main "$@"
