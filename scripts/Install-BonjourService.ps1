#Requires -RunAsAdministrator
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$bonjourHelpers = Join-Path $PSScriptRoot 'BonjourRuntimeHelpers.psm1'
if (-not (Test-Path -LiteralPath $bonjourHelpers -PathType Leaf)) {
    throw "找不到 Bonjour 執行元件輔助模組：$bonjourHelpers"
}
Import-Module -Name $bonjourHelpers -Force -ErrorAction Stop
$serviceName = 'Bonjour Service'
$bonjourDirectory = Get-BonjourRuntimeDirectory
$localExecutable = Join-Path $bonjourDirectory 'mDNSResponder.exe'
$localDnsSd = Join-Path $bonjourDirectory 'dnssd.dll'
$registryPath = 'HKLM:\SYSTEM\CurrentControlSet\Services\Bonjour Service'
$backupPath = Get-BonjourServiceBackupPath
$backupDirectory = Split-Path -Parent $backupPath
Protect-BonjourServiceBackupDirectory

foreach ($requiredFile in @($localExecutable, $localDnsSd)) {
    if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
        throw "找不到 Neo AirPlay 系統共用目錄中的 Bonjour 元件：$requiredFile。請先執行 scripts\Stage-BonjourRuntime.ps1。"
    }
}

if (-not (Test-UsableBonjourRuntimePair -ResponderPath $localExecutable -DnsSdPath $localDnsSd)) {
    throw 'Neo AirPlay 系統共用目錄中的 Bonjour 執行檔與 x64 DNS-SD 元件未通過產品資訊、版本、架構或信任路徑檢查。'
}

$serviceExists = Test-Path -LiteralPath $registryPath
$serviceInfo = if ($serviceExists) { Get-CimInstance Win32_Service -Filter "Name='Bonjour Service'" } else { $null }
if ($serviceExists -and (-not $serviceInfo -or [string]::IsNullOrWhiteSpace($serviceInfo.StartName))) {
    throw '無法讀取 Bonjour Service 的執行帳號。'
}
$serviceAccountName = if ($serviceExists) { [string]$serviceInfo.StartName } else { 'LocalSystem' }
$serviceAccountSid = switch -Regex ($serviceAccountName) {
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
            throw "無法解析 Bonjour Service 執行帳號的 SID：$serviceAccountName"
        }
    }
}
$serviceGrant = "*${serviceAccountSid}:(OI)(CI)RX"
& "$env:SystemRoot\System32\icacls.exe" $bonjourDirectory /reset /T /C | Out-Null
if ($LASTEXITCODE -ne 0) { throw '無法重設 Bonjour 系統共用目錄的 ACL。' }
& "$env:SystemRoot\System32\icacls.exe" $bonjourDirectory /inheritance:r /T /C | Out-Null
if ($LASTEXITCODE -ne 0) { throw '無法移除 Bonjour 系統共用目錄的繼承 ACL。' }
& "$env:SystemRoot\System32\icacls.exe" $bonjourDirectory /setowner '*S-1-5-32-544' /T /C | Out-Null
if ($LASTEXITCODE -ne 0) { throw '無法將 Bonjour 系統共用目錄擁有者設為系統管理員群組。' }
$directoryGrants = @(
    '*S-1-5-32-544:(OI)(CI)(F)'
    '*S-1-5-18:(OI)(CI)(RX)'
    '*S-1-5-32-545:(OI)(CI)(RX)'
    $serviceGrant
)
foreach ($grant in $directoryGrants) {
    & "$env:SystemRoot\System32\icacls.exe" $bonjourDirectory /grant:r $grant /T /C | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "無法限制 Bonjour 系統共用目錄權限：$serviceAccountName"
    }
}

$service = if ($serviceExists) { Get-Service -Name $serviceName } else { $null }
$originalImagePath = if ($serviceExists) {
    (Get-ItemProperty -LiteralPath $registryPath -Name ImagePath).ImagePath
}
else {
    $null
}
if ($serviceExists -and [string]::IsNullOrWhiteSpace($originalImagePath)) {
    throw '既有 Bonjour Service 沒有可備份的 ImagePath。'
}

function Get-UsableBonjourBackup {
    if (-not (Test-Path -LiteralPath $backupPath -PathType Leaf)) { return $null }
    try {
        $candidate = Get-Content -LiteralPath $backupPath -Raw | ConvertFrom-Json
        if ($candidate.ServiceName -ne $serviceName) {
            return $null
        }

        if ($candidate.PSObject.Properties.Name -contains 'ServiceExisted' -and -not $candidate.ServiceExisted) {
            if ($serviceExists -and
                (Resolve-ServiceExecutablePath $originalImagePath) -ine $normalizedTarget) {
                return $null
            }

            return $candidate
        }

        if (-not (Test-TrustedBonjourExecutablePath $candidate.OriginalImagePath)) {
            return $null
        }
        return $candidate
    }
    catch {
        return $null
    }
}

