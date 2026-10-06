#Requires -RunAsAdministrator
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$serviceName = 'Bonjour Service'
$bonjourHelpers = Join-Path $PSScriptRoot 'BonjourRuntimeHelpers.psm1'
if (-not (Test-Path -LiteralPath $bonjourHelpers -PathType Leaf)) {
    throw "找不到 Bonjour 執行元件輔助模組：$bonjourHelpers"
}
Import-Module -Name $bonjourHelpers -Force -ErrorAction Stop
$backupPath = Get-BonjourServiceBackupPath
Protect-BonjourServiceBackupDirectory
$registryPath = 'HKLM:\SYSTEM\CurrentControlSet\Services\Bonjour Service'
if (-not (Test-Path -LiteralPath $backupPath)) {
    throw "找不到 Bonjour Service 原始路徑備份：$backupPath"
}
$backup = Get-Content -LiteralPath $backupPath -Raw | ConvertFrom-Json
$serviceExisted = if ($backup.PSObject.Properties.Name -contains 'ServiceExisted') {
    [bool]$backup.ServiceExisted
}
else {
    $true
}
if ($backup.ServiceName -ne $serviceName) {
    throw 'Bonjour Service 備份檔的服務名稱或可信任原始 ImagePath 無效。'
}

if (-not $serviceExisted) {
    if (Test-Path -LiteralPath $registryPath) {
        $managedExecutable = Join-Path (Get-BonjourRuntimeDirectory) 'mDNSResponder.exe'
        $currentImagePath = (Get-ItemProperty -LiteralPath $registryPath -Name ImagePath).ImagePath
        $currentExecutable = Resolve-ServiceExecutablePath $currentImagePath
        if ([string]::IsNullOrWhiteSpace($currentExecutable) -or
            [IO.Path]::GetFullPath($currentExecutable) -ine [IO.Path]::GetFullPath($managedExecutable)) {
            throw 'Bonjour Service 路徑已不屬於 Neo AirPlay；為保護現有服務，還原腳本不會移除它。'
        }

        if (Test-TrustedBonjourExecutablePath $managedExecutable) {
            & $managedExecutable '-remove'
            if ($LASTEXITCODE -ne 0) {
                throw "無法移除 Neo AirPlay 建立的 Bonjour Service（結束代碼 $LASTEXITCODE）。"
            }
        }
        else {
            $service = Get-Service -Name $serviceName
            if ($service.Status -eq [System.ServiceProcess.ServiceControllerStatus]::Running) {
                Stop-Service -Name $serviceName -Force
                $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(20))
            }

            & "$env:SystemRoot\System32\sc.exe" delete $serviceName | Out-Null
            if ($LASTEXITCODE -ne 0) {
                throw "無法移除 Neo AirPlay 建立的 Bonjour Service（sc.exe exit code $LASTEXITCODE）。"
            }
        }

        $removalDeadline = (Get-Date).AddSeconds(20)
        while ((Test-Path -LiteralPath $registryPath) -and (Get-Date) -lt $removalDeadline) {
            Start-Sleep -Milliseconds 250
        }
        if (Test-Path -LiteralPath $registryPath) {
            throw 'Windows 尚未完成移除 Neo AirPlay 建立的 Bonjour Service。'
        }
    }

    Write-Host 'Bonjour Service 已還原為未安裝狀態。'
    return
}

if (-not (Test-TrustedBonjourExecutablePath $backup.OriginalImagePath)) {
    throw 'Bonjour Service 備份檔的服務名稱或可信任原始 ImagePath 無效。'
}
if (-not (Test-Path -LiteralPath $registryPath)) {
    throw '找不到目前的 Bonjour Service 註冊資料，無法還原原始路徑。'
}

$service = Get-Service -Name $serviceName
if ($service.Status -eq [System.ServiceProcess.ServiceControllerStatus]::Running) {
    Stop-Service -Name $serviceName -Force
    $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(20))
}

$originalPath = [string]$backup.OriginalImagePath
$quotedOriginalPath = if ($originalPath.StartsWith('"')) { $originalPath } else { '"{0}"' -f $originalPath }
& "$env:SystemRoot\System32\sc.exe" config $serviceName 'binPath=' $quotedOriginalPath
if ($LASTEXITCODE -ne 0) {
    throw "Windows 無法還原 Bonjour Service 路徑（sc.exe exit code $LASTEXITCODE）。"
}

if ($backup.OriginalWasRunning) {
    Start-Service -Name $serviceName
    $service = Get-Service -Name $serviceName
    $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
}

Write-Host "Bonjour Service 已還原至：$originalPath"
