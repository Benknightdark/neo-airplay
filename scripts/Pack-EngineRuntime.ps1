[CmdletBinding()]
param(
    [string]$Version
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$engineDirectory = Join-Path $projectRoot 'engine'
$buildDirectory = Join-Path $projectRoot '.build'
$appProject = Join-Path $projectRoot 'src\NeoAirPlay.App\NeoAirPlay.App.csproj'
$settingsPath = Join-Path $projectRoot 'src\NeoAirPlay.App\engine-package.json'
$assetName = 'neo-airplay-engine-win-x64.zip'

if (-not $Version) {
    [xml]$project = Get-Content -LiteralPath $appProject
    $Version = $project.Project.PropertyGroup.Version | Where-Object { $_ } | Select-Object -First 1
}
if ([string]::IsNullOrWhiteSpace($Version) -or $Version -notmatch '^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$') {
    throw "無效的 App 版本：$Version"
}

$androidManifest = Join-Path $PSScriptRoot 'scrcpy-runtime-files.txt'
$androidRequiredFiles = @(Get-Content -LiteralPath $androidManifest | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if ($androidRequiredFiles.Count -eq 0) {
    throw 'scrcpy runtime 檔案清單不可為空。'
}

$requiredPaths = @(
    'uxplay.exe',
    'runtime\bin',
    'runtime\lib\gstreamer-1.0',
    'android'
)
foreach ($relativePath in $requiredPaths) {
    $path = Join-Path $engineDirectory $relativePath
    if (-not (Test-Path -LiteralPath $path)) {
        throw "找不到引擎必要路徑：$path。請先執行 scripts\build-engine.ps1。"
    }
}

foreach ($file in $androidRequiredFiles) {
    if ([IO.Path]::GetFileName($file) -cne $file -or
        -not (Test-Path -LiteralPath (Join-Path $engineDirectory "android\$file") -PathType Leaf)) {
        throw "Android 引擎缺少必要檔案：$file"
    }
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Path $buildDirectory -Force | Out-Null
$stagingDirectory = Join-Path $buildDirectory ("engine-package-stage-" + [guid]::NewGuid().ToString('N'))
$archivePath = Join-Path $buildDirectory $assetName
$manifestName = 'engine-package-manifest.json'

try {
    $stagedEngine = Join-Path $stagingDirectory 'engine'
    New-Item -ItemType Directory -Path $stagedEngine -Force | Out-Null

    foreach ($engineSubdirectory in @('runtime', 'android', 'licenses')) {
        $source = Join-Path $engineDirectory $engineSubdirectory
        if (Test-Path -LiteralPath $source -PathType Container) {
            Copy-Item -LiteralPath $source -Destination $stagedEngine -Recurse -Force
        }
    }
    Copy-Item -LiteralPath (Join-Path $engineDirectory 'uxplay.exe') -Destination $stagedEngine -Force

    $airPlayFiles = @(
        (Get-Item -LiteralPath (Join-Path $stagedEngine 'uxplay.exe')).FullName
        Get-ChildItem -LiteralPath (Join-Path $stagedEngine 'runtime') -File -Recurse | ForEach-Object FullName
    ) | ForEach-Object { $_.Substring($stagedEngine.Length + 1).Replace('\', '/') } | Sort-Object -Unique
    $androidFiles = @(
        Get-ChildItem -LiteralPath (Join-Path $stagedEngine 'android') -File -Recurse |
            ForEach-Object { $_.FullName.Substring($stagedEngine.Length + 1).Replace('\', '/') }
    ) | Sort-Object -Unique
    $sharedFiles = @(
        Get-ChildItem -LiteralPath (Join-Path $stagedEngine 'licenses') -File -Recurse -ErrorAction SilentlyContinue |
            ForEach-Object { $_.FullName.Substring($stagedEngine.Length + 1).Replace('\', '/') }
    ) | Sort-Object -Unique

    $fileEntries = Get-ChildItem -LiteralPath $stagedEngine -File -Recurse | ForEach-Object {
        $relativePath = $_.FullName.Substring($stagingDirectory.Length + 1).Replace('\', '/')
        [ordered]@{
            Path = $relativePath
            Length = $_.Length
            Sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
    $packageManifest = [ordered]@{
        Version = $Version
        AirPlayFiles = @($airPlayFiles | ForEach-Object { "engine/$_" })
        AndroidFiles = @($androidFiles | ForEach-Object { "engine/$_" })
        SharedFiles = @($sharedFiles | ForEach-Object { "engine/$_" })
        Files = @($fileEntries)
    }
    $packageManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $stagingDirectory $manifestName) -Encoding utf8

    if (Test-Path -LiteralPath $archivePath) {
        Remove-Item -LiteralPath $archivePath -Force
    }
    [IO.Compression.ZipFile]::CreateFromDirectory(
        $stagingDirectory,
        $archivePath,
        [IO.Compression.CompressionLevel]::Optimal,
        $false)

    $archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    "$archiveHash  $assetName" | Set-Content -LiteralPath "$archivePath.sha256" -Encoding ascii
    $settings = [ordered]@{
        Version = $Version
        ReleaseTag = "v$Version"
        AssetName = $assetName
        Sha256 = $archiveHash
        ManifestName = $manifestName
        AirPlayFiles = @($packageManifest.AirPlayFiles)
        AndroidFiles = @($packageManifest.AndroidFiles)
        SharedFiles = @($packageManifest.SharedFiles)
    }
    $settings | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $settingsPath -Encoding utf8

    Write-Host "引擎套件已建立：$archivePath"
    Write-Host "Release 標籤：v$Version"
    Write-Host "SHA-256：$archiveHash"
    Write-Host "已更新 App 套件設定：$settingsPath"
}
finally {
    if (Test-Path -LiteralPath $stagingDirectory -PathType Container) {
        $resolvedBuildRoot = [IO.Path]::GetFullPath($buildDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        $resolvedStaging = [IO.Path]::GetFullPath($stagingDirectory)
        if (-not $resolvedStaging.StartsWith($resolvedBuildRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "拒絕清理專案 .build 目錄外的路徑：$resolvedStaging"
        }
        Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
    }
}
