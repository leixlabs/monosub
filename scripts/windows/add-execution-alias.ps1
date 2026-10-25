# Declares a `windows.appExecutionAlias` in a generated Package.appxmanifest.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File add-execution-alias.ps1 `
#       -Manifest <path\to\Package.appxmanifest> `
#       -Executable 'resources\bin\monosub-cli-x86_64-pc-windows-msvc.exe' `
#       -Alias 'monosub-cli.exe'
#
# Why a script and not a text substitution: the alias needs namespace prefixes
# (uap3, desktop and their IgnorableNamespaces entries) on <Package> and an
# element inserted inside the existing <Application>. A substitution that
# half-applies yields a manifest that packs fine and fails certification, which
# is the worst possible failure point. The XML DOM either applies the whole
# change or exits non-zero.
#
# The alias MUST sit inside <Application>\<Extensions>, not directly under
# <Application>. Measured with makeappx (Windows SDK 10.0.26100):
#
#   <Application><uap3:Extension .../></Application>
#     -> error C00CE014: manifest is not valid per schema
#   <Application><Extensions><uap3:Extension .../></Extensions></Application>
#     -> packs cleanly
#
# `Extensions` is a named element in the Application schema (`CT_Application`
# has an `xs:all` of VisualElements / ApplicationContentUriRules / Extensions),
# so an Extension cannot be its sibling. The generated manifest has no
# <Extensions> until something needs one, hence the create-if-missing below.
#
# Idempotent: re-running removes a previously added alias of the same name first,
# so a rebuild cannot accumulate duplicate Extension elements.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Manifest,
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][string]$Alias
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# The Chinese-free messages below are still written through Write-Host so the
# script stays readable in a plain console; the encoding line keeps any future
# localised text from arriving as mojibake.
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

$UAP3 = 'http://schemas.microsoft.com/appx/manifest/uap/windows10/3'
$DESKTOP = 'http://schemas.microsoft.com/appx/manifest/desktop/windows10'
$FOUNDATION = 'http://schemas.microsoft.com/appx/manifest/foundation/windows10'

if (-not (Test-Path -LiteralPath $Manifest)) {
    Write-Error "Manifest not found: $Manifest"
    exit 1
}

[xml]$doc = Get-Content -LiteralPath $Manifest -Raw
$package = $doc.DocumentElement
if ($null -eq $package -or $package.LocalName -ne 'Package') {
    Write-Error "Not a Package manifest: $Manifest"
    exit 1
}

# 1. Namespace declarations on <Package>. winapp's generated manifest carries the
#    foundation namespace and usually uap3/desktop already; they are added only
#    when missing. SetAttribute("xmlns:x", ...) is how the DOM records a
#    namespace declaration, and GetNamespaceOfPrefix is what reads one back.
function Ensure-Namespace([string]$Prefix, [string]$Uri) {
    $existing = $package.GetNamespaceOfPrefix($Prefix)
    if ($existing -ne $Uri) {
        $package.SetAttribute("xmlns:$Prefix", $Uri)
    }
}

Ensure-Namespace 'uap3' $UAP3
Ensure-Namespace 'desktop' $DESKTOP

# 2. IgnorableNamespaces must list the prefixes the package does not otherwise
#    use, or the packaging tool rejects the manifest as containing unknown parts.
$ignorable = $package.GetAttribute('IgnorableNamespaces')
$prefixes = @($ignorable -split '\s+' | Where-Object { $_ })
foreach ($needed in @('uap3', 'desktop')) {
    if ($prefixes -notcontains $needed) {
        $prefixes += $needed
    }
}
$package.SetAttribute('IgnorableNamespaces', ($prefixes -join ' '))

