[CmdletBinding()]
param(
    [switch]$Development,
    [switch]$Check,
    [switch]$Gui,
    [switch]$Elevated,
    [string]$EngineDirectory = $env:NEO_AIRPLAY_ENGINE_DIR,
    [string]$MarkerFile,
    [string]$ResultFile,
    [string]$ProgressFile,
    [string]$LogFile,
    [string]$OriginalUserProfile,
    [string]$OriginalLocalAppData,
    [string]$OriginalUserSid
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-StrictMode -Version Latest

$UxPlayVersion = '1.74'
$UxPlayCommit = 'f524d8aa28028ad5572ae1b462d119e4cec1f9f4'
$UxPlaySha256 = '6cd720c5e2debee120fb076df69e76d35d19d11d9f7e377c3a747d4b8d6b692a'
$UxPlayUrl = "https://github.com/FDH2/UxPlay/archive/$UxPlayCommit.tar.gz"
$ScrcpyVersion = '5.0.1'
$ScrcpyWin64Sha256 = 'b12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5'
$ScrcpyWin64Url = "https://github.com/Genymobile/scrcpy/releases/download/v$ScrcpyVersion/scrcpy-win64-v$ScrcpyVersion.zip"
$BunVersion = '1.4.2'
$NodeVersion = $null
$script:EngineDirectory = if ($EngineDirectory) { [IO.Path]::GetFullPath($EngineDirectory) } else { [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\engine')) }
$script:EngineBin = Join-Path $script:EngineDirectory 'bin'
$script:EngineAndroid = Join-Path $script:EngineDirectory 'android'
$env:NEO_AIRPLAY_ENGINE_DIR = $script:EngineDirectory
$script:SetupUserProfile = if ($OriginalUserProfile) { [IO.Path]::GetFullPath($OriginalUserProfile) } else { [IO.Path]::GetFullPath($env:USERPROFILE) }
$script:SetupLocalAppData = if ($OriginalLocalAppData) { [IO.Path]::GetFullPath($OriginalLocalAppData) } else { [IO.Path]::GetFullPath($env:LOCALAPPDATA) }
$script:SetupUserSid = if ($OriginalUserSid) { $OriginalUserSid } else { [Security.Principal.WindowsIdentity]::GetCurrent().User.Value }
$env:CARGO_HOME = Join-Path $script:SetupUserProfile '.cargo'
$env:RUSTUP_HOME = Join-Path $script:SetupUserProfile '.rustup'
$script:InstallStage = 'checking'
$script:InstallMessage = '正在檢查投影工具…'
$script:InstallFailure = $null
$MsysPackages = @(
    'mingw-w64-ucrt-x86_64-cmake',
    'mingw-w64-ucrt-x86_64-gcc',
    'mingw-w64-ucrt-x86_64-headers',
    'mingw-w64-ucrt-x86_64-crt',
    'mingw-w64-ucrt-x86_64-binutils',
    'mingw-w64-ucrt-x86_64-ninja',
    'mingw-w64-ucrt-x86_64-pkgconf',
    'mingw-w64-ucrt-x86_64-libplist',
    'mingw-w64-ucrt-x86_64-openssl',
    'mingw-w64-ucrt-x86_64-gstreamer',
    'mingw-w64-ucrt-x86_64-gst-plugins-base',
    'mingw-w64-ucrt-x86_64-gst-plugins-good',
    'mingw-w64-ucrt-x86_64-gst-plugins-bad',
    'mingw-w64-ucrt-x86_64-gst-libav'
)

function Write-Info([string]$Message) {
    if ($LogFile -and -not $Check) {
        $stream = [IO.FileStream]::new($LogFile, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
        $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false))
        try { $writer.WriteLine($Message) } finally { $writer.Dispose() }
    } else {
        Write-Host $Message
    }
}

function Stop-Setup([string]$Message, [string]$Summary = '') {
    if ($Summary) { $script:InstallFailure = $Summary }
    throw "Setup failed: $Message"
}

function Write-InstallJson([string]$Path, [hashtable]$Value) {
    $directory = Split-Path -Parent $Path
    if ($directory) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }
    $temporary = "$Path.tmp.$PID"
    try {
        [IO.File]::WriteAllText($temporary, ($Value | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            [IO.File]::Replace($temporary, $Path, [NullString]::Value)
        } else {
            [IO.File]::Move($temporary, $Path)
        }
    } finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
    }
}

function Set-InstallStage([string]$Stage, [string]$Message) {
    $script:InstallStage = $Stage
    $script:InstallMessage = $Message
    if ($ProgressFile -and -not $Check) {
        Write-InstallJson $ProgressFile @{ pid = $PID; stage = $Stage; message = $Message }
    }
    Write-Info $Message
}

function ConvertTo-ProcessArgument([string]$Value) {
    $escaped = [regex]::Replace($Value, '(\\*)"', '$1$1\"')
    return '"' + [regex]::Replace($escaped, '(\\+)$', '$1$1') + '"'
}

function Invoke-SetupProcess([string]$FileName, [string[]]$Arguments = @(), [hashtable]$Environment = @{}, [switch]$Quiet) {
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FileName
    $startInfo.Arguments = (($Arguments | ForEach-Object { ConvertTo-ProcessArgument $_ }) -join ' ')
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($name in $Environment.Keys) {
        if ($null -eq $Environment[$name]) { $startInfo.EnvironmentVariables.Remove($name) }
        else { $startInfo.EnvironmentVariables[$name] = [string]$Environment[$name] }
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $output = [Text.StringBuilder]::new()
    try {
        $null = $process.Start()
        $process.StandardInput.Close()
        $readers = @($process.StandardOutput, $process.StandardError)
        $pending = @($readers[0].ReadLineAsync(), $readers[1].ReadLineAsync())
        while ($null -ne $pending[0] -or $null -ne $pending[1]) {
            for ($index = 0; $index -lt 2; $index++) {
                if ($null -eq $pending[$index] -or -not $pending[$index].IsCompleted) { continue }
                $line = $pending[$index].GetAwaiter().GetResult()
                if ($null -eq $line) { $pending[$index] = $null; continue }
                if (-not $Quiet) { Write-Info $line }
                $null = $output.AppendLine($line)
                if ($output.Length -gt 65536) { $null = $output.Remove(0, $output.Length - 65536) }
                $pending[$index] = $readers[$index].ReadLineAsync()
            }
            if ($null -ne $pending[0] -or $null -ne $pending[1]) { Start-Sleep -Milliseconds 10 }
        }
        $process.WaitForExit()
        return @{ ExitCode = $process.ExitCode; Output = $output.ToString() }
    } finally {
        $process.Dispose()
    }
}

if ($Development) {
    $nodeVersionPath = Join-Path $PSScriptRoot '..\.node-version'
    if (-not (Test-Path -LiteralPath $nodeVersionPath -PathType Leaf)) { Stop-Setup "Node.js version file is unavailable: $nodeVersionPath" }
    $NodeVersion = (Get-Content -LiteralPath $nodeVersionPath -Raw).Trim()
    if (-not $NodeVersion) { Stop-Setup "Node.js version file is empty: $nodeVersionPath" }
}

function Test-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Invoke-ElevatedInstaller {
    if ($Check -or $Elevated -or (Test-Administrator)) {
        return $null
    }

    Set-InstallStage 'authorizing' '請完成系統授權提示，以便繼續準備工具。'
    $powershell = (Get-Process -Id $PID).Path
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass')
    if ($Gui) { $arguments += '-NonInteractive' }
    $arguments += @('-File', $PSCommandPath)
    if ($Development) { $arguments += '-Development' }
    if ($Gui) { $arguments += '-Gui' }
    $arguments += @('-EngineDirectory', $script:EngineDirectory)
    if ($MarkerFile) { $arguments += @('-MarkerFile', $MarkerFile) }
    if ($ResultFile) { $arguments += @('-ResultFile', $ResultFile) }
    if ($ProgressFile) { $arguments += @('-ProgressFile', $ProgressFile) }
    if ($LogFile) { $arguments += @('-LogFile', $LogFile) }
    $arguments += @('-OriginalUserProfile', $script:SetupUserProfile)
    $arguments += @('-OriginalLocalAppData', $script:SetupLocalAppData)
    $arguments += @('-OriginalUserSid', $script:SetupUserSid)
    $arguments += '-Elevated'
    $startOptions = @{
        FilePath = $powershell
        ArgumentList = (($arguments | ForEach-Object { ConvertTo-ProcessArgument $_ }) -join ' ')
        Verb = 'RunAs'
        Wait = $true
        PassThru = $true
    }
    if ($Gui) { $startOptions.WindowStyle = 'Hidden' }
    try {
        $process = Start-Process @startOptions
        try { return $process.ExitCode } finally { $process.Dispose() }
    } catch {
        $exception = $_.Exception
        while ($exception) {
            if ($exception -is [ComponentModel.Win32Exception] -and $exception.NativeErrorCode -eq 1223) {
                $script:InstallFailure = '已取消系統授權。請按重新檢查並安裝後，再允許必要的授權。'
                break
            }
            $exception = $exception.InnerException
        }
        throw
    }
}

function Get-WinGetPath {
    $command = Get-Command winget.exe -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    foreach ($localAppData in @($script:SetupLocalAppData, $env:LOCALAPPDATA)) {
        if (-not $localAppData) { continue }
        $knownPath = Join-Path $localAppData 'Microsoft\WindowsApps\winget.exe'
        if (Test-Path -LiteralPath $knownPath -PathType Leaf) { return $knownPath }
    }
    return $null
}

function Ensure-WinGet {
    $winget = Get-WinGetPath
    if ($winget) { return $winget }
    if ($Check) { return $null }

    $appInstaller = Get-AppxPackage -Name Microsoft.DesktopAppInstaller -ErrorAction SilentlyContinue
    if ($appInstaller) {
        Add-AppxPackage -RegisterByFamilyName -MainPackage Microsoft.DesktopAppInstaller_8wekyb3d8bbwe
        $winget = Get-WinGetPath
        if ($winget) { return $winget }
    }

    Write-Info 'Bootstrapping WinGet with the documented Microsoft PowerShell module.'
    Install-PackageProvider -Name NuGet -Force -Confirm:$false | Out-Null
    Install-Module -Name Microsoft.WinGet.Client -Force -Repository PSGallery -Confirm:$false
    Import-Module Microsoft.WinGet.Client -Force
    Repair-WinGetPackageManager -AllUsers -Force
    $winget = Get-WinGetPath
    if (-not $winget) { Stop-Setup 'WinGet bootstrap completed but winget.exe is not available. Install Microsoft App Installer and rerun setup.' }
    return $winget
}

function Invoke-WinGetInstall([string]$PackageId, [string]$Version = '', [string]$Override = '', [string]$Scope = '') {
    $winget = Ensure-WinGet
    if (-not $winget) { Stop-Setup 'WinGet is required to install Windows development tools.' }
    $arguments = @('install', '--id', $PackageId, '--exact', '--source', 'winget', '--silent', '--accept-source-agreements', '--accept-package-agreements', '--disable-interactivity')
    if ($Version) { $arguments += @('--version', $Version) }
    if ($Override) { $arguments += @('--override', $Override) }
    if ($Scope) { $arguments += @('--scope', $Scope) }
    $result = Invoke-SetupProcess $winget $arguments
    $alreadyInstalledCodes = @(-1978335189, -1978335135)
    if ($result.ExitCode -ne 0 -and $alreadyInstalledCodes -notcontains $result.ExitCode) {
        Stop-Setup "WinGet failed to install $PackageId (exit code $($result.ExitCode))."
    }
}

function Get-MsysRoot {
    $candidates = @()
    if ($env:NEO_AIRPLAY_MSYS2_ROOT) { $candidates += $env:NEO_AIRPLAY_MSYS2_ROOT }
    if ($env:SystemDrive) { $candidates += (Join-Path $env:SystemDrive 'msys64') }
    $candidates += 'C:\msys64'
    foreach ($localAppData in @($script:SetupLocalAppData, $env:LOCALAPPDATA)) {
        if ($localAppData) {
            $candidates += (Join-Path $localAppData 'Programs\MSYS2')
            $candidates += (Join-Path $localAppData 'MSYS2')
        }
    }
    foreach ($candidate in ($candidates | Select-Object -Unique)) {
        if ($candidate -and (Test-Path -LiteralPath (Join-Path $candidate 'usr\bin\bash.exe') -PathType Leaf)) {
            return (Get-Item -LiteralPath $candidate).FullName
        }
    }
    return $null
}

function Set-ProcessMsysEnvironment([string]$MsysRoot) {
    $ucrtBin = Join-Path $MsysRoot 'ucrt64\bin'
    $usrBin = Join-Path $MsysRoot 'usr\bin'
    $env:MSYSTEM = 'UCRT64'
    $env:MSYS2_PATH_TYPE = 'inherit'
    $env:CHERE_INVOKING = '1'
    $env:NEO_AIRPLAY_MSYS2_ROOT = $MsysRoot
    $priority = @()
    if ($NodeVersion) {
        $nodePath = Join-Path $script:SetupLocalAppData "NeoAirPlay\node-v$NodeVersion"
        if (Test-Path -LiteralPath $nodePath -PathType Container) { $priority += $nodePath }
    }
    $priority += (Join-Path $script:SetupUserProfile '.cargo\bin')
    $priority += (Join-Path $script:SetupUserProfile '.bun\bin')
    $priority += $ucrtBin
    $priority += $usrBin
    $priority += (Join-Path $script:SetupLocalAppData 'Microsoft\WinGet\Links')
    $env:Path = (($priority + @($env:Path -split ';')) | Where-Object { $_ } | Select-Object -Unique) -join ';'
}

function Get-MsysProcessEnvironment {
    $environment = @{
        MSYSTEM = 'UCRT64'
        MINGW_PREFIX = '/ucrt64'
        MSYS2_PATH_TYPE = 'minimal'
        CHERE_INVOKING = '1'
        LANG = 'C'
        LC_ALL = 'C'
        PATH = (@((Join-Path $script:MsysRoot 'ucrt64\bin'), (Join-Path $script:MsysRoot 'usr\bin'), (Join-Path $env:SystemRoot 'System32'), $env:SystemRoot) -join ';')
    }
    foreach ($name in @('CC', 'CXX', 'CPP', 'CFLAGS', 'CXXFLAGS', 'CPPFLAGS', 'LDFLAGS', 'CPATH', 'C_INCLUDE_PATH', 'CPLUS_INCLUDE_PATH', 'GCC_EXEC_PREFIX', 'COMPILER_PATH', 'LIBRARY_PATH', 'CMAKE_TOOLCHAIN_FILE', 'CMAKE_PREFIX_PATH', 'CMAKE_GENERATOR', 'CMAKE_GENERATOR_PLATFORM', 'CMAKE_GENERATOR_TOOLSET', 'PKG_CONFIG', 'PKG_CONFIG_PATH', 'PKG_CONFIG_LIBDIR', 'PKG_CONFIG_SYSROOT_DIR', 'INCLUDE', 'LIB')) {
        $environment[$name] = $null
    }
    return $environment
}

function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
    $bash = Join-Path $script:MsysRoot 'usr\bin\bash.exe'
    $Command = $Command.Replace("`r`n", "`n")
    $result = Invoke-SetupProcess $bash @('--noprofile', '--norc', '-c', $Command) (Get-MsysProcessEnvironment)
    if ($AllowFailure) { return $result }
    if ($result.ExitCode -ne 0) {
        Stop-Setup "MSYS2 command failed with exit code $($result.ExitCode): $Command"
    }
}

function Invoke-MsysPacmanWithLockRetry([string]$Command, [switch]$AllowFailure) {
    $retryLimit = 24
    $retryDelaySeconds = 5
    for ($attempt = 0; $attempt -le $retryLimit; $attempt++) {
        $result = Invoke-Msys $Command -AllowFailure
        if ($result.ExitCode -eq 0) {
            if ($AllowFailure) { return $result }
            return
        }

        if ($result.Output -notmatch '(?i)unable to lock database') {
            if ($AllowFailure) { return $result }
            Stop-Setup "MSYS2 command failed with exit code $($result.ExitCode): $Command"
        }

        if ($attempt -eq $retryLimit) {
            Stop-Setup "MSYS2 package database lock remained after $($retryLimit * $retryDelaySeconds) seconds: $Command" 'MSYS2 套件資料庫鎖定逾時，請確認沒有其他 MSYS2 套件作業，查看安裝記錄後再試。'
        }

        if ($attempt -eq 0) {
            Set-InstallStage 'waiting-for-package-lock' 'MSYS2 套件資料庫目前忙碌，等待後重試…'
            Write-Info "MSYS2 package database lock detected; retrying every $retryDelaySeconds seconds for up to $($retryLimit * $retryDelaySeconds) seconds."
        }
        Start-Sleep -Seconds $retryDelaySeconds
    }
}

function Update-Msys2 {
    Set-InstallStage 'updating' '正在更新工具套件…'
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $result = Invoke-MsysPacmanWithLockRetry 'pacman -Syu --noconfirm --noprogressbar' -AllowFailure
        if ($result.Output.Contains('To complete this update all MSYS2 processes')) {
            Write-Info 'MSYS2 core update requested a fresh shell; continuing the full update.'
            continue
        }
        if ($result.ExitCode -ne 0) {
            Stop-Setup "MSYS2 update failed with exit code $($result.ExitCode)." '工具套件更新失敗，請查看安裝記錄後重試。'
        }
        return
    }
    Stop-Setup 'MSYS2 core update still requires a restart after three shells.' '工具套件更新未完成，請查看安裝記錄後重試。'
}

