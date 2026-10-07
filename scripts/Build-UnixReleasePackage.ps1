#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$')][string]$Version,
    [Parameter(Mandatory)][ValidateSet('macos-aarch64', 'macos-x86_64', 'linux-x86_64')][string]$Target
)

$ErrorActionPreference = 'Stop'
if (($Target -like 'macos-*' -and -not $IsMacOS) -or ($Target -eq 'linux-x86_64' -and -not $IsLinux)) {
    throw "Target $Target must be built on its native operating system."
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$buildRoot = Join-Path $projectRoot '.build'
$outputDirectory = Join-Path $buildRoot "release-assets/$Version/$Target"
$engineArchiveName = "neo-airplay-engine-$Target.zip"
$engineArchive = Join-Path $buildRoot $engineArchiveName
$engineSettingsPath = Join-Path $projectRoot 'src-tauri/engine-package.json'
$targetTriple = switch ($Target) {
    'macos-aarch64' { 'aarch64-apple-darwin' }
    'macos-x86_64' { 'x86_64-apple-darwin' }
    'linux-x86_64' { 'x86_64-unknown-linux-gnu' }
}

New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
& (Join-Path $PSScriptRoot 'Set-AppVersion.ps1') -Version $Version -SkipLockUpdate

& (Join-Path $PSScriptRoot 'Stage-ScrcpyRuntime.ps1') -Target $Target
& (Join-Path $PSScriptRoot 'Pack-EngineRuntime.ps1') -Target $Target -Version $Version

$metadata = Get-Content -LiteralPath $engineSettingsPath -Raw -Encoding UTF8 | ConvertFrom-Json
$targetSettings = $metadata.Targets.$Target
$archiveHash = (Get-FileHash -LiteralPath $engineArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($metadata.SchemaVersion -ne 2 -or
    $metadata.Version -ne $Version -or
    $metadata.ReleaseTag -ne "v$Version" -or
    $targetSettings.AssetName -ne $engineArchiveName -or
    -not [string]::Equals($targetSettings.Sha256, $archiveHash, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Engine metadata does not match the $Target package."
}

Push-Location $projectRoot
try {
    if ($IsMacOS) {
        & rustup target add $targetTriple --toolchain 1.99.0
        if ($LASTEXITCODE -ne 0) {
            throw "Could not install Rust target $targetTriple."
        }
        & bun tauri build --target $targetTriple --bundles dmg
    }
    else {
        & bun tauri build --target $targetTriple --bundles appimage,deb
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri bundle build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

$bundleDirectory = Join-Path $projectRoot "target/$targetTriple/release/bundle"
$patterns = if ($Target -like 'macos-*') {
    @('dmg/*.dmg')
}
else {
    @('appimage/*.AppImage', 'deb/*.deb')
}
$bundles = @(
    foreach ($pattern in $patterns) {
        Get-ChildItem -Path (Join-Path $bundleDirectory $pattern) -File -ErrorAction SilentlyContinue
    }
)
if ($bundles.Count -ne $patterns.Count) {
    throw "Expected $($patterns.Count) app bundle(s) for $Target in $bundleDirectory; found $($bundles.Count)."
}

foreach ($bundle in $bundles) {
    $extension = $bundle.Extension.ToLowerInvariant()
    $destination = Join-Path $outputDirectory "NeoAirPlay-$Version-$Target$extension"
    Copy-Item -LiteralPath $bundle.FullName -Destination $destination -Force
}
Copy-Item -LiteralPath $engineArchive -Destination $outputDirectory -Force
Copy-Item -LiteralPath "$engineArchive.sha256" -Destination $outputDirectory -Force
Copy-Item -LiteralPath $engineSettingsPath -Destination $outputDirectory -Force

Write-Output "Created $Target release bundles in $outputDirectory"
