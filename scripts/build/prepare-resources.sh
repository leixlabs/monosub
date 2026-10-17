#!/bin/bash
# Usage: ./scripts/build/prepare-resources.sh
# Orchestrate all prerequisite scripts before the main Tauri compilation.
#
# Bundled resources are: bin/ffmpeg-*, bin/ffprobe-*, bin/audiocpp_cli-*, the
# ffmpeg dylibs in bin/lib/, bin/llama-server-* plus its lib*.dylib (flat in
# bin/, see fetch-llamacpp.sh for why), and
# models/silero_vad/silero_vad_16k.safetensors.

set -e

BLUE='\033[0;34m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"

log_info "1. 清理已废弃的 Python/uv/旧引擎资源..."

# Paths from the v1 Python/FunASR packaging. A checkout that still has them would
# otherwise be bundled by Tauri's `resources/**/*` glob. Enumerated explicitly so
# the fetched artifacts (bin/ffmpeg*, bin/ffprobe*, bin/audiocpp_cli*,
# bin/llama-server*, bin/lib/*.dylib, bin/libllama*.dylib, bin/libggml*.dylib,
# bin/libmtmd*.dylib, models/silero_vad/*) are never touched.
for obsolete in python python-venv core-engine lib uv; do
    if [ -e "$RESOURCES_DIR/$obsolete" ]; then
        log_info "  移除 $obsolete/"
        rm -rf "$RESOURCES_DIR/$obsolete"
    fi
done
rm -f "$RESOURCES_DIR"/bin/uv
rm -f "$RESOURCES_DIR"/bin/monosub-engine-*
rm -f "$RESOURCES_DIR"/monosub-engine-*

log_info "2. 检测 FFmpeg 二进制环境..."
bash "$SCRIPT_DIR/prepare-ffmpeg.sh"

log_info "3. 获取 audio.cpp 运行时 (CLI + Silero VAD)..."
bash "$SCRIPT_DIR/fetch-audiocpp.sh"

log_info "4. 获取 llama.cpp 运行时 (llama-server + dylib)..."
bash "$SCRIPT_DIR/fetch-llamacpp.sh"

log_info "5. 生成资源清单..."
# MANIFEST.txt documents what the build actually bundles. Generated rather than
# hand-maintained so it cannot drift; no timestamp, so re-runs are byte-identical.
# It walks resources/ recursively, so newly fetched artifacts (the flat llama
# dylibs included) need no change here.
MANIFEST="$RESOURCES_DIR/MANIFEST.txt"
{
    echo "# MonoSub 资源清单"
    echo "# 由 scripts/build/prepare-resources.sh 生成，请勿手工编辑。"
    echo
    echo "## 文件"
    while IFS= read -r rel_path; do
        size=$(stat -f '%z' "$RESOURCES_DIR/$rel_path" 2>/dev/null || echo '?')
        printf '%-56s %s\n' "$rel_path" "$size"
    done < <(cd "$RESOURCES_DIR" && find . -type f ! -name MANIFEST.txt | sed 's|^\./||' | sort)
} > "$MANIFEST"
log_info "  已写入 $MANIFEST"

log_info "✅ 资源整理准备完毕！进入打包环..."
exit 0
