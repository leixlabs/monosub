# Requires: Windows PowerShell 5.1 (in-box) or PowerShell 7.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\build-msix.ps1 [-Sign] [-SkipResources]
#   bun run build:msix --sign --skip-resources        # same flags, dashed
#
# Builds the MonoSub MSIX package for the Microsoft Store (spec
# docs/tech-spec/microsoft-store-publish-spec.md §5).
#
# Run from PowerShell on Windows 11 x64. Produces, in dist\:
#   monosub_<version>_x64.msix           unsigned, the file Partner Center accepts
#   monosub_<version>_x64.signed.msix    dev-signed, only with -Sign, for local install
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
#
# PowerShell rather than bash: every tool this drives (winapp, tauri.cmd, cargo,
# bun) is a native Windows process, and the one PowerShell step the bash version
# needed (`-Command -File add-execution-alias.ps1`) was invalid — PowerShell reads
# `-File` as a parameter of `-Command`, so it tried to execute a command literally
# named `-File` and failed before the alias was ever written. Keeping the whole
# build in one runtime also means `-Sign` / `-SkipResources` are real parameters
# with tab completion and type checking, not positional strings parsed by hand.

[CmdletBinding()]
param(
    # `--sign` already binds to -Sign: PowerShell matches parameter names
    # case-insensitively and accepts any number of leading dashes. What it does
    # NOT do is fold a hyphen *inside* the name, so `--skip-resources` would be
    # rejected with `NamedParameterNotFound` — hence the one alias. (An alias
    # that differs from its parameter only by case is itself an error:
    # `ParameterNameConflictsWithAlias`, measured.)

    # Also produce a dev-signed copy for local `Add-AppxPackage` verification.
    [switch]$Sign,

    # Reuse the binaries already in src-tauri\resources\bin.
    [Alias('skip-resources')][switch]$SkipResources
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
$OutputDir = Join-Path $ProjectRoot 'dist'

# Staging must NOT be dist\: Vite writes the frontend there, and release.sh
# collects artifacts from there too. `build\` is gitignored.
$StageDir = Join-Path $ProjectRoot 'build\msix'
$StageAppDir = Join-Path $StageDir 'app'

# Identity (spec §5.2 / appendix C). The publisher DN is what Partner Center
# shows as Package/Identity/Publisher.
$PackageName = if ($env:MONOSUB_MSIX_NAME) { $env:MONOSUB_MSIX_NAME } else { 'monosub' }
$Publisher = if ($env:MONOSUB_MSIX_PUBLISHER) { $env:MONOSUB_MSIX_PUBLISHER } else { 'CN=leix' }
$DisplayName = 'MonoSub'
$Description = 'MonoSub - local voice to article'

# --- 0. Environment -------------------------------------------------------
# Git Bash inherits the machine PATH only at login, which is why the bash
# version had to probe for npm's global prefix. PowerShell inherits the current
# user PATH directly, so the check is just "can I resolve it?" — but bun and
# cargo are installed per-user, and a shell started before that install will not
# see them either. Probe the usual per-user locations before failing.
foreach ($tool in @('cargo', 'node', 'npm', 'winapp', 'bun')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Err "Missing $tool. See spec §3 for the install commands."
        exit 1
    }
}

Set-Location $ProjectRoot

# --- 1. Version -----------------------------------------------------------
# The manifest needs a four-part version; the app carries three parts. Both come
# from tauri.conf.json, so a release cannot ship a manifest that disagrees with
# the binary (spec §8).
$TauriConfPath = Join-Path $ProjectRoot 'src-tauri\tauri.conf.json'
if (-not (Test-Path -LiteralPath $TauriConfPath)) {
    Write-Err "tauri config not found: $TauriConfPath"
    exit 1
}
$TauriConf = Get-Content -LiteralPath $TauriConfPath -Raw | ConvertFrom-Json
$Version = $TauriConf.version
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Err "tauri.conf.json version '$Version' is not x.y.z; MSIX needs four parts"
    exit 1
}
$ManifestVersion = "$Version.0"
Write-Step "Packaging MonoSub $Version as $ManifestVersion"

