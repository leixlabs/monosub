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
BUNDLE_ID="com.monosub.client"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target"
OUTPUT_DIR="$PROJECT_ROOT/dist"

# 保存原始环境变量，用于区分是特定清理还是全局清理
RAW_BUILD_TYPE="$BUILD_TYPE"
BUILD_TYPE="${BUILD_TYPE:-lite}"
VERSION=""
DMG_NAME=""

load_version_metadata() {
    VERSION=$(grep '"version"' "$PROJECT_ROOT/src-tauri/tauri.conf.json" | head -1 | sed 's/.*: "\(.*\)".*/\1/')
    DMG_NAME="${APP_NAME}-${VERSION}-${BUILD_TYPE}-universal.dmg"
}

auto_bump_version() {
    log_info "自动递增打包版本号..."
    local bump_result
    bump_result=$(bash "$SCRIPT_DIR/bump-version.sh")
    load_version_metadata
    log_success "版本号已更新: $bump_result"
}

load_version_metadata

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

    # 对于 Full 版本，强制使用基础方式（ditto）来正确处理大文件
    # create-dmg 工具在处理包含大量符号链接的大型 app bundle 时可能有问题
    local build_type="${BUILD_TYPE:-lite}"
    if [ "$build_type" = "full" ]; then
        log_info "Full 版本：使用基础方式创建 DMG（ditto + hdiutil）"
        USE_CREATE_DMG=false
    elif ! command -v create-dmg &> /dev/null; then
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
        bun run build
    else
        npm run build
    fi

    log_success "前端构建完成"
}

# 构建 Tauri 应用
build_tauri() {
    log_info "构建 Tauri 应用 (Release 模式)..."
    cd "$PROJECT_ROOT"

    # Workaround: tauri-build copies resources to target and overwrites them on subsequent builds.
    # If target resources/bin/uv is readonly (mode 555), overwrite fails with "Permission denied (os error 13)".
    # Clean stale copied uv binaries before invoking cargo tauri build.
    find "$BUILD_DIR" -path "*/release/resources/bin/uv" -type f 2>/dev/null | while read -r stale_uv; do
        chmod u+w "$stale_uv" 2>/dev/null || true
        rm -f "$stale_uv"
        log_info "已清理旧的只读 uv: $stale_uv"
    done

    # 检测 CPU 架构
    ARCH=$(uname -m)
    log_info "检测到架构: $ARCH"

    # 构建目标架构
    if [ "$ARCH" = "arm64" ]; then
        # Apple Silicon
        log_info "为 Apple Silicon (arm64) 构建..."
        cargo tauri build --target aarch64-apple-darwin
        TARGET_TRIPLE="aarch64-apple-darwin"
    else
        # Intel
        log_info "为 Intel (x86_64) 构建..."
        cargo tauri build --target x86_64-apple-darwin
        TARGET_TRIPLE="x86_64-apple-darwin"
    fi

    log_success "Tauri 应用构建完成"

    # 资源文件已通过 Tauri 的 resources 配置自动打包
    # prepare-venv.sh (full) 或 prepare-lite.sh 将资源准备到 src-tauri/resources/
    # Tauri 会自动将其复制到 app bundle，无需手动干预
}