function ConvertTo-MsysPath([string]$Path) {
    $result = Invoke-SetupProcess (Join-Path $script:MsysRoot 'usr\bin\cygpath.exe') @('-u', $Path) (Get-MsysProcessEnvironment) -Quiet
    if ($result.ExitCode -ne 0 -or -not $result.Output.Trim()) { Stop-Setup "Could not convert the MSYS2 path: $Path" }
    return $result.Output.Trim()
}

function Add-UserPathEntry([string]$PathEntry) {
    if (-not (Test-Path -LiteralPath $PathEntry -PathType Container)) { return }
    $existing = Get-OriginalUserEnvironmentValue 'Path'
    $parts = @()
    if ($existing) { $parts = @($existing -split ';' | Where-Object { $_ }) }
    if (-not ($parts | Where-Object { [string]::Equals($_.TrimEnd('\'), $PathEntry.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase) })) {
        $parts = @($PathEntry) + $parts
        Set-OriginalUserEnvironmentValue 'Path' ($parts -join ';')
    }
}

function Get-OriginalUserEnvironmentValue([string]$Name) {
    $currentSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    if ([string]::Equals($currentSid, $script:SetupUserSid, [StringComparison]::OrdinalIgnoreCase)) {
        return [Environment]::GetEnvironmentVariable($Name, 'User')
    }
    $key = $null
    try {
        $key = [Microsoft.Win32.Registry]::Users.OpenSubKey("$($script:SetupUserSid)\Environment", $false)
        if ($key) { return $key.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) }
        return $null
    } catch {
        Write-Info "Could not read the original Windows user's $Name environment value."
        return $null
    } finally {
        if ($key) { $key.Dispose() }
    }
}

function Set-OriginalUserEnvironmentValue([string]$Name, [string]$Value) {
    $currentSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    if ([string]::Equals($currentSid, $script:SetupUserSid, [StringComparison]::OrdinalIgnoreCase)) {
        [Environment]::SetEnvironmentVariable($Name, $Value, 'User')
        return $true
    }
    $key = $null
    try {
        $key = [Microsoft.Win32.Registry]::Users.CreateSubKey("$($script:SetupUserSid)\Environment")
        if (-not $key) { throw 'The original Windows user environment key is unavailable.' }
        $kind = if ($Name -eq 'Path') { [Microsoft.Win32.RegistryValueKind]::ExpandString } else { [Microsoft.Win32.RegistryValueKind]::String }
        $key.SetValue($Name, $Value, $kind)
        return $true
    } catch {
        Write-Info "Could not update the original Windows user's $Name environment value: $($_.Exception.Message)"
        return $false
    } finally {
        if ($key) { $key.Dispose() }
    }
}

function Set-InstalledEnvironment([string]$MsysRoot) {
    $null = Set-OriginalUserEnvironmentValue 'NEO_AIRPLAY_MSYS2_ROOT' $MsysRoot
    Add-UserPathEntry (Join-Path $MsysRoot 'ucrt64\bin')
    Add-UserPathEntry (Join-Path $script:SetupLocalAppData 'Microsoft\WinGet\Links')
    Add-UserPathEntry (Join-Path $script:SetupUserProfile '.bun\bin')
    $env:NEO_AIRPLAY_MSYS2_ROOT = $MsysRoot
    Set-ProcessMsysEnvironment $MsysRoot
}

function Test-Command([string]$Name, [System.Collections.Generic.List[string]]$Missing) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        [void]$Missing.Add($Name)
        return $false
    }
    return $true
}

function Invoke-BoundedProbe([string]$FileName, [string]$Arguments, [int]$TimeoutMilliseconds = 10000) {
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FileName
    $startInfo.Arguments = $Arguments
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        $null = $process.Start()
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            $process.Kill()
            $null = $process.WaitForExit(3000)
            return @{ TimedOut = $true; ExitCode = -1; Output = '' }
        }
        $null = $stdoutTask.Wait(1000)
        $null = $stderrTask.Wait(1000)
        return @{ TimedOut = $false; ExitCode = $process.ExitCode; Output = ($stdoutTask.Result + "`n" + $stderrTask.Result) }
    } finally {
        $process.Dispose()
    }
}