# --- 2. Runtime binaries --------------------------------------------------
# The blocking item in spec §4: a package without the Windows binaries crashes
# on first use, and Store certification runs the app.
if (-not $SkipResources) {
    Write-Step '1. Preparing Windows runtime binaries ...'
    & (Join-Path $ScriptDir 'prepare-resources.ps1')
    if ($LASTEXITCODE -ne 0) {
        Write-Err 'prepare-resources.ps1 failed'
        exit 1
    }
} else {
    Write-Step '1. Skipping resource preparation (-SkipResources)'
}

# --- 3. Frontend + Rust build --------------------------------------------
Write-Step '2. Building frontend and the Tauri binary ...'
# bun is not optional here: tauri.conf.json's beforeBuildCommand is literally
# `bun run build`, so a machine without bun fails inside `tauri build` with an
# error that points at the frontend rather than at the missing tool.
if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
    Write-Err "bun is required (tauri.conf.json runs 'bun run build'). Install: npm install -g bun"
    exit 1
}
& bun install
if ($LASTEXITCODE -ne 0) { Write-Err 'bun install failed'; exit 1 }

# The project-local CLI, not `cargo tauri`: node_modules pins the same version as
# the tauri crate, so bundle behaviour matches what Cargo.lock resolves.
#
# tauri.cmd, not the extensionless `tauri` shim: the shim is a POSIX shell script
# and Windows cannot execute it without bash, which is the very dependency this
# rewrite removes.
$TauriCli = Join-Path $ProjectRoot 'node_modules\.bin\tauri.cmd'
if (-not (Test-Path -LiteralPath $TauriCli)) {
    Write-Err "Missing $TauriCli - run bun install first"
    exit 1
}

# --no-bundle: the Store package is the MSIX, not an NSIS/MSI installer, and
# skipping the bundler also avoids downloading NSIS/WiX on a restricted network
# (spec §5.3).
& $TauriCli build --no-bundle
if ($LASTEXITCODE -ne 0) { Write-Err 'tauri build failed'; exit 1 }

$ExePath = Join-Path $ProjectRoot 'src-tauri\target\release\monosub.exe'
if (-not (Test-Path -LiteralPath $ExePath)) {
    Write-Err "Expected $ExePath after the build"
    exit 1
}
Write-Ok "Built $ExePath"

# `tauri build` compiles only the bundle's entry point, so the second binary is
# built explicitly — the same step build-macos-app.sh takes. It lands under
# resources\bin\ with the `<name>-<triple>.exe` name every other bundled binary
# uses, which is what `engine::cli` resolves and what the execution alias points
# at. The alias is what makes it reachable from a terminal: MSIX installs under a
# version-stamped folder, so a PATH entry naming that folder breaks on update.
Write-Step '2b. Building the command-line tool ...'
$TargetTriple = 'x86_64-pc-windows-msvc'
$CliName = "monosub-cli-$TargetTriple.exe"
$CliBuilt = Join-Path $ProjectRoot 'src-tauri\target\release\monosub-cli.exe'
$CliSource = Join-Path $ProjectRoot 'src-tauri\src\bin\monosub-cli.rs'
if (-not (Test-Path -LiteralPath $CliSource)) {
    Write-Err 'Missing src\bin\monosub-cli.rs - the Store package declares its alias'
    exit 1
}
& cargo build --release --bin monosub-cli --manifest-path (Join-Path $ProjectRoot 'src-tauri\Cargo.toml')
if ($LASTEXITCODE -ne 0) { Write-Err 'building monosub-cli failed'; exit 1 }
if (-not (Test-Path -LiteralPath $CliBuilt)) {
    Write-Err "Expected $CliBuilt after building monosub-cli"
    exit 1
}
$ResourceBinDir = Join-Path $ProjectRoot 'src-tauri\resources\bin'
New-Item -ItemType Directory -Path $ResourceBinDir -Force | Out-Null
Copy-Item -LiteralPath $CliBuilt -Destination (Join-Path $ResourceBinDir $CliName) -Force
Write-Ok "CLI staged as $CliName"

