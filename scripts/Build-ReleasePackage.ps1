[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "無效的穩定版 App 版本：$Version"
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$appProject = Join-Path $projectRoot 'src\NeoAirPlay.App\NeoAirPlay.App.csproj'
$appSettingsPath = Join-Path $projectRoot 'src\NeoAirPlay.App\engine-package.json'
$buildDirectory = Join-Path $projectRoot '.build'
$releaseRoot = Join-Path $buildDirectory 'release-assets'
$outputDirectory = Join-Path $releaseRoot $Version
$publishDirectory = Join-Path $buildDirectory "publish-release-$Version"
$engineArchive = Join-Path $buildDirectory 'neo-airplay-engine-win-x64.zip'
$engineSidecar = "$engineArchive.sha256"
$appArchiveName = "NeoAirPlay-$Version-win-x64.zip"
$engineArchiveName = 'neo-airplay-engine-win-x64.zip'
$appArchive = Join-Path $outputDirectory $appArchiveName
$releaseEngineArchive = Join-Path $outputDirectory $engineArchiveName

[xml]$project = Get-Content -LiteralPath $appProject -Raw
$projectVersion = [string]$project.Project.PropertyGroup.Version
if ($projectVersion -ne $Version) {
    throw "App csproj 版本 $projectVersion 與套件版本 $Version 不一致。"
}

New-Item -ItemType Directory -Path $buildDirectory -Force | Out-Null
$resolvedReleaseRoot = [IO.Path]::GetFullPath($releaseRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
$resolvedOutputDirectory = [IO.Path]::GetFullPath($outputDirectory)
if (Test-Path -LiteralPath $resolvedOutputDirectory) {
    if (-not $resolvedOutputDirectory.StartsWith($resolvedReleaseRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "拒絕清理 Release 輸出目錄外的路徑：$resolvedOutputDirectory"
    }

    Remove-Item -LiteralPath $resolvedOutputDirectory -Recurse -Force
}
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null

$buildEngineScript = Join-Path $PSScriptRoot 'build-engine.ps1'
& $buildEngineScript
if (-not $?) {
    throw '引擎建置失敗。'
}

$packEngineScript = Join-Path $PSScriptRoot 'Pack-EngineRuntime.ps1'
& $packEngineScript -Version $Version
if (-not $?) {
    throw '引擎套件建立失敗。'
}
if (-not (Test-Path -LiteralPath $engineArchive -PathType Leaf) -or
    -not (Test-Path -LiteralPath $engineSidecar -PathType Leaf)) {
    throw '引擎建置未產生 ZIP 與 SHA-256 sidecar。'
}

$publishArguments = @(
    'publish',
    $appProject,
    '--configuration', 'Release',
    '--runtime', 'win-x64',
    '--self-contained', 'true',
    '--output', $publishDirectory
)
& dotnet @publishArguments
if ($LASTEXITCODE -ne 0) {
    throw "dotnet publish 失敗，結束代碼：$LASTEXITCODE"
}

$settingsPath = Join-Path $publishDirectory 'engine-package.json'
if (-not (Test-Path -LiteralPath $settingsPath -PathType Leaf)) {
    throw "發佈目錄缺少引擎套件設定：$settingsPath"
}
$settings = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
$actualEngineHash = (Get-FileHash -LiteralPath $engineArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($settings.Version -ne $Version -or
    $settings.ReleaseTag -ne "v$Version" -or
    $settings.AssetName -ne $engineArchiveName -or
    $settings.Sha256 -ne $actualEngineHash) {
    throw 'App 發佈輸出的引擎版本、Release tag 或 SHA-256 與引擎 ZIP 不一致。'
}

Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination (Join-Path $publishDirectory 'README.md')
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory(
    $publishDirectory,
    $appArchive,
    [IO.Compression.CompressionLevel]::Optimal,
    $false)
Copy-Item -LiteralPath $engineArchive -Destination $releaseEngineArchive

function Get-ZipEntryNames([string]$Path) {
    $archive = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        return @($archive.Entries | ForEach-Object { $_.FullName.Replace('\', '/') })
    }
    finally {
        $archive.Dispose()
    }
}

function Assert-NoBonjourBinaries([string]$Path) {
    $forbiddenEntries = @(Get-ZipEntryNames $Path | Where-Object {
        $_ -match '(^|/)engine/bonjour/' -or
        $_ -match '(^|/)mDNSResponder\.exe$' -or
        $_ -match '(^|/)dnssd\.dll$'
    })
    if ($forbiddenEntries.Count -gt 0) {
        throw "公開套件包含 Bonjour 執行檔：$($forbiddenEntries -join ', ')"
    }
}

foreach ($archivePath in @($appArchive, $releaseEngineArchive)) {
    Assert-NoBonjourBinaries $archivePath
}

$appEntries = @(Get-ZipEntryNames $appArchive)
foreach ($requiredEntry in @('NeoAirPlay.exe', 'engine/uxplay.exe', 'engine/android/scrcpy.exe', 'engine-package.json')) {
    if ($requiredEntry -notin $appEntries) {
        throw "完整可攜套件缺少必要檔案：$requiredEntry"
    }
}

function Write-Sha256Sidecar([string]$Path) {
    $fileName = [IO.Path]::GetFileName($Path)
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$Path.sha256", "$hash  $fileName`n", [Text.ASCIIEncoding]::new())
    return "$hash  $fileName"
}

$hashLines = @(
    (Write-Sha256Sidecar $appArchive)
    (Write-Sha256Sidecar $releaseEngineArchive)
)
[IO.File]::WriteAllLines((Join-Path $outputDirectory 'SHA256SUMS.txt'), $hashLines, [Text.ASCIIEncoding]::new())
[IO.File]::WriteAllText((Join-Path $outputDirectory 'release-version.txt'), "$Version`n", [Text.ASCIIEncoding]::new())
Copy-Item -LiteralPath $settingsPath -Destination (Join-Path $outputDirectory 'engine-package.json')

Write-Host "App Release 套件：$appArchive"
Write-Host "引擎 Release 套件：$releaseEngineArchive"
Write-Host "版本：$Version"
