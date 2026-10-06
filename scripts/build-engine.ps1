[CmdletBinding()]
param(
    [string]$Msys2Root
)

$ErrorActionPreference = 'Stop'
$bonjourHelpers = Join-Path $PSScriptRoot 'BonjourRuntimeHelpers.psm1'
if (-not (Test-Path -LiteralPath $bonjourHelpers -PathType Leaf)) {
    throw "找不到 Bonjour 執行元件輔助模組：$bonjourHelpers"
}
Import-Module -Name $bonjourHelpers -Force -ErrorAction Stop
$projectRoot = Split-Path -Parent $PSScriptRoot
$buildRoot = Join-Path $projectRoot '.build'
if (-not $Msys2Root) {
    $Msys2Root = $env:MSYS2_ROOT
}
if (-not $Msys2Root) {
    $Msys2Root = @(
        (Join-Path $buildRoot 'msys64'),
        'C:\msys64',
        'C:\tools\msys64'
    ) | Where-Object { Test-Path (Join-Path $_ 'usr\bin\pacman.exe') } | Select-Object -First 1
}
if (-not $Msys2Root) {
    $Msys2Root = Join-Path $buildRoot 'msys64'
}
$Msys2Root = [IO.Path]::GetFullPath($Msys2Root)
$ucrtBin = Join-Path $Msys2Root 'ucrt64\bin'
$msysBin = Join-Path $Msys2Root 'usr\bin'
$pacman = Join-Path $msysBin 'pacman.exe'
New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null

function Save-RemoteFile([string]$Uri, [string]$Destination) {
    $curlPath = Join-Path $env:SystemRoot 'System32\curl.exe'
    if (-not (Test-Path -LiteralPath $curlPath)) {
        $curl = Get-Command curl.exe -ErrorAction SilentlyContinue | Where-Object { $_.Source -notlike "$Msys2Root*" } | Select-Object -First 1
        if ($curl) {
            $curlPath = $curl.Source
        }
    }
    if (-not (Test-Path -LiteralPath $curlPath)) {
        throw '找不到 Windows curl.exe，無法下載建置相依檔。'
    }

    if (Test-Path -LiteralPath $Destination) {
        Remove-Item -LiteralPath $Destination -Force
    }
    & $curlPath '--location' '--fail' '--retry' '5' '--retry-delay' '2' '--retry-connrefused' '--retry-all-errors' '--output' $Destination $Uri
    if ($LASTEXITCODE -ne 0) {
        throw "下載失敗：$Uri"
    }
}

if (-not (Test-Path -LiteralPath $pacman)) {
    Write-Host "在專案建置快取中安裝 MSYS2：$Msys2Root"
    $releaseIndex = Invoke-WebRequest -Uri 'https://repo.msys2.org/distrib/x86_64/' -UseBasicParsing
    $installerName = [regex]::Matches($releaseIndex.Content, 'msys2-x86_64-(\d{8})\.exe') |
        ForEach-Object { "msys2-x86_64-$($_.Groups[1].Value).exe" } |
        Sort-Object -Unique -Descending |
        Select-Object -First 1
    if (-not $installerName) {
        throw '無法從 MSYS2 官方發行頁找到 x64 安裝程式。請依 README.md 手動安裝 MSYS2。'
    }

    $installerPath = Join-Path $buildRoot $installerName
    Save-RemoteFile "https://repo.msys2.org/distrib/x86_64/$installerName" $installerPath
    $installerArguments = @(
        'in',
        '--confirm-command',
        '--accept-messages',
        '--root',
        $Msys2Root.Replace('\', '/')
    )
    $installerProcess = Start-Process -FilePath $installerPath -ArgumentList $installerArguments -WindowStyle Hidden -Wait -PassThru
    if ($installerProcess.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $pacman)) {
        throw "MSYS2 安裝失敗，結束代碼：$($installerProcess.ExitCode)"
    }
}

$env:MSYSTEM = 'UCRT64'
$env:CHERE_INVOKING = '1'
$env:PATH = "$ucrtBin;$msysBin;$env:PATH"
Write-Host '更新 MSYS2 套件索引與基礎工具。'
& $pacman -Syu --noconfirm
if ($LASTEXITCODE -ne 0) {
    & $pacman -Syu --noconfirm
    if ($LASTEXITCODE -ne 0) {
        throw "MSYS2 更新失敗，結束代碼：$LASTEXITCODE"
    }
}

