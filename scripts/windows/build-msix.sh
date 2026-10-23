#!/bin/bash
# Usage: ./scripts/windows/build-msix.sh [--sign] [--skip-resources]
# Builds the MonoSub MSIX package for the Microsoft Store (spec
# docs/tech-spec/microsoft-store-publish-spec.md §5).
#
# Run from Git Bash on Windows 11 x64. Produces, in dist/:
#   monosub_<version>_x64.msix           unsigned, the file Partner Center accepts
#   monosub_<version>_x64.signed.msix    dev-signed, only with --sign, for local install
#
# Why unsigned by default: the Store re-signs every package it ships, and a
# package carrying a self-signed certificate is rejected at submission. The dev
# certificate exists only so `Add-AppxPackage` can be run on this machine
# (spec §5.6); it is not a submission artifact.
#
# Identity values must match the app reserved in Partner Center, otherwise the
# upload is rejected as "package identity does not match". Override them with
# MONOSUB_MSIX_NAME / MONOSUB_MSIX_PUBLISHER when Partner Center shows different
# values than the defaults below.

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
OUTPUT_DIR="$PROJECT_ROOT/dist"

# Staging must NOT be dist/: Vite writes the frontend there, and
# release.sh collects artifacts from there too. `build/` is gitignored.
STAGE_DIR="$PROJECT_ROOT/build/msix"
STAGE_APP_DIR="$STAGE_DIR/app"

# Identity (spec §5.2 / appendix C). The publisher DN is what Partner Center
# shows as Package/Identity/Publisher.
PACKAGE_NAME="${MONOSUB_MSIX_NAME:-monosub}"
PUBLISHER="${MONOSUB_MSIX_PUBLISHER:-CN=leix}"
DISPLAY_NAME="MonoSub"
DESCRIPTION="MonoSub - local voice to article"

SIGN=false
SKIP_RESOURCES=false
for arg in "$@"; do
    case $arg in
        --sign) SIGN=true ;;
        --skip-resources) SKIP_RESOURCES=true ;;
        *) log_error "Unknown argument: $arg"; exit 1 ;;
    esac
done

# --- 0. Environment -------------------------------------------------------
# `case` is the portable way to match the uname prefix: inside `[ ]`, a glob on
# the right-hand side of `!=` is compared literally, so `!= "MINGW"*` would
# reject the very shell this script runs in.
case "$(uname -s)" in
    MSYS_NT*|MINGW*|CYGWIN*) ;;
    *)
        log_error "This script only runs on Windows (Git Bash). macOS uses scripts/build/build-macos-app.sh."
        exit 1
        ;;
esac

# Git Bash inherits the machine PATH only at login; a tool installed during this
# session may not be on it yet. Probe the usual locations before failing. bun is
# installed by `npm install -g bun`, so it lands in npm's global prefix, which is
# not one of the fixed guesses.
extra_paths="$HOME/.cargo/bin:$LOCALAPPDATA/Microsoft/WinGet/Links"
if npm_prefix="$(npm prefix -g 2>/dev/null)" && [ -d "$npm_prefix" ]; then
    extra_paths="$extra_paths:$npm_prefix"
fi
export PATH="$PATH:$extra_paths"

for tool in cargo node npm winapp; do
    if ! command -v "$tool" > /dev/null 2>&1; then
        log_error "Missing $tool. See spec §3 for the install commands."
        exit 1
    fi
done

cd "$PROJECT_ROOT"

# --- 1. Version -----------------------------------------------------------
# The manifest needs a four-part version; the app carries three parts. Both come
# from tauri.conf.json, so a release cannot ship a manifest that disagrees with
# the binary (spec §8).
source "$SCRIPT_DIR/../utils/version-utils.sh"
VERSION=$(get_tauri_version "$PROJECT_ROOT")
if ! validate_semver "$VERSION"; then
    log_error "tauri.conf.json version '$VERSION' is not x.y.z; MSIX needs four parts"
    exit 1
fi
MANIFEST_VERSION="${VERSION}.0"
log_info "Packaging MonoSub $VERSION as $MANIFEST_VERSION"

