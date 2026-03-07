#!/bin/bash
# Usage: Embedded script, do not run directly.
# MonoSub Python Engine Launcher (Lite version)
# Auto-installs dependencies on first run using uv
# This file is used by generate-launcher.sh during build

set -e

# Get the real path of the script (resolve symlinks)
SOURCE="${BASH_SOURCE[0]}"
while [ -h "$SOURCE" ]; do
    DIR="$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )"
    SOURCE="$(readlink "$SOURCE")"
    [[ $SOURCE != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )"

# In Tauri app bundle:
# - Sidecars are executed from: Contents/MacOS/
# - Resources are in: Contents/Resources/resources/ (array syntax preserves "resources/" prefix)
# During execution, check where we are
if [[ "$SCRIPT_DIR" == *"/Contents/MacOS"* ]]; then
    # Running as sidecar from MacOS folder
    APP_CONTENTS="$(dirname "$SCRIPT_DIR")"
    RESOURCES_DIR="$APP_CONTENTS/Resources/resources"
elif [[ "$SCRIPT_DIR" == *"/Contents/Resources/resources/bin"* ]]; then
    # Running from Resources/resources/bin folder
    RESOURCES_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"
else
    # Development mode
    RESOURCES_DIR="$(dirname "$SCRIPT_DIR")"
fi

# Paths
PYTHON_BIN="$RESOURCES_DIR/python/bin/python3"
UV_BIN="$RESOURCES_DIR/bin/uv"
MAIN_PY="$RESOURCES_DIR/core-engine/main.py"
CORE_ENGINE_DIR="$RESOURCES_DIR/core-engine"
VENV_DIR="$CORE_ENGINE_DIR/.venv"

# Check if Python exists
if [ ! -f "$PYTHON_BIN" ]; then
    echo "ERROR: Python not found: $PYTHON_BIN" >&2
    exit 1
fi

if [ ! -f "$MAIN_PY" ]; then
    echo "ERROR: main.py not found: $MAIN_PY" >&2
    exit 1
fi

# Check if dependencies are installed (check for venv marker)
if [ ! -d "$VENV_DIR" ]; then
    echo "MSG:首次运行，正在安装 AI 依赖..." >&2
    echo "MSG:这可能需要 3-5 分钟，请耐心等待..." >&2
    echo "PROGRESS:0" >&2

    # Install dependencies using uv
    if [ -f "$UV_BIN" ]; then
        echo "MSG:使用 uv 安装依赖..." >&2
        # Change to core-engine dir so uv can find pyproject.toml
        cd "$CORE_ENGINE_DIR"
        # Create venv and sync dependencies
        "$UV_BIN" venv --python "$PYTHON_BIN" "$VENV_DIR" 2>&1 | while read -r line; do
            echo "MSG:$line" >&2
        done
        "$UV_BIN" sync 2>&1 | while read -r line; do
            echo "MSG:$line" >&2
        done
    else
        echo "MSG:使用 pip 安装依赖..." >&2
        # Fallback: create venv and install with pip
        "$PYTHON_BIN" -m venv "$VENV_DIR"
        "$VENV_DIR/bin/pip" install funasr modelscope torch torchaudio ffmpeg-python 2>&1 | while read -r line; do
            echo "MSG:$line" >&2
        done
    fi

    echo "MSG:依赖安装完成！" >&2
    echo "PROGRESS:5" >&2
fi

# Run the Python script using the venv Python
exec "$VENV_DIR/bin/python" "$MAIN_PY" "$@"
