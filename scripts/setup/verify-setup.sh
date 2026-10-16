#!/bin/bash
# Usage: ./scripts/setup/verify-setup.sh
# 环境验证脚本
# 检查所有必需的工具是否已安装

set -e

echo "🔍 Verifying development environment..."
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查函数
check_command() {
    local cmd=$1
    local name=$2
    local install_hint=$3

    if command -v $cmd &> /dev/null; then
        local version=$($cmd --version 2>&1 | head -n 1)
        echo -e "${GREEN}✅ $name${NC} - $version"
        return 0
    else
        echo -e "${RED}❌ $name${NC} - Not installed"
        if [ -n "$install_hint" ]; then
            echo -e "   ${YELLOW}Install: $install_hint${NC}"
        fi
        return 1
    fi
}

errors=0

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📦 Package Managers"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

check_command "bun" "Bun" "curl -fsSL https://bun.sh/install | bash" || ((errors++))
check_command "uv" "uv" "curl -LsSf https://astral.sh/uv/install.sh | sh" || ((errors++))

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🔧 Required Tools"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

check_command "cargo" "Rust" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" || ((errors++))
check_command "ffmpeg" "FFmpeg" "brew install ffmpeg" || ((errors++))
check_command "python3" "Python 3" "brew install python3" || ((errors++))

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📁 Project Dependencies"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 检查前端依赖
if [ -d "node_modules" ]; then
    echo -e "${GREEN}✅ Frontend dependencies${NC} - node_modules exists"
else
    echo -e "${YELLOW}⚠️  Frontend dependencies${NC} - Run 'bun install'"
    ((errors++))
fi

# 检查 Python 虚拟环境
if [ -d "core-engine/.venv" ] || [ -d "core-engine/venv" ]; then
    echo -e "${GREEN}✅ Python virtual environment${NC} - Environment exists"
else
    echo -e "${YELLOW}⚠️  Python virtual environment${NC} - Run './scripts/setup/setup-dev-env.sh'"
    ((errors++))
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📊 Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ $errors -eq 0 ]; then
    echo -e "${GREEN}✅ All checks passed!${NC}"
    echo ""
    echo "You're ready to start development:"
    echo "  bun run tauri dev    # Tauri app (starts Vite via beforeDevCommand)"
    echo "  bun run dev          # Vite only, in a browser"
    exit 0
else
    echo -e "${RED}❌ Found $errors issue(s)${NC}"
    echo ""
    echo "Please fix the issues above and run this script again."
    exit 1
fi
