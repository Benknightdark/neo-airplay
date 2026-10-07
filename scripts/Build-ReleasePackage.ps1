[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) {
    throw 'Build-ReleasePackage.ps1 creates Windows packages and must run on Windows.'
}
if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "Invalid stable App version: $Version"
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$buildDirectory = Join-Path $projectRoot '.build'
$releaseRoot = Join-Path $buildDirectory 'release-assets'
$outputDirectory = Join-Path $releaseRoot $Version
$tauriRoot = Join-Path $projectRoot 'src-tauri'
$engineArchive = Join-Path $buildDirectory 'neo-airplay-engine-windows-x86_64.zip'
$engineSettings = Join-Path $tauriRoot 'engine-package.json'
$portableName = "NeoAirPlay-$Version-windows-x86_64.zip"
$installerName = "NeoAirPlay-$Version-windows-x86_64-setup.exe"
$portableArchive = Join-Path $outputDirectory $portableName
$installerArchive = Join-Path $outputDirectory $installerName

$resolvedReleaseRoot = [IO.Path]::GetFullPath($releaseRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
$resolvedOutputDirectory = [IO.Path]::GetFullPath($outputDirectory)
if (Test-Path -LiteralPath $resolvedOutputDirectory) {
    if (-not $resolvedOutputDirectory.StartsWith($resolvedReleaseRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove a release output outside .build/release-assets: $resolvedOutputDirectory"
    }
    Remove-Item -LiteralPath $resolvedOutputDirectory -Recurse -Force
}
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

& (Join-Path $PSScriptRoot 'build-engine.ps1') -Target windows-x86_64
& (Join-Path $PSScriptRoot 'Pack-EngineRuntime.ps1') -Target windows-x86_64 -Version $Version
if (-not (Test-Path -LiteralPath $engineArchive -PathType Leaf) -or
    -not (Test-Path -LiteralPath $engineSettings -PathType Leaf)) {
    throw 'Engine packaging did not produce the archive and schema v2 metadata.'
}

$metadata = Get-Content -LiteralPath $engineSettings -Raw -Encoding UTF8 | ConvertFrom-Json
$target = $metadata.Targets.'windows-x86_64'
$engineHash = (Get-FileHash -LiteralPath $engineArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($metadata.SchemaVersion -ne 2 -or
    $metadata.Version -ne $Version -or
    $metadata.ReleaseTag -ne "v$Version" -or
    $target.AssetName -ne 'neo-airplay-engine-windows-x86_64.zip' -or
    -not [string]::Equals($target.Sha256, $engineHash, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Windows engine metadata does not match the generated archive.'
}

Push-Location $projectRoot
try {
    & bun tauri build --bundles nsis
    if ($LASTEXITCODE -ne 0) {
        throw "Tauri NSIS build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

$installerDirectory = Join-Path $projectRoot 'target/release/bundle/nsis'
$installer = Get-ChildItem -LiteralPath $installerDirectory -File -Filter '*-setup.exe' |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $installer) {
    throw "Tauri build did not produce an NSIS installer in $installerDirectory"
}
Copy-Item -LiteralPath $installer.FullName -Destination $installerArchive -Force

$portableDirectory = Join-Path $buildDirectory ("portable-stage-" + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $portableDirectory -Force | Out-Null
    $appExecutable = Join-Path $projectRoot 'target/release/neo-airplay.exe'
    if (-not (Test-Path -LiteralPath $appExecutable -PathType Leaf)) {
        throw "Tauri release executable is missing: $appExecutable"
    }
    Copy-Item -LiteralPath $appExecutable -Destination $portableDirectory
    Copy-Item -LiteralPath $engineSettings -Destination $portableDirectory
    Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $portableDirectory

    $portableScripts = Join-Path $portableDirectory 'scripts'
    New-Item -ItemType Directory -Path $portableScripts -Force | Out-Null
    foreach ($file in @(
        'BonjourRuntimeHelpers.psm1',
        'BonjourServiceSettings.json',
        'Install-BonjourService.ps1',
        'Stage-BonjourRuntime.ps1',
        'Restore-BonjourService.ps1'
    )) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $portableScripts
    }

    foreach ($helper in @('install-uxplay-macos.sh', 'install-uxplay-linux.sh', 'uxplay-release.json')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $helper) -Destination $outputDirectory -Force
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::CreateFromDirectory(
        $portableDirectory,
        $portableArchive,
        [IO.Compression.CompressionLevel]::Optimal,
        $false)
}
finally {
    if (Test-Path -LiteralPath $portableDirectory -PathType Container) {
        $resolvedPortable = [IO.Path]::GetFullPath($portableDirectory)
        $resolvedBuildRoot = [IO.Path]::GetFullPath($buildDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        if (-not $resolvedPortable.StartsWith($resolvedBuildRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a portable staging directory outside .build: $resolvedPortable"
        }
        Remove-Item -LiteralPath $resolvedPortable -Recurse -Force
    }
}

Copy-Item -LiteralPath $engineArchive -Destination (Join-Path $outputDirectory ([IO.Path]::GetFileName($engineArchive))) -Force
Copy-Item -LiteralPath $engineSettings -Destination (Join-Path $outputDirectory 'engine-package.json') -Force

function Write-Sha256Sidecar {
    param([Parameter(Mandatory)][string]$Path)
    $fileName = [IO.Path]::GetFileName($Path)
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$Path.sha256", "$hash  $fileName$([Environment]::NewLine)", [Text.ASCIIEncoding]::new())
    "$hash  $fileName"
}

$checksums = @(
    (Write-Sha256Sidecar $installerArchive)
    (Write-Sha256Sidecar $portableArchive)
    (Write-Sha256Sidecar (Join-Path $outputDirectory ([IO.Path]::GetFileName($engineArchive))))
)
[IO.File]::WriteAllLines((Join-Path $outputDirectory 'SHA256SUMS.txt'), $checksums, [Text.ASCIIEncoding]::new())
[IO.File]::WriteAllText((Join-Path $outputDirectory 'release-version.txt'), "$Version$([Environment]::NewLine)", [Text.ASCIIEncoding]::new())

Add-Type -AssemblyName System.IO.Compression.FileSystem
foreach ($archivePath in @($portableArchive, (Join-Path $outputDirectory ([IO.Path]::GetFileName($engineArchive))))) {
    $archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
    try {
        $forbidden = @($archive.Entries | Where-Object {
            $_.FullName -match '(^|/)engine/bonjour(/|$)' -or
            $_.FullName -match '(^|/)(mDNSResponder\.exe|dnssd\.dll)$'
        })
        if ($forbidden.Count -gt 0) {
            throw "A public package contains Bonjour binaries: $($forbidden.FullName -join ', ')"
        }
    }
    finally {
        $archive.Dispose()
    }
}

Write-Output "Windows NSIS installer: $installerArchive"
Write-Output "Windows portable package: $portableArchive"
Write-Output "Windows engine package: $(Join-Path $outputDirectory ([IO.Path]::GetFileName($engineArchive)))"
