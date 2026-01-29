#!/bin/bash
# MonoSub 依赖安装脚本
# 用于首次运行或依赖缺失时安装 Python 依赖

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

echo "╔════════════════════════════════════════╗"
echo "║   MonoSub 依赖安装脚本                 ║"
echo "╔════════════════════════════════════════╗"
echo ""

# 检查 Python
log_info "检查 Python 安装..."
if ! command -v python3 &> /dev/null; then
    log_error "Python 3 未安装"
    echo ""
    echo "请安装 Python 3.9 或更高版本:"
    echo "  方法 1: 从官网下载 https://www.python.org/downloads/"
    echo "  方法 2: 使用 Homebrew: brew install python@3.11"
    echo ""
    exit 1
fi

PYTHON_VERSION=$(python3 --version | cut -d' ' -f2)
log_success "Python $PYTHON_VERSION 已安装"

# 检查 pip
log_info "检查 pip..."
if ! command -v pip3 &> /dev/null; then
    log_warning "pip 未找到，尝试安装..."
    python3 -m ensurepip --upgrade
fi

log_success "pip 可用"

# 安装 Python 依赖
log_info "安装 Python 依赖包..."
echo ""
log_info "这可能需要几分钟时间，请耐心等待..."
echo ""

# 获取资源目录
if [ -f "$HOME/Library/Application Support/com.monosub.client/core-engine/requirements.txt" ]; then
    REQUIREMENTS_FILE="$HOME/Library/Application Support/com.monosub.client/core-engine/requirements.txt"
elif [ -f "$(dirname "$0")/../core-engine/requirements.txt" ]; then
    REQUIREMENTS_FILE="$(dirname "$0")/../core-engine/requirements.txt"
else
    log_error "未找到 requirements.txt 文件"
    exit 1
fi

log_info "使用 requirements 文件: $REQUIREMENTS_FILE"

# 安装依赖
pip3 install -r "$REQUIREMENTS_FILE" --user

echo ""
log_success "════════════════════════════════════════"
log_success "依赖安装完成!"
log_success "════════════════════════════════════════"
echo ""
log_info "您现在可以启动 MonoSub 应用了"
echo ""