function Get-EngineToolPath([string]$Name) {
    $names = @($Name)
    if (-not $Name.EndsWith('.exe', [StringComparison]::OrdinalIgnoreCase)) { $names += "$Name.exe" }
    foreach ($directory in @($script:EngineBin, (Join-Path $script:EngineAndroid 'platform-tools'), $script:EngineAndroid, $script:EngineDirectory)) {
        foreach ($candidateName in $names) {
            $candidate = Join-Path $directory $candidateName
            if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
        }
    }
    return $null
}

function Test-Runtime {
    $missing = [System.Collections.Generic.List[string]]::new()
    $script:MsysRoot = Get-MsysRoot
    $tools = @{
        uxplay = Get-EngineToolPath 'uxplay'
        scrcpy = Get-EngineToolPath 'scrcpy'
        adb = Get-EngineToolPath 'adb'
        gst = Get-EngineToolPath 'gst-inspect-1.0'
    }
    if (-not $tools.uxplay) { [void]$missing.Add('uxplay 1.73.7 or newer') }
    if (-not $tools.scrcpy) { [void]$missing.Add('scrcpy') }
    $scrcpyServer = Join-Path $script:EngineBin 'scrcpy-server'
    if (-not (Test-Path -LiteralPath $scrcpyServer -PathType Leaf) -or (Get-Item -LiteralPath $scrcpyServer -ErrorAction SilentlyContinue).Length -le 0) {
        [void]$missing.Add('scrcpy server (scrcpy-server)')
    }
    if (-not $tools.adb) { [void]$missing.Add('adb') }
    if (-not $tools.gst) { [void]$missing.Add('GStreamer (gst-inspect-1.0)') }

    $env:Path = (($script:EngineBin, (Join-Path $script:EngineAndroid 'platform-tools'), $env:Path) -join ';')
    $pluginDirectory = Join-Path $script:EngineDirectory 'lib\gstreamer-1.0'
    $env:GST_PLUGIN_PATH_1_0 = $pluginDirectory
    $env:GST_PLUGIN_SYSTEM_PATH_1_0 = $pluginDirectory
    $scanner = Join-Path $script:EngineBin 'gst-plugin-scanner.exe'
    $env:GST_PLUGIN_SCANNER_1_0 = $scanner

    $probeSpecs = @(
        @{ Path = $tools.scrcpy; Arguments = '--version'; Label = 'scrcpy' },
        @{ Path = $tools.adb; Arguments = 'version'; Label = 'adb' }
    )
    foreach ($probeSpec in $probeSpecs) {
        if (-not $probeSpec.Path) { continue }
        $probe = Invoke-BoundedProbe $probeSpec.Path $probeSpec.Arguments
        if ($probe.TimedOut -or $probe.ExitCode -ne 0 -or -not $probe.Output.Trim()) {
            [void]$missing.Add("$($probeSpec.Label) (unusable or version probe timed out)")
        }
    }

    if ($tools.uxplay) {
        $uxplayProbe = Invoke-BoundedProbe $tools.uxplay '-v'
        $versionMatch = [regex]::Match($uxplayProbe.Output, '(?im)UxPlay version\s+([0-9]+(?:\.[0-9]+)+)')
        $minimumUxPlay = [Version]'1.73.7'
        $uxplayVersion = $null
        if ($versionMatch.Success) { $uxplayVersion = [Version]$versionMatch.Groups[1].Value }
        if ($uxplayProbe.TimedOut) {
            [void]$missing.Add('UxPlay version probe (timed out)')
        } elseif (-not $uxplayVersion -or $uxplayVersion -lt $minimumUxPlay) {
            [void]$missing.Add("UxPlay $minimumUxPlay or newer")
        }
    }
    if ($tools.gst) {
        foreach ($element in @('decodebin', 'h264parse', 'videoconvert', 'autovideosink', 'autoaudiosink', 'avdec_aac')) {
            try {
                $probe = Invoke-BoundedProbe $tools.gst $element 30000
                if ($probe.TimedOut) {
                    [void]$missing.Add("GStreamer element $element (probe timed out)")
                } elseif ($probe.ExitCode -ne 0) {
                    [void]$missing.Add("GStreamer element $element")
                }
            } catch {
                [void]$missing.Add("GStreamer element $element (probe failed)")
            }
        }
    }
    if (-not $script:MsysRoot -and $missing.Count -gt 0) {
        [void]$missing.Add('MSYS2 UCRT64 package installation source')
    }
    if ($missing.Count -gt 0) {
        Write-Info 'Missing runtime dependencies:'
        $missing | ForEach-Object { Write-Info "  - $_" }
        return $false
    }
    Write-Info 'Runtime dependencies are available.'
    return $true
}

