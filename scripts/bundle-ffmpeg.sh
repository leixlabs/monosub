#!/bin/bash
# Creates self-contained ffmpeg/ffprobe binaries for macOS app bundle distribution.
# Copies homebrew binaries + their dylib dependencies, then rewrites all library
# paths to use @loader_path so they work without homebrew installed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BIN_DIR="$PROJECT_DIR/src-tauri/resources/bin"
LIB_DIR="$BIN_DIR/lib"

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    TARGET="aarch64-apple-darwin"
elif [ "$ARCH" = "x86_64" ]; then
    TARGET="x86_64-apple-darwin"
else
    echo "Error: Unsupported architecture: $ARCH"
    exit 1
fi

FFMPEG_BIN=$(which ffmpeg 2>/dev/null || true)
FFPROBE_BIN=$(which ffprobe 2>/dev/null || true)

if [ -z "$FFMPEG_BIN" ] || [ -z "$FFPROBE_BIN" ]; then
    echo "Error: ffmpeg and ffprobe must be installed (brew install ffmpeg)"
    exit 1
fi

echo "==> Bundling ffmpeg/ffprobe for $TARGET"
echo "    ffmpeg:  $FFMPEG_BIN"
echo "    ffprobe: $FFPROBE_BIN"

# Clean and create directories
rm -rf "$LIB_DIR"
mkdir -p "$BIN_DIR" "$LIB_DIR"

# Copy binaries
cp "$FFMPEG_BIN" "$BIN_DIR/ffmpeg-$TARGET"
cp "$FFPROBE_BIN" "$BIN_DIR/ffprobe-$TARGET"
chmod 755 "$BIN_DIR/ffmpeg-$TARGET" "$BIN_DIR/ffprobe-$TARGET"

# Collect all non-system dylib dependencies recursively
collect_dylibs() {
    local binary="$1"
    otool -L "$binary" 2>/dev/null | tail -n +2 | awk '{print $1}' | while read -r lib; do
        # Skip system libs
        [[ "$lib" == /System/* ]] && continue
        [[ "$lib" == /usr/lib/* ]] && continue
        [[ "$lib" == @* ]] && continue

        local lib_name
        lib_name=$(basename "$lib")

        # Skip if already copied
        [ -f "$LIB_DIR/$lib_name" ] && continue

        if [ -f "$lib" ]; then
            echo "    Copying $lib_name"
            cp "$lib" "$LIB_DIR/$lib_name"
            chmod 755 "$LIB_DIR/$lib_name"
            # Recurse into this library's dependencies
            collect_dylibs "$LIB_DIR/$lib_name"
        else
            echo "    Warning: $lib not found"
        fi
    done
}

echo "==> Collecting dynamic libraries..."
collect_dylibs "$BIN_DIR/ffmpeg-$TARGET"
collect_dylibs "$BIN_DIR/ffprobe-$TARGET"

# Rewrite library paths in a binary/dylib
fix_references() {
    local file="$1"
    local prefix="$2"  # @loader_path/lib for binaries, @loader_path for dylibs

    otool -L "$file" 2>/dev/null | tail -n +2 | awk '{print $1}' | while read -r lib; do
        [[ "$lib" == /System/* ]] && continue
        [[ "$lib" == /usr/lib/* ]] && continue
        [[ "$lib" == @* ]] && continue

        local lib_name
        lib_name=$(basename "$lib")
        install_name_tool -change "$lib" "$prefix/$lib_name" "$file" 2>/dev/null || true
    done
}

echo "==> Rewriting library paths in binaries..."
fix_references "$BIN_DIR/ffmpeg-$TARGET" "@loader_path/lib"
fix_references "$BIN_DIR/ffprobe-$TARGET" "@loader_path/lib"

echo "==> Rewriting library paths in dylibs..."
for lib in "$LIB_DIR"/*.dylib; do
    local_name=$(basename "$lib")
    # Fix the library's own install name
    install_name_tool -id "@loader_path/$local_name" "$lib" 2>/dev/null || true
    # Fix references to other libraries
    fix_references "$lib" "@loader_path"
done

# Remove code signatures (they're invalidated by install_name_tool changes)
echo "==> Re-signing binaries..."
codesign --force --sign - "$BIN_DIR/ffmpeg-$TARGET" 2>/dev/null || true
codesign --force --sign - "$BIN_DIR/ffprobe-$TARGET" 2>/dev/null || true
for lib in "$LIB_DIR"/*.dylib; do
    codesign --force --sign - "$lib" 2>/dev/null || true
done

# Verify
echo ""
echo "==> Verifying ffmpeg-$TARGET dependencies:"
otool -L "$BIN_DIR/ffmpeg-$TARGET" | grep -v /System | grep -v /usr/lib | head -20

echo ""
echo "==> Verifying ffprobe-$TARGET dependencies:"
otool -L "$BIN_DIR/ffprobe-$TARGET" | grep -v /System | grep -v /usr/lib | head -20

echo ""
echo "==> Testing execution..."
"$BIN_DIR/ffmpeg-$TARGET" -version 2>&1 | head -1
"$BIN_DIR/ffprobe-$TARGET" -version 2>&1 | head -1

echo ""
echo "==> Done! Bundled files:"
ls -lh "$BIN_DIR/ffmpeg-$TARGET" "$BIN_DIR/ffprobe-$TARGET"
echo "    Libraries:"
ls -lh "$LIB_DIR/"
