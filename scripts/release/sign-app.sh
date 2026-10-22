#!/bin/bash
# Usage: ./scripts/release/sign-app.sh <DMG_PATH>
# 负责 macOS 应用打包后的苹果商业签名与公证

set -e
BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

DMG_PATH="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target"

sign_app() {
    if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
        log_info "发现 APPLE_SIGNING_IDENTITY=$APPLE_SIGNING_IDENTITY, 开始签名应用..."
        APP_PATH=$(find "$BUILD_DIR" -name "*.app" -type d | head -1)
        if [ -n "$APP_PATH" ]; then
            # audio.cpp 与 ffmpeg 以 adhoc/linker-signed 状态分发（PoC §2），必须在签 App 的
            # 外壳之前对每个内嵌可执行文件单独重签，否则 hardened runtime 下公证会失败。
            #
            # llama.cpp 同理：llama-server-* 与它的 lib*.dylib 都直接放在这个目录里
            # （不进 bin/lib/，因为 llama-server 的 LC_RPATH 只有 @loader_path），
            # 因此下面的 "*" 循环已经覆盖了它们。不要再加第二个循环，否则同一批文件
            # 会被重签两次。ffmpeg 的 bin/lib/ 仍然由它自己那套签名规则处理，不受影响。
            RESOURCE_BIN_DIR="$APP_PATH/Contents/Resources/resources/bin"
            if [ -d "$RESOURCE_BIN_DIR" ]; then
                for bundled_bin in "$RESOURCE_BIN_DIR"/*; do
                    [ -f "$bundled_bin" ] || continue
                    log_info "  签名 $(basename "$bundled_bin")"
                    codesign --force --options runtime --timestamp \
                        --sign "$APPLE_SIGNING_IDENTITY" "$bundled_bin"
                done
            else
                log_warning "未找到内嵌二进制目录: $RESOURCE_BIN_DIR"
            fi
            # CLI 正常作为 resources/bin/monosub-cli-<triple> 被打包（build-macos-app.sh
            # 生成），上面的 "*" 循环已经覆盖它，这里不再重签（重复签会违反上面的约定）。
            # 这段只兼顾另一种布局：用 tauri 的 bundle > externalBin 时，sidecar 落在
            # Contents/MacOS/，不在那个循环里。漏签的表现是公证能过、用户首次运行被
            # Gatekeeper 杀掉（killed: 9）。当前构建里它不存在，静默跳过。
            CLI_BIN="$APP_PATH/Contents/MacOS/monosub-cli"
            if [ -f "$CLI_BIN" ]; then
                log_info "  签名 monosub-cli"
                codesign --force --options runtime --timestamp \
                    --sign "$APPLE_SIGNING_IDENTITY" "$CLI_BIN"
            fi
            codesign --force --deep --sign "$APPLE_SIGNING_IDENTITY" "$APP_PATH"
            log_success "代码签名完成: $APP_PATH"
        else
            log_warning "未能找到编译的.app以进行签名"
        fi
    else
        log_warning "未设置 APPLE_SIGNING_IDENTITY，跳过签名"
    fi
}

notarize_app() {
    if [ -n "$APPLE_ID" ] && [ -n "$APPLE_PASSWORD" ] && [ -n "$APPLE_TEAM_ID" ]; then
        if [ -z "$DMG_PATH" ] || [ ! -f "$DMG_PATH" ]; then
            log_warning "要公证的 DMG 文件路径缺失或无效: $DMG_PATH"
            return
        fi
        log_info "将提交该 DMG 进行 Notarization公证..."
        xcrun notarytool submit "$DMG_PATH" \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_PASSWORD" \
            --team-id "$APPLE_TEAM_ID" \
            --wait
        xcrun stapler staple "$DMG_PATH"
        log_success "公证完成并已装订票据!"
    else
        log_warning "未设置 Apple Notary 凭据，跳过公证"
    fi
}

log_info "开始安全与分发认证流程..."
sign_app
notarize_app
exit 0
