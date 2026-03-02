#!/bin/bash

# MonoSub 打包脚本
# 用于统一构建和打包过程

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
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

# 配置
APP_NAME="MonoSub"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_TYPE="full"

# 解析参数
for arg in "$@"; do
    case $arg in
        --type=*)
            BUILD_TYPE="${arg#*=}"
            shift
            ;;
        --all)
            # 构建所有平台
            BUILD_ALL=true
            shift
            ;;
    esac
done

# 检查脚本是否存在
check_scripts() {
    if [ ! -f "$PROJECT_ROOT/scripts/build-dmg.sh" ]; then
        log_error "未找到 build-dmg.sh 脚本"
        exit 1
    fi
    
    if [ ! -f "$PROJECT_ROOT/scripts/build-windows.sh" ]; then
        log_warning "未找到 build-windows.sh 脚本，将跳过 Windows 构建"
    fi
}

# 构建 macOS DMG
build_macos() {
    log_info "构建 macOS DMG ($BUILD_TYPE)..."
    
    # 导出 BUILD_TYPE 环境变量
    export BUILD_TYPE
    
    # 调用 build-dmg.sh 脚本
    bash "$PROJECT_ROOT/scripts/build-dmg.sh" build
    
    log_success "macOS DMG 构建完成"
}

# 构建 Windows 安装程序
build_windows() {
    if [ -f "$PROJECT_ROOT/scripts/build-windows.sh" ]; then
        log_info "构建 Windows 安装程序 ($BUILD_TYPE)..."
        
        # 导出 BUILD_TYPE 环境变量
        export BUILD_TYPE
        
        # 调用 build-windows.sh 脚本
        bash "$PROJECT_ROOT/scripts/build-windows.sh" build
        
        log_success "Windows 安装程序构建完成"
    else
        log_warning "跳过 Windows 构建，因为 build-windows.sh 脚本不存在"
    fi
}

# 主函数
main() {
    log_info "开始打包流程..."
    log_info "应用名称: $APP_NAME"
    log_info "构建类型: $BUILD_TYPE"
    
    check_scripts
    
    # 构建 macOS DMG
    build_macos
    
    # 如果指定了 --all，则构建 Windows 安装程序
    if [ "$BUILD_ALL" = true ]; then
        build_windows
    fi
    
    log_success "========================================="
    log_success "打包完成!"
    log_success "安装程序文件位于 dist/ 目录"
    log_success "========================================="
}

# 执行主函数
main "$@"
