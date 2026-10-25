#!/usr/bin/env bash

# MonoSub Publish Script —— 第 2 步：把 dist/ 里已构建好的产物发到 GitHub Releases。
#
# 这个脚本只做「发布」：不构建、不打包、不签名、不碰 versions 之外的任何本地文件。
# 产物来自第 1 步 —— macOS 用 scripts/release/release.sh，Windows 用
# scripts/windows/build-msix.ps1 —— 两边都落在同一个 dist/ 目录，这里只认文件。
#
# 拆成两步的理由：DMG 构建要十几分钟，上传只是一次 HTTP。构建失败不该在 GitHub 上
# 留下半成品 release；上传失败（网络、权限、release 已存在）也不该逼你重新构建。
#
# Usage:
#   ./scripts/publish/publish.sh                     发布当前版本的所有产物
#   ./scripts/publish/publish.sh --version 2.0.0     指定发布哪个已构建版本
#   ./scripts/publish/publish.sh --recreate          已有同名 release：删除后重建
#   ./scripts/publish/publish.sh --add-assets        已有同名 release：只补资产
#   ./scripts/publish/publish.sh --skip-bump         发布成功后不自增版本
#   ./scripts/publish/publish.sh --draft             建为草稿（草稿不自增版本）
#   ./scripts/publish/publish.sh --asset <path>      只上传指定文件（可重复）
#   ./scripts/publish/publish.sh --notes-file <path> 用文件内容作为 release notes
#
# 环境变量:
#   RELEASE_REPO  目标仓库，默认 leixlabs/monosub
#
# 依赖 GitHub CLI，且本机已执行过 `gh auth login`。

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
Usage: ./scripts/publish/publish.sh [options]

  (无参数)                 发布 dist/ 里当前版本的全部产物（*.dmg + *.msix）
  --version <x.y.z>        指定要发布哪个已构建版本（默认取 tauri.conf.json）
  --recreate               已有同名 release 时删除后重建
  --add-assets             已有同名 release 时只上传资产，不改 notes、不自增版本
  --skip-bump              发布成功后不自增版本号
  --draft                  建为草稿；草稿不自增版本号
  --asset <path>           只上传指定文件（可重复，给定时不再扫描 dist/）
  --notes-file <path>      用该文件内容作为 release notes（默认自动生成）
  -h, --help               显示这段说明

本步骤不构建任何东西。先产出产物：
  macOS    ./scripts/release/release.sh
  Windows  powershell -File scripts\windows\build-msix.ps1
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DIST_DIR="$PROJECT_ROOT/dist"
APP_NAME="monosub"
RELEASE_REPO="${RELEASE_REPO:-leixlabs/monosub}"

# --- 参数 ----------------------------------------------------------------
VERSION_ARG=""
RECREATE=false
ADD_ASSETS=false
SKIP_BUMP=false
DRAFT=false
NOTES_FILE=""
ASSET_ARGS=()

need_value() {
    # $1 = 参数名, $2 = 后面还剩几个参数
    if [ "${2:-0}" -lt 1 ]; then
        log_error "$1 需要一个值"
        exit 1
    fi
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)      need_value "$1" $(($# - 1)); VERSION_ARG="$2"; shift ;;
        --version=*)    VERSION_ARG="${1#*=}" ;;
        --asset)        need_value "$1" $(($# - 1)); ASSET_ARGS+=("$2"); shift ;;
        --asset=*)      ASSET_ARGS+=("${1#*=}") ;;
        --notes-file)   need_value "$1" $(($# - 1)); NOTES_FILE="$2"; shift ;;
        --notes-file=*) NOTES_FILE="${1#*=}" ;;
        --recreate)     RECREATE=true ;;
        --add-assets)   ADD_ASSETS=true ;;
        --skip-bump)    SKIP_BUMP=true ;;
        --draft)        DRAFT=true ;;
        -h|--help)      usage; exit 0 ;;
        *)
            log_error "未知参数: $1"
            usage
            exit 1
            ;;
    esac
    shift
done

if [ "$RECREATE" = true ] && [ "$ADD_ASSETS" = true ]; then
    log_error "--recreate 和 --add-assets 是互斥的：一个换产物，一个只补资产。"
    exit 1
fi

# --- 前置检查 ------------------------------------------------------------
cd "$PROJECT_ROOT" || { log_error "无法进入项目根目录: $PROJECT_ROOT"; exit 1; }
if [ ! -f "package.json" ] || [ ! -d "src-tauri" ]; then
    log_error "看起来不在项目根目录（需要 package.json 和 src-tauri/）: $PROJECT_ROOT"
    exit 1
