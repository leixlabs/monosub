#!/bin/bash
# Usage: ./scripts/windows/release-msix.sh [--tag <tag>] [--skip-build] [--update-tag]
# Builds the Windows MSIX and attaches it to a GitHub release.
#
#   ./scripts/windows/release-msix.sh --tag v2.0.0 --update-tag
#
# Two different releases exist for a tag and they are easy to confuse:
#   * the git tag, which points at a commit
#   * the GitHub release, which is the page that holds the assets
# Moving a tag does not move the release, and adding an asset to a release does
# not change the tag. This script can do either, and says which.
#
# --update-tag rewrites the tag to HEAD and force-pushes it. That is a history
# rewrite of a published ref: anyone who already fetched the old tag keeps the
# old commit until they re-fetch. It is what you want when the release notes say
# "built from <sha>" and the artifact was built from a newer commit, and it is
# never what you want for a tag other people have based work on.

set -euo pipefail

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
RELEASE_REPO="${RELEASE_REPO:-leixlabs/monosub}"
OUTPUT_DIR="$PROJECT_ROOT/dist"

TAG=""
SKIP_BUILD=false
UPDATE_TAG=false
# `for arg in "$@"` expands the list once, so `shift` inside it does nothing —
# `--tag v2.0.0` would leave v2.0.0 to be parsed as a second flag. Consume the
# arguments one at a time instead.
while [ "$#" -gt 0 ]; do
    case "$1" in
        --tag)
            if [ "$#" -lt 2 ]; then
                log_error "--tag needs a value (e.g. --tag v2.0.0)"
                exit 1
            fi
            TAG="$2"
            shift 2
            ;;
        --tag=*) TAG="${1#*=}"; shift ;;
        --skip-build) SKIP_BUILD=true; shift ;;
        --update-tag) UPDATE_TAG=true; shift ;;
        *) log_error "Unknown argument: $1"; exit 1 ;;
    esac
done

cd "$PROJECT_ROOT"

command -v gh > /dev/null 2>&1 || { log_error "gh CLI is required"; exit 1; }

# --- 1. Version and tag ---------------------------------------------------
source "$SCRIPT_DIR/../utils/version-utils.sh"
VERSION=$(get_tauri_version "$PROJECT_ROOT")
[ -n "$TAG" ] || TAG="v$VERSION"
MANIFEST_VERSION="${VERSION}.0"
MSIX_NAME="monosub_${MANIFEST_VERSION}_x64.msix"

log_info "Version $VERSION, tag $TAG"

# The app's own update check compares its CARGO_PKG_VERSION against the release
# tag, so a tag that is not exactly v<version> makes "Check for Updates" report
# an update forever (see release.sh). The same rule applies to this path.
if [ "$TAG" != "v$VERSION" ]; then
    log_error "Tag $TAG does not match the app version v$VERSION."
    log_error "The app compares its own version to the release tag, so these must agree."
    exit 1
fi

# --- 2. Build ------------------------------------------------------------
if [ "$SKIP_BUILD" = false ]; then
    log_info "1. Building the MSIX ..."
    bash "$SCRIPT_DIR/build-msix.sh"
else
    log_info "1. Skipping build (--skip-build)"
    if [ ! -f "$OUTPUT_DIR/$MSIX_NAME" ]; then
        log_error "No $OUTPUT_DIR/$MSIX_NAME to publish"
        exit 1
    fi
fi

# The signed copy is a local-install artifact; the Store and GitHub get the
# unsigned one, because a self-signed package cannot be submitted or trusted by
# anyone who has not installed this machine's certificate.
if [ ! -f "$OUTPUT_DIR/$MSIX_NAME" ]; then
    log_error "Expected $OUTPUT_DIR/$MSIX_NAME"
    exit 1
fi

