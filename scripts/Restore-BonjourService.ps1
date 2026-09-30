#Requires -RunAsAdministrator
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$serviceName = 'Bonjour Service'
$backupPath = Join-Path (Join-Path $env:ProgramData 'NeoAirPlay') 'BonjourServiceBackup.json'
$registryPath = 'HKLM:\SYSTEM\CurrentControlSet\Services\Bonjour Service'
if (-not (Test-Path -LiteralPath $backupPath)) {
    throw "找不到 Bonjour Service 原始路徑備份：$backupPath"
}
if (-not (Test-Path -LiteralPath $registryPath)) {
    throw '找不到 Bonjour Service 註冊資料。'
}

$backup = Get-Content -LiteralPath $backupPath -Raw | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace($backup.OriginalImagePath)) {
    throw 'Bonjour Service 備份檔沒有原始 ImagePath。'
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