# --- 2. Runtime binaries --------------------------------------------------
# The blocking item in spec §4: a package without the Windows binaries crashes
# on first use, and Store certification runs the app.
if [ "$SKIP_RESOURCES" = false ]; then
    log_info "1. Preparing Windows runtime binaries ..."
    bash "$SCRIPT_DIR/prepare-resources.sh"
else
    log_info "1. Skipping resource preparation (--skip-resources)"
fi

# --- 3. Frontend + Rust build --------------------------------------------
log_info "2. Building frontend and the Tauri binary ..."
# bun is not optional here: tauri.conf.json's beforeBuildCommand is literally
# `bun run build`, so a machine without bun fails inside `tauri build` with an
# error that points at the frontend rather than at the missing tool.
if ! command -v bun > /dev/null 2>&1; then
    log_error "bun is required (tauri.conf.json runs 'bun run build'). Install: npm install -g bun"
    exit 1
fi
bun install

# The project-local CLI, not `cargo tauri`: node_modules pins the same version as
# the tauri crate, so bundle behaviour matches what Cargo.lock resolves.
TAURI_CLI="$PROJECT_ROOT/node_modules/.bin/tauri"
if [ ! -x "$TAURI_CLI" ]; then
    log_error "Missing $TAURI_CLI — run bun install first"
    exit 1
fi

# --no-bundle: the Store package is the MSIX, not an NSIS/MSI installer, and
# skipping the bundler also avoids downloading NSIS/WiX on a restricted network
# (spec §5.3).
"$TAURI_CLI" build --no-bundle

EXE_PATH="$PROJECT_ROOT/src-tauri/target/release/monosub.exe"
if [ ! -f "$EXE_PATH" ]; then
    log_error "Expected $EXE_PATH after the build"
    exit 1
fi
log_success "Built $EXE_PATH"

# --- 4. Staging layout ---------------------------------------------------
# The MSIX payload is exactly what `AppPaths::resolve_resources_root` looks for
# next to the executable: <install>/monosub.exe + <install>/resources/.
log_info "3. Assembling the package layout ..."
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_APP_DIR"
cp "$EXE_PATH" "$STAGE_APP_DIR/monosub.exe"
cp -R "$PROJECT_ROOT/src-tauri/resources" "$STAGE_APP_DIR/resources"

# The CLI binary is not part of the Store package: the manifest has one entry
# point (spec §5.2). Remove anything the resource script left behind for it.
rm -f "$STAGE_APP_DIR/resources/bin/monosub-cli-"*

# --- 5. Manifest + assets ------------------------------------------------
log_info "4. Generating Package.appxmanifest and Store assets ..."
# winapp manifest generate needs the executable to exist, which is why this
# happens after the build. Its defaults are overridden here so the identity is
# reproducible from this script alone — no interactive `winapp init` state.
#
# `--executable` is resolved against the CURRENT directory, not the target
# directory, so the generation runs from inside the staging app folder.
(
    cd "$STAGE_APP_DIR"
    winapp manifest generate . \
        --package-name "$PACKAGE_NAME" \
        --publisher-name "$PUBLISHER" \
        --version "$MANIFEST_VERSION" \
        --description "$DESCRIPTION" \
        --executable monosub.exe \
        --template Packaged \
        --if-exists overwrite \
        --quiet
)

MANIFEST="$STAGE_APP_DIR/Package.appxmanifest"

# Store tiles come from the same source icon Tauri uses, so the Store listing
# and the installed app cannot show different artwork.
winapp manifest update-assets "$PROJECT_ROOT/src-tauri/icons/icon.png" \
    --manifest "$MANIFEST" \
    --quiet

# DisplayName is user-visible in Start and Settings; the generator derives it
# from the package name, which is lowercase by Store rule.
sed -i \
    -e "s|<DisplayName>${PACKAGE_NAME}</DisplayName>|<DisplayName>${DISPLAY_NAME}</DisplayName>|" \
    -e "s|<PublisherDisplayName>[^<]*</PublisherDisplayName>|<PublisherDisplayName>leix</PublisherDisplayName>|" \
    "$MANIFEST"

