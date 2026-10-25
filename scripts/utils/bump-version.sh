#!/usr/bin/env bash
# Usage: ./scripts/utils/bump-version.sh
# Auto bump patch version for packaging builds.
#
# 调它的是「发布成功之后」那一步（scripts/publish/publish.sh），不是构建脚本：
# tag 必须等于打进 App 的版本号，构建前自增会让产物名比 tag 大一号。
# 两个平台的发布都改同一份 tauri.conf.json，所以放在 utils/ 而不是某个平台目录。
#
# Source of truth: src-tauri/tauri.conf.json
# Sync targets:
#   - src-tauri/Cargo.toml
#   - package.json

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
source "$SCRIPT_DIR/version-utils.sh"

bump_patch_version_and_sync "$PROJECT_ROOT"