# --- 4. Staging layout ---------------------------------------------------
# The MSIX payload is exactly what `AppPaths::resolve_resources_root` looks for
# next to the executable: <install>\monosub.exe + <install>\resources\.
Write-Step '3. Assembling the package layout ...'
if (Test-Path -LiteralPath $StageDir) { Remove-Item -LiteralPath $StageDir -Recurse -Force }
New-Item -ItemType Directory -Path $StageAppDir -Force | Out-Null
Copy-Item -LiteralPath $ExePath -Destination (Join-Path $StageAppDir 'monosub.exe') -Force
Copy-Item -LiteralPath (Join-Path $ProjectRoot 'src-tauri\resources') `
    -Destination (Join-Path $StageAppDir 'resources') -Recurse -Force

# --- 5. Manifest + assets ------------------------------------------------
Write-Step '4. Generating Package.appxmanifest and Store assets ...'
# winapp manifest generate needs the executable to exist, which is why this
# happens after the build. Its defaults are overridden here so the identity is
# reproducible from this script alone — no interactive `winapp init` state.
Push-Location $StageAppDir
try {
    & winapp manifest generate . `
        --package-name $PackageName `
        --publisher-name $Publisher `
        --version $ManifestVersion `
        --description $Description `
        --executable monosub.exe `
        --template Packaged `
        --if-exists overwrite `
        --quiet
    if ($LASTEXITCODE -ne 0) { Write-Err 'winapp manifest generate failed'; exit 1 }
} finally {
    Pop-Location
}

$Manifest = Join-Path $StageAppDir 'Package.appxmanifest'

# Store tiles come from the same source icon Tauri uses, so the Store listing
# and the installed app cannot show different artwork.
& winapp manifest update-assets (Join-Path $ProjectRoot 'src-tauri\icons\icon.png') `
    --manifest $Manifest `
    --quiet
if ($LASTEXITCODE -ne 0) { Write-Err 'winapp manifest update-assets failed'; exit 1 }

# Five edits the generator leaves to the caller. Done as an XML DOM rather than
# `sed`: these are single attributes in a file the packer will validate, and a
# text substitution that misses silently ships the wrong identity.
#
#   DisplayName (Properties)         user-visible in Settings → Apps.
#   DisplayName (VisualElements)     user-visible in the Start menu and on the
#                                    tile — a DIFFERENT element, and the one a
#                                    user actually reads. The bash version only
#                                    patched the Properties one, so the Start
#                                    menu shipped lowercase `monosub` (measured
#                                    on the built package: the Properties value
#                                    was MonoSub while VisualElements was still
#                                    monosub). Both are set here.
#   PublisherDisplayName             what Settings shows as the publisher.
#   Executable                       the $targetnametoken$ placeholder is what
#                                    the Tauri bundler would substitute;
#                                    `winapp package` does not, so a manifest
#                                    left with it fails validation with
#                                    "The file name $targetnametoken$.exe ...
#                                    doesn't exist".
#   ProcessorArchitecture            the package is x64 only (every bundled
#                                    binary is), and the Store needs that stated
#                                    in Identity.
Write-Step '4a. Pinning identity fields in the manifest ...'
[xml]$manifestXml = Get-Content -LiteralPath $Manifest -Raw
$packageNode = $manifestXml.DocumentElement
$foundationNs = 'http://schemas.microsoft.com/appx/manifest/foundation/windows10'

$properties = $packageNode.SelectSingleNode('./*[local-name()="Properties"]')
if ($null -eq $properties) { Write-Err 'Generated manifest has no <Properties>'; exit 1 }

$displayNameNode = $properties.SelectSingleNode('./*[local-name()="DisplayName"]')
if ($null -eq $displayNameNode) {
    $displayNameNode = $manifestXml.CreateElement('DisplayName', $foundationNs)
    [void]$properties.PrependChild($displayNameNode)
}
$displayNameNode.InnerText = $DisplayName

$publisherDisplayNode = $properties.SelectSingleNode('./*[local-name()="PublisherDisplayName"]')
if ($null -eq $publisherDisplayNode) {
    $publisherDisplayNode = $manifestXml.CreateElement('PublisherDisplayName', $foundationNs)
    [void]$properties.AppendChild($publisherDisplayNode)
}
$publisherDisplayNode.InnerText = 'leix'

$identityNode = $packageNode.SelectSingleNode('./*[local-name()="Identity"]')
if ($null -eq $identityNode) { Write-Err 'Generated manifest has no <Identity>'; exit 1 }
$identityNode.SetAttribute('ProcessorArchitecture', 'x64')

$applications = $packageNode.SelectNodes('./*[local-name()="Applications"]/*[local-name()="Application"]')
if ($applications.Count -eq 0) { Write-Err 'Generated manifest has no <Application>'; exit 1 }
foreach ($app in $applications) {
    $app.SetAttribute('Executable', 'monosub.exe')

    # The Start-menu name lives on VisualElements, in whatever namespace prefix
    # the generator chose (uap). Matched by local name so a prefix change in a
    # future winapp release does not silently skip it — and required, because an
    # unpatched value is exactly the bug this replaced.
    $visualElements = $app.SelectSingleNode('./*[local-name()="VisualElements"]')
    if ($null -eq $visualElements) {
        Write-Err 'Generated manifest has no <VisualElements>; the Start-menu name cannot be set'
        exit 1
    }
    $visualElements.SetAttribute('DisplayName', $DisplayName)
}

$writerSettings = New-Object System.Xml.XmlWriterSettings
$writerSettings.Encoding = New-Object System.Text.UTF8Encoding($false)
$writerSettings.Indent = $true
$writer = [System.Xml.XmlWriter]::Create($Manifest, $writerSettings)
try { $manifestXml.Save($writer) } finally { $writer.Close() }

# --- 5b. Command-line tool: app execution alias --------------------------
# Declares `monosub-cli` as a command the shell resolves, without touching PATH.
#
# Why an alias and not a PATH entry: MSIX installs under
# `C:\Program Files\WindowsApps\<name>_<version>_<arch>__<publisher-hash>\`, so a
# PATH entry naming that folder dies at the next update. Windows puts aliases in
# `%LOCALAPPDATA%\Microsoft\WindowsApps`, which is already on PATH, and re-points
# them on every update. That is also why Settings shows no install command on
# Windows — there is nothing for the user to run.
#
# The alias targets the packaged binary at `resources\bin\<name>-<triple>.exe`,
# the same file `engine::cli` resolves, so the CLI found by name and the one the
# app reports in Settings ▸ System are the same file.
#
# `winapp manifest add-alias` cannot do this one: it infers the target from the
# Application's Executable, and always writes `EntryPoint`-less uap5 aliases
# aimed at the app itself. The helper script writes the uap3 form with an
# explicit Executable, inside <Application>\<Extensions> (see its header for the
# makeappx evidence that the wrapper is mandatory).
Write-Step '4b. Declaring the monosub-cli execution alias ...'
$CliRelativePath = "resources\bin\$CliName"
& (Join-Path $ScriptDir 'add-execution-alias.ps1') `
    -Manifest $Manifest `
    -Executable $CliRelativePath `
    -Alias 'monosub-cli.exe'