# 创建 DMG
create_dmg() {
    log_info "创建 DMG 安装包..."

    # 创建输出目录
    mkdir -p "$OUTPUT_DIR"

    # 删除 Tauri 自动生成的 dmg（它不包含我们手动复制的资源）
    log_info "删除 Tauri 自动生成的 dmg..."
    rm -rf "$BUILD_DIR"/*/release/bundle/dmg/*.dmg

    # 查找 .app 文件（从 macos 目录，不是 dmg 目录）
    APP_PATH=$(find "$BUILD_DIR"/*/release/bundle/macos -name "*.app" -type d | head -1)

    if [ -z "$APP_PATH" ]; then
        log_error "未找到 .app 文件，请先运行构建"
        exit 1
    fi

    log_info "找到应用: $APP_PATH"

    # 验证 app 大小
    local build_type="${BUILD_TYPE:-lite}"
    local app_size=$(du -sm "$APP_PATH" | cut -f1)

    if [ "$build_type" = "full" ]; then
        # Full 版本应该 > 800M
        if [ "$app_size" -lt 800 ]; then
            log_error "App 大小异常 (${app_size}M < 800M)，资源可能未正确复制"
            exit 1
        fi
        log_info "App 大小验证通过 (Full): ${app_size}M"
    elif [ "$build_type" = "lite" ]; then
        # Lite 版本应该 > 80M（Python ~60M + FFmpeg ~50M + 其他）
        if [ "$app_size" -lt 80 ]; then
            log_error "App 大小异常 (${app_size}M < 80M)，资源可能未正确复制"
            exit 1
        fi
        log_info "App 大小验证通过 (Lite): ${app_size}M"
    fi

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
        log_info "临时目录: $TMP_DIR"

        # 使用 ditto 替代 cp，正确处理符号链接和扩展属性
        log_info "复制 app bundle 到临时目录 (使用 ditto)..."
        APP_NAME_BASE="$(basename "$APP_PATH")"
        ditto "$APP_PATH" "$TMP_DIR/$APP_NAME_BASE"

        # 验证临时目录中的 app
        local tmp_app_size=$(du -sm "$TMP_DIR/$APP_NAME_BASE" | cut -f1)
        log_info "临时目录中的 app 大小: ${tmp_app_size}M"

        # Tauri 的 resources 配置保留了目录结构：resources/ -> Contents/Resources/resources/
        if [ -d "$TMP_DIR/$APP_NAME_BASE/Contents/Resources/resources/python-venv" ]; then
            local tmp_venv_size=$(du -sh "$TMP_DIR/$APP_NAME_BASE/Contents/Resources/resources/python-venv" | cut -f1)
            log_info "venv 已复制到临时目录: $tmp_venv_size"
        else
            log_error "venv 未复制到临时目录！"
            log_info "临时目录内容:"
            ls -lh "$TMP_DIR/$APP_NAME_BASE/Contents/Resources/"
            log_info "Resources 子目录内容:"
            ls -lh "$TMP_DIR/$APP_NAME_BASE/Contents/Resources/resources/" 2>/dev/null || echo "resources/ 目录不存在"
            exit 1
        fi

        # 创建 Applications 链接
        ln -s /Applications "$TMP_DIR/Applications"

        # 创建 DMG
        log_info "开始创建 DMG，源目录: $TMP_DIR"
        log_info "hdiutil create 参数: -volname $APP_NAME -srcfolder $TMP_DIR -ov -format UDZO $DMG_PATH"

        # 使用 -verbose 参数查看详细输出
        hdiutil create -volname "$APP_NAME" \
            -srcfolder "$TMP_DIR" \
            -ov -format UDZO \
            -verbose \
            "$DMG_PATH" 2>&1 | tail -20

        # 验证创建的 DMG
        log_info "验证创建的 DMG..."
        hdiutil attach "$DMG_PATH" -quiet
        sleep 2
        local dmg_app_size=$(du -sm "/Volumes/$APP_NAME/$(basename "$APP_PATH")" 2>/dev/null | cut -f1 || echo "0")
        log_info "DMG 中的 app 大小: ${dmg_app_size}M"
        hdiutil detach "/Volumes/$APP_NAME" -quiet 2>/dev/null || true

        # 根据构建类型验证大小
        if [ "$build_type" = "full" ]; then
            if [ "$dmg_app_size" -lt 800 ]; then
                log_error "DMG 创建失败！app 大小异常: ${dmg_app_size}M < 800M"
                log_info "保留临时目录用于调试: $TMP_DIR"
                exit 1
            fi
        elif [ "$build_type" = "lite" ]; then
            if [ "$dmg_app_size" -lt 80 ]; then
                log_error "DMG 创建失败！app 大小异常: ${dmg_app_size}M < 80M"
                log_info "保留临时目录用于调试: $TMP_DIR"
                exit 1
            fi
        fi

        # 清理临时目录
        rm -rf "$TMP_DIR"
    fi

    log_success "DMG 创建完成: $DMG_PATH"

    # 清理 create-dmg 产生的临时文件
    if [ "$USE_CREATE_DMG" = true ]; then
        log_info "清理临时 DMG 文件..."
        rm -f "$OUTPUT_DIR"/rw.*.dmg
        log_success "临时文件清理完成"
    fi

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

# 检测当前资源类型
detect_resource_type() {
    local has_lite=false
    local has_full=false
    
    if [ -d "$PROJECT_ROOT/src-tauri/resources/python" ] || [ -f "$PROJECT_ROOT/src-tauri/resources/bin/uv" ]; then
        has_lite=true
    fi
    
    if [ -d "$PROJECT_ROOT/src-tauri/resources/python-venv" ]; then
        has_full=true
    fi
    
    if [ "$has_lite" = true ] && [ "$has_full" = true ]; then
        echo "mixed"
    elif [ "$has_lite" = true ]; then
        echo "lite"
    elif [ "$has_full" = true ]; then
        echo "full"
    else
        echo "none"
    fi
}