$packages = @(
    'mingw-w64-ucrt-x86_64-cmake',
    'mingw-w64-ucrt-x86_64-gcc',
    'mingw-w64-ucrt-x86_64-ninja',
    'mingw-w64-ucrt-x86_64-tools',
    'mingw-w64-ucrt-x86_64-libplist',
    'mingw-w64-ucrt-x86_64-gstreamer',
    'mingw-w64-ucrt-x86_64-gst-plugins-base',
    'mingw-w64-ucrt-x86_64-gst-plugins-good',
    'mingw-w64-ucrt-x86_64-gst-plugins-bad',
    'mingw-w64-ucrt-x86_64-gst-libav',
    'mingw-w64-ucrt-x86_64-openssl'
)
Write-Host '安裝 UxPlay、GStreamer 與原生 Windows 建置所需套件。'
& $pacman -S --needed --noconfirm @packages
if ($LASTEXITCODE -ne 0) {
    throw "MSYS2 相依套件安裝失敗，結束代碼：$LASTEXITCODE"
}

Write-Host '從 FDH2/UxPlay 官方 v1.73.7 原始碼封存檔建置，不會下載或使用 uxplay-windows 專案。'
$sourceArchive = Join-Path $buildRoot 'uxplay-v1.73.7.tar.gz'
$uxplaySourceRoot = Join-Path $buildRoot 'uxplay-source'
$uxplaySource = Join-Path $uxplaySourceRoot 'UxPlay-1.73.7'
if (-not (Test-Path -LiteralPath $sourceArchive)) {
    Save-RemoteFile 'https://github.com/FDH2/UxPlay/archive/refs/tags/v1.73.7.tar.gz' $sourceArchive
}
if (-not (Test-Path -LiteralPath $uxplaySource)) {
    New-Item -ItemType Directory -Path $uxplaySourceRoot -Force | Out-Null
    $cygpath = Join-Path $msysBin 'cygpath.exe'
    $tar = Join-Path $msysBin 'tar.exe'
    $sourceArchiveMsysPath = (& $cygpath '-u' $sourceArchive).Trim()
    $uxplaySourceRootMsysPath = (& $cygpath '-u' $uxplaySourceRoot).Trim()
    & $tar '-xzf' $sourceArchiveMsysPath '-C' $uxplaySourceRootMsysPath
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $uxplaySource 'CMakeLists.txt'))) {
        throw 'UxPlay v1.73.7 原始碼封存檔無法展開。'
    }
}

$service = Get-CimInstance Win32_Service -Filter "Name='Bonjour Service'"
if (-not $service) {
    throw '建置 UxPlay 需要先安裝 Bonjour Service。App 首次啟動時可在確認後完成安裝。'
}

$bonjourSourceImagePath = [string]$service.PathName
if (-not (Test-TrustedBonjourExecutablePath $bonjourSourceImagePath)) {
    $backupPath = Get-BonjourServiceBackupPath
    if (Test-Path -LiteralPath $backupPath -PathType Leaf) {
        $bonjourBackup = Get-Content -LiteralPath $backupPath -Raw | ConvertFrom-Json
        if (($bonjourBackup.ServiceName -eq 'Bonjour Service') -and
            (Test-TrustedBonjourExecutablePath $bonjourBackup.OriginalImagePath)) {
            $bonjourSourceImagePath = [string]$bonjourBackup.OriginalImagePath
        }
    }
}
if (-not (Test-TrustedBonjourExecutablePath $bonjourSourceImagePath)) {
    throw '建置 UxPlay 找不到可信任且可用的 Bonjour 安裝來源。請在 Program Files 安裝 Bonjour。'
}

$bonjourSourceExecutable = Resolve-ServiceExecutablePath $bonjourSourceImagePath
$bonjourDirectory = Split-Path -Parent $bonjourSourceExecutable
$bonjourDll = Get-CompatibleBonjourDnsSdPath -ResponderPath $bonjourSourceExecutable
if ([string]::IsNullOrWhiteSpace($bonjourDll)) {
    throw '建置 UxPlay 找不到與 Bonjour Service 版本相符的可信任 x64 DNS-SD 元件（dnssd.dll）。'
}
$bonjourSdk = Join-Path $buildRoot 'bonjour-sdk'
$bonjourInclude = Join-Path $bonjourSdk 'Include'
$bonjourLibDirectory = Join-Path $bonjourSdk 'Lib\x64'
New-Item -ItemType Directory -Path $bonjourInclude,$bonjourLibDirectory -Force | Out-Null

