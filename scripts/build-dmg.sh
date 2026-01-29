#!/bin/bash

# MonoSub DMG 打包脚本
# 用于构建 macOS DMG 安装包

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
BUNDLE_ID="com.monosub.app"
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/target/release/bundle/macos"
OUTPUT_DIR="$PROJECT_ROOT/dist"
DMG_NAME="${APP_NAME}-${VERSION}-universal.dmg"

# 检查依赖
check_dependencies() {
    log_info "检查依赖..."

    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo 未安装，请先安装 Rust"
        exit 1
    fi

    # 检查 Node.js / Bun
    if command -v bun &> /dev/null; then
        PKG_MANAGER="bun"
    elif command -v npm &> /dev/null; then
        PKG_MANAGER="npm"
    else
        log_error "Node.js/npm 或 Bun 未安装"
        exit 1
    fi

    # 检查 create-dmg (可选)
    if ! command -v create-dmg &> /dev/null; then
        log_warning "create-dmg 未安装，将使用基础方式创建 DMG"
        log_info "可通过 'brew install create-dmg' 安装以获得更好的 DMG 效果"
        USE_CREATE_DMG=false
    else
        USE_CREATE_DMG=true
    fi

    log_success "依赖检查完成"
}

# 安装前端依赖
install_frontend_deps() {
    log_info "安装前端依赖..."
    cd "$PROJECT_ROOT"

    if [ "$PKG_MANAGER" = "bun" ]; then
        bun install
    else
        npm install
    fi

    log_success "前端依赖安装完成"
}

# 构建前端
build_frontend() {
    log_info "构建前端..."
    cd "$PROJECT_ROOT"

    if [ "$PKG_MANAGER" = "bun" ]; then
        bun run build:frontend
    else
        npm run build:frontend
    fi

    log_success "前端构建完成"
}

# 构建 Tauri 应用
build_tauri() {
    log_info "构建 Tauri 应用 (Release 模式)..."
    cd "$PROJECT_ROOT"

    # 检测 CPU 架构
    ARCH=$(uname -m)
    log_info "检测到架构: $ARCH"

    # 构建目标架构
    if [ "$ARCH" = "arm64" ]; then
        # Apple Silicon
        log_info "为 Apple Silicon (arm64) 构建..."
        cargo tauri build --target aarch64-apple-darwin
    else
        # Intel
        log_info "为 Intel (x86_64) 构建..."
        cargo tauri build --target x86_64-apple-darwin
    fi

    log_success "Tauri 应用构建完成"
}

# 创建 DMG
create_dmg() {
    log_info "创建 DMG 安装包..."

    # 创建输出目录
    mkdir -p "$OUTPUT_DIR"

    # 查找 .app 文件
    APP_PATH=$(find "$BUILD_DIR" -name "*.app" -type d | head -1)

    if [ -z "$APP_PATH" ]; then
        log_error "未找到 .app 文件，请先运行构建"
        exit 1
    fi

    log_info "找到应用: $APP_PATH"

    DMG_PATH="$OUTPUT_DIR/$DMG_NAME"

    # 删除旧的 DMG
    rm -f "$DMG_PATH"

    if [ "$USE_CREATE_DMG" = true ]; then
        # 使用 create-dmg 创建美观的 DMG
        log_info "使用 create-dmg 创建 DMG..."

        create-dmg \
            --volname "$APP_NAME" \
            --volicon "$PROJECT_ROOT/src-tauri/icons/icon.icns" \
            --window-pos 200 120 \
            --window-size 660 400 \
            --icon-size 100 \
            --icon "$APP_NAME.app" 180 170 \
            --hide-extension "$APP_NAME.app" \
            --app-drop-link 480 170 \
            --background "$PROJECT_ROOT/scripts/dmg-background.png" \
            "$DMG_PATH" \
            "$APP_PATH" || {
                # 如果带背景失败，尝试不带背景
                log_warning "带背景创建失败，尝试不带背景..."
                create-dmg \
                    --volname "$APP_NAME" \
                    --volicon "$PROJECT_ROOT/src-tauri/icons/icon.icns" \
                    --window-pos 200 120 \
                    --window-size 660 400 \
                    --icon-size 100 \
                    --icon "$APP_NAME.app" 180 170 \
                    --hide-extension "$APP_NAME.app" \
                    --app-drop-link 480 170 \
                    "$DMG_PATH" \
                    "$APP_PATH"
            }
    else
        # 使用基础方式创建 DMG
        log_info "使用基础方式创建 DMG..."

        # 创建临时目录
        TMP_DIR=$(mktemp -d)
        cp -R "$APP_PATH" "$TMP_DIR/"

        # 创建 Applications 链接
        ln -s /Applications "$TMP_DIR/Applications"

        # 创建 DMG
        hdiutil create -volname "$APP_NAME" \
            -srcfolder "$TMP_DIR" \
            -ov -format UDZO \
            "$DMG_PATH"

        # 清理临时目录
        rm -rf "$TMP_DIR"
    fi

    log_success "DMG 创建完成: $DMG_PATH"

    # 显示 DMG 信息
    DMG_SIZE=$(du -h "$DMG_PATH" | cut -f1)
    log_info "DMG 大小: $DMG_SIZE"
}

