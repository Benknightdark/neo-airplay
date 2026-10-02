#Requires -RunAsAdministrator
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$bonjourHelpers = Join-Path $PSScriptRoot 'BonjourRuntimeHelpers.psm1'
if (-not (Test-Path -LiteralPath $bonjourHelpers -PathType Leaf)) {
    throw "找不到 Bonjour 執行元件輔助模組：$bonjourHelpers"
}
Import-Module -Name $bonjourHelpers -Force -ErrorAction Stop
$destinationDirectory = Get-BonjourRuntimeDirectory
$service = Get-CimInstance Win32_Service -Filter "Name='Bonjour Service'"
if (-not $service) {
    throw '找不到 Windows 的 Bonjour Service。請先安裝 Bonjour，再重新啟動 Neo AirPlay。'
}

$currentImagePath = [string]$service.PathName
$currentExecutable = Resolve-ServiceExecutablePath $currentImagePath
$managedExecutable = Join-Path $destinationDirectory 'mDNSResponder.exe'
$currentIsManagedTarget = $false
if (-not [string]::IsNullOrWhiteSpace($currentExecutable)) {
    $currentIsManagedTarget = [IO.Path]::GetFullPath($currentExecutable) -ieq [IO.Path]::GetFullPath($managedExecutable)
}

$sourceImagePath = $null
if ($currentIsManagedTarget -and (Test-TrustedBonjourExecutablePath $currentImagePath)) {
    $sourceImagePath = $currentImagePath
}
else {
    $backupPath = Get-BonjourServiceBackupPath
    if (Test-Path -LiteralPath $backupPath -PathType Leaf) {
        try {
            $backup = Get-Content -LiteralPath $backupPath -Raw | ConvertFrom-Json
            if (($backup.ServiceName -eq 'Bonjour Service') -and
                (Test-TrustedBonjourExecutablePath $backup.OriginalImagePath)) {
                $sourceImagePath = [string]$backup.OriginalImagePath
            }
        }
        catch {
            $sourceImagePath = $null
        }
    }

    if ([string]::IsNullOrWhiteSpace($sourceImagePath) -and
        (Test-TrustedBonjourExecutablePath $currentImagePath)) {
        $sourceImagePath = $currentImagePath
    }
}

if ([string]::IsNullOrWhiteSpace($sourceImagePath)) {
    throw '找不到可信任且可用的 Bonjour 原始安裝檔案。請在 Program Files 安裝 Bonjour，再重新啟動 Neo AirPlay。'
}
$sourceExecutable = Resolve-ServiceExecutablePath $sourceImagePath

$sourceDirectory = Split-Path -Parent $sourceExecutable
$sourceFiles = @('mDNSResponder.exe', 'dnssd.dll') | ForEach-Object {
    $path = Join-Path $sourceDirectory $_
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Bonjour 執行檔目錄缺少必要檔案：$path"
    }

    $item = Get-Item -LiteralPath $path
    if ($item.VersionInfo.ProductName -ne 'Bonjour') {
        throw "拒絕複製非 Bonjour 元件：$path"
    }

    [pscustomobject]@{
        Source = $path
        Name = $_
        Version = $item.VersionInfo.FileVersion
        Product = $item.VersionInfo.ProductName
    }
}

$versions = @($sourceFiles.Version | Select-Object -Unique)
if ($versions.Count -ne 1) {
    throw "mDNSResponder.exe 與 dnssd.dll 版本不一致：$($versions -join ', ')"
}

New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
foreach ($file in $sourceFiles) {
    $destination = Join-Path $destinationDirectory $file.Name
    if ([IO.Path]::GetFullPath($file.Source) -ine [IO.Path]::GetFullPath($destination)) {
        Copy-Item -LiteralPath $file.Source -Destination $destination -Force
    }

    $sourceHash = (Get-FileHash -LiteralPath $file.Source -Algorithm SHA256).Hash
    $destinationHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
    if (-not [string]::Equals($sourceHash, $destinationHash, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Bonjour 元件複製後 SHA-256 不符：$destination"
    }
}

$manifest = [ordered]@{
    Product = 'Bonjour'
    FileVersion = $versions[0]
    Files = @($sourceFiles | ForEach-Object {
        [ordered]@{
            Name = $_.Name
            Sha256 = (Get-FileHash -LiteralPath (Join-Path $destinationDirectory $_.Name) -Algorithm SHA256).Hash
        }
    })
}
$manifestPath = Join-Path $destinationDirectory 'runtime-manifest.json'
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding utf8

Write-Host "已將 Bonjour $($versions[0]) 的服務元件放入：$destinationDirectory"
Write-Host '僅複製 mDNSResponder.exe 與 dnssd.dll；沒有複製 UxPlay wrapper 或影音執行檔。'