$dnsSdHeader = Join-Path $bonjourInclude 'dns_sd.h'
Save-RemoteFile 'https://raw.githubusercontent.com/apple-oss-distributions/mDNSResponder/main/mDNSShared/dns_sd.h' $dnsSdHeader
$gendef = Join-Path $ucrtBin 'gendef.exe'
$dlltool = Join-Path $ucrtBin 'dlltool.exe'
if (-not (Test-Path -LiteralPath $gendef) -or -not (Test-Path -LiteralPath $dlltool)) {
    throw 'MSYS2 工具缺少 gendef.exe 或 dlltool.exe，請確認 mingw-w64-ucrt-x86_64-tools 與 binutils 已安裝。'
}
Push-Location $bonjourLibDirectory
try {
    & $gendef $bonjourDll
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $bonjourLibDirectory 'dnssd.def'))) {
        throw '無法從本機 dnssd.dll 產生 Bonjour 匯出定義檔。'
    }
    & $dlltool '-m' 'i386:x86-64' '-d' (Join-Path $bonjourLibDirectory 'dnssd.def') '-D' 'dnssd.dll' '-l' (Join-Path $bonjourLibDirectory 'dnssd.lib')
    if ($LASTEXITCODE -ne 0) {
        throw '無法產生 UxPlay 連結用的 Bonjour import library。'
    }
}
finally {
    Pop-Location
}

$uxplayBuild = Join-Path $buildRoot 'uxplay-build'
$env:BONJOUR_SDK_HOME = $bonjourSdk
$cmake = Join-Path $ucrtBin 'cmake.exe'
$ninja = Join-Path $ucrtBin 'ninja.exe'
& $cmake '-S' $uxplaySource '-B' $uxplayBuild '-G' 'Ninja' '-DCMAKE_BUILD_TYPE=Release' '-DNO_MARCH_NATIVE=ON'
if ($LASTEXITCODE -ne 0) {
    throw "UxPlay CMake 設定失敗，結束代碼：$LASTEXITCODE"
}
& $cmake '--build' $uxplayBuild '--config' 'Release' '--parallel'
if ($LASTEXITCODE -ne 0) {
    throw "UxPlay 建置失敗，結束代碼：$LASTEXITCODE"
}

$engineDirectory = Join-Path $projectRoot 'engine'
$uxplayExecutable = Join-Path $uxplayBuild 'uxplay.exe'
if (-not (Test-Path -LiteralPath $uxplayExecutable)) {
    throw "建置成功但找不到 UxPlay 執行檔：$uxplayExecutable"
}
Copy-Item -LiteralPath $uxplayExecutable -Destination (Join-Path $engineDirectory 'uxplay.exe') -Force

