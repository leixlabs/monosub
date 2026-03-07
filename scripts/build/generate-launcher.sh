#!/bin/bash
# Usage: ./scripts/build/generate-launcher.sh --type=[lite|full]
# 根据构建模式生成对应的运行时入口脚本

set -e

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

BUILD_TYPE="lite"
for arg in "$@"; do
    case $arg in
        --type=*)
            BUILD_TYPE="${arg#*=}"
            shift
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
RESOURCES_DIR="$PROJECT_ROOT/src-tauri/resources"

ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    TARGET_TRIPLE="aarch64-apple-darwin"
else
    TARGET_TRIPLE="x86_64-apple-darwin"
fi

LAUNCHER_PATH="$RESOURCES_DIR/bin/monosub-engine-$TARGET_TRIPLE"
TEMPLATE_LITE="$SCRIPT_DIR/launcher-template.sh"

log_info "生成启动器脚本..."
if [ "$BUILD_TYPE" = "lite" ]; then
    if [ ! -f "$TEMPLATE_LITE" ]; then
        log_error "轻量版启动器模板丢失!"
        exit 1
    fi
    cp "$TEMPLATE_LITE" "$LAUNCHER_PATH"
else
    # Full版内联嵌入
    cat > "$LAUNCHER_PATH" << 'EOF'
#!/bin/bash
# MonoSub Python Engine Launcher (Full venv version)

# Get the real path of the script (resolve symlinks)
SOURCE="${BASH_SOURCE[0]}"
while [ -h "$SOURCE" ]; do
    DIR="$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )"
    SOURCE="$(readlink "$SOURCE")"
    [[ $SOURCE != /* ]] && SOURCE="$DIR/$SOURCE"
done
SCRIPT_DIR="$( cd -P "$( dirname "$SOURCE" )" >/dev/null 2>&1 && pwd )"
APP_CONTENTS="$(dirname "$SCRIPT_DIR")"
RESOURCES_DIR="$APP_CONTENTS/Resources/resources"

VENV_PYTHON="$RESOURCES_DIR/python-venv/bin/python"
VENV_LIB="$RESOURCES_DIR/python-venv/lib"
MAIN_PY="$RESOURCES_DIR/core-engine/main.py"

if [ ! -f "$VENV_PYTHON" ]; then
    echo "ERROR: Python not found: $VENV_PYTHON" >&2
    exit 1
fi
if [ ! -f "$MAIN_PY" ]; then
    echo "ERROR: main.py not found: $MAIN_PY" >&2
    exit 1
fi

export DYLD_LIBRARY_PATH="$VENV_LIB:${DYLD_LIBRARY_PATH:-}"
exec "$VENV_PYTHON" "$MAIN_PY" "$@"
EOF
fi

chmod +x "$LAUNCHER_PATH"
log_success "成功生成启动器: monosub-engine-$TARGET_TRIPLE"
