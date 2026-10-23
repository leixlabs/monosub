#!/bin/bash
# Usage: ./scripts/windows/check-msix-package.sh <path-to-msix> [expected-version]
# Verifies the acceptance checklist from the Microsoft Store spec (§5.7) against
# a built package, so a bad package fails here rather than at certification.
#
# An MSIX is an OPC/ZIP container, so the contents can be inspected with `unzip`
# without unpacking it. That keeps this check dependency-free (no makeappx) and
# readable — the failure messages name the missing file directly.

set -euo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_ok() { echo -e "${GREEN}[OK]${NC} $1"; }
log_err() { echo -e "${RED}[ERROR]${NC} $1"; }

MSIX="${1:-}"
EXPECTED_VERSION="${2:-}"
TARGET_TRIPLE="x86_64-pc-windows-msvc"

if [ -z "$MSIX" ] || [ ! -f "$MSIX" ]; then
    log_err "usage: $0 <path-to-msix> [expected-version]"
    exit 1
fi

FAILED=0
fail() { log_err "$1"; FAILED=$((FAILED + 1)); }

log_info "Checking $(basename "$MSIX")"

# --- File name ------------------------------------------------------------
# Partner Center rejects an upload whose architecture it cannot infer, and the
# name is how a human picks the right file among several builds.
BASE="$(basename "$MSIX")"
case "$BASE" in
    *_x64.msix|*_x64.signed.msix) log_ok "File name declares x64: $BASE" ;;
    *) fail "File name does not end in _x64.msix: $BASE" ;;
esac

# --- Contents -------------------------------------------------------------
ENTRIES="$(unzip -Z1 "$MSIX")"

require() {
    local path="$1" what="$2"
    if grep -qxF "$path" <<< "$ENTRIES"; then
        log_ok "$what: $path"
    else
        fail "$what missing from the package: $path"
    fi
}

require "AppxManifest.xml" "Manifest"
require "monosub.exe" "Entry point"

# The binaries are the spec §4 blocking item: a package without them installs
# and then fails on the first transcription.
require "resources/bin/audiocpp_cli-$TARGET_TRIPLE.exe" "audio.cpp CLI"
require "resources/bin/ffmpeg-$TARGET_TRIPLE.exe" "FFmpeg"
require "resources/bin/ffprobe-$TARGET_TRIPLE.exe" "FFprobe"
require "resources/bin/llama-server-$TARGET_TRIPLE.exe" "llama.cpp server"
require "resources/bin/yt-dlp-$TARGET_TRIPLE.exe" "yt-dlp"
require "resources/models.json" "Model catalogue"
require "resources/models/silero_vad/silero_vad_16k.safetensors" "Silero VAD model"

# The CLI binary is deliberately excluded (spec §5.2); catching it here keeps a
# future resource script change from silently shipping a second entry point.
if grep -q "resources/bin/monosub-cli-" <<< "$ENTRIES"; then
    fail "monosub-cli must not be packaged (the Store package has one entry point)"
fi

# --- DLLs ---------------------------------------------------------------
# A missing DLL only shows up as a crash at run time, so require at least one of
# each family the two engines need.
dll_count=$(grep -c 'resources/bin/.*\.dll$' <<< "$ENTRIES" || true)
if [ "$dll_count" -ge 10 ]; then
    log_ok "Runtime DLLs present ($dll_count)"
else
    fail "Only $dll_count DLL(s) in the package; the engines cannot start"
fi
if ! grep -q '^resources/bin/llama-server-impl\.dll$' <<< "$ENTRIES"; then
    fail "llama-server-impl.dll missing (llama-server is a thin launcher over it)"
fi
if ! grep -q '^resources/bin/vcruntime140\.dll$' <<< "$ENTRIES"; then
    fail "vcruntime140.dll missing (the MSVC runtime is not assumed on the target)"
fi

# --- Manifest identity ---------------------------------------------------
# The version is compared against tauri.conf.json by the build script; here it
# only has to be four-part, which is the form the Store requires.
MANIFEST_XML="$(unzip -p "$MSIX" AppxManifest.xml)"

identity_version="$(grep -oE 'Version="[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+"' <<< "$MANIFEST_XML" | head -1 | sed -E 's/Version="([^"]+)"/\1/')"
if [ -n "$identity_version" ]; then
    log_ok "Manifest version: $identity_version"
    if [ -n "$EXPECTED_VERSION" ] && [ "$identity_version" != "$EXPECTED_VERSION" ]; then
        fail "Manifest version $identity_version does not match expected $EXPECTED_VERSION"
    fi
else
    fail "Manifest has no four-part Version"
fi

if grep -q 'x64' <<< "$MANIFEST_XML" && ! grep -q 'ProcessorArchitecture="x86"' <<< "$MANIFEST_XML"; then
    log_ok "Processor architecture: x64"
else
    fail "Manifest does not declare x64 (or declares x86)"
fi

# WebView2 must NOT be declared as a PackageDependency: it ships as an Evergreen
# runtime, not an MSIX framework, so the declaration makes the package fail to
# install with 0x80073CF3 even on machines that have WebView2.
if grep -q 'Microsoft.Win32WebView2' <<< "$MANIFEST_XML"; then
    fail "Manifest declares Microsoft.Win32WebView2 as a framework dependency; WebView2 is an Evergreen runtime, and this makes Add-AppxPackage fail with 0x80073CF3"
else
    log_ok "No WebView2 framework dependency (correct: it is an Evergreen runtime)"
fi

if grep -q 'Executable="monosub.exe"' <<< "$MANIFEST_XML"; then
    log_ok "Entry point pinned to monosub.exe"
else
    fail "Manifest entry point is not monosub.exe"
fi

if grep -q 'runFullTrust' <<< "$MANIFEST_XML"; then
    log_ok "runFullTrust capability declared"
else
    fail "runFullTrust missing; a desktop app launched from the Store needs it"
fi

# --- Result --------------------------------------------------------------
if [ "$FAILED" -gt 0 ]; then
    log_err "$FAILED check(s) failed"
    exit 1
fi
log_ok "All package checks passed"
