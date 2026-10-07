[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('windows-x86_64', 'macos-aarch64', 'macos-x86_64', 'linux-x86_64')]
    [string]$Target,
    [string]$Version
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$engineDirectory = Join-Path $projectRoot 'engine'
$buildDirectory = Join-Path $projectRoot '.build'
$packageSettingsPath = Join-Path $projectRoot 'src-tauri/engine-package.json'
$manifestName = 'engine-package-manifest.json'
$packageJson = Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = [string]$packageJson.version
}
if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "App version must be a stable three-part SemVer value: $Version"
}

$androidManifestPath = Join-Path $PSScriptRoot "scrcpy-runtime-files.$Target.txt"
if (-not (Test-Path -LiteralPath $androidManifestPath -PathType Leaf)) {
    throw "Missing Android runtime manifest: $androidManifestPath"
}
$androidFileNames = @(Get-Content -LiteralPath $androidManifestPath -Encoding UTF8 | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if ($androidFileNames.Count -eq 0 -or
    @($androidFileNames | Group-Object { $_.ToLowerInvariant() } | Where-Object Count -gt 1).Count -gt 0 -or
    @($androidFileNames | Where-Object { $_ -match '[/\\:]' }).Count -gt 0) {
    throw "Android runtime manifest for $Target is empty, duplicated, or unsafe."
}

$archiveName = "neo-airplay-engine-$Target.zip"
$archivePath = Join-Path $buildDirectory $archiveName
$temporaryArchivePath = "$archivePath.tmp"
$stagingDirectory = Join-Path $buildDirectory ("engine-package-stage-" + [guid]::NewGuid().ToString('N'))
$stagedEngineDirectory = Join-Path $stagingDirectory 'engine'
$isWindowsTarget = $Target -eq 'windows-x86_64'
$airPlayFiles = [Collections.Generic.List[string]]::new()
$androidFiles = [Collections.Generic.List[string]]::new()
$sharedFiles = [Collections.Generic.List[string]]::new()

function Add-PackageFiles {
    param(
        [Parameter(Mandatory)][string]$SourceDirectory,
        [Parameter(Mandatory)][string]$RelativeDirectory,
        [Parameter(Mandatory)][AllowEmptyCollection()][Collections.Generic.List[string]]$ManifestFiles
    )

    if (-not (Test-Path -LiteralPath $SourceDirectory -PathType Container)) {
        return
    }
    foreach ($item in Get-ChildItem -LiteralPath $SourceDirectory -File -Recurse -Force | Sort-Object FullName) {
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Engine package inputs cannot include reparse points: $($item.FullName)"
        }
        $relative = $item.FullName.Substring($SourceDirectory.Length).TrimStart([IO.Path]::DirectorySeparatorChar)
        $packageRelative = "$RelativeDirectory/$($relative.Replace('\', '/'))"
        $destination = Join-Path $stagedEngineDirectory ($packageRelative.Substring('engine/'.Length).Replace('/', [IO.Path]::DirectorySeparatorChar))
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath $item.FullName -Destination $destination -Force
        $ManifestFiles.Add($packageRelative)
    }
}

