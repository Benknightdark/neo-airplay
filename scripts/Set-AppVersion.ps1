[CmdletBinding()]
param(
    [string]$Version,
    [string]$ProjectRoot = (Split-Path -Parent $PSScriptRoot),
    [switch]$SkipLockUpdate
)

$ErrorActionPreference = 'Stop'
$packagePath = Join-Path $ProjectRoot 'package.json'
$cargoPath = Join-Path $ProjectRoot 'Cargo.toml'
$enginePackagePath = Join-Path $ProjectRoot 'src-tauri/engine-package.json'
foreach ($path in @($packagePath, $cargoPath)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required version file does not exist: $path"
    }
}

$packageContent = [IO.File]::ReadAllText($packagePath)
$package = $packageContent | ConvertFrom-Json -AsHashtable
$currentVersion = [string]$package.version
if ($currentVersion -notmatch '^(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)$') {
    throw "package.json does not contain a stable three-part SemVer version: $currentVersion"
}
if ([string]::IsNullOrWhiteSpace($Version)) {
    $patch = [long]::Parse($Matches.patch, [Globalization.CultureInfo]::InvariantCulture)
    if ($patch -eq [long]::MaxValue) {
        throw "App patch version has reached its limit: $currentVersion"
    }
    $Version = '{0}.{1}.{2}' -f $Matches.major, $Matches.minor, ($patch + 1)
}
elseif ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "Requested version must be a stable three-part SemVer value: $Version"
}

$packageVersionPattern = '(?m)^(\s*"version"\s*:\s*")[^"]+(")'
$packageVersionMatch = [regex]::Matches($packageContent, $packageVersionPattern)
if ($packageVersionMatch.Count -ne 1) {
    throw "Could not uniquely locate the root package version in $packagePath"
}
$packageContent = [regex]::Replace(
    $packageContent,
    $packageVersionPattern,
    [Text.RegularExpressions.MatchEvaluator]{
        param($match)
        $match.Groups[1].Value + $Version + $match.Groups[2].Value
    },
    1)

$cargoContent = [IO.File]::ReadAllText($cargoPath)
$workspaceSectionMatch = [regex]::Match($cargoContent, '(?m)^\[workspace\.package\]\r?$')
if (-not $workspaceSectionMatch.Success) {
    throw "Cargo.toml is missing the [workspace.package] section."
}
$sectionStart = $workspaceSectionMatch.Index
$sectionHeaderRegex = [regex]::new('(?m)^\[')
$sectionEndMatch = $sectionHeaderRegex.Match($cargoContent, $sectionStart + $workspaceSectionMatch.Length)
$sectionEnd = if ($sectionEndMatch.Success) { $sectionEndMatch.Index } else { $cargoContent.Length }
$workspaceSection = $cargoContent.Substring($sectionStart, $sectionEnd - $sectionStart)
$cargoVersionPattern = '(?m)^(\s*version\s*=\s*")[^"]+(")'
$cargoVersionMatches = [regex]::Matches($workspaceSection, $cargoVersionPattern)
if ($cargoVersionMatches.Count -ne 1) {
    throw "Cargo.toml [workspace.package] must contain exactly one version."
}
$updatedWorkspaceSection = [regex]::Replace(
    $workspaceSection,
    $cargoVersionPattern,
    [Text.RegularExpressions.MatchEvaluator]{
        param($match)
        $match.Groups[1].Value + $Version + $match.Groups[2].Value
    },
    1)
$cargoContent = $cargoContent.Substring(0, $sectionStart) + $updatedWorkspaceSection + $cargoContent.Substring($sectionEnd)

$enginePackageContent = $null
if (Test-Path -LiteralPath $enginePackagePath -PathType Leaf) {
    $enginePackage = Get-Content -LiteralPath $enginePackagePath -Raw -Encoding UTF8 | ConvertFrom-Json -AsHashtable
    if ($enginePackage.Version -ne $Version) {
        $enginePackage = [ordered]@{
            SchemaVersion = 2
            Version = $Version
            ReleaseTag = "v$Version"
            ManifestName = 'engine-package-manifest.json'
            Targets = [ordered]@{}
        }
    }
    else {
        $enginePackage.SchemaVersion = 2
        $enginePackage.ReleaseTag = "v$Version"
        $enginePackage.ManifestName = 'engine-package-manifest.json'
    }
    $enginePackageContent = $enginePackage | ConvertTo-Json -Depth 10
}

function Write-AtomicText {
    param([string]$Path, [string]$Content)
    $temporaryPath = "$Path.tmp"
    [IO.File]::WriteAllText($temporaryPath, $Content, [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporaryPath -Destination $Path -Force
}

Write-AtomicText -Path $packagePath -Content $packageContent
Write-AtomicText -Path $cargoPath -Content $cargoContent
if ($null -ne $enginePackageContent) {
    Write-AtomicText -Path $enginePackagePath -Content $enginePackageContent
}

if (-not $SkipLockUpdate) {
    Push-Location $ProjectRoot
    try {
        & cargo update --workspace --quiet
        if ($LASTEXITCODE -ne 0) {
            throw "cargo update --workspace failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

Write-Output $Version
