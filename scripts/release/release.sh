#!/usr/bin/env bash

# MonoSub Release Script —— 第 1 步：在本机产出可发布的产物。
#
# 这个脚本只做本地的事：读版本号 → 构建打包 → 收集并校验 DMG（落到 dist/）。
# 它不联网、不调用 gh、也不自增版本号。上传到 GitHub Releases 是第 2 步：
#
#   ./scripts/release/release.sh      # 1. 构建 + 本地校验，产物进 dist/
#   ./scripts/publish/publish.sh      # 2. 把 dist/ 里的产物发到 GitHub Releases
#
# 为什么拆成两步：DMG 构建要十几分钟，上传只是一次 HTTP 调用。构建失败不该在
# GitHub 上留下半成品 release；上传失败（网络、权限、release 已存在）也不该逼你
# 再构建一遍。两步各自可以单独重跑，中间只靠 dist/ 里的文件衔接。
#
# Usage:
#   ./scripts/release/release.sh                 # 构建并收集 DMG
#   ./scripts/release/release.sh --skip-build    # 跳过构建，只检查 dist/ 里已有的产物
#   ./scripts/release/release.sh --skip-package  # 旧参数名，等价于 --skip-build
#
# 分发签名：本机没有 Apple Developer 账号，产物是 ad-hoc 签名（见
# scripts/release/sign-app.sh 与 tauri.conf.json 的 bundle.macOS）。
# 不要设置 APPLE_SIGNING_IDENTITY / APPLE_ID，否则会复用本机证书或尝试公证而失败。

set -uo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

usage() {
    cat <<'EOF'
Usage: ./scripts/release/release.sh [--skip-build] [--skip-package]

  (无参数)          构建 + 打包 + 签名校验，产物落到 dist/
  --skip-build      跳过构建，只核对 dist/ 里已有的 DMG
  --skip-package    旧参数名，等价于 --skip-build
  -h, --help        显示这段说明

本步骤不联网、不发布。发布到 GitHub 用 ./scripts/publish/publish.sh。
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DIST_DIR="$PROJECT_ROOT/dist"
APP_NAME="MonoSub"

# --- 参数 ----------------------------------------------------------------
SKIP_BUILD=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --skip-build|--skip-package)
            # --skip-package 是拆分前的名字，保留以免旧习惯/旧笔记直接失效。
            SKIP_BUILD=true
            ;;
        --type=*)
            log_warn "--type 已废弃并被忽略（v2 起只有一种构建）。"
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_error "未知参数: $1"
            usage
            exit 1
            ;;
    esac
    shift
done

# --- 前置检查 ------------------------------------------------------------
cd "$PROJECT_ROOT" || { log_error "无法进入项目根目录: $PROJECT_ROOT"; exit 1; }
if [ ! -f "package.json" ] || [ ! -d "src-tauri" ]; then
    log_error "看起来不在项目根目录（需要 package.json 和 src-tauri/）: $PROJECT_ROOT"
    exit 1
fi

source "$PROJECT_ROOT/scripts/utils/version-utils.sh"

# --- 版本与 tag ----------------------------------------------------------
echo "==============================================="
echo "MonoSub release — 构建可发布产物"
echo "==============================================="

VERSION=$(get_tauri_version "$PROJECT_ROOT") || exit 1
if ! validate_semver "$VERSION"; then
    log_error "tauri.conf.json 里的版本 '$VERSION' 不是 x.y.z 形式"
    exit 1
fi
# tag 必须严格等于 v<版本号>：App 的「检查更新」拿自身 CARGO_PKG_VERSION 跟 release
# tag 比，多一个 -<sha> 后缀就永远不会相等，于是一直提示有新版本。构建阶段先把这个
# 约定算出来打印一次，publish 阶段再校验一遍。
TAG="v$VERSION"
CURRENT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
DMG_NAME="${APP_NAME}-${VERSION}.dmg"

log_info "版本: $VERSION（release tag 将是 $TAG）"
log_info "提交: $CURRENT_COMMIT"

# 构建产物来自本地工作区，但 gh 创建的 tag 指向远端 HEAD —— 两者可能不是一回事。
# 这里只提醒，不阻止：允许先构建再决定要不要提交。
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    log_warn "工作区有未提交改动：DMG 里包含这些改动，但 $TAG 会指向远端 HEAD，"
    log_warn "不含它们。要发布这次改动，先提交并推送。"
fi

# --- 构建 ----------------------------------------------------------------
if [ "$SKIP_BUILD" = true ]; then
    log_info "跳过构建（--skip-build）：直接使用 $DIST_DIR 里已有的产物"
else
    echo ""
    log_info "1/2 构建 + 打包 + 签名校验（scripts/build/build-macos-app.sh）..."
    if ! bash "$SCRIPT_DIR/../build/build-macos-app.sh"; then
        log_error "构建失败，release 中止。dist/ 里可能是上一次的旧产物，不要拿去发布。"
        exit 1
    fi
    log_success "构建完成"
fi

# --- 收集与核对产物 ------------------------------------------------------
echo ""
if [ "$SKIP_BUILD" = true ]; then
    log_info "核对 $DIST_DIR 里的产物..."
else
    log_info "2/2 核对 $DIST_DIR 里的产物..."
fi

if [ ! -d "$DIST_DIR" ]; then
    log_error "没有 $DIST_DIR 目录；先不带 --skip-build 跑一次，或先执行构建。"
    exit 1
fi

shopt -s nullglob
DMG_FILES=("$DIST_DIR"/*.dmg)
shopt -u nullglob

if [ ${#DMG_FILES[@]} -eq 0 ]; then
    log_error "$DIST_DIR 里没有 .dmg；这次没有可发布的东西。"
    exit 1
fi

for file in "${DMG_FILES[@]}"; do
    size=$(du -h "$file" | cut -f1)
    echo "  - $(basename "$file")  ($size)"
done

EXPECTED_DMG="$DIST_DIR/$DMG_NAME"
if [ ! -f "$EXPECTED_DMG" ]; then
    log_warn "没有找到预期名称 $DMG_NAME —— 版本号可能与产物不一致。"
    log_warn "发布时用 ./scripts/publish/publish.sh --version <产物里的版本> 指定。"
else
    log_success "产物与版本一致: $DMG_NAME"
fi

echo ""
echo "==============================================="
log_success "Release 完成：本地产物已就绪（未上传任何东西）"
echo "==============================================="
echo ""
echo "下一步 —— 发布到 GitHub Releases:"
echo "  ./scripts/publish/publish.sh"
echo ""
echo "  已有同名 release 时的处理方式:"
echo "  ./scripts/publish/publish.sh --recreate     # 删除后重建（换产物）"
echo "  ./scripts/publish/publish.sh --add-assets   # 只补资产，不动 notes"
echo ""