New-Item -ItemType Directory -Path $buildDirectory -Force | Out-Null
New-Item -ItemType Directory -Path $stagedEngineDirectory -Force | Out-Null
try {
    foreach ($file in $androidFileNames) {
        $source = Join-Path (Join-Path $engineDirectory 'android') $file
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Android runtime is missing required file: $source"
        }
        $destination = Join-Path $stagedEngineDirectory ("android$([IO.Path]::DirectorySeparatorChar)$file")
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        Copy-Item -LiteralPath $source -Destination $destination -Force
        $androidFiles.Add("engine/android/$file")
    }

    if ($isWindowsTarget) {
        foreach ($required in @('uxplay.exe', 'runtime/bin', 'runtime/lib/gstreamer-1.0')) {
            $path = Join-Path $engineDirectory $required.Replace('/', [IO.Path]::DirectorySeparatorChar)
            if (-not (Test-Path -LiteralPath $path)) {
                throw "Windows AirPlay runtime is missing required path: $path"
            }
        }
        Copy-Item -LiteralPath (Join-Path $engineDirectory 'uxplay.exe') -Destination $stagedEngineDirectory -Force
        $airPlayFiles.Add('engine/uxplay.exe')
        Add-PackageFiles -SourceDirectory (Join-Path $engineDirectory 'runtime') -RelativeDirectory 'engine/runtime' -ManifestFiles $airPlayFiles
    }

    Add-PackageFiles -SourceDirectory (Join-Path $engineDirectory 'licenses') -RelativeDirectory 'engine/licenses' -ManifestFiles $sharedFiles

    if (-not $isWindowsTarget -and $airPlayFiles.Count -gt 0) {
        throw "Only Windows targets may include a bundled UxPlay runtime."
    }
    if ($airPlayFiles.Count + $androidFiles.Count + $sharedFiles.Count -eq 0) {
        throw "No runtime files were staged for $Target."
    }

    $manifestFiles = @(Get-ChildItem -LiteralPath $stagedEngineDirectory -File -Recurse -Force | Sort-Object FullName | ForEach-Object {
        if (($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Engine package staging cannot include reparse points: $($_.FullName)"
        }
        $relative = $_.FullName.Substring($stagingDirectory.Length + 1).Replace('\', '/')
        [ordered]@{
            Path = $relative
            Length = $_.Length
            Sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    })
    $manifest = [ordered]@{
        Version = $Version
        AirPlayFiles = @($airPlayFiles | Sort-Object -Unique)
        AndroidFiles = @($androidFiles | Sort-Object -Unique)
        SharedFiles = @($sharedFiles | Sort-Object -Unique)
        Files = $manifestFiles
    }
    $manifestPath = Join-Path $stagingDirectory $manifestName
    [IO.File]::WriteAllText(
        $manifestPath,
        ($manifest | ConvertTo-Json -Depth 8),
        [Text.UTF8Encoding]::new($false))

    Add-Type -AssemblyName System.IO.Compression
    $output = [IO.File]::Open($temporaryArchivePath, [IO.FileMode]::Create, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    $zip = [IO.Compression.ZipArchive]::new($output, [IO.Compression.ZipArchiveMode]::Create, $false)
    try {
        $archiveFiles = @(Get-ChildItem -LiteralPath $stagingDirectory -File -Recurse -Force | Sort-Object FullName)
        foreach ($item in $archiveFiles) {
            $entryName = $item.FullName.Substring($stagingDirectory.Length + 1).Replace('\', '/')
            $entry = $zip.CreateEntry($entryName, [IO.Compression.CompressionLevel]::Optimal)
            if ($isWindowsTarget) {
                $unixMode = 420
            }
            else {
                $unixMode = [int][IO.File]::GetUnixFileMode($item.FullName) -band 511
                if ($item.FullName -match '/(adb|scrcpy)$') {
                    $unixMode = $unixMode -bor 73
                }
                $unixMode = $unixMode -bor 0x8000
            }
            $entry.ExternalAttributes = $unixMode -shl 16
            $entryStream = $entry.Open()
            $input = [IO.File]::OpenRead($item.FullName)
            try {
                $input.CopyTo($entryStream)
            }
            finally {
                $input.Dispose()
                $entryStream.Dispose()
            }
        }
    }
    finally {
        $zip.Dispose()
        $output.Dispose()
    }
    Move-Item -LiteralPath $temporaryArchivePath -Destination $archivePath -Force

    $archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(
        "$archivePath.sha256",
        "$archiveHash  $archiveName$([Environment]::NewLine)",
        [Text.ASCIIEncoding]::new())

    $settings = $null
    if (Test-Path -LiteralPath $packageSettingsPath -PathType Leaf) {
        $settings = Get-Content -LiteralPath $packageSettingsPath -Raw -Encoding UTF8 | ConvertFrom-Json -AsHashtable
    }
    if (-not $settings -or $settings.Version -ne $Version) {
        $settings = [ordered]@{
            SchemaVersion = 2
            Version = $Version
            ReleaseTag = "v$Version"
            ManifestName = $manifestName
            Targets = [ordered]@{}
        }
    }
    $settings.SchemaVersion = 2
    $settings.Version = $Version
    $settings.ReleaseTag = "v$Version"
    $settings.ManifestName = $manifestName
    $settings.Targets[$Target] = [ordered]@{
        AssetName = $archiveName
        Sha256 = $archiveHash
        AirPlayFiles = @($manifest.AirPlayFiles)
        AndroidFiles = @($manifest.AndroidFiles)
        SharedFiles = @($manifest.SharedFiles)
    }
    $settingsTemporary = "$packageSettingsPath.tmp"
    New-Item -ItemType Directory -Path (Split-Path -Parent $packageSettingsPath) -Force | Out-Null
    [IO.File]::WriteAllText(
        $settingsTemporary,
        ($settings | ConvertTo-Json -Depth 10),
        [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $settingsTemporary -Destination $packageSettingsPath -Force

    Write-Output "Created $archivePath"
    Write-Output "Updated package metadata at $packageSettingsPath"
    Write-Output "SHA-256: $archiveHash"
}
finally {
    if (Test-Path -LiteralPath $temporaryArchivePath -PathType Leaf) {
        Remove-Item -LiteralPath $temporaryArchivePath -Force
    }
    if (Test-Path -LiteralPath $stagingDirectory -PathType Container) {
        $resolvedBuildRoot = [IO.Path]::GetFullPath($buildDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        $resolvedStaging = [IO.Path]::GetFullPath($stagingDirectory)
        if (-not $resolvedStaging.StartsWith($resolvedBuildRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a staging directory outside .build: $resolvedStaging"
        }
        Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
    }
}
