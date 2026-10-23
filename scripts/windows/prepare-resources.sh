#!/bin/bash
# Usage: ./scripts/windows/prepare-resources.sh
#
# Windows-only. The macOS resource flow is scripts/build/prepare-resources.sh;
# the two directories are kept separate so neither platform can break the other.
# Fetches the Windows x64 runtime binaries that the MSIX package bundles.
#
# This is the Windows counterpart of prepare-resources.sh (which only handles
# macOS). Run it from Git Bash on Windows; it is idempotent, so re-running with
# matching artifacts is a no-op.
#
# Layout it produces, under src-tauri/resources/:
#
#   bin/audiocpp_cli-x86_64-pc-windows-msvc.exe   audio.cpp CLI (spec §4.2)
#   bin/ffmpeg-x86_64-pc-windows-msvc.exe         static ffmpeg
#   bin/ffprobe-x86_64-pc-windows-msvc.exe
#   bin/llama-server-x86_64-pc-windows-msvc.exe   + llama/ggml DLLs, flat
#   bin/yt-dlp-x86_64-pc-windows-msvc.exe
#   bin/*.dll                                     MSVC runtime + llama DLLs
#   models/silero_vad/silero_vad_16k.safetensors
#   MANIFEST.txt
#
# Why the `.exe` suffix sits after the target triple: `engine::cli`'s candidate
# rule is `<name>-<target-triple>.exe` on Windows (spec §4.1). The DLLs are flat
# next to the executables because Windows resolves a child process's imports from
# its own directory first — the same reason the macOS llama dylibs are flat.
#
# The macOS artifacts are never touched: every file written here carries the
# `x86_64-pc-windows-msvc` triple or is a `.dll` that only Windows can load.

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
LICENSE_DIR="$RESOURCES_DIR/licenses"

TARGET_TRIPLE="x86_64-pc-windows-msvc"

# --- Pinned upstream ------------------------------------------------------
# Every pin is the archive the URL resolves to today; bump them together after a
# smoke test of the packaged app (spec §5.6), never one at a time.
AUDIOCPP_VERSION="v0.9.1"
AUDIOCPP_ASSET="audio-v0.9.1-bin-windows-x64-cpu-portable.zip"
AUDIOCPP_SHA256="2fccc73dc97af84da1e81c3af545fa619348a46d023ea7d927ba80bc8680d2a0"

LLAMA_BUILD="b11696"
LLAMA_ASSET="llama-${LLAMA_BUILD}-bin-win-cpu-x64.zip"
LLAMA_SHA256="1e7c47dff58467540d53e6cf302d83951bcb165afb252c2952ed3872190fc60d"

# gyan.dev re-points the version-less URL on every release, so the pinned file
# name is what keeps this reproducible. `packages/` is the immutable path.
FFMPEG_VERSION="8.1.2"
FFMPEG_ASSET="ffmpeg-${FFMPEG_VERSION}-essentials_build.zip"
FFMPEG_SHA256="db580001caa24ac104c8cb856cd113a87b0a443f7bdf47d8c12b1d740584a2ec"

YTDLP_VERSION="2026.08.19"
YTDLP_ASSET="yt-dlp.exe"
YTDLP_SHA256="66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a"

# Same upstream tag as the macOS script; the VAD model is platform-neutral.
SILERO_URL="https://raw.githubusercontent.com/0xShug0/audio.cpp/${AUDIOCPP_VERSION}/assets/framework/models/silero_vad/silero_vad_16k.safetensors"
SILERO_SHA256="c59271c284ae9c8335d795d60e0bfdb71aaaceec578d9bd9ffc1b8153c319ea1"
SILERO_TARGET="$MODEL_DIR/silero_vad_16k.safetensors"

# GitHub and the ffmpeg CDN are both far more reliable through the local proxy in
# CN; harmless when unset.
CURL_OPTS=(--fail --location --silent --show-error --retry 5 --retry-delay 2 --retry-all-errors)

verify_sha256() {
    local file="$1" expected="$2"
    [ -z "$expected" ] && return 0
    local actual
    actual="$(sha256sum "$file" | awk '{print $1}')"
    if [ "$actual" != "$expected" ]; then
        log_err "Checksum mismatch for $(basename "$file")"
        log_err "  expected $expected"
        log_err "  actual   $actual"
        return 1
    fi
    return 0
}