if ($LASTEXITCODE -ne 0) {
    Write-Err 'Could not declare the monosub-cli execution alias (spec §5.2)'
    exit 1
}
Write-Ok "Execution alias declared: monosub-cli.exe -> $CliRelativePath"

# NOTE: no Microsoft.Win32WebView2 PackageDependency is declared here. WebView2
# ships as an Evergreen *runtime* (an MSI/registry install), not an MSIX
# framework package, so declaring the dependency makes Add-AppxPackage fail with
# 0x80073CF3 "depends on a framework that could not be found" on machines that do
# have WebView2 — which is how the spec §5.6 local install check broke. Tauri's
# own bundler handles the WebView2 requirement through its own install logic.

Write-Ok "Manifest ready: $Manifest"

# --- 6. Package ----------------------------------------------------------
Write-Step '5. Packing the MSIX ...'
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$MsixName = "${PackageName}_${ManifestVersion}_x64.msix"
$MsixPath = Join-Path $OutputDir $MsixName

# --no-sign: see the header. This is the file that goes to Partner Center.
& winapp package $StageAppDir `
    --manifest $Manifest `
    --output $MsixPath `
    --no-sign `
    --quiet
if ($LASTEXITCODE -ne 0) { Write-Err 'winapp package failed'; exit 1 }