# 清理
clean() {
    # 优先使用传入的参数，其次使用原始环境变量设置，最后为空（表示清理所有）
    local target_type="${1:-$RAW_BUILD_TYPE}"
    cd "$PROJECT_ROOT"

    if [ -z "$target_type" ]; then
        # 未指定 BUILD_TYPE，清理所有
        log_info "清理所有构建产物..."
        rm -rf target/release/bundle
        rm -rf dist/*.dmg
        # 清理资源目录
        rm -rf src-tauri/resources/python
        rm -rf src-tauri/resources/python-venv
        rm -rf src-tauri/resources/core-engine
    else
        # 指定了 BUILD_TYPE，只清理对应类型的文件
        log_info "清理 ${target_type} 版本构建产物..."
        
        # 1. 清理 dist 中的对应 DMG
        rm -f dist/*-${target_type}-*.dmg
        
        # 2. 清理资源目录中的对应内容，防止混用
        if [ "$target_type" = "full" ]; then
            log_info "清理 Full 版本资源..."
            rm -rf src-tauri/resources/python-venv
        else
            log_info "清理 Lite 版本资源..."
            rm -rf src-tauri/resources/python
            rm -rf src-tauri/resources/bin/uv
        fi
        
        # 注意：target/release/bundle 通常包含共享的编译产物，
        # 在特定类型的清理中，我们不删除整个目录，以免破坏其他版本的增量编译。
        # 但我们会删除 Tauri 在该目录下生成的临时 DMG
        rm -f "$BUILD_DIR"/*/release/bundle/dmg/*.dmg 2>/dev/null || true
    fi

    log_success "清理完成"
}

# 智能清理：只在资源类型不匹配时清理
# 注意：Lite 资源（python + uv）是 Full 的子集，Lite->Full 不需要清理
smart_clean() {
    local target_type="${1:-$BUILD_TYPE}"
    target_type="${target_type:-lite}"
    
    local current_type=$(detect_resource_type)
    
    log_info "当前资源类型: $current_type, 目标类型: $target_type"
    
    # 如果资源类型混合，执行完整清理
    if [ "$current_type" = "mixed" ]; then
        log_warning "检测到混合资源，执行完整清理..."
        clean ""
    elif [ "$current_type" = "full" ] && [ "$target_type" = "lite" ]; then
        # Full -> Lite: 清理 Full 特有的 venv，保留 Lite 资源
        log_warning "Full -> Lite 切换，清理 venv（保留 python + uv）..."
        rm -rf src-tauri/resources/python-venv
    elif [ "$current_type" = "none" ]; then
        log_info "没有现有资源，准备全新下载..."
    else
        # 包括：
        # - none -> lite/full: 全新下载
        # - lite -> lite: 保持不变
        # - lite -> full: 保留 lite，添加 venv
        # - full -> full: 保持不变
        log_info "资源兼容，跳过资源清理（保留 cache）"
    fi
    
    # 始终清理构建产物（DMG 等）
    rm -f dist/*-${target_type}-*.dmg 2>/dev/null || true
    rm -f "$BUILD_DIR"/*/release/bundle/dmg/*.dmg 2>/dev/null || true
}

# 准备资源文件
prepare_resources() {
    log_info "准备资源文件..."
    cd "$PROJECT_ROOT"

    # 根据 BUILD_TYPE 选择不同的准备脚本
    # BUILD_TYPE=lite (默认): Python + uv 动态依赖（推荐）
    # BUILD_TYPE=full: 使用完整 venv 打包（稳定，体积大）
    local build_type="${BUILD_TYPE:-lite}"

    if [ "$build_type" = "full" ]; then
        log_info "使用 Full 版本打包（完整 Python venv）"
        if [ -f "$SCRIPT_DIR/prepare-venv.sh" ]; then
            bash "$SCRIPT_DIR/prepare-venv.sh"
            log_success "Full 版本资源文件准备完成"
        else
            log_error "未找到 prepare-venv.sh"
            exit 1
        fi
    elif [ "$build_type" = "lite" ]; then
        log_info "使用 Lite 版本打包（Python + uv 动态依赖）"
        if [ -f "$SCRIPT_DIR/prepare-lite.sh" ]; then
            bash "$SCRIPT_DIR/prepare-lite.sh"
            log_success "Lite 版本资源文件准备完成"
        else
            log_error "未找到 prepare-lite.sh"
            exit 1
        fi
    else
        log_error "未知的 BUILD_TYPE: $build_type"
        log_info "支持的类型: lite (默认), full"
        exit 1
    fi
}

# 完整构建流程
full_build() {
    # 智能清理：只在资源类型不匹配时清理，保留同类型的 cache
    smart_clean "$BUILD_TYPE"

    check_dependencies
    auto_bump_version

    log_info "开始完整构建流程..."
    log_info "应用名称: $APP_NAME"
    log_info "版本: $VERSION"
    log_info "构建类型: $BUILD_TYPE"
    prepare_resources
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
    echo "  prepare   仅准备资源文件 (FFmpeg, Python)"
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
        prepare)
            prepare_resources
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
