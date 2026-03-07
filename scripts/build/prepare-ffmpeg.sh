#!/bin/bash
# Usage: ./scripts/build/prepare-ffmpeg.sh
# 下载/复制 macOS 下的 FFmpeg 和 FFprobe 二进制文件

set -e

# 颜色和日志函数
BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"
BIN_DIR="$RESOURCES_DIR/bin"
mkdir -p "$BIN_DIR"

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    TARGET_TRIPLE="aarch64-apple-darwin"
else
    TARGET_TRIPLE="x86_64-apple-darwin"
fi

FFMPEG_TARGET="$BIN_DIR/ffmpeg-$TARGET_TRIPLE"
FFPROBE_TARGET="$BIN_DIR/ffprobe-$TARGET_TRIPLE"

log_info "准备 FFmpeg for macOS ($ARCH / $TARGET_TRIPLE)..."

if [ -f "$FFMPEG_TARGET" ] && [ -f "$FFPROBE_TARGET" ]; then
    log_success "FFmpeg 已存在 ($TARGET_TRIPLE)，跳过下载"
    exit 0
fi

if command -v brew &> /dev/null; then
    BREW_FFMPEG=$(brew --prefix ffmpeg 2>/dev/null || echo "")
    if [ -n "$BREW_FFMPEG" ]; then
        log_info "从 Homebrew 复制 FFmpeg..."
        cp "$BREW_FFMPEG/bin/ffmpeg" "$FFMPEG_TARGET"
        cp "$BREW_FFMPEG/bin/ffprobe" "$FFPROBE_TARGET"
        chmod +x "$FFMPEG_TARGET" "$FFPROBE_TARGET"
        log_success "FFmpeg 复制完成"
        exit 0
    fi
fi

if command -v ffmpeg &> /dev/null && command -v ffprobe &> /dev/null; then
    log_info "从系统路径复制 FFmpeg..."
    cp "$(which ffmpeg)" "$FFMPEG_TARGET"
    cp "$(which ffprobe)" "$FFPROBE_TARGET"
    chmod +x "$FFMPEG_TARGET" "$FFPROBE_TARGET"
    log_success "FFmpeg 复制完成"
    exit 0
fi

log_error "未找到 FFmpeg，请先安装: brew install ffmpeg"
exit 1
