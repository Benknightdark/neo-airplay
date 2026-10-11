[CmdletBinding()]
param(
    [switch]$Development,
    [switch]$Check,
    [switch]$Gui,
    [switch]$Elevated,
    [string]$EngineDirectory = $env:NEO_AIRPLAY_ENGINE_DIR,
    [string]$MarkerFile,
    [string]$ResultFile,
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
$MsysPackages = @(
    'mingw-w64-ucrt-x86_64-cmake',
    'mingw-w64-ucrt-x86_64-gcc',
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
    Write-Host $Message
}

function Stop-Setup([string]$Message) {
    throw "Setup failed: $Message"
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

    Write-Info 'Requesting administrator authorization to install the MSYS2 runtime dependencies.'
    $powershell = (Get-Process -Id $PID).Path
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $PSCommandPath + '"'))
    if ($Development) { $arguments += '-Development' }
    if ($Gui) { $arguments += '-Gui' }
    $arguments += @('-EngineDirectory', ('"' + $script:EngineDirectory + '"'))
    if ($MarkerFile) { $arguments += @('-MarkerFile', ('"' + $MarkerFile + '"')) }
    if ($ResultFile) { $arguments += @('-ResultFile', ('"' + $ResultFile + '"')) }
    $arguments += @('-OriginalUserProfile', ('"' + $script:SetupUserProfile + '"'))
    $arguments += @('-OriginalLocalAppData', ('"' + $script:SetupLocalAppData + '"'))
    $arguments += @('-OriginalUserSid', ('"' + $script:SetupUserSid + '"'))
    $arguments += '-Elevated'
    $process = Start-Process -FilePath $powershell -ArgumentList $arguments -Verb RunAs -Wait -PassThru
    return $process.ExitCode
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
    & $winget @arguments
    if ($LASTEXITCODE -ne 0) { Stop-Setup "WinGet failed to install $PackageId (exit code $LASTEXITCODE)." }
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

function Invoke-Msys([string]$Command) {
    $bash = Join-Path $script:MsysRoot 'usr\bin\bash.exe'
    & $bash -lc $Command
    if ($LASTEXITCODE -ne 0) { Stop-Setup "MSYS2 command failed with exit code ${LASTEXITCODE}: $Command" }
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
    if (-not [IO.Path]::GetExtension($Name)) { $names += "$Name.exe" }
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
                $probe = Invoke-BoundedProbe $tools.gst $element 10000
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
        $missing | ForEach-Object { Write-Host "  - $_" }
        return $false
    }
    Write-Info 'Runtime dependencies are available.'
    return $true
}

