#!/bin/bash

# MonoSub Release Script
# 用于构建和发布 MonoSub 到 GitHub Releases

# Usage:
#   ./scripts/release/release.sh                    # 默认构建 Lite 版本并发布
#   ./scripts/release/release.sh --type=full        # 构建 Full 版本并发布
#   ./scripts/release/release.sh --skip-package     # 跳过构建，仅发布已有文件
#
# 环境变量:
#   APPLE_ID              # Apple ID for notarization
#   APPLE_PASSWORD        # App-specific password for notarization
#   APPLE_TEAM_ID         # Apple Team ID for notarization
#   APPLE_SIGNING_IDENTITY # Code signing identity

# Configuration
APP_NAME="monosub"
DIST_DIR="dist"
RELEASE_REPO="leixlabs/monosub"

# Function to check if we're in the correct directory
check_directory() {
    if [ ! -f "package.json" ] || [ ! -d "src-tauri" ]; then
        echo "Error: This script must be run from the project root directory."
        echo "Expected directory structure:"
        echo "- package.json"
        echo "- src-tauri/"
        exit 1
    fi
    
    echo "Running from correct directory."
}

# Function to check if gh command is available
check_gh() {
    if ! command -v gh &> /dev/null; then
        echo "Error: gh command not found. Please install GitHub CLI."
        echo "Visit https://cli.github.com/ for installation instructions."
        exit 1
    fi
}

# Function to get version from tauri.conf.json
get_version_from_tauri_conf() {
    local VERSION
    VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed -E 's/.*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')
    
    if [ -z "$VERSION" ]; then
        echo "Error: Could not read version from src-tauri/tauri.conf.json"
        exit 1
    fi
    
    echo "$VERSION"
}

# Function to get current commit hash from code repo
get_current_commit_hash() {
    if command -v git &> /dev/null; then
        git rev-parse --short HEAD
    else
        echo "unknown"
    fi
}

# Function to get the last release commit hash from release repo
get_last_release_commit_hash() {
    local LAST_RELEASE_TAG
    LAST_RELEASE_TAG=$(gh release list --repo "$RELEASE_REPO" --limit 1 --json tagName --jq '.[0].tagName' 2>/dev/null)
    
    if [ -n "$LAST_RELEASE_TAG" ]; then
        # Extract commit hash from version format: v1.1.0-abc1234
        echo "$LAST_RELEASE_TAG" | sed 's/.*-\([a-f0-9]\{7,\}\)$/\1/'
    else
        echo ""
    fi
}