# 签名 (可选)
sign_app() {
    if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
        log_info "签名应用..."

        APP_PATH=$(find "$BUILD_DIR" -name "*.app" -type d | head -1)
        codesign --force --deep --sign "$APPLE_SIGNING_IDENTITY" "$APP_PATH"

        log_success "签名完成"
    else
        log_warning "未设置 APPLE_SIGNING_IDENTITY，跳过签名"
        log_info "如需签名，请设置环境变量: export APPLE_SIGNING_IDENTITY='Developer ID Application: Your Name'"
    fi
}

# 公证 (可选)
notarize_app() {
    if [ -n "$APPLE_ID" ] && [ -n "$APPLE_PASSWORD" ] && [ -n "$APPLE_TEAM_ID" ]; then
        log_info "提交公证..."

        DMG_PATH="$OUTPUT_DIR/$DMG_NAME"

        xcrun notarytool submit "$DMG_PATH" \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_PASSWORD" \
            --team-id "$APPLE_TEAM_ID" \
            --wait

        # 装订公证票据
        xcrun stapler staple "$DMG_PATH"

        log_success "公证完成"
    else
        log_warning "未设置 Apple 公证凭证，跳过公证"
        log_info "如需公证，请设置环境变量:"
        log_info "  export APPLE_ID='your@email.com'"
        log_info "  export APPLE_PASSWORD='app-specific-password'"
        log_info "  export APPLE_TEAM_ID='XXXXXXXXXX'"
    fi
}

# 清理
clean() {
    log_info "清理构建产物..."
    cd "$PROJECT_ROOT"

    rm -rf target/release/bundle
    rm -rf dist/*.dmg

    log_success "清理完成"
}

# 完整构建流程
full_build() {
    log_info "开始完整构建流程..."
    log_info "应用名称: $APP_NAME"
    log_info "版本: $VERSION"

    check_dependencies
    install_frontend_deps
    build_frontend
    build_tauri
    sign_app
    create_dmg
    notarize_app

    log_success "========================================="
    log_success "构建完成!"
    log_success "DMG 文件: $OUTPUT_DIR/$DMG_NAME"
    log_success "========================================="
}

# 帮助信息
show_help() {
    echo "MonoSub DMG 打包脚本"
    echo ""
    echo "用法: $0 [命令]"
    echo ""
    echo "命令:"
    echo "  build     完整构建流程 (默认)"
    echo "  frontend  仅构建前端"
    echo "  tauri     仅构建 Tauri 应用"
    echo "  dmg       仅创建 DMG (需要先构建)"
    echo "  sign      签名应用"
    echo "  notarize  公证应用"
    echo "  clean     清理构建产物"
    echo "  help      显示此帮助信息"
    echo ""
    echo "环境变量:"
    echo "  APPLE_SIGNING_IDENTITY  签名身份"
    echo "  APPLE_ID               Apple ID (公证用)"
    echo "  APPLE_PASSWORD         App 专用密码 (公证用)"
    echo "  APPLE_TEAM_ID          团队 ID (公证用)"
}

# 主函数
main() {
    cd "$PROJECT_ROOT"

    case "${1:-build}" in
        build)
            full_build
            ;;
        frontend)
            build_frontend
            ;;
        tauri)
            build_tauri
            ;;
        dmg)
            create_dmg
            ;;
        sign)
            sign_app
            ;;
        notarize)
            notarize_app
            ;;
        clean)
            clean
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