function New-CurrentBonjourServiceBackupRecord {
    $originalWasRunning = $serviceExists -and
        $service.Status -eq [System.ServiceProcess.ServiceControllerStatus]::Running
    [pscustomobject][ordered]@{
        ServiceName = $serviceName
        ServiceExisted = [bool]$serviceExists
        OriginalImagePath = if ($serviceExists) { $originalImagePath } else { $null }
        OriginalWasRunning = $originalWasRunning
        SavedAt = (Get-Date).ToString('o')
    }
}

function Save-BonjourServiceBackupRecord {
    param([Parameter(Mandatory)][object]$Record)

    $temporaryBackup = "$backupPath.tmp"
    $Record | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $temporaryBackup -Encoding utf8
    Move-Item -LiteralPath $temporaryBackup -Destination $backupPath -Force
}

$normalizedTarget = [IO.Path]::GetFullPath($localExecutable)
if ($serviceExists -and (Resolve-ServiceExecutablePath $originalImagePath) -ieq $normalizedTarget) {
    if ($service.Status -ne [System.ServiceProcess.ServiceControllerStatus]::Running) {
        Start-Service -Name $serviceName
        $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
    }
    Write-Host "Bonjour Service 已使用受保護的系統共用執行檔：$normalizedTarget"
    return
}

if (Test-Path -LiteralPath $backupPath -PathType Leaf) {
    if (-not (Get-UsableBonjourBackup)) {
        if ($serviceExists -and -not (Test-TrustedBonjourExecutablePath $originalImagePath)) {
            throw "Bonjour Service 備份檔無效，且目前服務路徑也不可信任；為保留還原能力而停止安裝：$backupPath"
        }

        Save-BonjourServiceBackupRecord (New-CurrentBonjourServiceBackupRecord)
        Write-Warning '舊的 Bonjour Service 備份無法安全沿用；已依目前服務狀態更新還原備份。'
    }
}
else {
    New-Item -ItemType Directory -Path $backupDirectory -Force | Out-Null
    Save-BonjourServiceBackupRecord (New-CurrentBonjourServiceBackupRecord)
}

if (-not $serviceExists) {
    try {
        & $localExecutable '-install'
        if ($LASTEXITCODE -ne 0) {
            $serviceExists = Test-Path -LiteralPath $registryPath
            throw "Bonjour Service 安裝命令失敗（結束代碼 $LASTEXITCODE）。"
        }

        $serviceExists = Test-Path -LiteralPath $registryPath
        if (-not $serviceExists) {
            throw 'Bonjour 安裝命令完成，但沒有建立 Windows 服務。'
        }

        $service = Get-Service -Name $serviceName
        $service.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(20))
        $registeredPath = (Get-ItemProperty -LiteralPath $registryPath -Name ImagePath).ImagePath
        $registeredExecutable = Resolve-ServiceExecutablePath $registeredPath
        if ([string]::IsNullOrWhiteSpace($registeredExecutable) -or
            [IO.Path]::GetFullPath($registeredExecutable) -ine $normalizedTarget) {
            throw "Bonjour Service 安裝後路徑檢查失敗：$registeredPath"
        }

        Write-Host "已建立並啟動 Bonjour Service：$normalizedTarget"
        Write-Host "服務備份：$backupPath"
        return
    }
    catch {
        if (Test-Path -LiteralPath $registryPath) {
            & $localExecutable '-remove' | Out-Null
            if ($LASTEXITCODE -ne 0) {
                Write-Warning "安裝失敗後無法移除新建立的 Bonjour Service（結束代碼 $LASTEXITCODE）。"
            }
        }

        throw
    }
}

$rollbackImagePath = $originalImagePath
if (-not (Test-TrustedBonjourExecutablePath $originalImagePath)) {
    $backupForRollback = Get-UsableBonjourBackup
    if ($backupForRollback) {
        $rollbackImagePath = [string]$backupForRollback.OriginalImagePath
    }
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

    Write-Host "Bonjour Service 現在從受保護的系統共用目錄執行：$normalizedTarget"
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

        $quotedRollbackPath = if ($rollbackImagePath.StartsWith('"')) { $rollbackImagePath } else { '"{0}"' -f $rollbackImagePath }
        & "$env:SystemRoot\System32\sc.exe" config $serviceName 'binPath=' $quotedRollbackPath
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
