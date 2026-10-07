[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('windows-x86_64', 'macos-aarch64', 'macos-x86_64', 'linux-x86_64')]
    [string]$Target
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$buildRoot = Join-Path $projectRoot '.build'
$releaseSettingsPath = Join-Path $PSScriptRoot 'scrcpy-release.json'
$requiredFilesPath = Join-Path $PSScriptRoot "scrcpy-runtime-files.$Target.txt"
$releaseSettings = Get-Content -LiteralPath $releaseSettingsPath -Raw -Encoding UTF8 | ConvertFrom-Json
$release = $releaseSettings.Targets.$Target
if (-not $release -or $releaseSettings.Version -ne '5.0') {
    throw "scrcpy release settings do not contain target $Target."
}
if (-not (Test-Path -LiteralPath $requiredFilesPath -PathType Leaf)) {
    throw "Missing required runtime manifest: $requiredFilesPath"
}

$assetName = [string]$release.AssetName
if ($assetName -notmatch '^scrcpy-[A-Za-z0-9-]+-v5\.0\.(zip|tar\.gz)$' -or
    [string]$release.Sha256 -notmatch '^[A-Fa-f0-9]{64}$' -or
    [string]$release.DirectoryName -notmatch '^scrcpy-[A-Za-z0-9-]+-v5\.0$' -or
    $release.ArchiveType -notin @('zip', 'tar.gz')) {
    throw "Invalid pinned scrcpy settings for target $Target."
}

$archivePath = Join-Path $buildRoot $assetName
$extractPath = Join-Path $buildRoot ("scrcpy-stage-" + [guid]::NewGuid().ToString('N'))
$engineDirectory = Join-Path (Join-Path $projectRoot 'engine') 'android'
$releaseUrl = "https://github.com/Genymobile/scrcpy/releases/download/v$($releaseSettings.Version)/$assetName"
$requiredFiles = @(Get-Content -LiteralPath $requiredFilesPath -Encoding UTF8 | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$duplicateFiles = @($requiredFiles | Group-Object { $_.ToLowerInvariant() } | Where-Object Count -gt 1)
if ($requiredFiles.Count -eq 0 -or $duplicateFiles.Count -gt 0 -or
    @($requiredFiles | Where-Object { $_ -match '[/\\:]' }).Count -gt 0) {
    throw "The runtime manifest for $Target is empty, duplicated, or contains an unsafe file name."
}

New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null
try {
    if (Test-Path -LiteralPath $archivePath -PathType Leaf) {
        $cachedHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
        if (-not [string]::Equals($cachedHash, $release.Sha256, [StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $archivePath -Force
        }
    }

    if (-not (Test-Path -LiteralPath $archivePath -PathType Leaf)) {
        Invoke-WebRequest -Uri $releaseUrl -OutFile $archivePath -MaximumRedirection 10
    }
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
    if (-not [string]::Equals($actualHash, $release.Sha256, [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $archivePath -Force
        throw "scrcpy v5.0 SHA-256 mismatch for $Target. Expected $($release.Sha256), received $actualHash."
    }

    New-Item -ItemType Directory -Path $extractPath -Force | Out-Null
    if ($release.ArchiveType -eq 'zip') {
        Expand-Archive -LiteralPath $archivePath -DestinationPath $extractPath -Force
    }
    else {
        & tar -xzf $archivePath -C $extractPath
        if ($LASTEXITCODE -ne 0) {
            throw "Could not extract the scrcpy v5.0 archive for $Target."
        }
    }

    $releaseDirectory = Join-Path $extractPath $release.DirectoryName
    if (-not (Test-Path -LiteralPath $releaseDirectory -PathType Container)) {
        throw "The official archive is missing its expected directory: $($release.DirectoryName)"
    }
    foreach ($file in $requiredFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $releaseDirectory $file) -PathType Leaf)) {
            throw "The official scrcpy archive for $Target is missing required file: $file"
        }
    }

    New-Item -ItemType Directory -Path $engineDirectory -Force | Out-Null
    foreach ($file in $requiredFiles) {
        Copy-Item -LiteralPath (Join-Path $releaseDirectory $file) -Destination (Join-Path $engineDirectory $file) -Force
    }
    if ($release.ArchiveType -eq 'tar.gz') {
        foreach ($file in $release.ExecutableFiles) {
            & chmod +x (Join-Path $engineDirectory $file)
            if ($LASTEXITCODE -ne 0) {
                throw "Could not mark the staged executable as runnable: $file"
            }
        }
    }

    Write-Output "Staged scrcpy v$($releaseSettings.Version) for $Target in $engineDirectory"
}
finally {
    if (Test-Path -LiteralPath $extractPath -PathType Container) {
        $resolvedRoot = [IO.Path]::GetFullPath($buildRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        $resolvedExtract = [IO.Path]::GetFullPath($extractPath)
        if (-not $resolvedExtract.StartsWith($resolvedRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove extraction directory outside .build: $resolvedExtract"
        }
        Remove-Item -LiteralPath $resolvedExtract -Recurse -Force
    }
}
