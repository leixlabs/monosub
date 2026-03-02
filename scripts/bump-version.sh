#!/bin/bash
# Auto bump patch version for packaging builds.
# Updates:
#   - src-tauri/tauri.conf.json
#   - src-tauri/Cargo.toml

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
TAURI_CONF="$PROJECT_ROOT/src-tauri/tauri.conf.json"
CARGO_TOML="$PROJECT_ROOT/src-tauri/Cargo.toml"

if [ ! -f "$TAURI_CONF" ]; then
  echo "tauri config not found: $TAURI_CONF" >&2
  exit 1
fi

if [ ! -f "$CARGO_TOML" ]; then
  echo "cargo manifest not found: $CARGO_TOML" >&2
  exit 1
fi

current_version=$(grep '"version"' "$TAURI_CONF" | head -1 | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')

if [[ ! "$current_version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  echo "unsupported version format: $current_version (expected x.y.z)" >&2
  exit 1
fi

major="${BASH_REMATCH[1]}"
minor="${BASH_REMATCH[2]}"
patch="${BASH_REMATCH[3]}"
new_version="${major}.${minor}.$((patch + 1))"

# Update first "version" key in tauri.conf.json.
perl -i -pe 's/"version"\s*:\s*"[^"]+"/"version": "'"$new_version"'"/ if !$done++' "$TAURI_CONF"

# Update first package version in Cargo.toml.
perl -i -pe 's/^version\s*=\s*"[^"]+"/version = "'"$new_version"'"/ if !$done++' "$CARGO_TOML"

echo "${current_version} -> ${new_version}"