# Download once, verify, then reuse for the rest of the run. The temp dir is
# outside the repo so an interrupted run cannot leave a half-written archive
# where `resources/**/*` would bundle it.
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fetch() {
    local url="$1" dest="$2" sha="$3"
    log_info "Downloading $(basename "$dest") ..."
    curl "${CURL_OPTS[@]}" -o "$dest" "$url"
    verify_sha256 "$dest" "$sha"
}

mkdir -p "$BIN_DIR" "$MODEL_DIR" "$LICENSE_DIR"

# --- 1. audio.cpp CLI + its MSVC runtime ----------------------------------
CLI_TARGET="$BIN_DIR/audiocpp_cli-$TARGET_TRIPLE.exe"
if [ -f "$CLI_TARGET" ]; then
    log_ok "audiocpp_cli already present, skipping ($TARGET_TRIPLE)"
else
    fetch "https://github.com/0xShug0/audio.cpp/releases/download/${AUDIOCPP_VERSION}/${AUDIOCPP_ASSET}" \
        "$TMP_DIR/$AUDIOCPP_ASSET" "$AUDIOCPP_SHA256"

    log_info "Extracting audiocpp_cli and its runtime DLLs ..."
    unzip -o -q "$TMP_DIR/$AUDIOCPP_ASSET" -d "$TMP_DIR/audiocpp"

    install -m 755 "$TMP_DIR/audiocpp/audiocpp_cli.exe" "$CLI_TARGET"

    # The portable build is statically linked for ggml, so the only DLLs it needs
    # are the MSVC runtime ones. They go next to the executable, which is where
    # the child process looks for them.
    runtime_dlls=0
    for dll in "$TMP_DIR/audiocpp"/*.dll; do
        [ -f "$dll" ] || continue
        install -m 644 "$dll" "$BIN_DIR/$(basename "$dll")"
        runtime_dlls=$((runtime_dlls + 1))
    done
    log_ok "Installed $CLI_TARGET (+$runtime_dlls runtime DLLs)"
fi

# --- 2. llama-server + its DLLs -------------------------------------------
SERVER_TARGET="$BIN_DIR/llama-server-$TARGET_TRIPLE.exe"
if [ -f "$SERVER_TARGET" ] && ls "$BIN_DIR"/llama*.dll >/dev/null 2>&1; then
    log_ok "llama-server already present, skipping ($TARGET_TRIPLE)"
else
    fetch "https://github.com/ggml-org/llama.cpp/releases/download/${LLAMA_BUILD}/${LLAMA_ASSET}" \
        "$TMP_DIR/$LLAMA_ASSET" "$LLAMA_SHA256"

    log_info "Extracting llama-server and lib*.dll ..."
    unzip -o -q "$TMP_DIR/$LLAMA_ASSET" -d "$TMP_DIR/llama"

    install -m 755 "$TMP_DIR/llama/llama-server.exe" "$SERVER_TARGET"

    # The zip ships one executable per tool plus the shared libraries they all
    # need. Only the server is installed; every DLL is, because the server is a
    # thin launcher over `llama-server-impl.dll` and its ggml backends.
    dll_count=0
    for dll in "$TMP_DIR/llama"/*.dll; do
        [ -f "$dll" ] || continue
        install -m 644 "$dll" "$BIN_DIR/$(basename "$dll")"
        dll_count=$((dll_count + 1))
    done
    if [ "$dll_count" -eq 0 ]; then
        log_err "no DLLs in the llama archive; llama-server cannot start"
        exit 1
    fi
    log_ok "Installed $SERVER_TARGET (+$dll_count DLLs flat in bin/)"
fi

# --- 3. FFmpeg / FFprobe --------------------------------------------------
FFMPEG_TARGET="$BIN_DIR/ffmpeg-$TARGET_TRIPLE.exe"
FFPROBE_TARGET="$BIN_DIR/ffprobe-$TARGET_TRIPLE.exe"
if [ -f "$FFMPEG_TARGET" ] && [ -f "$FFPROBE_TARGET" ]; then
    log_ok "ffmpeg already present, skipping ($TARGET_TRIPLE)"
else
    fetch "https://www.gyan.dev/ffmpeg/builds/packages/${FFMPEG_ASSET}" \
        "$TMP_DIR/$FFMPEG_ASSET" "$FFMPEG_SHA256"

    log_info "Extracting ffmpeg and ffprobe ..."
    unzip -o -q "$TMP_DIR/$FFMPEG_ASSET" -d "$TMP_DIR/ffmpeg"
    FFMPEG_ROOT="$(find "$TMP_DIR/ffmpeg" -maxdepth 2 -type d -name bin | head -1)"
    if [ -z "$FFMPEG_ROOT" ]; then
        log_err "unexpected ffmpeg archive layout: no bin/ directory"
        exit 1
    fi

    # The essentials build is static: a single .exe, no lib/*.dll to ship.
    install -m 755 "$FFMPEG_ROOT/ffmpeg.exe" "$FFMPEG_TARGET"
    install -m 755 "$FFMPEG_ROOT/ffprobe.exe" "$FFPROBE_TARGET"
    log_ok "Installed $FFMPEG_TARGET, $FFPROBE_TARGET"
fi

# --- 4. yt-dlp ------------------------------------------------------------
YTDLP_TARGET="$BIN_DIR/yt-dlp-$TARGET_TRIPLE.exe"
if [ -f "$YTDLP_TARGET" ] && verify_sha256 "$YTDLP_TARGET" "$YTDLP_SHA256" 2>/dev/null; then
    log_ok "yt-dlp already present, skipping ($TARGET_TRIPLE)"
else
    fetch "https://github.com/yt-dlp/yt-dlp/releases/download/${YTDLP_VERSION}/${YTDLP_ASSET}" \
        "$TMP_DIR/$YTDLP_ASSET" "$YTDLP_SHA256"
    install -m 755 "$TMP_DIR/$YTDLP_ASSET" "$YTDLP_TARGET"
    log_ok "Installed $YTDLP_TARGET"
fi

# --- 5. Silero VAD (shared with macOS, cross-platform weights) ------------
if [ -f "$SILERO_TARGET" ] && verify_sha256 "$SILERO_TARGET" "$SILERO_SHA256" 2>/dev/null; then
    log_ok "silero_vad_16k.safetensors already present, skipping"
else
    fetch "$SILERO_URL" "$TMP_DIR/silero.safetensors" "$SILERO_SHA256"
    install -m 644 "$TMP_DIR/silero.safetensors" "$SILERO_TARGET"
    log_ok "Installed $SILERO_TARGET"
fi

# --- 6. Resource manifest -------------------------------------------------
# Same purpose as the macOS script's MANIFEST.txt: it records what the build
# actually bundles. Generated, never hand-edited, and byte-stable across runs.
#
# It is the same file name on purpose — there is one `resources/` directory, and
# both platform scripts document it. It therefore lists whatever is present,
# including macOS binaries on a machine that has built both; the per-file
# triple in the name is what says which platform each belongs to.
MANIFEST="$RESOURCES_DIR/MANIFEST.txt"
{
    echo "# MonoSub 资源清单"
    echo "# 由 scripts/windows/prepare-resources.sh 生成，请勿手工编辑。"
    echo
    echo "## 文件"
    while IFS= read -r rel_path; do
        size=$(stat -c '%s' "$RESOURCES_DIR/$rel_path" 2>/dev/null || echo '?')
        printf '%-56s %s\n' "$rel_path" "$size"
    done < <(cd "$RESOURCES_DIR" && find . -type f ! -name MANIFEST.txt | sed 's|^\./||' | sort)
} > "$MANIFEST"
log_ok "已写入 $MANIFEST"

# --- 7. Smoke check -------------------------------------------------------
# Each binary must at least start on this machine. A missing DLL shows up here,
# not as a crash after the package is installed (spec §4.3).
log_info "Verifying bundled binaries ..."
"$CLI_TARGET" --version
"$SERVER_TARGET" --version
"$FFMPEG_TARGET" -version 2>&1 | head -1
"$YTDLP_TARGET" --version

log_ok "Windows 资源准备完毕。"
echo "    $BIN_DIR/audiocpp_cli-$TARGET_TRIPLE.exe"
echo "    $BIN_DIR/llama-server-$TARGET_TRIPLE.exe"
echo "    $BIN_DIR/ffmpeg-$TARGET_TRIPLE.exe"
echo "    $BIN_DIR/yt-dlp-$TARGET_TRIPLE.exe"
echo "    $SILERO_TARGET"