$runtimeDirectory = Join-Path $engineDirectory 'runtime'
$runtimeBin = Join-Path $runtimeDirectory 'bin'
$pluginDirectory = Join-Path $runtimeDirectory 'lib\gstreamer-1.0'
$runtimeScanner = Join-Path $runtimeDirectory 'libexec\gstreamer-1.0'
$ucrtPluginDirectory = Join-Path $Msys2Root 'ucrt64\lib\gstreamer-1.0'
$ucrtScanner = Join-Path $Msys2Root 'ucrt64\libexec\gstreamer-1.0\gst-plugin-scanner.exe'
$runtimeFullPath = [IO.Path]::GetFullPath($runtimeDirectory)
$projectRootPrefix = [IO.Path]::GetFullPath($projectRoot).TrimEnd('\') + '\'
if (-not $runtimeFullPath.StartsWith($projectRootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "拒絕清理專案目錄外的 GStreamer 執行期路徑：$runtimeFullPath"
}
foreach ($protectedPath in @($engineDirectory, $runtimeDirectory)) {
    if (Test-Path -LiteralPath $protectedPath) {
        $item = Get-Item -LiteralPath $protectedPath -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "拒絕清理重新解析點路徑：$protectedPath"
        }
    }
}
if (Test-Path -LiteralPath $runtimeDirectory) {
    Remove-Item -LiteralPath $runtimeDirectory -Recurse -Force
}
New-Item -ItemType Directory -Path $runtimeBin,$pluginDirectory,$runtimeScanner -Force | Out-Null
$requiredPlugins = @(
    'libgstapp.dll',
    'libgstautodetect.dll',
    'libgstcoreelements.dll',
    'libgstd3d.dll',
    'libgstd3d11.dll',
    'libgstd3d12.dll',
    'libgstdirectsound.dll',
    'libgstaudioconvert.dll',
    'libgstaudioresample.dll',
    'libgstlevel.dll',
    'libgstlibav.dll',
    'libgstjpeg.dll',
    'libgstplayback.dll',
    'libgsttypefindfunctions.dll',
    'libgstvideoconvertscale.dll',
    'libgstvideoparsersbad.dll',
    'libgstvolume.dll',
    'libgstwasapi.dll',
    'libgstwasapi2.dll'
)
foreach ($plugin in $requiredPlugins) {
    $pluginPath = Join-Path $ucrtPluginDirectory $plugin
    if (-not (Test-Path -LiteralPath $pluginPath -PathType Leaf)) {
        throw "MSYS2 GStreamer 缺少 UxPlay 必要外掛：$plugin"
    }
    Copy-Item -LiteralPath $pluginPath -Destination $pluginDirectory -Force
}
if (Test-Path -LiteralPath $ucrtScanner) {
    Copy-Item -LiteralPath $ucrtScanner -Destination (Join-Path $runtimeScanner 'gst-plugin-scanner.exe') -Force
}

$objdump = Join-Path $ucrtBin 'objdump.exe'
if (-not (Test-Path -LiteralPath $objdump)) {
    throw 'MSYS2 缺少 objdump.exe，無法整理 GStreamer 執行期 DLL。'
}
$queue = [Collections.Generic.Queue[string]]::new()
$visited = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$queue.Enqueue((Join-Path $engineDirectory 'uxplay.exe'))
Get-ChildItem -LiteralPath $pluginDirectory -Filter '*.dll' -File | ForEach-Object { $queue.Enqueue($_.FullName) }
$stagedScanner = Join-Path $runtimeScanner 'gst-plugin-scanner.exe'
if (Test-Path -LiteralPath $stagedScanner) { $queue.Enqueue($stagedScanner) }

while ($queue.Count -gt 0) {
    $binary = $queue.Dequeue()
    if (-not $visited.Add($binary)) { continue }

    $imports = & $objdump '-p' $binary 2>$null |
        ForEach-Object { if ($_ -match '^\s*DLL Name:\s*(.+?)\s*$') { $Matches[1] } }
    foreach ($importName in $imports) {
        $dependency = Join-Path $ucrtBin $importName
        if (Test-Path -LiteralPath $dependency -PathType Leaf) {
            $stagedDependency = Join-Path $runtimeBin $importName
            if (-not (Test-Path -LiteralPath $stagedDependency)) {
                Copy-Item -LiteralPath $dependency -Destination $stagedDependency -Force
            }
            $queue.Enqueue($stagedDependency)
        }
    }
}

$licenseDirectory = Join-Path $runtimeDirectory 'licenses'
$msysLicenses = Join-Path $Msys2Root 'ucrt64\share\licenses'
if (Test-Path -LiteralPath $msysLicenses) {
    New-Item -ItemType Directory -Path $licenseDirectory -Force | Out-Null
    Copy-Item -Path (Join-Path $msysLicenses '*') -Destination $licenseDirectory -Recurse -Force
}
$engineLicenseDirectory = Join-Path $engineDirectory 'licenses'
New-Item -ItemType Directory -Path $engineLicenseDirectory -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $uxplaySource 'LICENSE') -Destination (Join-Path $engineLicenseDirectory 'UxPlay-GPL-3.0.txt') -Force

Write-Host 'UxPlay 與 GStreamer 執行檔已放入專案 engine 目錄。'
Write-Host "UxPlay: $(Join-Path $engineDirectory 'uxplay.exe')"
Write-Host "Bonjour: $bonjourDirectory"
Write-Host "影音執行期: $runtimeDirectory"

& (Join-Path $PSScriptRoot 'Stage-ScrcpyRuntime.ps1')
if ($LASTEXITCODE -ne 0) {
    throw "scrcpy 執行環境 staging 失敗，結束代碼：$LASTEXITCODE"
}