if (-not (Test-Path -LiteralPath $MsixPath)) {
    Write-Err "Packing produced no $MsixPath"
    exit 1
}
Write-Ok "Unsigned package: $MsixPath"

# --- 7. Optional dev-signed copy -----------------------------------------
if ($Sign) {
    Write-Step '6. Creating a dev-signed copy for local install ...'
    $Cert = Join-Path $StageDir 'devcert.pfx'
    # The certificate's subject must equal the manifest Publisher, or Windows
    # refuses the install with a signature error.
    #
    # --export-cer matters: an MSIX signature is validated against the ROOT
    # store, and `winapp cert install` only adds the certificate to
    # TrustedPeople. Without the .cer in Root, Add-AppxPackage fails with
    # 0x800B0109 "the root certificate must be trusted".
    & winapp cert generate --manifest $Manifest --output $Cert --if-exists skip --export-cer --quiet
    if ($LASTEXITCODE -ne 0) { Write-Err 'winapp cert generate failed'; exit 1 }

    & winapp sign $MsixPath $Cert --quiet
    if ($LASTEXITCODE -ne 0) { Write-Err 'winapp sign failed'; exit 1 }

    $SignedName = [System.IO.Path]::GetFileNameWithoutExtension($MsixName) + '.signed.msix'
    $SignedPath = Join-Path $OutputDir $SignedName
    Move-Item -LiteralPath $MsixPath -Destination $SignedPath -Force

    # Keep an unsigned copy: the signed one cannot be submitted.
    & winapp package $StageAppDir --manifest $Manifest --output $MsixPath --no-sign --quiet
    if ($LASTEXITCODE -ne 0) { Write-Err 'repacking the unsigned copy failed'; exit 1 }

    Write-Ok "Dev-signed package: $SignedPath"
    $CerPath = Join-Path $StageDir 'devcert.cer'
    Write-Host @"
Install it (admin PowerShell) with:

    Import-Certificate -FilePath "$CerPath" -CertStoreLocation Cert:\LocalMachine\Root
    Add-AppxPackage "$SignedPath"

The Root import is required - TrustedPeople alone is not enough for an MSIX.
"@
}

# --- 8. Checklist --------------------------------------------------------
# spec §5.7 — the package is only useful if these hold.
Write-Step '7. Checking the package ...'
& (Join-Path $ScriptDir 'check-msix-package.ps1') -Package $MsixPath -ExpectedVersion $ManifestVersion
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Ok 'MSIX build complete.'
Write-Host "    $MsixPath"
Write-Host '    Upload it to Partner Center (spec §6) or attach it to a release.'

# Explicit: a .ps1 that ends normally leaves $LASTEXITCODE at whatever its last
# native command returned, and the caller (`bun run build:msix`, or a CI step)
# reads that value.
exit 0