# --- 3. Optional tag move ------------------------------------------------
if [ "$UPDATE_TAG" = true ]; then
    if [ -n "$(git status --porcelain)" ]; then
        log_error "Working tree is dirty. Commit the packaging scripts first:"
        git status --short
        exit 1
    fi

    OLD_SHA="$(git rev-parse --short "$TAG" 2>/dev/null || echo "")"
    NEW_SHA="$(git rev-parse --short HEAD)"
    log_info "2. Moving $TAG from ${OLD_SHA:-<none>} to $NEW_SHA ..."

    if [ "$OLD_SHA" = "$NEW_SHA" ]; then
        log_success "$TAG already points at HEAD; nothing to move"
    else
        # An annotated tag needs a committer identity. Without one git fails with
        # "Committer identity unknown" halfway through — after the old tag has
        # already been deleted — so check first rather than leaving a broken tag.
        if ! git var GIT_COMMITTER_IDENT > /dev/null 2>&1; then
            log_error "No git committer identity configured; an annotated tag cannot be created."
            log_error "Set one with: git config user.name '...' && git config user.email '...'"
            exit 1
        fi

        # An annotated tag is what `gh release create` produces and what the
        # existing v2.0.0 is, so recreate it the same way instead of with
        # `git tag -f`, which would turn it into a lightweight tag.
        git tag -d "$TAG" > /dev/null 2>&1 || true
        git tag -a "$TAG" -m "MonoSub $VERSION"
        git push origin "refs/tags/$TAG" --force
        log_success "$TAG now points at $NEW_SHA"
        log_warning "Anyone who already fetched the old tag keeps the old commit until they re-fetch."
    fi
else
    TAG_SHA="$(git rev-parse --short "$TAG" 2>/dev/null || echo "")"
    if [ -z "$TAG_SHA" ]; then
        log_error "Tag $TAG does not exist. Run with --update-tag to create it."
        exit 1
    fi
    log_info "2. Leaving $TAG where it is ($TAG_SHA)"
fi

# --- 4. Upload -----------------------------------------------------------
# `--clobber` replaces an asset of the same name, so re-running after a fix does
# not fail on the second upload. The release itself must already exist: creating
# one here would hide the DMG that belongs to the same version.
log_info "3. Uploading $MSIX_NAME to release $TAG ..."
if ! gh release view "$TAG" --repo "$RELEASE_REPO" > /dev/null 2>&1; then
    log_error "Release $TAG does not exist in $RELEASE_REPO."
    log_error "Create it first, or publish through the normal release flow."
    exit 1
fi

gh release upload "$TAG" "$OUTPUT_DIR/$MSIX_NAME" \
    --repo "$RELEASE_REPO" \
    --clobber

# --- 5. Refresh the notes ------------------------------------------------
# The asset table in the existing notes only lists the DMG. Appending rather than
# rewriting keeps the hand-written sections (macOS first launch, etc.) intact.
log_info "4. Updating the release notes with the Windows package ..."
CURRENT_NOTES="$(gh release view "$TAG" --repo "$RELEASE_REPO" --json body --jq '.body')"

# When the tag moved, the "Built from commit" line still names the old commit —
# and it is the one line a reader uses to tell which code the assets came from.
if [ "$UPDATE_TAG" = true ]; then
    NEW_SHA="$(git rev-parse --short HEAD)"
    CURRENT_NOTES="$(sed -E "s/(Built from commit: \`)[a-f0-9]+(\`)/\1${NEW_SHA}\2/" <<< "$CURRENT_NOTES")"
fi

if grep -q "$MSIX_NAME" <<< "$CURRENT_NOTES"; then
    log_success "Release notes already mention $MSIX_NAME"
    if [ "$UPDATE_TAG" = true ]; then
        gh release edit "$TAG" --repo "$RELEASE_REPO" --notes "$CURRENT_NOTES"
        log_success "Commit reference refreshed"
    fi
else
    # The table lists `*.dmg` as a glob; add the MSIX as a row and note that the
    # Windows build is a Store package rather than a standalone installer.
    NEW_NOTES="$CURRENT_NOTES

## Windows (Microsoft Store package)

| File | Description |
|------|-------------|
| $MSIX_NAME | MSIX package, Windows 11 x64 |

This package is prepared for Microsoft Store submission; the Store re-signs it on
publish. Installing the file attached here requires trusting a development
certificate, so Windows users should install MonoSub from the Store instead.

---

"
    gh release edit "$TAG" --repo "$RELEASE_REPO" --notes "$NEW_NOTES"
    log_success "Release notes updated"
fi

log_success "Done: https://github.com/$RELEASE_REPO/releases/tag/$TAG"