function Test-Development {
    $ErrorActionPreference = 'Continue'
    $missing = [System.Collections.Generic.List[string]]::new()
    $rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if (-not $rustup) {
        [void]$missing.Add('Rust stable (rustup, rustc, cargo)')
    } else {
        $rustcProbe = Invoke-BoundedProbe 'rustc' '+stable --version'
        if ($rustcProbe.TimedOut -or $rustcProbe.ExitCode -ne 0) { [void]$missing.Add('Rust stable (rustup, rustc, cargo)') }
        $cargoProbe = Invoke-BoundedProbe 'cargo' '+stable --version'
        if (($cargoProbe.TimedOut -or $cargoProbe.ExitCode -ne 0) -and -not $missing.Contains('Rust stable (rustup, rustc, cargo)')) { [void]$missing.Add('Rust stable (rustup, rustc, cargo)') }
        $componentsProbe = Invoke-BoundedProbe $rustup.Source 'component list --toolchain stable --installed'
        if ($componentsProbe.TimedOut -or $componentsProbe.ExitCode -ne 0 -or $componentsProbe.Output -notmatch '(?m)^rustfmt(?:-|\s)' -or $componentsProbe.Output -notmatch '(?m)^clippy(?:-|\s)') {
            [void]$missing.Add('Rust stable components (rustfmt, clippy)')
        }
    }
    $bun = Get-Command bun -ErrorAction SilentlyContinue
    if (-not $bun -or ((& bun --version 2>$null | Out-String).Trim() -ne $BunVersion)) { [void]$missing.Add("Bun $BunVersion") }
    $node = Get-Command node -ErrorAction SilentlyContinue
    if (-not $node -or ((& node --version 2>$null | Out-String).Trim() -ne "v$NodeVersion")) { [void]$missing.Add("Node.js $NodeVersion") }
    if (-not (Test-VisualStudioCppTools)) { [void]$missing.Add('Visual Studio C++ build tools (MSVC)') }
    if (-not (Test-WebView2Runtime)) { [void]$missing.Add('Microsoft Edge WebView2 Runtime') }
    if ($missing.Count -gt 0) {
        Write-Info 'Missing development tools:'
        $missing | ForEach-Object { Write-Info "  - $_" }
        return $false
    }
    Write-Info "Development tools are available (Rust stable, Bun 1.4.2, Node.js $NodeVersion)."
    Write-Info 'Visual Studio C++ build tools are available.'
    if (-not (Get-Command pwsh -ErrorAction SilentlyContinue)) {
        Write-Info 'PowerShell 7 (pwsh) is not installed; Windows PowerShell remains available.'
    }
    return $true
}

function Sync-MsysTrust {
    $anchorsDir = Join-Path $script:MsysRoot 'etc\pki\ca-trust\source\anchors'
    if (Test-Path -LiteralPath $anchorsDir -PathType Container) {
        $pemBuilder = [System.Text.StringBuilder]::new()
        $certs = Get-ChildItem Cert:\LocalMachine\Root -ErrorAction SilentlyContinue | Sort-Object NotBefore -Descending
        $seenSubjects = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($cert in $certs) {
            if ($cert.Subject -eq $cert.Issuer -and $seenSubjects.Add($cert.Subject)) {
                [void]$pemBuilder.AppendLine('-----BEGIN CERTIFICATE-----')
                [void]$pemBuilder.AppendLine([Convert]::ToBase64String($cert.RawData, [Base64FormattingOptions]::InsertLineBreaks))
                [void]$pemBuilder.AppendLine('-----END CERTIFICATE-----')
            }
        }
        if ($pemBuilder.Length -gt 0) {
            [IO.File]::WriteAllText((Join-Path $anchorsDir 'windows-roots.pem'), $pemBuilder.ToString(), [Text.UTF8Encoding]::new($false))
            Invoke-Msys 'update-ca-trust'
        }
    }
    $pacmanConf = Join-Path $script:MsysRoot 'etc\pacman.conf'
    if (Test-Path -LiteralPath $pacmanConf -PathType Leaf) {
        $confContent = Get-Content -LiteralPath $pacmanConf -Raw
        if ($confContent -match '(?m)^SigLevel\s*=') {
            $confContent = [regex]::Replace($confContent, '(?m)^SigLevel\s*=.*$', 'SigLevel = Never')
            [IO.File]::WriteAllText($pacmanConf, $confContent, [Text.UTF8Encoding]::new($false))
        }
    }
    Invoke-Msys 'pacman-key --init && pacman-key --populate msys2'
}

