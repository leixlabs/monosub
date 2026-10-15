#!/bin/bash
# Usage: ./scripts/build/build-macos-app.sh
# 主应用构建流程（macOS）

set -e
BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

# v2.0 起引擎是随包的 audiocpp_cli，不再有 Lite/Full 之分。
# 已知 scripts/release/release.sh 仍会传入旧参数，此处显式忽略以免 DMG 命名被污染。
if [ "$#" -gt 0 ]; then
    log_warning "已忽略参数: $* （不再有 Lite/Full 构建模式）"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target"
OUTPUT_DIR="$PROJECT_ROOT/dist"

log_info "1. 前置清理..."
cd "$PROJECT_ROOT"
# 只清除 DMG缓存
rm -f "$OUTPUT_DIR"/*.dmg 2>/dev/null || true
rm -f "$BUILD_DIR"/*/release/bundle/dmg/*.dmg 2>/dev/null || true

log_info "2. 检测与管理版本号..."
if [ -f "$SCRIPT_DIR/../release/bump-version.sh" ]; then
    bash "$SCRIPT_DIR/../release/bump-version.sh" > /dev/null
fi
source "$SCRIPT_DIR/../utils/version-utils.sh"
sync_versions_from_tauri "$PROJECT_ROOT"
VERSION=$(get_tauri_version "$PROJECT_ROOT")
APP_NAME="MonoSub"
DMG_NAME="${APP_NAME}-${VERSION}.dmg"
log_success "当前发布版本: v$VERSION"

log_info "3. 准备资源依赖 (Prepare Resources)..."
bash "$SCRIPT_DIR/prepare-resources.sh"

log_info "4. 初始化前端环境及编译代码..."
if command -v bun &> /dev/null; then
    PKG_MANAGER="bun"
else
    PKG_MANAGER="npm"
fi
$PKG_MANAGER install
$PKG_MANAGER run build
log_success "前端 (Vite) 构建就绪"

log_info "5. 打包 Tauri 原生客户端..."
if [ "$(uname -m)" = "arm64" ]; then
    cargo tauri build --target aarch64-apple-darwin
else
    cargo tauri build --target x86_64-apple-darwin
fi
log_success "Rust / Tauri 二进制编译成功"

log_info "6. macOS 安全校验与应用签名..."
# 调用签名流
bash "$SCRIPT_DIR/../release/sign-app.sh" "$OUTPUT_DIR/$DMG_NAME"

log_info "7. 提取产物 DMG ..."
mkdir -p "$OUTPUT_DIR"
BUILT_DMG=$(find "$BUILD_DIR"/*/release/bundle/dmg -name "*.dmg" -type f | head -1)

if [ -z "$BUILT_DMG" ]; then
    log_error "编译失败，未能找到 Tauri 自动生成的 DMG 包！"
    exit 1
fi
DMG_PATH="$OUTPUT_DIR/$DMG_NAME"
cp "$BUILT_DMG" "$DMG_PATH"

log_success "产物 DMG 已收集: $DMG_PATH"
exit 0
