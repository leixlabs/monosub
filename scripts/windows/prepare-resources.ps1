# Requires: Windows PowerShell 5.1 (in-box) or PowerShell 7.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\prepare-resources.ps1
#
# Windows-only. The macOS resource flow is scripts/build/prepare-resources.sh;
# the two directories are kept separate so neither platform can break the other.
# Fetches the Windows x64 runtime binaries that the MSIX package bundles.
#
# Idempotent: re-running with matching artifacts is a no-op, so the build script
# can call it on every run without paying for the downloads twice.
#
# Layout it produces, under src-tauri\resources\:
#
#   bin\audiocpp_cli-x86_64-pc-windows-msvc.exe   audio.cpp CLI (spec §4.2)
#   bin\ffmpeg-x86_64-pc-windows-msvc.exe         static ffmpeg
#   bin\ffprobe-x86_64-pc-windows-msvc.exe
#   bin\llama-server-x86_64-pc-windows-msvc.exe   + llama/ggml DLLs, flat
#   bin\yt-dlp-x86_64-pc-windows-msvc.exe
#   bin\*.dll                                     MSVC runtime + llama DLLs
#   models\silero_vad\silero_vad_16k.safetensors
#   MANIFEST.txt
#
# Why the `.exe` suffix sits after the target triple: `engine::cli`'s candidate
# rule is `<name>-<target-triple>.exe` on Windows (spec §4.1). The DLLs are flat
# next to the executables because Windows resolves a child process's imports from
# its own directory first — the same reason the macOS llama dylibs are flat.
#
# The macOS artifacts are never touched: every file written here carries the
# `x86_64-pc-windows-msvc` triple or is a `.dll` that only Windows can load.
#
# This is a PowerShell script rather than bash on purpose. The build it belongs to
# can only run on Windows, and the pieces it drives (winapp, the .NET zip types,
# Get-FileHash) are Windows-native; a bash script here had to shell out to
# PowerShell for anything beyond text handling, and each of those seams was a
# place where quoting or a POSIX path silently broke. Everything below is one
# runtime, one path convention, one error model.

