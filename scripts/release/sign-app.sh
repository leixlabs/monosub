#!/bin/bash
# Usage: ./scripts/release/sign-app.sh <DMG_PATH>
#
# 校验 macOS 分发产物：DMG 里的 App 必须是 ad-hoc 签名，且内嵌引擎能正常启动。
#
# 本机没有 Apple Developer 账号，因此分发走 ad-hoc，配置在 tauri.conf.json：
#   bundle.macOS.signingIdentity = "-"     → Tauri 用 ad-hoc 签名
#   bundle.macOS.hardenedRuntime   = false  → 见下面的实测说明
#
# 为什么这个脚本不再自己签名：
#   `cargo tauri build` 在结束前就已经把 DMG 生成好了。旧版脚本在这之后才对磁盘上的
#   .app 重签，而 DMG 里装的是签名前的副本——签了个寂寞，用户下载到的仍是未签名的 App。
#   正确的做法是让 Tauri 在打包流程内部签名（配置项即可做到），本脚本改为校验产物。
#
# 为什么 hardenedRuntime 必须是 false（本机实测，2026-10）：
#   llama-server 的 LC_RPATH 只有 @loader_path，依赖同目录的 libllama-*.dylib。
#   ad-hoc + `--options runtime` 会开启 dyld 的 library validation，而 ad-hoc 签名没有
#   Team ID，于是拒绝加载同目录的库：
#     Library not loaded: @rpath/libllama-server-impl.dylib
#     ... mapping process and mapped file (non-platform) have different Team IDs
#   去掉 runtime 后同一组文件正常启动。ffmpeg / audiocpp_cli 不受影响，但
#   llama-server 会整个翻译功能不可用——这是"签名成功、功能坏掉"的静默故障，
#   所以下面的冒烟测试必须真的把三个引擎各启动一次。
#
# 退出码：0 = 校验通过；非 0 = 产物不合格，release.sh 必须中止。

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

DMG_PATH="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target"

fail() {
    log_error "$1"
    exit 1
}

if [ -z "$DMG_PATH" ] || [ ! -f "$DMG_PATH" ]; then
    fail "DMG 不存在: ${DMG_PATH:-<未提供>}"
fi

# 本机没有 Developer ID。若环境里残留了别的签名身份，Tauri 会用它覆盖配置里的
# ad-hoc，结果是一个"看起来签过、实际用户打不开"的包。这里直接拦下。
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ] && [ "${APPLE_SIGNING_IDENTITY}" != "-" ]; then
    fail "APPLE_SIGNING_IDENTITY=$APPLE_SIGNING_IDENTITY 覆盖了 ad-hoc 配置。
      本机没有 Apple Developer 账号，分发必须用 ad-hoc（signingIdentity = \"-\"）。
      请在构建前 unset APPLE_SIGNING_IDENTITY。"
fi

MOUNT_POINT="$(mktemp -d "${TMPDIR:-/tmp}/monosub-dmg-verify.XXXXXX")"
DETACHED=0

cleanup() {
    if [ "$DETACHED" -eq 0 ]; then
        hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || hdiutil detach "$MOUNT_POINT" -force -quiet 2>/dev/null || true
    fi
    rmdir "$MOUNT_POINT" 2>/dev/null || true
}
trap cleanup EXIT

log_info "挂载 DMG 以校验产物..."
hdiutil attach "$DMG_PATH" -nobrowse -readonly -mountpoint "$MOUNT_POINT" -quiet \
    || fail "无法挂载 DMG: $DMG_PATH"

APP_PATH=$(find "$MOUNT_POINT" -maxdepth 2 -name '*.app' -type d | head -1)
[ -n "$APP_PATH" ] || fail "DMG 内没有找到 .app"
log_info "  已找到: $(basename "$APP_PATH")"

# 1. 签名本身：必须存在、必须完整、必须是 ad-hoc（而不是某个开发者证书）。
log_info "1. 校验 App 签名..."
codesign --verify --deep --strict "$APP_PATH" 2>/dev/null \
    || fail "App 签名校验失败（codesign --verify --deep --strict）"

SIGN_INFO=$(codesign -dv "$APP_PATH" 2>&1)
if ! echo "$SIGN_INFO" | grep -q '^Signature=adhoc'; then
    echo "$SIGN_INFO" | grep -E '^(Signature|Authority|TeamIdentifier)=' >&2 || true
    fail "App 不是 ad-hoc 签名。若看到 Authority=Apple Development / Developer ID，
      说明构建复用了本机证书——这不是可分发签名。"