function Install-Msys2 {
    $script:MsysRoot = Get-MsysRoot
    if (-not $script:MsysRoot) {
        Set-InstallStage 'msys2' '正在下載並安裝建置工具…'
        Write-Info 'Installing MSYS2 from the official WinGet package.'
        Invoke-WinGetInstall 'MSYS2.MSYS2'
        $script:MsysRoot = Get-MsysRoot
        if (-not $script:MsysRoot) { Stop-Setup 'MSYS2 was installed but its UCRT64 root was not found.' }
    }
    Set-ProcessMsysEnvironment $script:MsysRoot
    Sync-MsysTrust
    $env:NEO_SETUP_PACKAGES = $MsysPackages -join ' '
    try {
        Update-Msys2
        Set-InstallStage 'packages' '正在安裝投影與編譯所需的套件…'
        Invoke-MsysPacmanWithLockRetry 'pacman -S --needed --noconfirm --noprogressbar $NEO_SETUP_PACKAGES'
        Ensure-UcrtToolchain
    } finally {
        Remove-Item Env:\NEO_SETUP_PACKAGES -ErrorAction SilentlyContinue
    }
}

function Test-UcrtToolchain {
    $probeRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $probeRoot -Force | Out-Null
    try {
        $cSource = @'
#include <winsock2.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stddef.h>
#include <time.h>
#include <vadefs.h>
int main(void) {
    WSADATA data;
    if (WSAStartup(MAKEWORD(2, 2), &data) != 0) return 1;
    WSACleanup();
    puts("C toolchain ready");
    return 0;
}
'@
        $cppSource = @'
#include <iostream>
#include <vector>
int main() {
    std::vector<int> values(1, 42);
    std::cout << values.at(0) << std::endl;
    return values.at(0) == 42 ? 0 : 1;
}
'@
        [IO.File]::WriteAllText((Join-Path $probeRoot 'probe.c'), $cSource, [Text.UTF8Encoding]::new($false))
        [IO.File]::WriteAllText((Join-Path $probeRoot 'probe.cpp'), $cppSource, [Text.UTF8Encoding]::new($false))
        $env:NEO_SETUP_PROBE = ConvertTo-MsysPath $probeRoot
        $command = @'
for header in stdio.h stdlib.h stdint.h stddef.h time.h vadefs.h winsock2.h; do
    if [ ! -s "/ucrt64/include/$header" ]; then printf 'Missing UCRT64 header: %s\n' "$header" >&2; exit 1; fi
done
/ucrt64/bin/gcc "$NEO_SETUP_PROBE/probe.c" -o "$NEO_SETUP_PROBE/probe-c.exe" -lws2_32 &&
"$NEO_SETUP_PROBE/probe-c.exe" &&
/ucrt64/bin/g++ "$NEO_SETUP_PROBE/probe.cpp" -o "$NEO_SETUP_PROBE/probe-cpp.exe" &&
"$NEO_SETUP_PROBE/probe-cpp.exe"
'@
        $result = Invoke-Msys $command -AllowFailure
        return $result.ExitCode -eq 0
    } finally {
        Remove-Item Env:\NEO_SETUP_PROBE -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $probeRoot -Recurse -Force
    }
}

function Ensure-UcrtToolchain {
    Set-InstallStage 'toolchain' '正在驗證編譯工具…'
    if (Test-UcrtToolchain) { return }
    Set-InstallStage 'repairing' '編譯工具不完整，正在修復…'
    $null = Invoke-Msys 'pacman -Q mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-headers mingw-w64-ucrt-x86_64-crt mingw-w64-ucrt-x86_64-binutils; pacman -Qkk mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-headers mingw-w64-ucrt-x86_64-crt mingw-w64-ucrt-x86_64-binutils; /ucrt64/bin/gcc -v -E -x c /dev/null' -AllowFailure
    Invoke-MsysPacmanWithLockRetry 'pacman -S --noconfirm --noprogressbar mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-headers mingw-w64-ucrt-x86_64-crt mingw-w64-ucrt-x86_64-binutils'
    if (-not (Test-UcrtToolchain)) {
        Stop-Setup 'UCRT64 C/C++ compilation or linking still failed after reinstalling the toolchain.' '編譯工具修復後仍無法使用，請查看安裝記錄後重試。'
    }
}

function Stage-MsysRuntime {
    Set-InstallStage 'staging' '正在整理投影工具檔案…'
    $ucrtBin = Join-Path $script:MsysRoot 'ucrt64\bin'
    $pluginSource = Join-Path $script:MsysRoot 'ucrt64\lib\gstreamer-1.0'
    $pluginTarget = Join-Path $script:EngineDirectory 'lib\gstreamer-1.0'
    $shareSource = Join-Path $script:MsysRoot 'ucrt64\share\gstreamer-1.0'
    $shareTarget = Join-Path $script:EngineDirectory 'share\gstreamer-1.0'
    New-Item -ItemType Directory -Path $script:EngineBin -Force | Out-Null

    foreach ($library in (Get-ChildItem -LiteralPath $ucrtBin -Filter '*.dll' -File)) {
        $targetLibrary = Join-Path $script:EngineBin $library.Name
        if (-not (Test-Path -LiteralPath $targetLibrary -PathType Leaf) -or $library.Name -match '^(av|sw|postproc)') {
            Copy-Item -LiteralPath $library.FullName -Destination $targetLibrary -Force
        }
    }
    $libexecScanner = Join-Path $script:MsysRoot 'ucrt64\libexec\gstreamer-1.0\gst-plugin-scanner.exe'
    foreach ($source in @(
        (Join-Path $ucrtBin 'gst-inspect-1.0.exe'),
        (Join-Path $ucrtBin 'gst-plugin-scanner.exe'),
        $libexecScanner
    )) {
        if (Test-Path -LiteralPath $source -PathType Leaf) {
            Copy-Item -LiteralPath $source -Destination (Join-Path $script:EngineBin ([IO.Path]::GetFileName($source))) -Force
        }
    }
    if (Test-Path -LiteralPath $pluginSource -PathType Container) {
        New-Item -ItemType Directory -Path $pluginTarget -Force | Out-Null
        Get-ChildItem -LiteralPath $pluginSource -Force | Copy-Item -Destination $pluginTarget -Recurse -Force
    }
    if (Test-Path -LiteralPath $shareSource -PathType Container) {
        New-Item -ItemType Directory -Path $shareTarget -Force | Out-Null
        Get-ChildItem -LiteralPath $shareSource -Force | Copy-Item -Destination $shareTarget -Recurse -Force
    }
    $gstCache = Join-Path $script:SetupLocalAppData 'Microsoft\Windows\INetCache\gstreamer-1.0'
    if (Test-Path -LiteralPath $gstCache -PathType Container) {
        Remove-Item -Path (Join-Path $gstCache 'registry*') -Force -ErrorAction SilentlyContinue
    }
}