fi

if ! command -v gh > /dev/null 2>&1; then
    log_error "没有找到 gh（GitHub CLI）。安装方式见 https://cli.github.com/"
    exit 1
fi
if ! gh auth status > /dev/null 2>&1; then
    log_error "gh 未登录。先执行 gh auth login。"
    exit 1
fi

source "$PROJECT_ROOT/scripts/utils/version-utils.sh"

echo "==============================================="
echo "MonoSub publish — 上传产物到 GitHub Releases"
echo "==============================================="
log_info "目标仓库: $RELEASE_REPO"

# --- 版本与 tag ----------------------------------------------------------
CONF_VERSION=$(get_tauri_version "$PROJECT_ROOT") || exit 1

if [ -n "$VERSION_ARG" ]; then
    VERSION="$VERSION_ARG"
    # publish 认「产物版本」而不是「配置里的版本」：release 之后版本号还没动，
    # 但两个平台先后发布时，先发的那次收尾已经把它自增了。此时 tauri.conf.json
    # 比 dist/ 里的产物新一版，按配置发就会 tag 与产物对不上。
    if [ "$VERSION" != "$CONF_VERSION" ]; then
        log_warn "按 $VERSION 发布；tauri.conf.json 里是 $CONF_VERSION（通常是上一轮发布后自增的下一版）。"
    fi
else
    VERSION="$CONF_VERSION"
fi

if ! validate_semver "$VERSION"; then
    log_error "版本 '$VERSION' 不是 x.y.z 形式"
    exit 1
fi

# tag 必须严格等于 v<版本号>：App 的「检查更新」拿自身 CARGO_PKG_VERSION 跟 release
# tag 比，带 -<sha> 之类的后缀永远不会相等，于是一直提示有新版本。
TAG="v$VERSION"
CURRENT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

log_info "版本: $VERSION"
log_info "Tag:  $TAG"
log_info "提交: $CURRENT_COMMIT"

