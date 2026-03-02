#!/bin/bash
# Usage: ./scripts/setup/setup-dev-env.sh
# Python 环境设置脚本
# 使用 uv 进行快速包管理

set -e

echo "🐍 Setting up Python environment with uv..."

# 检查 uv 是否安装
if ! command -v uv &> /dev/null; then
    echo "❌ uv is not installed."
    echo "📦 Install with: curl -LsSf https://astral.sh/uv/install.sh | sh"
    echo "   Or use Homebrew: brew install uv"
    exit 1
fi

echo "✅ Found uv $(uv --version)"

# 检查 FFmpeg 是否安装
if ! command -v ffmpeg &> /dev/null; then
    echo "❌ FFmpeg is not installed."
    echo "📦 Install with: brew install ffmpeg"
    exit 1
fi

echo "✅ FFmpeg is installed"

# 进入 core-engine 目录
cd "$(dirname "$0")/../../core-engine"

# 使用 uv sync 安装依赖（会自动创建虚拟环境）
echo "📦 Installing Python dependencies with uv..."
uv sync

echo ""
echo "✅ Setup complete!"
echo ""
echo "📝 To run commands in the virtual environment:"
echo "   cd core-engine"
echo "   uv run python main.py <video_file> <output_file>"
echo ""
echo "🔍 Or activate the virtual environment:"
echo "   source .venv/bin/activate"
echo "   python main.py <video_file> <output_file>"
echo ""