# Function to generate release notes
generate_release_notes() {
    local VERSION=$1
    local CURRENT_HASH=$2
    local LAST_HASH=$3
    
    local GIT_CHANGES
    
    if command -v git &> /dev/null; then
        if [ -n "$LAST_HASH" ] && git cat-file -e "$LAST_HASH" 2>/dev/null; then
            # Get commits from last release hash to current
            GIT_CHANGES=$(git log --pretty=format:"- %s" "$LAST_HASH".."$CURRENT_HASH" 2>/dev/null | head -20)
            echo "Changes from commit $LAST_HASH to $CURRENT_HASH:" >&2
        else
            # No previous release or hash not found, get recent commits
            GIT_CHANGES=$(git log --pretty=format:"- %s" -20 2>/dev/null)
            echo "No previous release found or invalid hash. Showing recent commits." >&2
        fi
        
        if [ -z "$GIT_CHANGES" ]; then
            GIT_CHANGES="- No new changes since last release"
        fi
    else
        GIT_CHANGES="- Could not retrieve git history"
    fi
    
    cat << EOF
## What's New

- Release version $VERSION
- Built from commit: \`$CURRENT_HASH\`

## Changes

$GIT_CHANGES

## Download

| File | Description |
|------|-------------|
| *.dmg | macOS Universal Installer |

See attached files below for installation.

---

EOF
}

# Main script
echo "==============================================="
echo "Starting Release Process for $APP_NAME"
echo "==============================================="

# Parse arguments
SKIP_PACKAGE=false
BUILD_TYPE="lite"
for arg in "$@"; do
    case $arg in
        --skip-package)
            SKIP_PACKAGE=true
            shift
            ;;
        --type=*)
            BUILD_TYPE="${arg#*=}"
            shift
            ;;
    esac
done

# Check if we're in the correct directory
check_directory

# Check if gh is available
check_gh

echo "Release Repository: $RELEASE_REPO"

# Get version from tauri.conf.json
BASE_VERSION=$(get_version_from_tauri_conf)
echo "Base Version: $BASE_VERSION"

# Get current commit hash from code repo
CURRENT_COMMIT=$(get_current_commit_hash)
echo "Current Commit: $CURRENT_COMMIT"

# Full version tag
VERSION="v${BASE_VERSION}-${CURRENT_COMMIT}"
echo "Release Version: $VERSION"

# Get last release commit hash from release repo
echo "Checking last release from $RELEASE_REPO..."
LAST_RELEASE_HASH=$(get_last_release_commit_hash)
if [ -n "$LAST_RELEASE_HASH" ]; then
    echo "Last Release Commit: $LAST_RELEASE_HASH"
else
    echo "No previous release found"
fi

# Run build-macos-app.sh to build and create installers if not skipping
if [ "$SKIP_PACKAGE" = false ]; then
    echo "Running build-macos-app.sh to build and package..."
    
    if [ -n "$BUILD_TYPE" ]; then
        echo "指定构建类型: $BUILD_TYPE"
        ./scripts/build/build-macos-app.sh $BUILD_TYPE
    else
        echo "使用默认构建类型"
        ./scripts/build/build-macos-app.sh
    fi
    
    if [ $? -ne 0 ]; then
        echo "Error: build-macos-app.sh failed. Aborting release."
        exit 1
    fi
else
    echo "Skipping build-macos-app.sh execution..."
fi

# Check if installer files were created
if [ ! -d "$DIST_DIR" ]; then
    echo "Error: Installer files not found in $DIST_DIR."
    exit 1
fi

# Collect installer files
INSTALLER_FILES=()
shopt -s nullglob
for file in "$DIST_DIR"/*.dmg; do
    if [ -f "$file" ]; then
        INSTALLER_FILES+=("$file")
    fi
done
shopt -u nullglob

if [ ${#INSTALLER_FILES[@]} -eq 0 ]; then
    echo "Error: No installer files found to upload."
    exit 1
fi

echo "Found installer files:"
for file in "${INSTALLER_FILES[@]}"; do
    echo "- $(basename "$file")"
done

# Generate release notes
echo "Generating release notes..."
RELEASE_NOTES=$(generate_release_notes "$VERSION" "$CURRENT_COMMIT" "$LAST_RELEASE_HASH")

# Check if release already exists
echo "Checking if release $VERSION already exists..."
EXISTING_RELEASE=$(gh release view "$VERSION" --repo "$RELEASE_REPO" 2>&1)
if [ $? -eq 0 ]; then
    echo "==============================================="
    echo "⚠ Release $VERSION already exists!"
    echo "==============================================="
    echo ""
    echo "Existing release details:"
    echo "$EXISTING_RELEASE"
    echo ""
    read -p "Do you want to delete and recreate? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Deleting existing release..."
        gh release delete "$VERSION" --repo "$RELEASE_REPO" --yes
        if [ $? -ne 0 ]; then
            echo "Failed to delete existing release. Aborting."
            exit 1
        fi
    else
        echo "Aborting release."
        exit 1
    fi
fi

# Create release with gh
echo "Creating GitHub release $VERSION..."

GH_OUTPUT=$(gh release create "$VERSION" \
    --repo "$RELEASE_REPO" \
    --title "$APP_NAME $VERSION" \
    --notes "$RELEASE_NOTES" \
    --draft=false \
    "${INSTALLER_FILES[@]}" 2>&1)
GH_EXIT_CODE=$?

if [ $GH_EXIT_CODE -eq 0 ]; then
    echo "==============================================="
    echo "✓ Release $VERSION created successfully!"
    echo "==============================================="
    echo "Release URL: https://github.com/$RELEASE_REPO/releases/tag/$VERSION"
else
    echo "==============================================="
    echo "⚠ Failed to create release."
    echo "==============================================="
    echo ""
    echo "--- Detailed Error Log ---"
    echo "$GH_OUTPUT"
    echo "--- End of Error Log ---"
    echo ""
    echo "Exit code: $GH_EXIT_CODE"
    exit 1
fi

# Bump version after successful release so next build gets the incremented number
echo ""
echo "Bumping version for next release..."
BUMP_RESULT=$(./scripts/release/bump-version.sh 2>&1)
if [ $? -eq 0 ]; then
    echo "✓ Version bumped: $BUMP_RESULT"
    echo "  Commit the version bump when ready for next release."
else
    echo "⚠ Version bump failed (non-fatal): $BUMP_RESULT"
fi

echo "==============================================="
echo "Release process completed."
echo "==============================================="