function Install-UxPlay {
    $existing = Get-EngineToolPath 'uxplay'
    if ($existing) {
        $probe = Invoke-BoundedProbe $existing '-v'
        $versionMatch = [regex]::Match($probe.Output, '(?im)UxPlay version\s+([0-9]+(?:\.[0-9]+)+)')
        if (-not $probe.TimedOut -and $versionMatch.Success -and [Version]$versionMatch.Groups[1].Value -ge [Version]'1.73.7') { return }
    }
    $tempRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
    $archive = Join-Path $tempRoot 'uxplay.tar.gz'
    $source = Join-Path $tempRoot 'source'
    $build = Join-Path $tempRoot 'build'
    New-Item -ItemType Directory -Path $source -Force | Out-Null
    try {
        Set-InstallStage 'uxplay-download' '正在下載 AirPlay 投影工具…'
        Write-Info "Downloading pinned UxPlay $UxPlayVersion experimental source ($UxPlayCommit)."
        Invoke-WebRequest -Uri $UxPlayUrl -OutFile $archive -UseBasicParsing
        $actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $UxPlaySha256) { Stop-Setup 'UxPlay source archive SHA-256 verification failed.' }

        $env:NEO_SETUP_ARCHIVE = ConvertTo-MsysPath $archive
        $env:NEO_SETUP_SOURCE = ConvertTo-MsysPath $source
        $env:NEO_SETUP_BUILD = ConvertTo-MsysPath $build
        $env:NEO_SETUP_ENGINE = ConvertTo-MsysPath $script:EngineDirectory
        Set-InstallStage 'uxplay-build' '正在編譯 AirPlay 投影工具…'
        Invoke-Msys 'tar -xzf "$NEO_SETUP_ARCHIVE" -C "$NEO_SETUP_SOURCE" --strip-components=1 && /ucrt64/bin/cmake -S "$NEO_SETUP_SOURCE" -B "$NEO_SETUP_BUILD" -G Ninja -DCMAKE_C_COMPILER=/ucrt64/bin/gcc.exe -DCMAKE_CXX_COMPILER=/ucrt64/bin/g++.exe -DCMAKE_INSTALL_PREFIX="$NEO_SETUP_ENGINE" && /ucrt64/bin/cmake --build "$NEO_SETUP_BUILD" --parallel'
        Set-InstallStage 'uxplay-install' '正在安裝 AirPlay 投影工具…'
        Invoke-Msys 'cmake --install "$NEO_SETUP_BUILD"'
    }
    finally {
        Remove-Item Env:\NEO_SETUP_ARCHIVE, Env:\NEO_SETUP_SOURCE, Env:\NEO_SETUP_BUILD, Env:\NEO_SETUP_ENGINE -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
    }
}

function Install-Scrcpy {
    $scrcpyPath = Join-Path $script:EngineBin 'scrcpy.exe'
    $scrcpyServer = Join-Path $script:EngineBin 'scrcpy-server'
    $adbPath = Join-Path $script:EngineBin 'adb.exe'
    if ((Test-Path -LiteralPath $scrcpyPath -PathType Leaf) -and
        (Test-Path -LiteralPath $adbPath -PathType Leaf) -and
        (Test-Path -LiteralPath $scrcpyServer -PathType Leaf) -and
        (Get-Item -LiteralPath $scrcpyServer -ErrorAction SilentlyContinue).Length -gt 0) {
        $scrcpyProbe = Invoke-BoundedProbe $scrcpyPath '--version'
        $adbProbe = Invoke-BoundedProbe $adbPath 'version'
        if (-not $scrcpyProbe.TimedOut -and $scrcpyProbe.ExitCode -eq 0 -and $scrcpyProbe.Output.Trim() -and
            -not $adbProbe.TimedOut -and $adbProbe.ExitCode -eq 0 -and $adbProbe.Output.Trim()) { return }
    }
    $tempRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
    $archive = Join-Path $tempRoot "scrcpy-win64-v$ScrcpyVersion.zip"
    $extract = Join-Path $tempRoot 'extract'
    New-Item -ItemType Directory -Path $extract -Force | Out-Null
    try {
        Set-InstallStage 'scrcpy-download' '正在下載 Android 投影工具…'
        Write-Info "Downloading official scrcpy $ScrcpyVersion Windows distribution."
        Invoke-WebRequest -Uri $ScrcpyWin64Url -OutFile $archive -UseBasicParsing
        $actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $ScrcpyWin64Sha256) { Stop-Setup 'The official Windows scrcpy archive SHA-256 verification failed.' }
        Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
        $bundle = Get-ChildItem -LiteralPath $extract -Directory | Select-Object -First 1
        if (-not $bundle) { Stop-Setup 'The official Windows scrcpy archive did not contain its distribution directory.' }
        foreach ($requiredFile in @('scrcpy.exe', 'scrcpy-server', 'adb.exe')) {
            if (-not (Test-Path -LiteralPath (Join-Path $bundle.FullName $requiredFile) -PathType Leaf)) {
                Stop-Setup "The official Windows scrcpy archive is missing $requiredFile."
            }
        }
        New-Item -ItemType Directory -Path $script:EngineBin -Force | Out-Null
        Get-ChildItem -LiteralPath $bundle.FullName -File -Force | Copy-Item -Destination $script:EngineBin -Force
    }
    finally {
        if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
    }
}

function Install-DevelopmentTools {
    Refresh-ProcessPath
    Install-RustStable
    $rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if (-not $rustup) { Stop-Setup 'rustup was installed but could not be found on PATH.' }
    $rustcProbe = Invoke-BoundedProbe 'rustc' '+stable --version'
    $cargoProbe = Invoke-BoundedProbe 'cargo' '+stable --version'
    if ($rustcProbe.TimedOut -or $rustcProbe.ExitCode -ne 0 -or $cargoProbe.TimedOut -or $cargoProbe.ExitCode -ne 0) {
        & $rustup.Source toolchain install stable --profile minimal --no-self-update
        if ($LASTEXITCODE -ne 0) { Stop-Setup "rustup failed to install Rust stable (exit code $LASTEXITCODE)." }
    }
    $componentsProbe = Invoke-BoundedProbe $rustup.Source 'component list --toolchain stable --installed'
    if ($componentsProbe.TimedOut -or $componentsProbe.ExitCode -ne 0 -or $componentsProbe.Output -notmatch '(?m)^rustfmt(?:-|\s)' -or $componentsProbe.Output -notmatch '(?m)^clippy(?:-|\s)') {
        & $rustup.Source component add rustfmt clippy --toolchain stable
        if ($LASTEXITCODE -ne 0) { Stop-Setup "rustup failed to install rustfmt and clippy (exit code $LASTEXITCODE)." }
    }
    & $rustup.Source default stable
    if ($LASTEXITCODE -ne 0) { Stop-Setup "rustup failed to select Rust stable (exit code $LASTEXITCODE)." }

    Install-Node22
    if (-not (Test-VisualStudioCppTools)) {
        $override = '--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
        if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') {
            $override += ' --add Microsoft.VisualStudio.Component.VC.Tools.ARM64'
        }
        Invoke-WinGetInstall 'Microsoft.VisualStudio.BuildTools' '' $override 'machine'
    }
    if (-not (Test-WebView2Runtime)) {
        Invoke-WinGetInstall 'Microsoft.EdgeWebView2Runtime' '' '' 'machine'
    }
    Install-Bun
    $pwsh = Get-Command pwsh -ErrorAction SilentlyContinue
    if (-not $pwsh) {
        try { Invoke-WinGetInstall 'Microsoft.PowerShell' '' '' 'machine' }
        catch { Write-Info 'PowerShell 7 could not be installed from WinGet; Windows PowerShell remains available.' }
    }
    Refresh-ProcessPath
}

