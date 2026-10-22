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

# CLI 产物名里的 triple 必须与 `cargo tauri build --target` 一致，也必须与 App 运行时的
# 查找名一致（engine/cli.rs::target_triple() 是编译期的，跟的是 build target，不是 host）。
# 所以这里用与 prepare-ffmpeg.sh / fetch-*.sh 同一套 uname -m 映射，而不是 rustc 的 host：
# 在 arm64 机器上交叉构建 x86_64 时两者会不同，用 host 会产出一个运行时找不到的名字。
TARGET_TRIPLE=""
case "$(uname -m)" in
    arm64)  TARGET_TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TARGET_TRIPLE="x86_64-apple-darwin" ;;
    *) log_error "不支持的架构: $(uname -m)"; exit 1 ;;
esac

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

log_info "5. 编译命令行工具 (monosub-cli)..."
# CLI 的源码与 [[bin]] 由独立任务落地（docs/tech-spec/cli-design.md §3.2/§7.1）。
# 产物落到 resources/bin/monosub-cli-<triple>，与 audiocpp_cli / llama-server / yt-dlp
# 同一个目录，因此：`resources/**/*` 的 glob 会把它带进 App，sign-app.sh 现有的
# RESOURCE_BIN_DIR 循环会逐个重签，engine/cli.rs::bundle_resources_dir() 也已经能解析
# 这个位置——不需要 externalBin，也不需要新代码。
CLI_SOURCE="$PROJECT_ROOT/src-tauri/src/bin/monosub-cli.rs"
CLI_TARGET_DIR="$PROJECT_ROOT/src-tauri/resources/bin"
CLI_TARGET="$CLI_TARGET_DIR/monosub-cli-$TARGET_TRIPLE"
if [ -f "$CLI_SOURCE" ]; then
    cargo build --release --bin monosub-cli \
        --manifest-path "$PROJECT_ROOT/src-tauri/Cargo.toml" \
        --target "$TARGET_TRIPLE"
    mkdir -p "$CLI_TARGET_DIR"
    install -m 755 "$BUILD_DIR/$TARGET_TRIPLE/release/monosub-cli" "$CLI_TARGET"
    log_success "CLI 已就位: $CLI_TARGET"
else
    # 不硬失败：CLI 还没落地时，打包管线不能挡住别人。降级为「本次构建不含 CLI」，
    # 并删掉上一轮可能留下的旧产物，避免打进一个过期的二进制。
    # ${CLI_SOURCE} 必须带花括号：macOS 自带的 bash 3.2 会把紧跟 $var 之后的第一个
    # 多字节字符的首字节吃掉（“，”变乱码），花括号断开边界就不受影响。
    log_warning "未找到 ${CLI_SOURCE}，跳过 CLI 编译：本次构建不含 monosub-cli"
    rm -f "$CLI_TARGET"
fi

log_info "6. 打包 Tauri 原生客户端..."
cargo tauri build --target "$TARGET_TRIPLE"
log_success "Rust / Tauri 二进制编译成功"

log_info "7. macOS 安全校验与应用签名..."
# 调用签名流
bash "$SCRIPT_DIR/../release/sign-app.sh" "$OUTPUT_DIR/$DMG_NAME"

log_info "8. 提取产物 DMG ..."
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
