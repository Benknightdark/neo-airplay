[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$buildRoot = Join-Path $projectRoot '.build'
$archivePath = Join-Path $buildRoot 'scrcpy-win64-v4.1.zip'
$extractPath = Join-Path $buildRoot ("scrcpy-stage-" + [guid]::NewGuid().ToString('N'))
$engineDirectory = Join-Path $projectRoot 'engine\android'
$releaseUrl = 'https://github.com/Genymobile/scrcpy/releases/download/v4.1/scrcpy-win64-v4.1.zip'
$expectedSha256 = '5b12172b3264b2889f4583ee64752ce832e29bc8b1089dca81093459697165db'

New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Download-OfficialRelease([string]$Destination) {
    $curlPath = Join-Path $env:SystemRoot 'System32\curl.exe'
    if (-not (Test-Path -LiteralPath $curlPath)) {
        $curl = Get-Command curl.exe -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($curl) {
            $curlPath = $curl.Source
        }
    }
    if (-not (Test-Path -LiteralPath $curlPath)) {
        throw '找不到 Windows curl.exe，無法下載 scrcpy 官方執行環境。'
    }

    & $curlPath '--location' '--fail' '--retry' '5' '--retry-delay' '2' '--retry-connrefused' '--retry-all-errors' '--output' $Destination $releaseUrl
    if ($LASTEXITCODE -ne 0) {
        throw "scrcpy 官方發行包下載失敗：$releaseUrl"
    }
}

try {
    if ((Test-Path -LiteralPath $archivePath) -and (Get-Sha256 $archivePath) -ne $expectedSha256) {
        Remove-Item -LiteralPath $archivePath -Force
    }

    if (-not (Test-Path -LiteralPath $archivePath)) {
        Write-Host '下載 Genymobile/scrcpy v4.1 官方 Windows x64 發行包。'
        Download-OfficialRelease $archivePath
    }

    $actualSha256 = Get-Sha256 $archivePath
    if ($actualSha256 -ne $expectedSha256) {
        throw "scrcpy 發行包 SHA-256 不符。預期 $expectedSha256，實際 $actualSha256。"
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractPath
    $releaseDirectory = Join-Path $extractPath 'scrcpy-win64-v4.1'
    if (-not (Test-Path -LiteralPath $releaseDirectory -PathType Container)) {
        throw 'scrcpy 官方封存檔缺少 scrcpy-win64-v4.1 目錄。'
    }

    $requiredFilesManifest = Join-Path $PSScriptRoot 'scrcpy-runtime-files.txt'
    if (-not (Test-Path -LiteralPath $requiredFilesManifest -PathType Leaf)) {
        throw '找不到 scrcpy runtime 檔案清單 scripts\scrcpy-runtime-files.txt。'
    }

    $requiredFiles = @(Get-Content -LiteralPath $requiredFilesManifest | ForEach-Object { $_.Trim() } | Where-Object { $_.Length -gt 0 })
    if ($requiredFiles.Count -eq 0) {
        throw 'scrcpy runtime 檔案清單不可為空。'
    }

    $invalidFileNames = @($requiredFiles | Where-Object { [IO.Path]::GetFileName($_) -cne $_ })
    if ($invalidFileNames.Count -gt 0) {
        throw "scrcpy runtime 檔案清單只能包含檔名：$($invalidFileNames -join ', ')"
    }

    $duplicateFileNames = @($requiredFiles | Group-Object { $_.ToLowerInvariant() } | Where-Object Count -gt 1)
    if ($duplicateFileNames.Count -gt 0) {
        throw "scrcpy runtime 檔案清單不可重複：$(($duplicateFileNames | ForEach-Object Name) -join ', ')"
    }
    $missingFiles = $requiredFiles | Where-Object {
        -not (Test-Path -LiteralPath (Join-Path $releaseDirectory $_) -PathType Leaf)
    }
    if ($missingFiles) {
        throw "scrcpy 官方封存檔缺少必要檔案：$($missingFiles -join ', ')"
    }

    New-Item -ItemType Directory -Path $engineDirectory -Force | Out-Null
    Copy-Item -Path (Join-Path $releaseDirectory '*') -Destination $engineDirectory -Recurse -Force

    foreach ($file in $requiredFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $engineDirectory $file) -PathType Leaf)) {
            throw "scrcpy staging 後找不到必要檔案：$file"
        }
    }

    Write-Host "scrcpy v4.1 已放入 $engineDirectory"
    Write-Host "SHA-256: $actualSha256"
}
finally {
    if (Test-Path -LiteralPath $extractPath -PathType Container) {
        $resolvedBuildRoot = [IO.Path]::GetFullPath($buildRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        $resolvedExtractPath = [IO.Path]::GetFullPath($extractPath)
        if (-not $resolvedExtractPath.StartsWith($resolvedBuildRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "refusing to remove extraction directory outside .build: $resolvedExtractPath"
        }

        Remove-Item -LiteralPath $resolvedExtractPath -Recurse -Force
    }
}
