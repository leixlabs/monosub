#!/bin/bash
# Usage: ./scripts/release/bump-version.sh
# Auto bump patch version for packaging builds.
# Source of truth: src-tauri/tauri.conf.json
# Sync targets:
#   - src-tauri/Cargo.toml
#   - package.json
#   - core-engine/pyproject.toml
#   - src-tauri/resources/core-engine/pyproject.toml

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
source "$SCRIPT_DIR/../utils/version-utils.sh"

bump_patch_version_and_sync "$PROJECT_ROOT"