fi

if echo "$SIGN_INFO" | grep -qE '^Authority=(Apple Development|Developer ID)'; then
    echo "$SIGN_INFO" | grep '^Authority=' >&2
    fail "App 带了本机开发者证书签名，必须改为 ad-hoc。"
fi

if echo "$SIGN_INFO" | grep -q 'flags=.*runtime'; then
    fail "App 启用了 hardened runtime。ad-hoc + runtime 会让 llama-server 加载不了
      同目录的 dylib（实测）。请确认 tauri.conf.json 里 hardenedRuntime=false。"
fi

log_success "  App 为 ad-hoc 签名，未启用 hardened runtime"

# 2. 内嵌二进制：每个都必须是 ad-hoc，且不能是残留的开发者证书签名。
RESOURCE_BIN_DIR="$APP_PATH/Contents/Resources/resources/bin"
[ -d "$RESOURCE_BIN_DIR" ] || fail "DMG 内缺少内嵌二进制目录: Contents/Resources/resources/bin"

log_info "2. 校验内嵌二进制签名..."
BIN_COUNT=0
for bundled_bin in "$RESOURCE_BIN_DIR"/*; do
    [ -f "$bundled_bin" ] || continue
    BIN_COUNT=$((BIN_COUNT + 1))
    BIN_NAME=$(basename "$bundled_bin")
    if ! codesign -dv "$bundled_bin" 2>&1 | grep -q '^Signature=adhoc'; then
        fail "内嵌二进制不是 ad-hoc 签名: $BIN_NAME"
    fi
done
log_success "  $BIN_COUNT 个内嵌二进制均为 ad-hoc 签名"

# 3. 冒烟测试：签名"成功"但引擎起不来是这类问题最典型的失败方式，必须真的跑一次。
#    llama-server 启动会走 @loader_path 加载 dylib，正是 hardened runtime 会破坏的那一步。
log_info "3. 冒烟测试内嵌引擎（从 DMG 内的副本运行）..."
"$RESOURCE_BIN_DIR"/ffprobe-* -version >/dev/null 2>&1 \
    || fail "ffprobe 无法启动"
log_success "  ffprobe 正常"

"$RESOURCE_BIN_DIR"/audiocpp_cli-* --version >/dev/null 2>&1 \
    || fail "audiocpp_cli 无法启动"
log_success "  audiocpp_cli 正常"

# llama-server 没有纯 --version 退出路径时会打印后退出；两种都算成功。
LLAMA_OUT=$("$RESOURCE_BIN_DIR"/llama-server-* --version 2>&1) || {
    echo "$LLAMA_OUT" >&2
    fail "llama-server 无法启动——通常是 hardened runtime 破坏了 @loader_path 的 dylib 加载"
}
echo "$LLAMA_OUT" | grep -qi 'version' || {
    echo "$LLAMA_OUT" >&2
    fail "llama-server 输出异常"
}
log_success "  llama-server 正常（dylib 加载通过）"

CLI_BIN=$(find "$RESOURCE_BIN_DIR" -maxdepth 1 -name 'monosub-cli-*' -type f | head -1)
if [ -n "$CLI_BIN" ]; then
    "$CLI_BIN" --help >/dev/null 2>&1 || fail "monosub-cli 无法启动"
    log_success "  monosub-cli 正常"
else
    log_warning "  未找到 monosub-cli，跳过（本次构建不含 CLI）"
fi

hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || hdiutil detach "$MOUNT_POINT" -force -quiet 2>/dev/null || true
DETACHED=1

# 4. 公证：没有 Apple Developer 账号就没有可公证的签名，跳过是预期行为，不是错误。
log_info "4. 公证检查..."
if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    log_warning "检测到 Apple Notary 凭据，但产物是 ad-hoc 签名，公证会失败，已跳过。"
    log_warning "ad-hoc 签名的 App 无法公证；用户首次打开需在 系统设置 → 隐私与安全性 放行。"
else
    log_info "  无 Apple Notary 凭据，跳过公证（ad-hoc 分发预期如此）"
fi

log_success "产物校验通过: $DMG_PATH"
exit 0