# 3. Exactly one <Application> carries the entry point.
#
#    The manager is built *after* step 1: a prefix that was only declared above
#    is not visible to a manager created earlier, and `SelectNodes` then throws
#    "namespace prefix uap3 is not defined" (measured).
$ns = New-Object System.Xml.XmlNamespaceManager($doc.NameTable)
$ns.AddNamespace('f', $FOUNDATION)
$ns.AddNamespace('uap3', $UAP3)
$ns.AddNamespace('desktop', $DESKTOP)
$applications = $package.SelectNodes('f:Applications/f:Application', $ns)
if ($applications.Count -ne 1) {
    Write-Error "Expected exactly one <Application>, found $($applications.Count)"
    exit 1
}
$application = $applications[0]

# 4. Drop any alias we added on a previous run, then add the current one.
#
#    Both nesting shapes are searched: an older build of this script appended the
#    Extension directly to <Application>, and a manifest left behind by that
#    version must still be cleanable rather than failing the "not owned by this
#    script" guard below.
$existingAliases = @()
$extensionsNode = $application.SelectSingleNode('f:Extensions', $ns)
if ($null -ne $extensionsNode) {
    $existingAliases += @($extensionsNode.SelectNodes(
        'uap3:Extension[@Category="windows.appExecutionAlias"]', $ns))
}
$existingAliases += @($application.SelectNodes(
    'uap3:Extension[@Category="windows.appExecutionAlias"]', $ns))

foreach ($old in $existingAliases) {
    # Matched by local name, not by prefix: winapp's own `manifest add-alias`
    # writes a uap5 variant, and either flavour has to be recognised as ours or
    # a rebuild would leave two aliases with the same name in the manifest.
    if ($old.SelectSingleNode('*[local-name()="AppExecutionAlias"]/*[local-name()="ExecutionAlias"]')) {
        [void]$old.ParentNode.RemoveChild($old)
    } else {
        # Someone else's alias: refuse rather than silently replacing it.
        Write-Error 'An appExecutionAlias not owned by this script is already declared'
        exit 1
    }
}

# 5. Create <Extensions> if the generated manifest has none, and append there.
#    `CT_Application` is an `xs:all`, so the order relative to VisualElements does
#    not matter — but appending last is what the schema validator accepts and what
#    winapp's own `manifest add-alias` produces.
$extensionsNode = $application.SelectSingleNode('f:Extensions', $ns)
if ($null -eq $extensionsNode) {
    $extensionsNode = $doc.CreateElement('Extensions', $FOUNDATION)
    [void]$application.AppendChild($extensionsNode)
}

$extension = $doc.CreateElement('uap3', 'Extension', $UAP3)
$extension.SetAttribute('Category', 'windows.appExecutionAlias')
$extension.SetAttribute('Executable', $Executable)
# Required: without it the activation does not forward arguments to the binary.
$extension.SetAttribute('EntryPoint', 'Windows.FullTrustApplication')

$appAlias = $doc.CreateElement('uap3', 'AppExecutionAlias', $UAP3)
$executionAlias = $doc.CreateElement('desktop', 'ExecutionAlias', $DESKTOP)
$executionAlias.SetAttribute('Alias', $Alias)

[void]$appAlias.AppendChild($executionAlias)
[void]$extension.AppendChild($appAlias)
[void]$extensionsNode.AppendChild($extension)

# 6. Write back as UTF-8 without a BOM: appx packaging rejects a BOM-prefixed
#    manifest, and PowerShell 5.1's `Out-File -Encoding utf8` adds one.
$settings = New-Object System.Xml.XmlWriterSettings
$settings.Encoding = New-Object System.Text.UTF8Encoding($false)
$settings.Indent = $true
$writer = [System.Xml.XmlWriter]::Create($Manifest, $settings)
try {
    $doc.Save($writer)
} finally {
    $writer.Close()
}

Write-Output "Declared $Alias -> $Executable"

# Explicit exit 0: a .ps1 that ends normally inherits $LASTEXITCODE from the last
# native command it ran, and the build script reads that value to decide whether
# to continue.
exit 0
