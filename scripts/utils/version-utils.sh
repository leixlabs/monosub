#!/bin/bash
# Usage: source ./scripts/utils/version-utils.sh
# Shared version helpers.
# Source of truth: src-tauri/tauri.conf.json version

get_tauri_version() {
    local project_root="$1"
    local tauri_conf="$project_root/src-tauri/tauri.conf.json"

    if [ ! -f "$tauri_conf" ]; then
        echo "tauri config not found: $tauri_conf" >&2
        return 1
    fi

    local version
    version=$(grep '"version"' "$tauri_conf" | head -1 | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
    if [ -z "$version" ]; then
        echo "failed to read version from: $tauri_conf" >&2
        return 1
    fi

    echo "$version"
}

validate_semver() {
    local version="$1"
    [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]
}

set_cargo_version() {
    local project_root="$1"
    local new_version="$2"
    local cargo_toml="$project_root/src-tauri/Cargo.toml"
    perl -i -pe 'if (!$done && s/^version\s*=\s*"[^"]+"/version = "'"$new_version"'"/) { $done = 1 }' "$cargo_toml"
}

set_tauri_conf_version() {
    local project_root="$1"
    local new_version="$2"
    local tauri_conf="$project_root/src-tauri/tauri.conf.json"
    perl -i -pe 'if (!$done && s/^\s*"version"\s*:\s*"[^"]+"/  "version": "'"$new_version"'"/) { $done = 1 }' "$tauri_conf"
}

set_package_json_version() {
    local project_root="$1"
    local new_version="$2"
    local package_json="$project_root/package.json"
    perl -i -pe 'if (!$done && s/^\s*"version"\s*:\s*"[^"]+"/  "version": "'"$new_version"'"/) { $done = 1 }' "$package_json"
}

sync_versions_from_tauri() {
    local project_root="$1"
    local version
    version=$(get_tauri_version "$project_root") || return 1

    set_cargo_version "$project_root" "$version"
    set_package_json_version "$project_root" "$version"
}

bump_patch_version_and_sync() {
    local project_root="$1"
    local current_version
    current_version=$(get_tauri_version "$project_root") || return 1

    if ! validate_semver "$current_version"; then
        echo "unsupported version format: $current_version (expected x.y.z)" >&2
        return 1
    fi

    local major="${BASH_REMATCH[1]}"
    local minor="${BASH_REMATCH[2]}"
    local patch="${BASH_REMATCH[3]}"
    local new_version="${major}.${minor}.$((patch + 1))"

    set_tauri_conf_version "$project_root" "$new_version"
    sync_versions_from_tauri "$project_root"

    echo "${current_version} -> ${new_version}"
}

# Backward-compatible wrappers for existing scripts.
get_cargo_version() {
    get_tauri_version "$1"
}

sync_versions_from_cargo() {
    sync_versions_from_tauri "$1"
}
