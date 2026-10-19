#!/bin/bash
# Usage: ./scripts/build/fetch-ytdlp.sh
# Fetches the official yt-dlp executable that the app bundles for URL import.
# Idempotent: re-running with the matching binary is a no-op.
#
# Artifacts (pinned; bump together after re-running the URL-import smoke checks):
#   bin/yt-dlp-<target-triple>            yt-dlp 2026.08.19 (Unlicense)
#   licenses/yt-dlp-LICENSE               upstream LICENSE
#   licenses/yt-dlp-THIRD_PARTY_LICENSES.txt
#
# Why the binary is bundled rather than discovered on PATH (url-import-design
# §6.1): Bilibili's extraction logic changes often, so a user-installed yt-dlp
# produces "this version is wrong" problems that cannot be diagnosed from a
# support report. `MONOSUB_YTDLP_PATH` still overrides the bundled file for
# debugging and hot-fixing. There is no self-update in this release.
#
# The macOS asset `yt-dlp_macos` is a Universal binary (arm64 + x86_64 in one
# file), so both macOS triples install the same download. That is also why the
# SHA256 pin is shared instead of per-architecture.

set -euo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_ok() { echo -e "${GREEN}[OK]${NC} $1"; }
log_err() { echo -e "${RED}[ERROR]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"
BIN_DIR="$RESOURCES_DIR/bin"
LICENSE_DIR="$RESOURCES_DIR/licenses"

# --- Pinned upstream ------------------------------------------------------
YTDLP_VERSION="2026.08.19"
# SHA2-256SUMS from the release page; the size below is what the CDN serves.
YTDLP_SHA256="0f192b7ec147ab6288885d6351d9ab67367640029b4377576ef46dd79cf7b202"
YTDLP_SIZE=37146048
YTDLP_RAW_BASE="https://raw.githubusercontent.com/yt-dlp/yt-dlp/${YTDLP_VERSION}"

case "$(uname -s)" in
    Darwin)
        case "$(uname -m)" in
            arm64)  TARGET_TRIPLE="aarch64-apple-darwin" ;;
            x86_64) TARGET_TRIPLE="x86_64-apple-darwin" ;;
            *) log_err "Unsupported architecture: $(uname -m)"; exit 1 ;;
        esac
        YTDLP_ASSET="yt-dlp_macos"
        ;;
    Linux)
        case "$(uname -m)" in
            aarch64|arm64) TARGET_TRIPLE="aarch64-unknown-linux-gnu"; YTDLP_ASSET="yt-dlp_linux_aarch64" ;;
            x86_64)        TARGET_TRIPLE="x86_64-unknown-linux-gnu";  YTDLP_ASSET="yt-dlp_linux" ;;
            *) log_err "Unsupported architecture: $(uname -m)"; exit 1 ;;
        esac
        # The Linux pins are filled in the first time those assets are fetched;
        # an empty pin skips verification rather than failing the build.
        YTDLP_SHA256=""
        YTDLP_SIZE=0
        ;;
    *)
        log_err "Unsupported platform: $(uname -s)"
        exit 1
        ;;
esac

YTDLP_URL="https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}/${YTDLP_ASSET}"
TARGET="$BIN_DIR/yt-dlp-$TARGET_TRIPLE"
LICENSE_TARGET="$LICENSE_DIR/yt-dlp-LICENSE"
NOTICES_TARGET="$LICENSE_DIR/yt-dlp-THIRD_PARTY_LICENSES.txt"

# `--retry-all-errors` matters here: the yt-dlp release CDN regularly drops a
# connection mid-transfer on a 35 MB asset, and a plain `--retry` does not cover
# a truncated body (curl exit 18).
CURL_OPTS=(--fail --location --silent --show-error --retry 5 --retry-delay 2 --retry-all-errors)

verify_sha256() {
    local file="$1" expected="$2"
    [ -z "$expected" ] && return 0
    local actual
    actual="$(shasum -a 256 "$file" | awk '{print $1}')"
    if [ "$actual" != "$expected" ]; then
        log_err "Checksum mismatch for $(basename "$file")"
        log_err "  expected $expected"
        log_err "  actual   $actual"
        return 1
    fi
    return 0
}

mkdir -p "$BIN_DIR" "$LICENSE_DIR"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# --- 1. the executable ----------------------------------------------------
# "Already present" means executable AND matching the pinned digest. A stale
# binary from an older pin must be replaced, not silently kept, or the pin stops
# describing what ships.
if [ -x "$TARGET" ] && verify_sha256 "$TARGET" "$YTDLP_SHA256" 2>/dev/null; then
    log_ok "yt-dlp already present, skipping ($TARGET_TRIPLE)"
else
    log_info "Downloading $YTDLP_ASSET ($YTDLP_VERSION) ..."
    curl "${CURL_OPTS[@]}" -o "$TMP_DIR/$YTDLP_ASSET" "$YTDLP_URL"

    verify_sha256 "$TMP_DIR/$YTDLP_ASSET" "$YTDLP_SHA256"

    if [ "$YTDLP_SIZE" -gt 0 ]; then
        actual_size=$(stat -f '%z' "$TMP_DIR/$YTDLP_ASSET" 2>/dev/null || stat -c '%s' "$TMP_DIR/$YTDLP_ASSET")
        if [ "$actual_size" != "$YTDLP_SIZE" ]; then
            log_err "Unexpected size for $YTDLP_ASSET: $actual_size (expected $YTDLP_SIZE)"
            exit 1
        fi
    fi

    install -m 755 "$TMP_DIR/$YTDLP_ASSET" "$TARGET"
    log_ok "Installed $TARGET"
fi

# --- 2. licences ----------------------------------------------------------
# yt-dlp is Unlicense, but it statically bundles third-party code, so both files
# ship with the app (AGENTS.md → Licensing / Attribution).
fetch_license() {
    local remote_path="$1" local_path="$2"
    if [ -s "$local_path" ]; then
        log_ok "$(basename "$local_path") already present, skipping"
        return 0
    fi
    log_info "Fetching $remote_path ..."
    curl "${CURL_OPTS[@]}" -o "$local_path" "$YTDLP_RAW_BASE/$remote_path"
}

fetch_license "LICENSE" "$LICENSE_TARGET"
fetch_license "THIRD_PARTY_LICENSES.txt" "$NOTICES_TARGET"

# NOTE: the binary ships adhoc/linker-signed. Hardened-runtime signing happens in
# scripts/release/sign-app.sh; do not re-sign here or the SHA256 pin stops
# describing the file we ship.

# --- Verify ---------------------------------------------------------------
log_info "Verifying yt-dlp ..."
"$TARGET" --version

log_ok "yt-dlp resources ready."
echo "    $TARGET"
echo "    $LICENSE_TARGET"
echo "    $NOTICES_TARGET"
