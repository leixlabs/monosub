# Requires: Windows PowerShell 5.1 (in-box) or PowerShell 7, and the GitHub CLI.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\publish-msix.ps1 `
#       [-Tag v2.0.0] [-Version 2.0.0] [-UpdateTag]
#
# Publishes an ALREADY BUILT MSIX to a GitHub release. It does not build anything:
# the build step is build-msix.ps1 (or `bun run build:msix`), which writes
# dist\monosub_<version>.0_x64.msix. Keeping build and publish apart means a failed
# upload never costs another build, and a failed build never leaves a half-made
# release on GitHub. The macOS side is split the same way (scripts/release/
# release.sh builds, scripts/publish/publish.sh publishes).
#
# -Version picks which built package to publish. It defaults to the tauri.conf.json
# version, but after a previous publish that file has already been bumped for the
# next release, so pass -Version explicitly when publishing an older build (e.g.
# the macOS DMG release already bumped 2.0.0 -> 2.0.1 and you are attached the
# 2.0.0 MSIX). The MSIX in dist\ is the source of truth, not the config.
#
# Two different things exist for a tag and they are easy to confuse:
#   * the git tag, which points at a commit
#   * the GitHub release, which is the page that holds the assets
# Moving a tag does not move the release, and adding an asset to a release does
# not change the tag. This script can do either, and says which.
#
# -UpdateTag rewrites the tag to HEAD and force-pushes it. That is a history
# rewrite of a published ref: anyone who already fetched the old tag keeps the
# old commit until they re-fetch. It is what you want when the release notes say
# "built from <sha>" and the artifact was built from a newer commit, and it is
# never what you want for a tag other people have based work on.

[CmdletBinding()]
param(
    # `--tag` already binds to -Tag: PowerShell matches parameter names
    # case-insensitively and accepts any number of leading dashes. What it does
    # NOT do is fold a hyphen *inside* the name, so `--update-tag` needs the alias
    # below. (An alias that differs from its parameter only by case is itself an
    # error: `ParameterNameConflictsWithAlias`.)
    [string]$Tag = '',
    [string]$Version = '',
    [Alias('update-tag')][switch]$UpdateTag
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

function Write-Step([string]$Message) { Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Ok([string]$Message) { Write-Host "[SUCCESS] $Message" -ForegroundColor Green }
function Write-Err([string]$Message) { Write-Host "[ERROR] $Message" -ForegroundColor Red }
function Write-Warn([string]$Message) { Write-Host "[WARNING] $Message" -ForegroundColor Yellow }

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$ReleaseRepo = if ($env:RELEASE_REPO) { $env:RELEASE_REPO } else { 'leixlabs/monosub' }
$OutputDir = Join-Path $ProjectRoot 'dist'

Set-Location $ProjectRoot

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Err 'gh CLI is required'
    Exit 1
}

Write-Host '==============================================='
Write-Host 'MonoSub publish (MSIX) - upload to GitHub Releases'
Write-Host '==============================================='
Write-Step "Target repository: $ReleaseRepo"

# --- 1. Version and tag ---------------------------------------------------
$TauriConf = Get-Content -LiteralPath (Join-Path $ProjectRoot 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$ConfVersion = $TauriConf.version
if ($ConfVersion -notmatch '^\d+\.\d+\.\d+$') {
    Write-Err "tauri.conf.json version '$ConfVersion' is not x.y.z"
    Exit 1
}

if ([string]::IsNullOrEmpty($Version)) {
    $Version = $ConfVersion
} elseif ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Err "Version '$Version' is not x.y.z"
    Exit 1
} elseif ($Version -ne $ConfVersion) {
    Write-Warn "Publishing $Version while tauri.conf.json says $ConfVersion"
    Write-Warn '(the config was already bumped for the next release - using the package version)'
}
$ManifestVersion = "$Version.0"
$MsixName = "monosub_${ManifestVersion}_x64.msix"
$MsixPath = Join-Path $OutputDir $MsixName

if ([string]::IsNullOrEmpty($Tag)) { $Tag = "v$Version" }

# The app's own update check compares its CARGO_PKG_VERSION against the release
# tag, so a tag that is not exactly v<version> makes "Check for Updates" report
# an update forever (see scripts/publish/publish.sh). Same rule here, and it is
# checked against the package version - the thing users actually install.
if ($Tag -ne "v$Version") {
    Write-Err "Tag $Tag does not match the package version v$Version."
    Write-Err 'The app compares its own version to the release tag, so these must agree.'
    Exit 1
}

Write-Step "Version $Version, tag $Tag"

# --- 2. The artifact is the input ----------------------------------------
# Publish consumes files; it never produces them. A missing package means the
# build step has not run (or ran for a different version), and there is nothing
# honest this script could upload.
if (-not (Test-Path -LiteralPath $MsixPath)) {
    Write-Err "No $MsixPath to publish."
    Write-Err 'Build it first: powershell -File scripts\windows\build-msix.ps1'
    $FromDist = @(Get-ChildItem -LiteralPath $OutputDir -Filter '*.msix' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notlike '*.signed.msix' } |
        ForEach-Object { $_.Name })
    if ($FromDist.Count -gt 0) {
        Write-Err ("dist\ has: " + ($FromDist -join ', '))
        Write-Err 'Pass -Version <the one in that file name> to publish it instead.'
    }
    Exit 1
}

# The signed copy is a local-install artifact; the Store and GitHub get the
# unsigned one, because a self-signed package cannot be submitted or trusted by
# anyone who has not installed this machine's certificate.

