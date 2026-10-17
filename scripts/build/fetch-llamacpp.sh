#!/bin/bash
# Usage: ./scripts/build/fetch-llamacpp.sh
# Fetches the llama.cpp inference server that the app bundles for local
# subtitle translation. Idempotent: re-running with matching artifacts is a no-op.
#
# Artifacts (pinned; bump together after re-running the Phase 0 regression):
#   llama-server            llama.cpp b11696, MIT
#   lib*.dylib              the same tarball's shared libraries
#
# Why the dylibs land FLAT in resources/bin/ and not in resources/bin/lib/:
# llama-server's only LC_RPATH is @loader_path, so its libraries must sit in the
# executable's own directory. (ffmpeg's dylibs live in bin/lib/ and are resolved
# by that binary's own rpath — the two layouts must not be merged.) Verified on
# this machine: with the flat layout the server starts with no DYLD_LIBRARY_PATH.
#
# The translation model GGUF is NOT fetched here — it is a user-downloaded model
# (see src-tauri/resources/models.json, `hy_mt_1_8b_q4_k_m`).
#
# `b11696` is a rolling build rather than a release tag: v0.7.0 is a
# pointer-only release with no binary assets, so only the rolling tag has
# artifacts to download.

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

# --- Pinned upstream ------------------------------------------------------
LLAMA_BUILD="b11696"
# Directory the tarball unpacks into (flat, one level).
LLAMA_ARCHIVE_DIR="llama-${LLAMA_BUILD}"

case "$(uname -m)" in
    arm64)  LLAMA_ASSET="llama-${LLAMA_BUILD}-bin-macos-arm64.tar.gz"
            LLAMA_SHA256="6aac007fbe153efefe06250d2cf05d0487583aa0706379082ebef2b05579f649"
            TARGET_TRIPLE="aarch64-apple-darwin" ;;
    x86_64) LLAMA_ASSET="llama-${LLAMA_BUILD}-bin-macos-x64.tar.gz"
            # Fill in after fetching the x64 asset once; leave empty to skip verification.
            LLAMA_SHA256=""
            TARGET_TRIPLE="x86_64-apple-darwin" ;;
    *) log_err "Unsupported architecture: $(uname -m)"; exit 1 ;;
esac

LLAMA_URL="https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_BUILD}/${LLAMA_ASSET}"
SERVER_TARGET="$BIN_DIR/llama-server-$TARGET_TRIPLE"

# Use the local Clash/proxy if one is exported; GitHub is far more reliable
# through it in CN. Harmless when unset.
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

# --- 1. llama-server + its dylibs -----------------------------------------
mkdir -p "$BIN_DIR"

# "Already present" means the executable plus the rpath libraries it cannot run
# without. The exact dylib set is an upstream detail, so the check only asks for
# both families rather than a list that would rot with every bump.
DYLIBS_PRESENT=$( { ls "$BIN_DIR"/libllama*.dylib "$BIN_DIR"/libggml*.dylib 2>/dev/null || true; } | wc -l | tr -d ' ')
if [ -x "$SERVER_TARGET" ] && [ "$DYLIBS_PRESENT" -gt 0 ]; then
    log_ok "llama-server already present, skipping ($TARGET_TRIPLE)"
else
    log_info "Downloading $LLAMA_ASSET ..."
    curl "${CURL_OPTS[@]}" -o "$TMP_DIR/$LLAMA_ASSET" "$LLAMA_URL"
    verify_sha256 "$TMP_DIR/$LLAMA_ASSET" "$LLAMA_SHA256"

    # The tarball also carries llama-cli, llama-tokenize and friends; the whole
    # archive goes to the temp dir and only the server and the shared libraries
    # are installed, so nothing else can reach the DMG.
    log_info "Extracting llama-server and lib*.dylib ..."
    tar -xzf "$TMP_DIR/$LLAMA_ASSET" -C "$TMP_DIR"

    if [ ! -f "$TMP_DIR/$LLAMA_ARCHIVE_DIR/llama-server" ]; then
        log_err "llama-server missing from the archive layout: $TMP_DIR/$LLAMA_ARCHIVE_DIR"
        exit 1
    fi

    install -m 755 "$TMP_DIR/$LLAMA_ARCHIVE_DIR/llama-server" "$SERVER_TARGET"

    found_dylib=0
    for dylib in "$TMP_DIR/$LLAMA_ARCHIVE_DIR"/lib*.dylib; do
        [ -f "$dylib" ] || continue
        install -m 755 "$dylib" "$BIN_DIR/$(basename "$dylib")"
        found_dylib=$((found_dylib + 1))
    done
    if [ "$found_dylib" -eq 0 ]; then
        log_err "no lib*.dylib in the archive; llama-server could not start (@loader_path)"
        exit 1
    fi

    log_ok "Installed $SERVER_TARGET (+$found_dylib dylibs flat in bin/)"
fi

# NOTE: the binary ships adhoc/linker-signed. Hardened-runtime signing happens in
# scripts/release/sign-app.sh; do not re-sign here or the SHA256 pin stops
# describing the file we ship.

# --- Verify ---------------------------------------------------------------
log_info "Verifying llama-server ..."
"$SERVER_TARGET" --version

log_ok "llama.cpp resources ready."
echo "    $SERVER_TARGET"
echo "    $BIN_DIR/lib{llama,ggml,mtmd}*.dylib"
