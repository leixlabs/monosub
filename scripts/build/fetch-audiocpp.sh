#!/bin/bash
# Usage: ./scripts/build/fetch-audiocpp.sh
# Fetches the audio.cpp inference binary and the Silero VAD model that are
# bundled with the app. Idempotent: re-running with matching artifacts is a no-op.
#
# Artifacts (all pinned; bump together after running the Phase 0 regression):
#   audiocpp_cli            audio.cpp v0.9.1, Apache-2.0
#   silero_vad_16k          from the same upstream tag
#
# The SenseVoice GGUF is NOT fetched here — it is a user-downloaded model
# (see src-tauri/resources/models.json).

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
MODEL_DIR="$RESOURCES_DIR/models/silero_vad"

# --- Pinned upstream ------------------------------------------------------
AUDIOCPP_VERSION="v0.9.1"

case "$(uname -m)" in
    arm64)  AUDIOCPP_ASSET="audio-v0.9.1-bin-macos-arm64-metal.tar.gz"
            AUDIOCPP_SHA256="960436787b84bf137a70ea1713ac460207ef2ac7b2617380fb7a1f4650d1100d"
            TARGET_TRIPLE="aarch64-apple-darwin" ;;
    x86_64) AUDIOCPP_ASSET="audio-v0.9.1-bin-macos-x64-metal.tar.gz"
            # Fill in after fetching the x64 asset once; leave empty to skip verification.
            AUDIOCPP_SHA256=""
            TARGET_TRIPLE="x86_64-apple-darwin" ;;
    *) log_err "Unsupported architecture: $(uname -m)"; exit 1 ;;
esac

AUDIOCPP_URL="https://github.com/0xShug0/audio.cpp/releases/download/${AUDIOCPP_VERSION}/${AUDIOCPP_ASSET}"
CLI_TARGET="$BIN_DIR/audiocpp_cli-$TARGET_TRIPLE"

SILERO_URL="https://raw.githubusercontent.com/0xShug0/audio.cpp/${AUDIOCPP_VERSION}/assets/framework/models/silero_vad/silero_vad_16k.safetensors"
SILERO_SHA256="c59271c284ae9c8335d795d60e0bfdb71aaaceec578d9bd9ffc1b8153c319ea1"
SILERO_TARGET="$MODEL_DIR/silero_vad_16k.safetensors"

# Use the local Clash/proxy if one is exported; HF and GitHub are both far more
# reliable through it in CN. Harmless when unset.
CURL_OPTS=(--fail --location --silent --show-error --retry 3 --retry-delay 2)

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

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# --- 1. audiocpp_cli ------------------------------------------------------
mkdir -p "$BIN_DIR"
if [ -x "$CLI_TARGET" ]; then
    log_ok "audiocpp_cli already present, skipping ($TARGET_TRIPLE)"
else
    log_info "Downloading $AUDIOCPP_ASSET ..."
    curl "${CURL_OPTS[@]}" -o "$TMP_DIR/$AUDIOCPP_ASSET" "$AUDIOCPP_URL"
    verify_sha256 "$TMP_DIR/$AUDIOCPP_ASSET" "$AUDIOCPP_SHA256"

    log_info "Extracting audiocpp_cli ..."
    tar -xzf "$TMP_DIR/$AUDIOCPP_ASSET" -C "$TMP_DIR" audiocpp_cli
    install -m 755 "$TMP_DIR/audiocpp_cli" "$CLI_TARGET"
    log_ok "Installed $CLI_TARGET"
fi

# --- 2. Silero VAD --------------------------------------------------------
mkdir -p "$MODEL_DIR"
if [ -f "$SILERO_TARGET" ] && verify_sha256 "$SILERO_TARGET" "$SILERO_SHA256" 2>/dev/null; then
    log_ok "silero_vad_16k.safetensors already present, skipping"
else
    log_info "Downloading silero_vad_16k.safetensors ..."
    curl "${CURL_OPTS[@]}" -o "$TMP_DIR/silero.safetensors" "$SILERO_URL"
    verify_sha256 "$TMP_DIR/silero.safetensors" "$SILERO_SHA256"
    install -m 644 "$TMP_DIR/silero.safetensors" "$SILERO_TARGET"
    log_ok "Installed $SILERO_TARGET"
fi

# NOTE: the binary ships adhoc/linker-signed. Hardened-runtime signing happens in
# scripts/release/sign-app.sh; do not re-sign here or the SHA256 pin stops
# describing the file we ship.

# --- Verify ---------------------------------------------------------------
log_info "Verifying audiocpp_cli ..."
"$CLI_TARGET" --version

log_ok "audio.cpp resources ready."
echo "    $CLI_TARGET"
echo "    $SILERO_TARGET"