function Install-RustStable {
    $cargoHome = Join-Path $script:SetupUserProfile '.cargo'
    $rustupHome = Join-Path $script:SetupUserProfile '.rustup'
    $env:CARGO_HOME = $cargoHome
    $env:RUSTUP_HOME = $rustupHome
    $rustup = Join-Path $cargoHome 'bin\rustup.exe'
    if (-not (Test-Path -LiteralPath $rustup -PathType Leaf)) {
        $architecture = $env:PROCESSOR_ARCHITECTURE
        if ($env:PROCESSOR_ARCHITEW6432) { $architecture = $env:PROCESSOR_ARCHITEW6432 }
        if ($architecture -eq 'ARM64') { $target = 'aarch64-pc-windows-msvc' }
        elseif ($architecture -eq 'AMD64') { $target = 'x86_64-pc-windows-msvc' }
        else { Stop-Setup "Rust stable is unavailable for Windows architecture $architecture." }
        $temporaryRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
        $installer = Join-Path $temporaryRoot 'rustup-init.exe'
        $checksumFile = "$installer.sha256"
        $installerUrl = "https://static.rust-lang.org/rustup/dist/$target/rustup-init.exe"
        New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
        try {
            Invoke-WebRequest -Uri $installerUrl -OutFile $installer -UseBasicParsing
            Invoke-WebRequest -Uri "$installerUrl.sha256" -OutFile $checksumFile -UseBasicParsing
            $expectedHash = ((Get-Content -LiteralPath $checksumFile -Raw) -split '\s+')[0].ToLowerInvariant()
            $actualHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualHash -ne $expectedHash) { Stop-Setup 'Official rustup-init SHA-256 verification failed.' }
            New-Item -ItemType Directory -Path $cargoHome, $rustupHome -Force | Out-Null
            $env:CARGO_HOME = $cargoHome
            $env:RUSTUP_HOME = $rustupHome
            & $installer -y --no-modify-path --profile minimal --default-toolchain stable
            if ($LASTEXITCODE -ne 0) { Stop-Setup "rustup-init failed with exit code $LASTEXITCODE." }
        } finally {
            if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
        }
        Add-UserPathEntry (Join-Path $cargoHome 'bin')
        $env:Path = "$(Join-Path $cargoHome 'bin');$env:Path"
    }
}

function Test-VcRedistRuntime {
    if (-not $env:SystemRoot) { return $false }
    return (Test-Path -LiteralPath (Join-Path $env:SystemRoot 'System32\vcruntime140.dll') -PathType Leaf)
}

function Install-Bun {
    $ErrorActionPreference = 'Continue'
    $bun = Join-Path $script:SetupUserProfile '.bun\bin\bun.exe'
    if ((Test-Path -LiteralPath $bun -PathType Leaf) -and ((& $bun --version 2>$null | Out-String).Trim() -eq $BunVersion)) { return }
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $architecture = $env:PROCESSOR_ARCHITEW6432 }
    if ($architecture -eq 'ARM64') {
        $asset = 'bun-windows-aarch64.zip'
        $expectedHash = 'a7a16b876a305fd1029c66dbd27007b4f6112ae896532f675878731a21e50cfd'
        $redist = 'Microsoft.VCRedist.2015+.arm64'
    } elseif ($architecture -eq 'AMD64') {
        $asset = 'bun-windows-x64.zip'
        $expectedHash = 'ce4c17497b2f29712a99d3d53f028de28cd42e3bacb8589599e7f000e49b6405'
        $redist = 'Microsoft.VCRedist.2015+.x64'
    } else {
        Stop-Setup "Official Bun $BunVersion binaries are unavailable for architecture $architecture."
    }
    if (-not (Test-VcRedistRuntime)) {
        Invoke-WinGetInstall $redist '' '' 'machine'
    }
    $temporaryRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
    $archive = Join-Path $temporaryRoot $asset
    $extractRoot = Join-Path $temporaryRoot 'extract'
    $bunRoot = Join-Path $script:SetupUserProfile '.bun\bin'
    New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null
    try {
        Invoke-WebRequest -Uri "https://github.com/oven-sh/bun/releases/download/bun-v$BunVersion/$asset" -OutFile $archive -UseBasicParsing
        $actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $expectedHash) { Stop-Setup "Bun $BunVersion archive SHA-256 verification failed." }
        Expand-Archive -LiteralPath $archive -DestinationPath $extractRoot -Force
        $source = Get-ChildItem -LiteralPath $extractRoot -Filter bun.exe -Recurse -File | Select-Object -First 1
        if (-not $source) { Stop-Setup 'Official Bun archive did not contain bun.exe.' }
        New-Item -ItemType Directory -Path $bunRoot -Force | Out-Null
        Copy-Item -LiteralPath $source.FullName -Destination (Join-Path $bunRoot 'bun.exe') -Force
        Add-UserPathEntry $bunRoot
        $env:Path = "$bunRoot;$env:Path"
    } finally {
        if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
    }
}

function Install-Node22 {
    $ErrorActionPreference = 'Continue'
    if ((Get-Command node -ErrorAction SilentlyContinue) -and ((& node --version 2>$null | Out-String).Trim() -eq "v$NodeVersion")) { return }
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $architecture = $env:PROCESSOR_ARCHITEW6432 }
    if ($architecture -eq 'ARM64') {
        $asset = "node-v$NodeVersion-win-arm64.zip"
        $expectedHash = '33dad22e4cef5ee8f9fbb1b0d037fdacd0e56d12a4580f0d63f68b894deab535'
    } elseif ($architecture -eq 'AMD64') {
        $asset = "node-v$NodeVersion-win-x64.zip"
        $expectedHash = '2b0ff57b049cda1bbcea2240eec20467018713c1efe1f7360c2681859b90ed71'
    } else {
        Stop-Setup "Official Node.js $NodeVersion binaries are unavailable for architecture $architecture."
    }
    $temporaryRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
    $archive = Join-Path $temporaryRoot $asset
    $extractRoot = Join-Path $temporaryRoot 'extract'
    $nodeRoot = Join-Path $script:SetupLocalAppData "NeoAirPlay\node-v$NodeVersion"
    New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null
    try {
        Invoke-WebRequest -Uri "https://nodejs.org/dist/v$NodeVersion/$asset" -OutFile $archive -UseBasicParsing
        $actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $expectedHash) { Stop-Setup "Node.js $NodeVersion archive SHA-256 verification failed." }
        Expand-Archive -LiteralPath $archive -DestinationPath $extractRoot -Force
        $sourceRoot = Get-ChildItem -LiteralPath $extractRoot -Directory | Select-Object -First 1
        if (-not $sourceRoot -or -not (Test-Path -LiteralPath (Join-Path $sourceRoot.FullName 'node.exe'))) {
            Stop-Setup "Official Node.js archive did not contain node.exe."
        }
        New-Item -ItemType Directory -Path $nodeRoot -Force | Out-Null
        Copy-Item -Path (Join-Path $sourceRoot.FullName '*') -Destination $nodeRoot -Recurse -Force
        Add-UserPathEntry $nodeRoot
        $env:Path = "$nodeRoot;$env:Path"
    } finally {
        if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
    }
}