# --- 3. Optional tag move ------------------------------------------------
if ($UpdateTag) {
    # @() around the call: git writes one line per entry, and PowerShell turns
    # zero lines into $null and one line into a bare string — neither has a
    # Count under Set-StrictMode, and `-ne $null` on an empty array is false
    # anyway, so the wrapping is what makes this test say what it means.
    $dirty = @(git status --porcelain)
    if ($dirty.Count -gt 0) {
        Write-Err 'Working tree is dirty. Commit the packaging scripts first:'
        git status --short
        Exit 1
    }

    $OldSha = (@(git rev-parse --short $Tag 2>$null) | Select-Object -First 1)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrEmpty($OldSha)) { $OldSha = '' }
    $NewSha = (@(git rev-parse --short HEAD) | Select-Object -First 1)
    if ([string]::IsNullOrEmpty($OldSha)) { $OldSha = '<none>' }
    Write-Step "Moving $Tag from $OldSha to $NewSha ..."

    if ($OldSha -eq $NewSha) {
        Write-Ok "$Tag already points at HEAD; nothing to move"
    } else {
        # An annotated tag needs a committer identity. Without one git fails with
        # "Committer identity unknown" halfway through — after the old tag has
        # already been deleted — so check first rather than leaving a broken tag.
        git var GIT_COMMITTER_IDENT > $null 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Err 'No git committer identity configured; an annotated tag cannot be created.'
            Write-Err "Set one with: git config user.name '...' && git config user.email '...'"
            Exit 1
        }

        # An annotated tag is what `gh release create` produces and what the
        # existing v2.0.0 is, so recreate it the same way instead of with
        # `git tag -f`, which would turn it into a lightweight tag.
        git tag -d $Tag > $null 2>&1
        git tag -a $Tag -m "MonoSub $Version"
        if ($LASTEXITCODE -ne 0) { Write-Err "Could not create tag $Tag"; Exit 1 }
        git push origin "refs/tags/$Tag" --force
        if ($LASTEXITCODE -ne 0) { Write-Err "Could not push tag $Tag"; Exit 1 }
        Write-Ok "$Tag now points at $NewSha"
        Write-Warn 'Anyone who already fetched the old tag keeps the old commit until they re-fetch.'
    }
} else {
    $TagSha = (@(git rev-parse --short $Tag 2>$null) | Select-Object -First 1)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrEmpty($TagSha)) {
        Write-Err "Tag $Tag does not exist. Run with -UpdateTag to create it."
        Exit 1
    }
    Write-Step "Leaving $Tag where it is ($TagSha)"
}

# --- 4. Upload -----------------------------------------------------------
# `--clobber` replaces an asset of the same name, so re-running after a fix does
# not fail on the second upload. The release itself must already exist: creating
# one here would hide the DMG that belongs to the same version.
Write-Step "Uploading $MsixName to release $Tag ..."
gh release view $Tag --repo $ReleaseRepo > $null 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Err "Release $Tag does not exist in $ReleaseRepo."
    Write-Err 'Create it first (scripts/publish/publish.sh on macOS), or run the normal release flow.'
    Exit 1
}

gh release upload $Tag $MsixPath --repo $ReleaseRepo --clobber
if ($LASTEXITCODE -ne 0) { Write-Err "Uploading $MsixName failed"; Exit 1 }

# --- 5. Refresh the notes ------------------------------------------------
# The asset table in the existing notes only lists the DMG. Appending rather than
# rewriting keeps the hand-written sections (macOS first launch, etc.) intact.
Write-Step 'Updating the release notes with the Windows package ...'
$CurrentNotes = (& gh release view $Tag --repo $ReleaseRepo --json body --jq '.body') -join "`n"

# When the tag moved, the "Built from commit" line still names the old commit —
# and it is the one line a reader uses to tell which code the assets came from.
if ($UpdateTag) {
    $NewSha = (@(git rev-parse --short HEAD) | Select-Object -First 1)
    $CurrentNotes = [regex]::Replace($CurrentNotes,
        '(Built from commit: `)[a-f0-9]+(`)', "`${1}$NewSha`${2}")
}

if ($CurrentNotes -match [regex]::Escape($MsixName)) {
    Write-Ok "Release notes already mention $MsixName"
    if ($UpdateTag) {
        gh release edit $Tag --repo $ReleaseRepo --notes $CurrentNotes
        Write-Ok 'Commit reference refreshed'
    }
} else {
    # The table lists `*.dmg` as a glob; add the MSIX as a row and note that the
    # Windows build is a Store package rather than a standalone installer.
    $NewNotes = @"
$CurrentNotes

## Windows (Microsoft Store package)

| File | Description |
|------|-------------|
| $MsixName | MSIX package, Windows 11 x64 |

This package is prepared for Microsoft Store submission; the Store re-signs it on
publish. Installing the file attached here requires trusting a development
certificate, so Windows users should install MonoSub from the Store instead.

---

"@
    gh release edit $Tag --repo $ReleaseRepo --notes $NewNotes
    if ($LASTEXITCODE -ne 0) { Write-Err 'Updating the release notes failed'; Exit 1 }
    Write-Ok 'Release notes updated'
}

Write-Ok "Done: https://github.com/$ReleaseRepo/releases/tag/$Tag"

# Explicit: a .ps1 that ends normally leaves $LASTEXITCODE at whatever its last
# native command returned, and the caller (`bun run publish:msix`, or a CI step)
# reads that value.
exit 0