function Test-Development {
    $missing = [System.Collections.Generic.List[string]]::new()
    $rustup = Get-Command rustup -ErrorAction SilentlyContinue
    if (-not $rustup) {
        [void]$missing.Add('Rust stable (rustup, rustc, cargo)')
    } else {
        & rustc +stable --version *> $null
        if ($LASTEXITCODE -ne 0) { [void]$missing.Add('Rust stable (rustup, rustc, cargo)') }
        & cargo +stable --version *> $null
        if ($LASTEXITCODE -ne 0 -and -not $missing.Contains('Rust stable (rustup, rustc, cargo)')) { [void]$missing.Add('Rust stable (rustup, rustc, cargo)') }
        $components = (& rustup component list --toolchain stable --installed 2>$null | Out-String)
        if ($LASTEXITCODE -ne 0 -or $components -notmatch '(?m)^rustfmt(?:-|\s)' -or $components -notmatch '(?m)^clippy(?:-|\s)') {
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
        $missing | ForEach-Object { Write-Host "  - $_" }
        return $false
    }
    Write-Info "Development tools are available (Rust stable, Bun 1.4.2, Node.js $NodeVersion)."
    Write-Info 'Visual Studio C++ build tools are available.'
    if (-not (Get-Command pwsh -ErrorAction SilentlyContinue)) {
        Write-Info 'PowerShell 7 (pwsh) is not installed; Windows PowerShell remains available.'
    }
    return $true
}

function Install-Msys2 {
    $script:MsysRoot = Get-MsysRoot
    if (-not $script:MsysRoot) {
        Write-Info 'Installing MSYS2 from the official WinGet package.'
        Invoke-WinGetInstall 'MSYS2.MSYS2'
        $script:MsysRoot = Get-MsysRoot
        if (-not $script:MsysRoot) { Stop-Setup 'MSYS2 was installed but its UCRT64 root was not found.' }
    }
    Set-ProcessMsysEnvironment $script:MsysRoot
    $bash = Join-Path $script:MsysRoot 'usr\bin\bash.exe'
    $env:NEO_SETUP_PACKAGES = $MsysPackages -join ' '
    Write-Info 'Updating MSYS2 package databases and installing UCRT64 GStreamer and UxPlay build dependencies.'
    Invoke-Msys 'pacman -Syu --noconfirm'
    Invoke-Msys 'pacman -Syu --noconfirm'
    Invoke-Msys 'pacman -S --needed --noconfirm $NEO_SETUP_PACKAGES'
    Remove-Item Env:\NEO_SETUP_PACKAGES -ErrorAction SilentlyContinue
}

function Stage-MsysRuntime {
    $ucrtBin = Join-Path $script:MsysRoot 'ucrt64\bin'
    $pluginSource = Join-Path $script:MsysRoot 'ucrt64\lib\gstreamer-1.0'
    $pluginTarget = Join-Path $script:EngineDirectory 'lib\gstreamer-1.0'
    $shareSource = Join-Path $script:MsysRoot 'ucrt64\share\gstreamer-1.0'
    $shareTarget = Join-Path $script:EngineDirectory 'share\gstreamer-1.0'
    New-Item -ItemType Directory -Path $script:EngineBin -Force | Out-Null

    foreach ($library in (Get-ChildItem -LiteralPath $ucrtBin -Filter '*.dll' -File)) {
        $targetLibrary = Join-Path $script:EngineBin $library.Name
        if (-not (Test-Path -LiteralPath $targetLibrary -PathType Leaf)) {
            Copy-Item -LiteralPath $library.FullName -Destination $targetLibrary
        }
    }
    foreach ($toolName in @('gst-inspect-1.0.exe', 'gst-plugin-scanner.exe')) {
        $source = Join-Path $ucrtBin $toolName
        if (Test-Path -LiteralPath $source -PathType Leaf) {
            Copy-Item -LiteralPath $source -Destination (Join-Path $script:EngineBin $toolName) -Force
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
}

function Install-UxPlay {
    $existing = Get-EngineToolPath 'uxplay'
    if ($existing) {
        $output = (& $existing -v 2>&1 | Out-String)
        $versionMatch = [regex]::Match($output, '(?im)UxPlay version\s+([0-9]+(?:\.[0-9]+)+)')
        if ($versionMatch.Success -and [Version]$versionMatch.Groups[1].Value -ge [Version]'1.73.7') { return }
    }
    $tempRoot = Join-Path $env:TEMP ([Guid]::NewGuid().ToString('N'))
    $archive = Join-Path $tempRoot 'uxplay.tar.gz'
    $source = Join-Path $tempRoot 'source'
    $build = Join-Path $tempRoot 'build'
    New-Item -ItemType Directory -Path $source -Force | Out-Null
    try {
        Write-Info "Downloading pinned UxPlay $UxPlayVersion experimental source ($UxPlayCommit)."
        Invoke-WebRequest -Uri $UxPlayUrl -OutFile $archive -UseBasicParsing
        $actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $UxPlaySha256) { Stop-Setup 'UxPlay source archive SHA-256 verification failed.' }

        $env:NEO_SETUP_ARCHIVE = (& (Join-Path $script:MsysRoot 'usr\bin\cygpath.exe') -u $archive | Out-String).Trim()
        $env:NEO_SETUP_SOURCE = (& (Join-Path $script:MsysRoot 'usr\bin\cygpath.exe') -u $source | Out-String).Trim()
        $env:NEO_SETUP_BUILD = (& (Join-Path $script:MsysRoot 'usr\bin\cygpath.exe') -u $build | Out-String).Trim()
        $env:NEO_SETUP_ENGINE = (& (Join-Path $script:MsysRoot 'usr\bin\cygpath.exe') -u $script:EngineDirectory | Out-String).Trim()
        Invoke-Msys 'tar -xzf "$NEO_SETUP_ARCHIVE" -C "$NEO_SETUP_SOURCE" --strip-components=1 && cmake -S "$NEO_SETUP_SOURCE" -B "$NEO_SETUP_BUILD" -G Ninja -DCMAKE_INSTALL_PREFIX="$NEO_SETUP_ENGINE" && cmake --build "$NEO_SETUP_BUILD" --parallel'
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
    & $rustup.Source toolchain install stable --profile minimal
    if ($LASTEXITCODE -ne 0) { Stop-Setup "rustup failed to install Rust stable (exit code $LASTEXITCODE)." }
    & $rustup.Source component add rustfmt clippy --toolchain stable
    if ($LASTEXITCODE -ne 0) { Stop-Setup "rustup failed to install rustfmt and clippy (exit code $LASTEXITCODE)." }
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

function Install-Bun {
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
    Invoke-WinGetInstall $redist '' '' 'machine'
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
    $nodeRoot = Join-Path $env:LOCALAPPDATA "NeoAirPlay\node-v$NodeVersion"
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
    $programFilesX86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if (-not $programFilesX86) { return $false }
    $vswhere = Join-Path $programFilesX86 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) { return $false }
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ($env:PROCESSOR_ARCHITEW6432) { $architecture = $env:PROCESSOR_ARCHITEW6432 }
    $component = if ($architecture -eq 'ARM64') { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
    $installation = & $vswhere -products '*' -requires $component -property installationPath 2>$null
    return ($LASTEXITCODE -eq 0 -and [bool]$installation)
}

function Test-WebView2Runtime {
    $registryPaths = @(
        'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients',
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients',
        "Registry::HKEY_USERS\$($script:SetupUserSid)\Software\Microsoft\EdgeUpdate\Clients"
    )
    foreach ($registryPath in $registryPaths) {
        if (-not (Test-Path -LiteralPath $registryPath)) { continue }
        foreach ($client in (Get-ChildItem -LiteralPath $registryPath -ErrorAction SilentlyContinue)) {
            $properties = Get-ItemProperty -LiteralPath $client.PSPath -ErrorAction SilentlyContinue
            if ($properties.name -match 'WebView2 Runtime' -and $properties.pv -and $properties.pv -ne '0.0.0.0') { return $true }
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
            $resultDirectory = Split-Path -Parent $ResultFile
            if ($resultDirectory) { New-Item -ItemType Directory -Path $resultDirectory -Force | Out-Null }
            $temporaryResult = Join-Path $resultDirectory (([IO.Path]::GetFileName($ResultFile)) + ".tmp.$PID")
            $resultJson = @{ pid = $PID; exitCode = $ExitCode } | ConvertTo-Json -Compress
            [IO.File]::WriteAllText($temporaryResult, $resultJson, [Text.UTF8Encoding]::new($false))
            Move-Item -LiteralPath $temporaryResult -Destination $ResultFile -Force
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
    Refresh-ProcessPath
    $runtimeReady = Test-Runtime
    $developmentReady = $true
    if ($Development) { $developmentReady = Test-Development }
    if ($runtimeReady -and $developmentReady) {
        Write-InstallMarker
        Write-Info 'All requested dependencies are already available.'
    } else {
        Write-InstallMarker
        $elevatedExitCode = Invoke-ElevatedInstaller
        if ($null -ne $elevatedExitCode) {
            $script:DelegatedToElevated = $true
            $InstallExitCode = $elevatedExitCode
            exit $elevatedExitCode
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
        $runtimeReady = Test-Runtime
        $developmentReady = $true
        if ($Development) { $developmentReady = Test-Development }
        if (-not $runtimeReady -or -not $developmentReady) { throw 'One or more requested dependencies remain unavailable after installation.' }
    }
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    $InstallExitCode = 1
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