function Test-VisualStudioCppTools {
    $ErrorActionPreference = 'Continue'
    $programFilesX86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if (-not $programFilesX86) { return $false }
    $vswhere = Join-Path $programFilesX86 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) { return $false }
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $architecture = $env:PROCESSOR_ARCHITEW6432 }
    $component = if ($architecture -eq 'ARM64') { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
    $installation = & $vswhere -products '*' -requires $component -property installationPath 2>$null
    if ($LASTEXITCODE -eq 0 -and [bool]$installation) { return $true }
    $targetArch = if ($architecture -eq 'ARM64') { 'arm64' } else { 'x64' }
    $clPath = & $vswhere -products '*' -find "VC\Tools\MSVC\**\bin\Host*\$targetArch\cl.exe" 2>$null
    $linkPath = & $vswhere -products '*' -find "VC\Tools\MSVC\**\bin\Host*\$targetArch\link.exe" 2>$null
    $sdkRoot = Join-Path $programFilesX86 'Windows Kits\10\Lib'
    $hasSdk = (Test-Path -LiteralPath $sdkRoot -PathType Container) -and [bool](Get-ChildItem -LiteralPath $sdkRoot -Directory -ErrorAction SilentlyContinue | Where-Object {
        Test-Path -LiteralPath (Join-Path $_.FullName "um\$targetArch\kernel32.lib") -PathType Leaf
    } | Select-Object -First 1)
    return ([bool]$clPath -and [bool]$linkPath -and $hasSdk)
}

function Test-WebView2Runtime {
    $webView2ClientId = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $registryPaths = @(
        'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients',
        "Registry::HKEY_USERS\$($script:SetupUserSid)\Software\Microsoft\EdgeUpdate\Clients"
    )
    foreach ($registryPath in $registryPaths) {
        if (-not (Test-Path -LiteralPath $registryPath)) { continue }
        foreach ($client in (Get-ChildItem -LiteralPath $registryPath -ErrorAction SilentlyContinue)) {
            $properties = Get-ItemProperty -LiteralPath $client.PSPath -ErrorAction SilentlyContinue
            if (-not $properties) { continue }
            $pvProp = $properties.PSObject.Properties['pv']
            $pv = if ($pvProp) { [string]$pvProp.Value } else { '' }
            if (-not $pv -or $pv -eq '0.0.0.0') { continue }
            $nameProp = $properties.PSObject.Properties['name']
            $name = if ($nameProp) { [string]$nameProp.Value } else { '' }
            if ([string]::Equals($client.PSChildName, $webView2ClientId, [StringComparison]::OrdinalIgnoreCase) -or $name -match 'WebView2') {
                return $true
            }
        }
    }
    $programFiles = @([Environment]::GetEnvironmentVariable('ProgramFiles(x86)'), $env:ProgramFiles, $script:SetupLocalAppData)
    foreach ($root in ($programFiles | Where-Object { $_ } | Select-Object -Unique)) {
        $runtimeRoot = Join-Path $root 'Microsoft\EdgeWebView\Application'
        if (Test-Path -LiteralPath $runtimeRoot -PathType Container) {
            $runtime = Get-ChildItem -LiteralPath $runtimeRoot -Filter msedgewebview2.exe -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($runtime) { return $true }
        }
    }
    return $false
}

function Test-NeedsElevation([bool]$RuntimeReady, [bool]$DevelopmentReady) {
    if ($RuntimeReady -and $DevelopmentReady) { return $false }
    if (-not (Get-WinGetPath)) { return $true }
    if ($Development -and -not $DevelopmentReady) {
        if (-not (Test-VisualStudioCppTools) -or -not (Test-WebView2Runtime) -or -not (Test-VcRedistRuntime)) {
            return $true
        }
    }
    return $false
}

function Refresh-ProcessPath {
    $machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $userPath = Get-OriginalUserEnvironmentValue 'Path'
    $priority = @()
    if ($NodeVersion) {
        $nodePath = Join-Path $script:SetupLocalAppData "NeoAirPlay\node-v$NodeVersion"
        if (Test-Path -LiteralPath $nodePath -PathType Container) { $priority += $nodePath }
    }
    $priority += (Join-Path $script:SetupUserProfile '.cargo\bin')
    $priority += (Join-Path $script:SetupUserProfile '.bun\bin')
    $msysRoot = Get-MsysRoot
    if ($msysRoot) {
        $priority += (Join-Path $msysRoot 'ucrt64\bin')
        $priority += (Join-Path $msysRoot 'usr\bin')
    }
    $priority += (Join-Path $script:SetupLocalAppData 'Microsoft\WinGet\Links')
    $entries = @($priority) + @($userPath -split ';') + @($machinePath -split ';') + @($env:Path -split ';')
    $seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $ordered = foreach ($entry in $entries) {
        $candidate = if ($entry) { $entry.Trim() } else { '' }
        if ($candidate -and $seen.Add($candidate)) { $candidate }
    }
    $env:Path = $ordered -join ';'
}

function Write-InstallMarker {
    if ($MarkerFile) {
        $directory = Split-Path -Parent $MarkerFile
        if ($directory) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }
        $temporaryMarker = "$MarkerFile.tmp.$PID"
        [IO.File]::WriteAllText($temporaryMarker, "$PID`n", [Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $temporaryMarker -Destination $MarkerFile -Force
    }
}

function Complete-Install([int]$ExitCode) {
    if ($script:DelegatedToElevated) { return }
    try {
        if ($ResultFile) {
            Write-InstallJson $ResultFile @{ pid = $PID; exitCode = $ExitCode; stage = $script:InstallStage; message = $script:InstallFailure }
        }
    } finally {
        if ($MarkerFile -and (Test-Path -LiteralPath $MarkerFile)) {
            Remove-Item -LiteralPath $MarkerFile -Force -ErrorAction SilentlyContinue
        }
    }
}

if ($Check) {
    Refresh-ProcessPath
    $runtimeReady = Test-Runtime
    $developmentReady = $true
    if ($Development) { $developmentReady = Test-Development }
    if ($runtimeReady -and $developmentReady) { exit 0 }
    exit 1
}

$script:DelegatedToElevated = $false
$InstallExitCode = 0
try {
    if ($LogFile) {
        $logDirectory = Split-Path -Parent $LogFile
        if ($logDirectory) { New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null }
    }
    Write-InstallMarker
    Set-InstallStage 'checking' '正在檢查投影工具…'
    Refresh-ProcessPath
    $runtimeReady = Test-Runtime
    $developmentReady = $true
    if ($Development) { $developmentReady = Test-Development }
    if ($runtimeReady -and $developmentReady) {
        Write-Info 'All requested dependencies are already available.'
    } else {
        if (Test-NeedsElevation $runtimeReady $developmentReady) {
            $elevatedExitCode = Invoke-ElevatedInstaller
            if ($null -ne $elevatedExitCode) {
                $script:DelegatedToElevated = $true
                $InstallExitCode = $elevatedExitCode
                exit $elevatedExitCode
            }
        }
        if (-not $runtimeReady) {
            $null = Ensure-WinGet
            Install-Msys2
            Set-InstalledEnvironment $script:MsysRoot
            Install-UxPlay
            Install-Scrcpy
            Stage-MsysRuntime
        }
        if ($Development -and -not $developmentReady) { Install-DevelopmentTools }
        if ($script:MsysRoot) { Set-InstalledEnvironment $script:MsysRoot }
        Set-InstallStage 'verifying' '正在驗證已安裝的投影工具…'
        $runtimeReady = Test-Runtime
        $developmentReady = $true
        if ($Development) { $developmentReady = Test-Development }
        if (-not $runtimeReady -or -not $developmentReady) { throw 'One or more requested dependencies remain unavailable after installation.' }
    }
    Set-InstallStage 'ready' '投影工具已就緒。'
}
catch {
    $InstallExitCode = 1
    $detail = $_.Exception.Message
    if (-not $script:InstallFailure) { $script:InstallFailure = "$($script:InstallMessage.TrimEnd([char[]]'…。'))時發生錯誤，請查看安裝記錄。" }
    Write-Info $detail
}
finally {
    try {
        Complete-Install $InstallExitCode
    } catch {
        [Console]::Error.WriteLine("Could not write installer result: $($_.Exception.Message)")
        $InstallExitCode = 1
    }
}
exit $InstallExitCode
