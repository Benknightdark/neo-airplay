#Requires -RunAsAdministrator
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$serviceName = 'Bonjour Service'
$projectRoot = Split-Path -Parent $PSScriptRoot
$bonjourDirectory = Join-Path $projectRoot 'engine\bonjour'
$localExecutable = Join-Path $bonjourDirectory 'mDNSResponder.exe'
$localDnsSd = Join-Path $bonjourDirectory 'dnssd.dll'
$registryPath = 'HKLM:\SYSTEM\CurrentControlSet\Services\Bonjour Service'
$backupDirectory = Join-Path $env:ProgramData 'NeoAirPlay'
$backupPath = Join-Path $backupDirectory 'BonjourServiceBackup.json'

foreach ($requiredFile in @($localExecutable, $localDnsSd)) {
    if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
        throw "找不到專案內的 Bonjour 元件：$requiredFile。請先執行 scripts\Stage-BonjourRuntime.ps1。"
    }
}

if ((Get-Item -LiteralPath $localExecutable).VersionInfo.ProductName -ne 'Bonjour' -or
    (Get-Item -LiteralPath $localDnsSd).VersionInfo.ProductName -ne 'Bonjour') {
    throw '專案內的服務檔案未通過 Bonjour 產品資訊檢查。'
}
$versions = @(
    (Get-Item -LiteralPath $localExecutable).VersionInfo.FileVersion
    (Get-Item -LiteralPath $localDnsSd).VersionInfo.FileVersion
) | Select-Object -Unique
if ($versions.Count -ne 1) {
    throw "專案內的 Bonjour 服務元件版本不一致：$($versions -join ', ')"
}

if (-not (Test-Path -LiteralPath $registryPath)) {
    throw '找不到既有 Bonjour Service。此腳本只會切換現有服務，不會建立新的服務。'
}

$serviceInfo = Get-CimInstance Win32_Service -Filter "Name='Bonjour Service'"
if (-not $serviceInfo -or [string]::IsNullOrWhiteSpace($serviceInfo.StartName)) {
    throw '無法讀取 Bonjour Service 的執行帳號。'
}
$serviceAccountSid = switch -Regex ($serviceInfo.StartName) {
    '^(LocalSystem|NT AUTHORITY\\SYSTEM)$' { 'S-1-5-18'; break }
    '^(LocalService|NT AUTHORITY\\LocalService)$' { 'S-1-5-19'; break }
    '^(NetworkService|NT AUTHORITY\\NetworkService)$' { 'S-1-5-20'; break }
    default {
        try {
            ([System.Security.Principal.NTAccount]::new($serviceInfo.StartName)).Translate(
                [System.Security.Principal.SecurityIdentifier]
            ).Value
        }
        catch {
            throw "無法解析 Bonjour Service 執行帳號的 SID：$($serviceInfo.StartName)"
        }
    }
}
$serviceGrant = "*${serviceAccountSid}:(OI)(CI)RX"
& "$env:SystemRoot\System32\icacls.exe" $bonjourDirectory /grant $serviceGrant | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "無法授予 Bonjour Service 執行帳號讀取權限：$($serviceInfo.StartName)"
}

$service = Get-Service -Name $serviceName
$originalImagePath = (Get-ItemProperty -LiteralPath $registryPath -Name ImagePath).ImagePath
if ([string]::IsNullOrWhiteSpace($originalImagePath)) {
    throw '既有 Bonjour Service 沒有可備份的 ImagePath。'
}

$normalizedTarget = [IO.Path]::GetFullPath($localExecutable)
if ($originalImagePath.Trim('"') -ieq $normalizedTarget) {
    if ($service.Status -ne [System.ServiceProcess.ServiceControllerStatus]::Running) {
        Start-Service -Name $serviceName
        $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
    }
    Write-Host "Bonjour Service 已使用專案內的執行檔：$normalizedTarget"
    return
}

if (-not (Test-Path -LiteralPath $backupPath)) {
    New-Item -ItemType Directory -Path $backupDirectory -Force | Out-Null
    $backup = [ordered]@{
        ServiceName = $serviceName
        OriginalImagePath = $originalImagePath
        OriginalWasRunning = ($service.Status -eq [System.ServiceProcess.ServiceControllerStatus]::Running)
        SavedAt = (Get-Date).ToString('o')
    }
    $temporaryBackup = "$backupPath.tmp"
    $backup | ConvertTo-Json | Set-Content -LiteralPath $temporaryBackup -Encoding utf8
    Move-Item -LiteralPath $temporaryBackup -Destination $backupPath -Force
}

$startServiceWasRunning = $service.Status -eq [System.ServiceProcess.ServiceControllerStatus]::Running
$serviceStopped = $false
try {
    if ($startServiceWasRunning) {
        Stop-Service -Name $serviceName -Force
        $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(20))
        $serviceStopped = $true
    }

    $quotedExecutable = '"{0}"' -f $normalizedTarget
    & "$env:SystemRoot\System32\sc.exe" config $serviceName 'binPath=' $quotedExecutable
    if ($LASTEXITCODE -ne 0) {
        throw "Windows 無法更新 Bonjour Service 路徑（sc.exe exit code $LASTEXITCODE）。"
    }

    Start-Service -Name $serviceName
    $service = Get-Service -Name $serviceName
    $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
    $registeredImagePath = (Get-ItemProperty -LiteralPath $registryPath -Name ImagePath).ImagePath
    if ($registeredImagePath.Trim('"') -ine $normalizedTarget) {
        throw "Bonjour Service 路徑檢查失敗：$registeredImagePath"
    }

    Write-Host "Bonjour Service 現在從專案執行：$normalizedTarget"
    Write-Host "原路徑備份：$backupPath"
}
catch {
    $failure = $_
    try {
        $service = Get-Service -Name $serviceName
        if ($service.Status -eq [System.ServiceProcess.ServiceControllerStatus]::Running) {
            Stop-Service -Name $serviceName -Force
            $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, [TimeSpan]::FromSeconds(20))
        }

        $quotedOriginalPath = if ($originalImagePath.StartsWith('"')) { $originalImagePath } else { '"{0}"' -f $originalImagePath }
        & "$env:SystemRoot\System32\sc.exe" config $serviceName 'binPath=' $quotedOriginalPath
        if ($LASTEXITCODE -ne 0) { throw '無法還原原 Bonjour Service 路徑。' }
        if ($startServiceWasRunning) {
            Start-Service -Name $serviceName
            $service = Get-Service -Name $serviceName
            $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
        }
    }
    catch {
        Write-Warning "自動還原 Bonjour Service 失敗：$($_.Exception.Message)。原始 ImagePath 備份在 $backupPath。"
    }

    throw $failure
}