# Pin the entry point instead of leaving the $targetnametoken$ placeholder: a
# manifest that names the executable explicitly cannot silently package the
# wrong binary if the token is ever substituted differently.
sed -i -e 's|Executable="\$targetnametoken\$\.exe"|Executable="monosub.exe"|' "$MANIFEST"

# NOTE: no Microsoft.Win32WebView2 PackageDependency is declared here. WebView2
# ships as an Evergreen *runtime* (an MSI/registry install), not an MSIX
# framework package, so declaring the dependency makes Add-AppxPackage fail with
# 0x80073CF3 "depends on a framework that could not be found" on machines that do
# have WebView2 — which is how the spec §5.6 local install check broke. Tauri's
# own bundler handles the WebView2 requirement through its own install logic.

log_success "Manifest ready: $MANIFEST"

# --- 6. Package ----------------------------------------------------------
log_info "5. Packing the MSIX ..."
mkdir -p "$OUTPUT_DIR"
MSIX_NAME="${PACKAGE_NAME}_${MANIFEST_VERSION}_x64.msix"

# --no-sign: see the header. This is the file that goes to Partner Center.
winapp package "$STAGE_APP_DIR" \
    --manifest "$MANIFEST" \
    --output "$OUTPUT_DIR/$MSIX_NAME" \
    --no-sign \
    --quiet

if [ ! -f "$OUTPUT_DIR/$MSIX_NAME" ]; then
    log_error "Packing produced no $OUTPUT_DIR/$MSIX_NAME"
    exit 1
fi
log_success "Unsigned package: $OUTPUT_DIR/$MSIX_NAME"

# --- 7. Optional dev-signed copy -----------------------------------------
if [ "$SIGN" = true ]; then
    log_info "6. Creating a dev-signed copy for local install ..."
    CERT="$STAGE_DIR/devcert.pfx"
    # The certificate's subject must equal the manifest Publisher, or Windows
    # refuses the install with a signature error.
    #
    # `--export-cer` matters: an MSIX signature is validated against the ROOT
    # store, and `winapp cert install` only adds the certificate to
    # TrustedPeople. Without the .cer in Root, Add-AppxPackage fails with
    # 0x800B0109 "the root certificate must be trusted".
    winapp cert generate --manifest "$MANIFEST" --output "$CERT" \
        --if-exists skip --export-cer --quiet
    winapp sign "$OUTPUT_DIR/$MSIX_NAME" "$CERT" --quiet

    SIGNED_NAME="${MSIX_NAME%.msix}.signed.msix"
    mv "$OUTPUT_DIR/$MSIX_NAME" "$OUTPUT_DIR/$SIGNED_NAME"
    # Keep an unsigned copy: the signed one cannot be submitted.
    winapp package "$STAGE_APP_DIR" \
        --manifest "$MANIFEST" \
        --output "$OUTPUT_DIR/$MSIX_NAME" \
        --no-sign \
        --quiet

    log_success "Dev-signed package: $OUTPUT_DIR/$SIGNED_NAME"
    cat <<EOF
Install it (admin PowerShell) with:

    Import-Certificate -FilePath "$STAGE_DIR/devcert.cer" -CertStoreLocation Cert:\\LocalMachine\\Root
    Add-AppxPackage "$OUTPUT_DIR/$SIGNED_NAME"

The Root import is required — TrustedPeople alone is not enough for an MSIX.
EOF
fi

# --- 8. Checklist --------------------------------------------------------
# spec §5.7 — the package is only useful if these hold.
log_info "7. Checking the package ..."
"$SCRIPT_DIR/check-msix-package.sh" "$OUTPUT_DIR/$MSIX_NAME" "$MANIFEST_VERSION" || exit 1

log_success "MSIX build complete."
echo "    $OUTPUT_DIR/$MSIX_NAME"
echo "    Upload it to Partner Center (spec §6) or attach it to a release."