[CmdletBinding()]
param(
    # Skip the final `--version` smoke test. The build script uses this when the
    # binaries were already proven on a previous run.
    [switch]$SkipSmokeTest
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Console output is UTF-8: the messages below are Chinese in places, and the
# default OEM code page turns them into mojibake in every captured log.
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

function Write-Step([string]$Message) { Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Ok([string]$Message) { Write-Host "[OK] $Message" -ForegroundColor Green }
function Write-Err([string]$Message) { Write-Host "[ERROR] $Message" -ForegroundColor Red }

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$ResourcesDir = Join-Path $ProjectRoot 'src-tauri\resources'
$BinDir = Join-Path $ResourcesDir 'bin'
$ModelDir = Join-Path $ResourcesDir 'models\silero_vad'
$LicenseDir = Join-Path $ResourcesDir 'licenses'

$TargetTriple = 'x86_64-pc-windows-msvc'

# --- Pinned upstream ------------------------------------------------------
# Every pin is the archive the URL resolves to today; bump them together after a
# smoke test of the packaged app (spec §5.6), never one at a time.
$AudioCppVersion = 'v0.9.1'
$AudioCppAsset = 'audio-v0.9.1-bin-windows-x64-cpu-portable.zip'
$AudioCppSha256 = '2fccc73dc97af84da1e81c3af545fa619348a46d023ea7d927ba80bc8680d2a0'

$LlamaBuild = 'b11696'
$LlamaAsset = "llama-$LlamaBuild-bin-win-cpu-x64.zip"
$LlamaSha256 = '1e7c47dff58467540d53e6cf302d83951bcb165afb252c2952ed3872190fc60d'

# gyan.dev re-points the version-less URL on every release, so the pinned file
# name is what keeps this reproducible. `packages/` is the immutable path.
$FfmpegVersion = '8.1.2'
$FfmpegAsset = "ffmpeg-$FfmpegVersion-essentials_build.zip"
$FfmpegSha256 = 'db580001caa24ac104c8cb856cd113a87b0a443f7bdf47d8c12b1d740584a2ec'

$YtDlpVersion = '2026.08.19'
$YtDlpAsset = 'yt-dlp.exe'
$YtDlpSha256 = '66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a'

# Same upstream tag as the macOS script; the VAD model is platform-neutral.
$SileroUrl = "https://raw.githubusercontent.com/0xShug0/audio.cpp/$AudioCppVersion/assets/framework/models/silero_vad/silero_vad_16k.safetensors"
$SileroSha256 = 'c59271c284ae9c8335d795d60e0bfdb71aaaceec578d9bd9ffc1b8153c319ea1'
$SileroTarget = Join-Path $ModelDir 'silero_vad_16k.safetensors'

function Assert-Sha256 {
    param([string]$File, [string]$Expected)

    if ([string]::IsNullOrEmpty($Expected)) { return }
    $Actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Actual -ne $Expected.ToLowerInvariant()) {
        Write-Err "Checksum mismatch for $(Split-Path -Leaf $File)"
        Write-Err "  expected $Expected"
        Write-Err "  actual   $Actual"
        throw "Checksum mismatch for $(Split-Path -Leaf $File)"
    }
}

function Test-Sha256 {
    param([string]$File, [string]$Expected)

    if (-not (Test-Path -LiteralPath $File)) { return $false }
    if ([string]::IsNullOrEmpty($Expected)) { return $true }
    try {
        Assert-Sha256 -File $File -Expected $Expected
        return $true
    } catch {
        return $false
    }
}

# --- Download -------------------------------------------------------------
# curl.exe ships in Windows 10 1803+ (System32), and `--retry-all-errors` is what
# makes a flaky CDN recover instead of leaving a truncated archive that then
# fails the checksum. No proxy configuration: the machine's own settings apply,
# and the caller can export HTTPS_PROXY in the environment if one is needed.
$Curl = Join-Path $env:SystemRoot 'System32\curl.exe'
if (-not (Test-Path -LiteralPath $Curl)) { throw "curl.exe not found at $Curl (Windows 10 1803+ required)" }

function Get-RemoteFile {
    param([string]$Url, [string]$Destination)

    Write-Step "Downloading $(Split-Path -Leaf $Destination) ..."
    & $Curl --fail --location --silent --show-error --retry 5 --retry-delay 2 --retry-all-errors `
        --output $Destination $Url
    if ($LASTEXITCODE -ne 0) {
        throw "curl failed ($LASTEXITCODE) for $Url"
    }
}

function Expand-Zip {
    param([string]$Archive, [string]$Destination)

    # Expand-Archive cannot read every archive these projects ship (it rejects
    # zips whose entries lack a directory record), so the .NET zip API is used
    # directly: it extracts what is there and reports a real exception if not.
    if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination -Recurse -Force }
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $Zip = [System.IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        foreach ($Entry in $Zip.Entries) {
            if ($Entry.FullName.EndsWith('/')) { continue }
            $OutPath = Join-Path $Destination $Entry.FullName.Replace('/', '\')
            $OutDir = Split-Path -Parent $OutPath
            if (-not (Test-Path -LiteralPath $OutDir)) {
                New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
            }
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($Entry, $OutPath, $true)
        }
    } finally {
        $Zip.Dispose()
    }
}

# Everything downloaded lands outside the repo so an interrupted run cannot leave
# a half-written archive where `resources\**\*` would bundle it.
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("monosub-win-res-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

foreach ($Dir in @($BinDir, $ModelDir, $LicenseDir)) {
    New-Item -ItemType Directory -Path $Dir -Force | Out-Null
}

try {
    # --- 1. audio.cpp CLI + its MSVC runtime ------------------------------
    $CliTarget = Join-Path $BinDir "audiocpp_cli-$TargetTriple.exe"
    if (Test-Path -LiteralPath $CliTarget) {
        Write-Ok "audiocpp_cli already present, skipping ($TargetTriple)"
    } else {
        $Archive = Join-Path $TmpDir $AudioCppAsset
        Get-RemoteFile "https://github.com/0xShug0/audio.cpp/releases/download/$AudioCppVersion/$AudioCppAsset" $Archive
        Assert-Sha256 -File $Archive -Expected $AudioCppSha256

        Write-Step "Extracting audiocpp_cli and its runtime DLLs ..."
        $Extracted = Join-Path $TmpDir 'audiocpp'
        Expand-Zip -Archive $Archive -Destination $Extracted

        Copy-Item -LiteralPath (Join-Path $Extracted 'audiocpp_cli.exe') -Destination $CliTarget -Force

        # The portable build is statically linked for ggml, so the only DLLs it
        # needs are the MSVC runtime ones. They go next to the executable, which
        # is where the child process looks for them.
        $RuntimeDlls = 0
        foreach ($Dll in Get-ChildItem -LiteralPath $Extracted -Filter '*.dll' -File) {
            Copy-Item -LiteralPath $Dll.FullName -Destination (Join-Path $BinDir $Dll.Name) -Force
            $RuntimeDlls++
        }
        Write-Ok "Installed $CliTarget (+$RuntimeDlls runtime DLLs)"
    }

    # --- 2. llama-server + its DLLs ---------------------------------------
    $ServerTarget = Join-Path $BinDir "llama-server-$TargetTriple.exe"
    # @() around the pipeline, not a bare `.Count`: under Set-StrictMode a
    # single result is a scalar with no Count property, and an empty one is
    # $null — both throw here instead of answering "none".
    $HasLlamaDlls = @(Get-ChildItem -LiteralPath $BinDir -Filter 'llama*.dll' -File -ErrorAction SilentlyContinue).Count -gt 0
    if ((Test-Path -LiteralPath $ServerTarget) -and $HasLlamaDlls) {
        Write-Ok "llama-server already present, skipping ($TargetTriple)"
    } else {
        $Archive = Join-Path $TmpDir $LlamaAsset
        Get-RemoteFile "https://github.com/ggml-org/llama.cpp/releases/download/$LlamaBuild/$LlamaAsset" $Archive
        Assert-Sha256 -File $Archive -Expected $LlamaSha256

        Write-Step "Extracting llama-server and lib*.dll ..."
        $Extracted = Join-Path $TmpDir 'llama'
        Expand-Zip -Archive $Archive -Destination $Extracted

        Copy-Item -LiteralPath (Join-Path $Extracted 'llama-server.exe') -Destination $ServerTarget -Force

        # The zip ships one executable per tool plus the shared libraries they
        # all need. Only the server is installed; every DLL is, because the
        # server is a thin launcher over `llama-server-impl.dll` and its ggml
        # backends.
        $DllCount = 0
        foreach ($Dll in Get-ChildItem -LiteralPath $Extracted -Filter '*.dll' -File) {
            Copy-Item -LiteralPath $Dll.FullName -Destination (Join-Path $BinDir $Dll.Name) -Force
            $DllCount++
        }
        if ($DllCount -eq 0) {
            throw 'no DLLs in the llama archive; llama-server cannot start'
        }
        Write-Ok "Installed $ServerTarget (+$DllCount DLLs flat in bin\)"
    }

    # --- 3. FFmpeg / FFprobe ----------------------------------------------
    $FfmpegTarget = Join-Path $BinDir "ffmpeg-$TargetTriple.exe"
    $FfprobeTarget = Join-Path $BinDir "ffprobe-$TargetTriple.exe"
    if ((Test-Path -LiteralPath $FfmpegTarget) -and (Test-Path -LiteralPath $FfprobeTarget)) {
        Write-Ok "ffmpeg already present, skipping ($TargetTriple)"
    } else {
        $Archive = Join-Path $TmpDir $FfmpegAsset
        Get-RemoteFile "https://www.gyan.dev/ffmpeg/builds/packages/$FfmpegAsset" $Archive
        Assert-Sha256 -File $Archive -Expected $FfmpegSha256

        Write-Step "Extracting ffmpeg and ffprobe ..."
        $Extracted = Join-Path $TmpDir 'ffmpeg'
        Expand-Zip -Archive $Archive -Destination $Extracted

        $FfmpegBin = Get-ChildItem -LiteralPath $Extracted -Directory -Recurse -Depth 2 -Filter 'bin' |
            Select-Object -First 1
        if ($null -eq $FfmpegBin) {
            throw 'unexpected ffmpeg archive layout: no bin directory'
        }

        # The essentials build is static: a single .exe, no lib\*.dll to ship.
        Copy-Item -LiteralPath (Join-Path $FfmpegBin.FullName 'ffmpeg.exe') -Destination $FfmpegTarget -Force
        Copy-Item -LiteralPath (Join-Path $FfmpegBin.FullName 'ffprobe.exe') -Destination $FfprobeTarget -Force
        Write-Ok "Installed $FfmpegTarget, $FfprobeTarget"
    }

    # --- 4. yt-dlp --------------------------------------------------------
    $YtDlpTarget = Join-Path $BinDir "yt-dlp-$TargetTriple.exe"
    if (Test-Sha256 -File $YtDlpTarget -Expected $YtDlpSha256) {
        Write-Ok "yt-dlp already present, skipping ($TargetTriple)"
    } else {
        $Download = Join-Path $TmpDir $YtDlpAsset
        Get-RemoteFile "https://github.com/yt-dlp/yt-dlp/releases/download/$YtDlpVersion/$YtDlpAsset" $Download
        Assert-Sha256 -File $Download -Expected $YtDlpSha256
        Copy-Item -LiteralPath $Download -Destination $YtDlpTarget -Force
        Write-Ok "Installed $YtDlpTarget"
    }

    # --- 5. Silero VAD (shared with macOS, cross-platform weights) --------
    if (Test-Sha256 -File $SileroTarget -Expected $SileroSha256) {
        Write-Ok "silero_vad_16k.safetensors already present, skipping"
    } else {
        $Download = Join-Path $TmpDir 'silero.safetensors'
        Get-RemoteFile $SileroUrl $Download
        Assert-Sha256 -File $Download -Expected $SileroSha256
        Copy-Item -LiteralPath $Download -Destination $SileroTarget -Force
        Write-Ok "Installed $SileroTarget"
    }

    # --- 6. Resource manifest ---------------------------------------------
    # Same purpose as the macOS script's MANIFEST.txt: it records what the build
    # actually bundles. Generated, never hand-edited, and byte-stable across runs.
    #
    # It is the same file name on purpose — there is one `resources\` directory,
    # and both platform scripts document it. It therefore lists whatever is
    # present, including macOS binaries on a machine that has built both; the
    # per-file triple in the name is what says which platform each belongs to.
    $ManifestPath = Join-Path $ResourcesDir 'MANIFEST.txt'
    $Lines = New-Object System.Collections.Generic.List[string]
    $Lines.Add('# MonoSub 资源清单')
    $Lines.Add('# 由 scripts/windows/prepare-resources.ps1 生成，请勿手工编辑。')
    $Lines.Add('')
    $Lines.Add('## 文件')

    $RelativePaths = Get-ChildItem -LiteralPath $ResourcesDir -Recurse -File |
        Where-Object { $_.Name -ne 'MANIFEST.txt' } |
        ForEach-Object { $_.FullName.Substring($ResourcesDir.Length + 1).Replace('\', '/') } |
        Sort-Object
    foreach ($RelPath in $RelativePaths) {
        $Size = (Get-Item -LiteralPath (Join-Path $ResourcesDir $RelPath.Replace('/', '\'))).Length
        $Lines.Add(('{0,-56} {1}' -f $RelPath, $Size))
    }
    # UTF-8 without BOM: this file is read by humans and diffed, and a BOM would
    # make the first line differ from the macOS script's output.
    [System.IO.File]::WriteAllLines($ManifestPath, $Lines, (New-Object System.Text.UTF8Encoding($false)))
    Write-Ok "已写入 $ManifestPath"

    # --- 7. Smoke check ---------------------------------------------------
    # Each binary must at least start on this machine. A missing DLL shows up
    # here, not as a crash after the package is installed (spec §4.3).
    if (-not $SkipSmokeTest) {
        Write-Step 'Verifying bundled binaries ...'
        & $CliTarget --version
        if ($LASTEXITCODE -ne 0) { throw "audiocpp_cli failed its smoke test (exit $LASTEXITCODE)" }
        & $ServerTarget --version
        if ($LASTEXITCODE -ne 0) { throw "llama-server failed its smoke test (exit $LASTEXITCODE)" }
        & $FfmpegTarget -version | Select-Object -First 1
        if ($LASTEXITCODE -ne 0) { throw "ffmpeg failed its smoke test (exit $LASTEXITCODE)" }
        & $YtDlpTarget --version
        if ($LASTEXITCODE -ne 0) { throw "yt-dlp failed its smoke test (exit $LASTEXITCODE)" }
    }

    Write-Ok 'Windows 资源准备完毕。'
    Write-Host "    $CliTarget"
    Write-Host "    $ServerTarget"
    Write-Host "    $FfmpegTarget"
    Write-Host "    $YtDlpTarget"
    Write-Host "    $SileroTarget"
} finally {
    Remove-Item -LiteralPath $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}

# Explicit, because a .ps1 that ends normally leaves $LASTEXITCODE at whatever
# the last native command inside it returned (measured: a `curl` that succeeded
# and a `--version` that exited 1 both leak through). The caller reads
# $LASTEXITCODE to decide whether to continue, so a stale value would fail a
# build that actually succeeded.
exit 0
