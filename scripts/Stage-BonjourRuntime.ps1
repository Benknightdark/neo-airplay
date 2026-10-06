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
$currentImagePath = if ($service) { [string]$service.PathName } else { $null }
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

    if (-not [string]::IsNullOrWhiteSpace($sourceImagePath) -and
        -not (Test-TrustedBonjourExecutablePath $sourceImagePath)) {
        $sourceImagePath = $null
    }

    if ([string]::IsNullOrWhiteSpace($sourceImagePath) -and (Test-TrustedBonjourExecutablePath $currentImagePath)) {
        $sourceImagePath = $currentImagePath
    }

    if ([string]::IsNullOrWhiteSpace($sourceImagePath)) {
        $sourceCandidates = @($managedExecutable) + @(Get-BonjourSourceExecutablePaths)
        $sourceImagePath = $sourceCandidates | Where-Object { Test-TrustedBonjourExecutablePath $_ } | Select-Object -First 1
    }
}

if ([string]::IsNullOrWhiteSpace($sourceImagePath)) {
    throw '找不到可信任且可用的 Bonjour 原始安裝檔案。請依 Apple 官方說明安裝 Bonjour，再重新啟動 Neo AirPlay。'
}
$sourceExecutable = Resolve-ServiceExecutablePath $sourceImagePath
$sourceDnsSd = Get-CompatibleBonjourDnsSdPath -ResponderPath $sourceExecutable
if ([string]::IsNullOrWhiteSpace($sourceDnsSd)) {
    throw (Get-BonjourMissingCompatibleX64DnsSdMessage)
}
if (-not (Test-UsableBonjourRuntimePair -ResponderPath $sourceExecutable -DnsSdPath $sourceDnsSd)) {
    throw 'Bonjour Service 執行檔與 x64 DNS-SD 元件未通過產品資訊、版本、架構或信任路徑檢查。'
}

$sourceFiles = @(
    [pscustomobject]@{ Source = $sourceExecutable; Name = 'mDNSResponder.exe' }
    [pscustomobject]@{ Source = $sourceDnsSd; Name = 'dnssd.dll' }
) | ForEach-Object {
    $item = Get-Item -LiteralPath $_.Source
    [pscustomobject]@{
        Source = $_.Source
        Name = $_.Name
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