# --- 收集资产 ------------------------------------------------------------
ASSETS=()
if [ ${#ASSET_ARGS[@]} -gt 0 ]; then
    for asset in "${ASSET_ARGS[@]}"; do
        if [ ! -f "$asset" ]; then
            log_error "指定的 --asset 不存在: $asset"
            exit 1
        fi
        ASSETS+=("$asset")
    done
else
    DIST_FILES=()
    shopt -s nullglob
    for file in "$DIST_DIR"/*.dmg "$DIST_DIR"/*.msix; do
        DIST_FILES+=("$file")
        # *.signed.msix 只用于本机 Add-AppxPackage 验证：自签名包给谁都不能用，
        # Store 会重新签名，GitHub 上应该只有未签名那份。
        case "$file" in
            *.signed.msix) continue ;;
        esac
        ASSETS+=("$file")
    done
    shopt -u nullglob

    # 别把上一版的产物挂到这一版的 tag 下：那是最难发现的一类错发，release 页面
    # 看起来正常，下载下来是旧代码。所以扫 dist/ 时要求至少有个文件名带当前版本。
    MATCHED=false
    for file in ${ASSETS[@]+"${ASSETS[@]}"}; do
        case "$(basename "$file")" in
            *"$VERSION"*) MATCHED=true ;;
        esac
    done
    if [ ${#ASSETS[@]} -gt 0 ] && [ "$MATCHED" = false ]; then
        log_error "$DIST_DIR 里没有文件名包含 $VERSION 的产物 —— 这多半是上一版留下来的。"
        log_error "现有文件："
        for file in "${DIST_FILES[@]}"; do
            echo "  - $(basename "$file")"
        done
        log_error "用 --version <上面文件名里的版本> 发布那一版，或先重新构建。"
        exit 1
    fi
fi

if [ ${#ASSETS[@]} -eq 0 ]; then
    log_error "$DIST_DIR 里没有可发布的产物（*.dmg / *.msix）。"
    log_error "先产出产物：macOS 跑 ./scripts/release/release.sh，Windows 跑 build-msix.ps1。"
    exit 1
fi

echo ""
log_info "将上传 ${#ASSETS[@]} 个文件:"
for file in "${ASSETS[@]}"; do
    size=$(du -h "$file" | cut -f1)
    echo "  - $(basename "$file")  ($size)"
    # 名字里没有版本号，多半是上一轮留下的旧产物被一起捞了进来。
    case "$(basename "$file")" in
        *"$VERSION"*) ;;
        *) log_warn "    文件名里没有 $VERSION，确认一下不是旧产物。" ;;
    esac
done

# --- Release notes -------------------------------------------------------
get_last_release_tag() {
    # 取最近发布过的 tag，跳过这次要发的那个（重发同一版本时要跟上一次比）。
    local tags t
    tags=$(gh release list --repo "$RELEASE_REPO" --limit 20 --json tagName \
        --jq '.[].tagName' 2>/dev/null) || tags=""
    for t in $tags; do
        if [ -n "$t" ] && [ "$t" != "$TAG" ]; then
            echo "$t"
            return 0
        fi
    done
    echo ""
}

generate_release_notes() {
    local version="$1"
    local current_commit="$2"
    local last_tag="$3"
    shift 3
    local assets=("$@")

    # 注意：这个函数被 $(...) 捕获，所有日志必须走 stderr。
    local changes=""
    if [ -n "$last_tag" ] && git rev-parse -q --verify "refs/tags/$last_tag" > /dev/null 2>&1; then
        changes=$(git log --pretty=format:"- %s" "$last_tag..HEAD" 2>/dev/null | head -20)
        echo "[INFO] 改动范围: $last_tag..HEAD" >&2
    else
        changes=$(git log --pretty=format:"- %s" -20 2>/dev/null)
        if [ -n "$last_tag" ]; then
            echo "[WARNING] 本地没有 $last_tag 这个 tag，改动列表退化为最近 20 条提交" >&2
        fi
    fi
    [ -n "$changes" ] || changes="- 与上一个 release 相比没有新提交"

    local table="" has_dmg=false has_msix=false
    local f base desc
    # ${assets[@]+...} 而不是直接 "${assets[@]}"：macOS 自带 bash 3.2 在 `set -u`
    # 下展开空数组会直接报 unbound variable。
    for f in ${assets[@]+"${assets[@]}"}; do
        base=$(basename "$f")
        case "$base" in
            *.dmg)  desc="macOS installer (Apple Silicon, ad-hoc signed)"; has_dmg=true ;;
            *.msix) desc="MSIX package, Windows 11 x64 (Microsoft Store)"; has_msix=true ;;
            *)      desc="attached file" ;;
        esac
        table="${table}| \`${base}\` | ${desc} |"$'\n'
    done

    cat << EOF
## What's New

- Release version $version
- Built from commit: \`$current_commit\`

## Changes

$changes

## Download

| File | Description |
|------|-------------|
$table
EOF

    if [ "$has_dmg" = true ]; then
        cat << 'EOF'

## macOS first launch

This build is **ad-hoc signed** (no Apple Developer ID, not notarized). macOS will
refuse the first launch with "cannot be opened because the developer cannot be
verified". To run it:

1. Open the .dmg and drag MonoSub to Applications.
2. Launch it once; when the warning appears, click **Done**.
3. Go to **System Settings → Privacy & Security**, scroll down, and click
   **Open Anyway** next to MonoSub.

This is required once per machine. It is not a sign of a broken download.
EOF
    fi

    if [ "$has_msix" = true ]; then
        cat << 'EOF'

## Windows note

This MSIX is prepared for Microsoft Store submission; the Store re-signs it on
publish. Installing the file attached here requires trusting a development
certificate, so Windows users should install MonoSub from the Store instead.
EOF
    fi

    printf '\n---\n\n'
}

echo ""
if [ -n "$NOTES_FILE" ]; then
    if [ ! -f "$NOTES_FILE" ]; then
        log_error "--notes-file 指向的文件不存在: $NOTES_FILE"
        exit 1
    fi
    RELEASE_NOTES=$(cat "$NOTES_FILE")
    log_info "Release notes 来自 $NOTES_FILE"
else
    log_info "生成 release notes..."
    LAST_RELEASE_TAG=$(get_last_release_tag)
    RELEASE_NOTES=$(generate_release_notes "$VERSION" "$CURRENT_COMMIT" "$LAST_RELEASE_TAG" "${ASSETS[@]}")
fi

# --- 是否已有同名 release ------------------------------------------------
RELEASE_EXISTS=false
EXISTING_RELEASE=""
if EXISTING_RELEASE=$(gh release view "$TAG" --repo "$RELEASE_REPO" 2>&1); then
    RELEASE_EXISTS=true
fi

if [ "$RELEASE_EXISTS" = true ]; then
    echo ""
    log_warn "Release $TAG 已经存在。"
    echo "$EXISTING_RELEASE" | head -12
    echo ""

    if [ "$ADD_ASSETS" = true ]; then
        log_info "只补资产（--add-assets）：notes 和版本号都不动"
        if gh release upload "$TAG" "${ASSETS[@]}" --repo "$RELEASE_REPO" --clobber; then
            log_success "资产已更新: https://github.com/$RELEASE_REPO/releases/tag/$TAG"
            exit 0
        fi
        log_error "上传失败"
        exit 1
    fi

    if [ "$RECREATE" = false ]; then
        if [ ! -t 0 ]; then
            log_error "非交互环境，无法询问。用 --recreate 换产物，或用 --add-assets 只补资产。"
            exit 1
        fi
        printf "  [r] 删除后重建（换产物）  [a] 只补资产  [c] 取消: "
        read -r -n 1 REPLY
        echo
        case "$REPLY" in
            [rR]) RECREATE=true ;;
            [aA]) ;;
            *)
                log_info "已取消，什么都没改。"
                exit 1
                ;;
        esac
        if [ "$RECREATE" = false ]; then
            log_info "只补资产"
            if gh release upload "$TAG" "${ASSETS[@]}" --repo "$RELEASE_REPO" --clobber; then
                log_success "资产已更新: https://github.com/$RELEASE_REPO/releases/tag/$TAG"
                exit 0
            fi
            log_error "上传失败"
            exit 1
        fi
    fi

    log_info "删除已有的 release $TAG（不动 tag 本身）..."
    if ! gh release delete "$TAG" --repo "$RELEASE_REPO" --yes; then
        log_error "删除失败，发布中止。"
        exit 1
    fi
fi

# --- 创建 release --------------------------------------------------------
# 没有本地 tag 时 gh 会在远端默认分支 HEAD 上创建它，而产物可能来自本地未推送的
# 提交 —— 那会让 release 指向的代码和 DMG 里的代码不是同一个。
if git rev-parse -q --verify "refs/tags/$TAG" > /dev/null 2>&1; then
    log_info "本地已有 tag $TAG，release 挂在该提交上"
else
    log_warn "本地没有 tag $TAG：gh 会在远端默认分支 HEAD 上创建它。"
    log_warn "如果这份产物是从未推送的提交构建的，先 push 再发布。"
fi

echo ""
log_info "创建 GitHub release $TAG 并上传资产..."
GH_ARGS=(release create "$TAG" --repo "$RELEASE_REPO" --title "$APP_NAME $TAG")
if [ "$DRAFT" = true ]; then
    GH_ARGS+=(--draft)
else
    GH_ARGS+=(--draft=false)
fi
GH_ARGS+=(--notes "$RELEASE_NOTES")
for file in "${ASSETS[@]}"; do
    GH_ARGS+=("$file")
done

if GH_OUTPUT=$(gh "${GH_ARGS[@]}" 2>&1); then
    echo ""
    echo "==============================================="
    log_success "Release $TAG 创建成功"
    echo "==============================================="
    log_success "https://github.com/$RELEASE_REPO/releases/tag/$TAG"
else
    echo ""
    log_error "创建 release 失败"
    echo "--- gh 输出 ---"
    echo "$GH_OUTPUT"
    echo "--- 结束 ---"
    exit 1
fi

# --- 版本自增 ------------------------------------------------------------
# 自增必须发生在这里（发布成功之后），不能挪到 release 阶段：tag 必须等于打进 App
# 的版本号，先自增就会让 DMG 的文件名比 tag 大一号——线上出过 tag v2.0.0 / 产物
# MonoSub-2.0.1.dmg 这种事。
echo ""
if [ "$DRAFT" = true ]; then
    log_info "草稿不自增版本号（草稿还没发布）。"
elif [ "$SKIP_BUMP" = true ]; then
    log_info "跳过版本自增（--skip-bump）。"
else
    log_info "自增版本号，为下次构建做准备..."
    if BUMP_RESULT=$(bash "$PROJECT_ROOT/scripts/utils/bump-version.sh" 2>&1); then
        log_success "版本已自增: $BUMP_RESULT"
        echo "  tauri.conf.json / Cargo.toml / package.json 都改了，记得提交。"
    else
        log_warn "版本自增失败（不影响已发布的 release）: $BUMP_RESULT"
    fi
fi

echo ""
echo "==============================================="
log_success "Publish 完成"
echo "==============================================="
