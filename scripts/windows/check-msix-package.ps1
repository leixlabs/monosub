# Requires: Windows PowerShell 5.1 (in-box) or PowerShell 7.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\check-msix-package.ps1 `
#       -Package dist\monosub_2.0.0.0_x64.msix -ExpectedVersion 2.0.0.0
#
# Verifies the acceptance checklist from the Microsoft Store spec (§5.7) against
# a built package, so a bad package fails here rather than at certification.
#
# An MSIX is an OPC/ZIP container, so its contents can be listed and read without
# unpacking it. That keeps this check dependency-free (no makeappx, no unzip) and
# readable — every failure message names the missing file directly.
#
# This is a PowerShell script because the checks that matter most are structural:
# "is the appExecutionAlias a child of <Application>?" and "does the manifest
# parse?" are XML questions, and the bash version answered them by piping the
# manifest into an inline `powershell -Command` with `\"`-escaped XPath. PowerShell
# escapes with a backtick, not a backslash, so the inline form received literal
# `\"` and failed to parse on every run — the check reported "unparseable" and
# failed unconditionally, whatever the manifest said. One runtime removes that
# whole class of bug.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Package,
    [string]$ExpectedVersion = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

$TargetTriple = 'x86_64-pc-windows-msvc'

function Write-Info([string]$Message) { Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Ok([string]$Message) { Write-Host "[OK] $Message" -ForegroundColor Green }
function Write-Err([string]$Message) { Write-Host "[ERROR] $Message" -ForegroundColor Red }

if (-not (Test-Path -LiteralPath $Package)) {
    Write-Err "usage: check-msix-package.ps1 -Package <path-to-msix> [-ExpectedVersion <x.y.z.w>]"
    Write-Err "no such package: $Package"
    exit 1
}

$script:Failed = 0
function Add-Failure([string]$Message) {
    Write-Err $Message
    $script:Failed++
}

Write-Info "Checking $(Split-Path -Leaf $Package)"

# --- Read the container ---------------------------------------------------
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead((Resolve-Path -LiteralPath $Package).Path)
try {
    $entries = @{}
    foreach ($entry in $zip.Entries) { $entries[$entry.FullName] = $entry }

    # --- File name --------------------------------------------------------
    # Partner Center rejects an upload whose architecture it cannot infer, and
    # the name is how a human picks the right file among several builds.
    $baseName = Split-Path -Leaf $Package
    if ($baseName -match '_x64(\.signed)?\.msix$') {
        Write-Ok "File name declares x64: $baseName"
    } else {
        Add-Failure "File name does not end in _x64.msix: $baseName"
    }

    # --- Contents ---------------------------------------------------------
    $required = [ordered]@{
        'AppxManifest.xml'                                             = 'Manifest'
        'monosub.exe'                                                  = 'Entry point'
        "resources/bin/audiocpp_cli-$TargetTriple.exe"                 = 'audio.cpp CLI'
        "resources/bin/ffmpeg-$TargetTriple.exe"                       = 'FFmpeg'
        "resources/bin/ffprobe-$TargetTriple.exe"                      = 'FFprobe'
        "resources/bin/llama-server-$TargetTriple.exe"                 = 'llama.cpp server'
        "resources/bin/yt-dlp-$TargetTriple.exe"                       = 'yt-dlp'
        'resources/models.json'                                        = 'Model catalogue'
        'resources/models/silero_vad/silero_vad_16k.safetensors'       = 'Silero VAD model'
        # The command-line tool ships too, and must be *reachable from a
        # terminal*: the manifest declares an execution alias rather than
        # relying on a PATH entry, because the install directory is
        # version-stamped and changes on every update. A package with the file
        # but no alias passes packing and looks fine until a user types
        # `monosub-cli` and gets "not recognized".
        "resources/bin/monosub-cli-$TargetTriple.exe"                  = 'Command-line tool'
    }
    foreach ($path in $required.Keys) {
        if ($entries.ContainsKey($path)) {
            Write-Ok "$($required[$path]): $path"
        } else {
            Add-Failure "$($required[$path]) missing from the package: $path"
        }
    }

    # --- DLLs -------------------------------------------------------------
    # A missing DLL only shows up as a crash at run time, so require at least
    # one of each family the two engines need.
    $dllCount = @($entries.Keys | Where-Object { $_ -match '^resources/bin/.*\.dll$' }).Count
    if ($dllCount -ge 10) {
        Write-Ok "Runtime DLLs present ($dllCount)"
    } else {
        Add-Failure "Only $dllCount DLL(s) in the package; the engines cannot start"
    }
    if (-not $entries.ContainsKey('resources/bin/llama-server-impl.dll')) {
        Add-Failure 'llama-server-impl.dll missing (llama-server is a thin launcher over it)'
    }
    if (-not $entries.ContainsKey('resources/bin/vcruntime140.dll')) {
        Add-Failure 'vcruntime140.dll missing (the MSVC runtime is not assumed on the target)'
    }

    # --- Manifest identity ------------------------------------------------
    $manifestText = $null
    if ($entries.ContainsKey('AppxManifest.xml')) {
        $reader = New-Object System.IO.StreamReader($entries['AppxManifest.xml'].Open())
        try { $manifestText = $reader.ReadToEnd() } finally { $reader.Close() }
    }
    if ($null -eq $manifestText) {
        Add-Failure 'AppxManifest.xml is unreadable; every manifest check below is skipped'
    } else {
        [xml]$manifest = $manifestText
        $packageNode = $manifest.DocumentElement

        # The version is compared against tauri.conf.json by the build script;
        # here it only has to be four-part, which is the form the Store requires.
        $identity = $packageNode.SelectSingleNode(
            './*[local-name()="Identity"]')
        if ($null -eq $identity) {
            Add-Failure 'Manifest has no <Identity>'
        } else {
            $identityVersion = $identity.GetAttribute('Version')
            if ($identityVersion -match '^\d+\.\d+\.\d+\.\d+$') {
                Write-Ok "Manifest version: $identityVersion"
                if ($ExpectedVersion -and $identityVersion -ne $ExpectedVersion) {
                    Add-Failure "Manifest version $identityVersion does not match expected $ExpectedVersion"
                }
            } else {
                Add-Failure "Manifest Version is not four-part: '$identityVersion'"
            }

            # x64 is what the bundled binaries are, and what the Store expects
            # for this package name. Absent ProcessorArchitecture is legal (the
            # package then applies to any architecture) but wrong here.
            $arch = $identity.GetAttribute('ProcessorArchitecture')
            if ($arch -eq 'x64') {
                Write-Ok 'Processor architecture: x64'
            } elseif ([string]::IsNullOrEmpty($arch)) {
                Add-Failure 'Identity declares no ProcessorArchitecture; expected x64'
            } else {
                Add-Failure "Identity declares ProcessorArchitecture '$arch'; expected x64"
            }
        }

        # WebView2 must NOT be declared as a PackageDependency: it ships as an
        # Evergreen runtime, not an MSIX framework, so the declaration makes the
        # package fail to install with 0x80073CF3 even on machines that have it.
        if ($manifestText -match 'Microsoft\.Win32WebView2') {
            Add-Failure ('Manifest declares Microsoft.Win32WebView2 as a framework dependency; ' +
                'WebView2 is an Evergreen runtime, and this makes Add-AppxPackage fail with 0x80073CF3')
        } else {
            Write-Ok 'No WebView2 framework dependency (correct: it is an Evergreen runtime)'
        }

        $applications = $packageNode.SelectNodes('./*[local-name()="Applications"]/*[local-name()="Application"]')
        if ($applications.Count -eq 0) {
            Add-Failure 'Manifest declares no <Application>'
        }

        $entryPointOk = $false
        foreach ($app in $applications) {
            if ($app.GetAttribute('Executable') -eq 'monosub.exe') { $entryPointOk = $true }
        }
        if ($entryPointOk) {
            Write-Ok 'Entry point pinned to monosub.exe'
        } else {
            Add-Failure 'Manifest entry point is not monosub.exe'
        }

        # --- User-visible name -------------------------------------------
        # Two different DisplayNames exist and only one of them is the name a
        # user reads. The Start menu and the tile use
        # <Application>\<VisualElements DisplayName>; Settings → Apps uses
        # <Properties>\<DisplayName>. The generator derives both from the
        # lowercase package name, so leaving either unpatched ships `monosub`
        # where `MonoSub` belongs — measured on a real build: Properties said
        # MonoSub while VisualElements still said monosub, and nothing failed
        # because this check did not exist.
        $propertiesNode = $packageNode.SelectSingleNode('./*[local-name()="Properties"]')
        $propertiesName = if ($null -ne $propertiesNode) {
            $n = $propertiesNode.SelectSingleNode('./*[local-name()="DisplayName"]')
            if ($null -ne $n) { $n.InnerText } else { '' }
        } else { '' }
        # -ceq, not -eq: PowerShell's -eq on strings is case-INSENSITIVE, so a
        # `-eq` here accepted a package whose name really was lowercase
        # `monosub` — the exact defect this check exists to catch (measured).
        if ($propertiesName -ceq 'MonoSub') {
            Write-Ok 'Properties DisplayName: MonoSub'
        } else {
            Add-Failure "Properties DisplayName is '$propertiesName'; expected 'MonoSub'"
        }

        $visualNameOk = $false
        foreach ($app in $applications) {
            $visualElements = $app.SelectSingleNode('./*[local-name()="VisualElements"]')
            if ($null -ne $visualElements -and $visualElements.GetAttribute('DisplayName') -ceq 'MonoSub') {
                $visualNameOk = $true
            }
        }
        if ($visualNameOk) {
            Write-Ok 'Start-menu DisplayName: MonoSub'
        } else {
            Add-Failure ('VisualElements DisplayName is not MonoSub; the Start menu and tile ' +
                'would show the lowercase package name')
        }

        # --- Execution alias ---------------------------------------------
        # The CLI reaches a terminal through this alias, not through PATH. Both
        # halves are needed: the Extension carries the target, the
        # ExecutionAlias carries the name the user types, and
        # EntryPoint="Windows.FullTrustApplication" forwards arguments.
        #
        # The Extension must be a child of <Application> *under <Extensions>*;
        # an alias anywhere else is ignored by the package runtime with no
        # error. That is checked as a real DOM query rather than a text search,
        # because the whole point is to catch a manifest that is well-formed but
        # structurally wrong — which a regex cannot see. Measured with makeappx:
        # an Extension appended directly to <Application> fails the schema with
        # C00CE014, so this check also catches "it packed, but only by luck".
        $aliasOk = $false
        $aliasNameOk = $false
        $aliasTargetOk = $false
        foreach ($app in $applications) {
            $aliasExtensions = $app.SelectNodes(
                './*[local-name()="Extensions"]/*[local-name()="Extension"]' +
                '[@Category="windows.appExecutionAlias"]')
            foreach ($ext in $aliasExtensions) {
                $aliasOk = $true
                if ($ext.GetAttribute('Executable') -eq "resources\bin\monosub-cli-$TargetTriple.exe") {
                    $aliasTargetOk = $true
                }
                $aliases = $ext.SelectNodes(
                    './*[local-name()="AppExecutionAlias"]/*[local-name()="ExecutionAlias"]')
                foreach ($a in $aliases) {
                    if ($a.GetAttribute('Alias') -eq 'monosub-cli.exe') { $aliasNameOk = $true }
                }
            }
        }
        if ($aliasOk) {
            Write-Ok 'Execution alias declared inside <Application>/<Extensions>'
        } else {
            Add-Failure ('No windows.appExecutionAlias under <Application>/<Extensions>; ' +
                'typing monosub-cli would fail (an alias outside that node is ignored)')
        }
        if ($aliasNameOk) {
            Write-Ok 'Alias name is monosub-cli.exe'
        } else {
            Add-Failure 'The execution alias is not named monosub-cli.exe'
        }
        if ($aliasTargetOk) {
            Write-Ok "Alias targets resources\bin\monosub-cli-$TargetTriple.exe"
        } else {
            Add-Failure ("Alias does not target resources\bin\monosub-cli-$TargetTriple.exe; " +
                'it would launch a different binary')
        }

        $capabilities = $packageNode.SelectNodes('./*[local-name()="Capabilities"]')
        $hasRunFullTrust = $false
        foreach ($caps in $capabilities) {
            foreach ($cap in $caps.ChildNodes) {
                if ($cap.NodeType -eq 'Element' -and $cap.GetAttribute('Name') -eq 'runFullTrust') {
                    $hasRunFullTrust = $true
                }
            }
        }
        if ($hasRunFullTrust) {
            Write-Ok 'runFullTrust capability declared'
        } else {
            Add-Failure 'runFullTrust missing; a desktop app launched from the Store needs it'
        }
    }
} finally {
    $zip.Dispose()
}

# --- Result ---------------------------------------------------------------
if ($script:Failed -gt 0) {
    Write-Err "$script:Failed check(s) failed"
    exit 1
}
Write-Ok 'All package checks passed'
exit 0
