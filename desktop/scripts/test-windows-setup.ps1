[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$InstallerPath)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Load functions without running top-level initialization or system installers.
$tokens = $null
$parseErrors = $null
$installer = [Management.Automation.Language.Parser]::ParseFile($InstallerPath, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -gt 0) { throw ($parseErrors | Out-String) }
foreach ($statement in $installer.EndBlock.Statements) {
    if ($statement -is [Management.Automation.Language.FunctionDefinitionAst]) {
        . ([scriptblock]::Create($statement.Extent.Text))
    }
}
# Main entry scenarios below replace all package and system operations with mocks.
$mainEntry = @($installer.EndBlock.Statements | Where-Object { $_ -is [Management.Automation.Language.TryStatementAst] })[-1]

$root = Join-Path ([IO.Path]::GetTempPath()) ('neo-windows-setup-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
$script:MsysRoot = Join-Path $root 'msys64'
$script:EngineDirectory = Join-Path $root 'engine'
$script:EngineBin = Join-Path $script:EngineDirectory 'bin'
$script:SetupUserProfile = Join-Path $root 'original user'
$script:SetupLocalAppData = Join-Path $root 'original data'
$script:SetupUserSid = 'S-1-5-21-1000-2000-3000-1001'
$script:InstallStage = 'checking'
$script:InstallMessage = 'Checking tools'
$script:InstallFailure = $null
$script:DelegatedToElevated = $false
$script:ResolvedUxPlaySha256 = ''
$script:passed = 0
$Check = $false
$Upgrade = $false
$Development = $false
$Elevated = $false
$ElevatedPrerequisitesOnly = $false
$Gui = $true
$LogFile = Join-Path $root 'install.log'
$ProgressFile = Join-Path $root 'install-progress.json'
$ResultFile = Join-Path $root 'install-result.json'
$MarkerFile = Join-Path $root 'install.pid'
$powershell = (Get-Process -Id $PID).Path

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Invoke-Scenario([string]$Name, [scriptblock]$Body) {
    try {
        & $Body
        $script:passed++
    } catch {
        throw "${Name}: $($_.Exception.Message)`n$($_.ScriptStackTrace)"
    }
}

try {
    Invoke-Scenario 'Native output drains both pipes and preserves a nonzero exit' {
        $child = "[Console]::Out.WriteLine('stdout:' + ('x' * 131072)); [Console]::Error.WriteLine('stderr:' + ('y' * 131072)); [Console]::Out.WriteLine('finished'); exit 7"
        $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($child))
        $result = Invoke-SetupProcess $powershell @('-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded)
        Assert-True ($result.ExitCode -eq 7) 'Native exit code was lost'
        $log = [IO.File]::ReadAllText($LogFile)
        Assert-True ($log.Contains('stdout:') -and $log.Contains('stderr:') -and $log.Contains('finished')) 'Both pipes must reach the log'
        Assert-True ($result.Output.Length -le 65536) 'Captured output must remain bounded'
    }

    Invoke-Scenario 'Arguments are passed literally, including quotes and trailing slashes' {
        $node = (Get-Command node -ErrorAction Stop).Source
        $echoArguments = "process.stdout.write(process.argv.slice(1).map(value => { const bytes = Buffer.from(value, 'utf8'); return bytes.length + ':' + bytes.toString('base64'); }).join('.'))"
        $values = @('a b', 'a"b', 'C:\folder with spaces\', '', '$(must-stay-literal)', '中文路徑')
        $result = Invoke-SetupProcess $node (@('-e', $echoArguments) + $values) -Quiet
        Assert-True ($result.ExitCode -eq 0) 'Native argument fixture failed'
        $expectedArguments = @(
            foreach ($value in $values) {
                $bytes = [Text.Encoding]::UTF8.GetBytes($value)
                '{0}:{1}' -f $bytes.Length, [Convert]::ToBase64String($bytes)
            }
        )
        $expected = $expectedArguments -join '.'
        $actual = $result.Output.Trim()
        Assert-True ($actual -ceq $expected) "Arguments changed (expected $expected; actual $actual)"
    }

    if ($env:OS -eq 'Windows_NT') {
        Invoke-Scenario 'A background native child has no console' {
            $child = 'Add-Type -Name ConsoleProbe -Namespace NeoSetup -MemberDefinition ''[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow();''; if ([NeoSetup.ConsoleProbe]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 9 }; exit 0'
            $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($child))
            $result = Invoke-SetupProcess $powershell @('-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded)
            Assert-True ($result.ExitCode -eq 0) 'A background child allocated a console'
        }

        Invoke-Scenario 'MSYS2 environment excludes outside compiler settings' {
            $environment = Get-MsysProcessEnvironment
            Assert-True ($environment.MSYSTEM -eq 'UCRT64' -and $environment.MSYS2_PATH_TYPE -eq 'minimal') 'Wrong MSYS2 environment'
            Assert-True ($environment.PATH.StartsWith((Join-Path $script:MsysRoot 'ucrt64\bin'))) 'UCRT64 tools must come first'
            foreach ($name in @('CC', 'CXX', 'CPATH', 'GCC_EXEC_PREFIX', 'CFLAGS', 'CMAKE_TOOLCHAIN_FILE')) {
                Assert-True ($environment.ContainsKey($name) -and $null -eq $environment[$name]) "Inherited $name was not cleared"
            }
        }
    }

    Invoke-Scenario 'Core update interruption continues in a new shell' {
        $script:commands = [Collections.Generic.List[string]]::new()
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:commands.Add($Command)
            if ($script:commands.Count -eq 1) { return @{ ExitCode = 1; Output = 'To complete this update all MSYS2 processes including this terminal will be closed.' } }
            return @{ ExitCode = 0; Output = 'Full system upgrade complete' }
        }
        Update-Msys2
        Assert-True ($script:commands.Count -eq 2) 'Core update must continue exactly once'
    }

    Invoke-Scenario 'A database lock clears and the update retries' {
        $script:commands = [Collections.Generic.List[string]]::new()
        $script:sleeps = 0
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:commands.Add($Command)
            if ($script:commands.Count -eq 1) { return @{ ExitCode = 1; Output = 'error: failed to synchronize all databases (unable to lock database)' } }
            return @{ ExitCode = 0; Output = 'Full system upgrade complete' }
        }
        function Start-Sleep([int]$Seconds, [int]$Milliseconds) {
            if ($Seconds -gt 0) { $script:sleeps += $Seconds }
        }
        Update-Msys2
        Assert-True ($script:commands.Count -eq 2 -and $script:sleeps -eq 5) 'A temporary database lock must wait and retry once'
    }

    Invoke-Scenario 'A database lock clears during package installation' {
        $script:commands = [Collections.Generic.List[string]]::new()
        $script:sleeps = 0
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:commands.Add($Command)
            if ($script:commands.Count -eq 1) { return @{ ExitCode = 1; Output = 'error: failed to synchronize all databases (unable to lock database)' } }
            return @{ ExitCode = 0; Output = 'Packages installed' }
        }
        function Start-Sleep([int]$Seconds, [int]$Milliseconds) {
            if ($Seconds -gt 0) { $script:sleeps += $Seconds }
        }
        Invoke-MsysPacmanWithLockRetry 'pacman -S --needed package'
        Assert-True ($script:commands.Count -eq 2 -and $script:sleeps -eq 5) 'Package installation must use the database lock retry'
    }

    Invoke-Scenario 'A persistent database lock stops after the bounded wait' {
        $script:calls = 0
        $script:sleeps = 0
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:calls++
            return @{ ExitCode = 1; Output = 'error: failed to synchronize all databases (unable to lock database)' }
        }
        function Start-Sleep([int]$Seconds, [int]$Milliseconds) {
            if ($Seconds -gt 0) { $script:sleeps += $Seconds }
        }
        $failed = $false
        try { Update-Msys2 } catch { $failed = $true }
        Assert-True ($failed -and $script:calls -eq 25 -and $script:sleeps -eq 120) 'A persistent database lock must stop after two minutes'
        Assert-True ($script:InstallFailure.Contains('資料庫鎖定逾時')) 'The lock timeout summary was not preserved'
    }

    Invoke-Scenario 'A genuine update failure stops without retry' {
        $script:calls = 0
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:calls++
            return @{ ExitCode = 23; Output = 'failed to synchronize databases' }
        }
        $failed = $false
        $failure = ''
        try { Update-Msys2 } catch { $failed = $true; $failure = $_.Exception.Message }
        Assert-True ($failed -and $script:calls -eq 1) "An ordinary failure was treated as a restart (calls=$script:calls, error=$failure)"
    }

    Invoke-Scenario 'Core update restarts have a finite limit' {
        $script:calls = 0
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:calls++
            return @{ ExitCode = 0; Output = 'To complete this update all MSYS2 processes including this terminal will be closed.' }
        }
        $failed = $false
        try { Update-Msys2 } catch { $failed = $true }
        Assert-True ($failed -and $script:calls -eq 3) 'Repeated core updates must stop after three shells'
    }

    Invoke-Scenario 'Compilation runs once and Windows line endings are normalized' {
        $script:calls = 0
        function Get-MsysProcessEnvironment { return @{} }
        function Invoke-SetupProcess([string]$FileName, [string[]]$Arguments, [hashtable]$Environment) {
            $script:calls++
            Assert-True (-not $Arguments[-1].Contains("`r")) 'Bash must not receive CRLF script lines'
            return @{ ExitCode = 1; Output = 'fatal error: stdio.h: No such file or directory' }
        }
        $failed = $false
        try { Invoke-Msys "cmake --build broken-build`r`nprintf 'unreachable'" } catch { $failed = $true }
        Assert-True ($failed -and $script:calls -eq 1) 'Compilation must run only once'
    }

    Invoke-Scenario 'A healthy toolchain skips repair' {
        function Test-UcrtToolchain { return $true }
        function Invoke-Msys { throw 'Healthy tools must not trigger package changes' }
        Ensure-UcrtToolchain
    }

    Invoke-Scenario 'An already prepared runtime skips package managers and downloads' {
        $script:InstallFailure = $null
        $InstallExitCode = 0
        function Refresh-ProcessPath { }
        function Test-Runtime { return $true }
        function Invoke-SetupProcess { throw 'Ready tools must not start native installers' }
        function Install-Msys2 { throw 'Ready tools must not install packages' }
        function Install-UxPlay { throw 'Ready tools must not download UxPlay' }
        function Install-Scrcpy { throw 'Ready tools must not download scrcpy' }
        . ([scriptblock]::Create($mainEntry.Extent.Text))
        $result = [IO.File]::ReadAllText($ResultFile) | ConvertFrom-Json
        Assert-True ($InstallExitCode -eq 0 -and $result.exitCode -eq 0 -and $result.stage -eq 'ready') 'Ready runtime was not accepted'
    }

    Invoke-Scenario 'An explicit upgrade refreshes tools when runtime checks pass' {
        $script:Upgrade = $true
        $InstallFailure = $null
        $InstallExitCode = 0
        $script:upgradeCalls = [Collections.Generic.List[string]]::new()
        function Refresh-ProcessPath { }
        function Test-Runtime { return $true }
        function Test-NeedsElevation { return $false }
        function Ensure-WinGet { $script:upgradeCalls.Add('winget') }
        function Install-Msys2 { $script:upgradeCalls.Add('packages') }
        function Set-InstalledEnvironment { }
        function Install-UxPlay { $script:upgradeCalls.Add('uxplay') }
        function Install-Scrcpy { $script:upgradeCalls.Add('scrcpy') }
        function Stage-MsysRuntime { $script:upgradeCalls.Add('staging') }
        . ([scriptblock]::Create($mainEntry.Extent.Text))
        Assert-True ($InstallExitCode -eq 0) 'A healthy upgrade did not complete'
        Assert-True ($script:upgradeCalls -join ',' -eq 'winget,packages,uxplay,scrcpy,staging') 'Upgrade skipped a refresh stage'
        $script:Upgrade = $false
    }

    foreach ($repairSucceeds in @($true, $false)) {
        Invoke-Scenario "Toolchain repair outcome $repairSucceeds" {
            $script:probes = 0
            $script:repairs = 0
            function Test-UcrtToolchain {
                $script:probes++
                return $repairSucceeds -and $script:probes -gt 1
            }
            function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
                if ($Command -match '^pacman -S ') {
                    Assert-True ($Command -notmatch '--needed') 'Repair must reinstall even current packages'
                    foreach ($package in @('gcc', 'headers', 'crt', 'binutils')) {
                        Assert-True ($Command.Contains("mingw-w64-ucrt-x86_64-$package")) "Repair omitted $package"
                    }
                    $script:repairs++
                }
                return @{ ExitCode = 0; Output = '' }
            }
            $failed = $false
            try { $null = Ensure-UcrtToolchain } catch { $failed = $true }
            Assert-True ($script:repairs -eq 1 -and $script:probes -eq 2) 'Repair must run once and verify again'
            Assert-True ($failed -ne $repairSucceeds) 'Toolchain failure was incorrectly accepted'
        }
    }

    Invoke-Scenario 'The compiler probe covers headers and both native executables' {
        $previousTemp = $env:TEMP
        $env:TEMP = $root
        function ConvertTo-MsysPath([string]$Path) { return $Path }
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $c = [IO.File]::ReadAllText((Join-Path $env:NEO_SETUP_PROBE 'probe.c'))
            $cpp = [IO.File]::ReadAllText((Join-Path $env:NEO_SETUP_PROBE 'probe.cpp'))
            Assert-True ($c.Contains('<stdio.h>') -and $c.Contains('<vadefs.h>') -and $c.Contains('WSAStartup')) 'C probe does not cover the reported failure'
            Assert-True ($cpp.Contains('<vector>')) 'C++ runtime must also be checked'
            Assert-True ($Command.Contains('/ucrt64/bin/gcc') -and $Command.Contains('/ucrt64/bin/g++') -and $Command.Contains('"$NEO_SETUP_PROBE/probe-cpp.exe"')) 'Both compilers and execution must be checked'
            return @{ ExitCode = 1; Output = 'Missing UCRT64 header: stdio.h' }
        }
        try {
            Assert-True (-not (Test-UcrtToolchain)) 'A failed compiler probe was accepted'
            Assert-True (-not (Test-Path Env:\NEO_SETUP_PROBE)) 'Probe environment was not cleaned up'
        } finally { $env:TEMP = $previousTemp }
    }

    Invoke-Scenario 'Elevation runs only prerequisites and withholds user paths and engine directory' {
        $script:Upgrade = $true
        $script:Development = $true
        function Test-Administrator { return $false }
        function Start-Process {
            param($FilePath, $ArgumentList, $Verb, $WindowStyle, [switch]$Wait, [switch]$PassThru)
            $script:launchOptions = $PSBoundParameters
            $mockProcess = [pscustomobject]@{ ExitCode = 5 }
            $mockProcess | Add-Member -MemberType ScriptMethod -Name Dispose -Value { }
            return $mockProcess
        }
        $code = Invoke-ElevatedInstaller
        Assert-True ($code -eq 5) 'Elevated exit code was lost'
        Assert-True ($script:launchOptions.FilePath.EndsWith('System32\WindowsPowerShell\v1.0\powershell.exe')) 'Elevation must use full System32 powershell.exe path'
        Assert-True ($script:launchOptions.Verb -eq 'RunAs' -and $script:launchOptions.WindowStyle -eq 'Hidden') 'Elevation must keep UAC and hide its console'
        Assert-True ($script:launchOptions.ArgumentList.Contains('"-NonInteractive"')) 'Hidden elevation must not wait for console input'
        Assert-True ($script:launchOptions.ArgumentList.Contains('"-ElevatedPrerequisitesOnly"')) 'Elevation must be scoped to prerequisites only'
        Assert-True (-not $script:launchOptions.ArgumentList.Contains('"-Upgrade"')) 'Elevation must not run upgrade steps'
        foreach ($requiredValue in @($LogFile, $ProgressFile, $script:SetupUserSid)) {
            Assert-True ($script:launchOptions.ArgumentList.Contains((ConvertTo-ProcessArgument $requiredValue))) 'Elevation lost a required log/progress/SID argument'
        }
        foreach ($withheldValue in @($ResultFile, $MarkerFile, $script:EngineDirectory, $script:SetupUserProfile, $script:SetupLocalAppData)) {
            Assert-True (-not $script:launchOptions.ArgumentList.Contains((ConvertTo-ProcessArgument $withheldValue))) "Elevation leaked unprivileged path/marker: $withheldValue"
        }
        $script:Upgrade = $false
        $script:Development = $false
    }

    Invoke-Scenario 'Cancellation produces a readable completion result' {
        $script:InstallFailure = $null
        $InstallExitCode = 0
        function Refresh-ProcessPath { }
        function Test-Runtime { return $false }
        function Test-NeedsElevation { return $true }
        function Test-Administrator { return $false }
        function Start-Process { throw [ComponentModel.Win32Exception]::new(1223) }
        function Install-Msys2 { throw 'Cancelled authorization must not install packages' }
        . ([scriptblock]::Create($mainEntry.Extent.Text))
        Assert-True ($InstallExitCode -eq 1 -and $script:InstallFailure.Contains('已取消系統授權')) 'Cancellation summary was lost'
        $result = [IO.File]::ReadAllText($ResultFile) | ConvertFrom-Json
        Assert-True ($result.pid -eq $PID -and $result.exitCode -eq 1 -and $result.stage -eq 'authorizing') 'Completion metadata was lost'
        Assert-True ($result.message.Contains('已取消系統授權') -and -not (Test-Path -LiteralPath $MarkerFile)) 'Result must precede marker cleanup'
        Assert-True ([IO.File]::ReadAllText($LogFile).Contains('stdout:')) 'Elevation must retain earlier log output'
    }

    Invoke-Scenario 'Sync-MsysTrust preserves GPG verification and restores legacy SigLevel = Never' {
        $msysFakeRoot = Join-Path $root 'msys-gpg-test'
        $etcDir = Join-Path $msysFakeRoot 'etc'
        $anchorsDir = Join-Path $etcDir 'pki\ca-trust\source\anchors'
        New-Item -ItemType Directory -Path $anchorsDir -Force | Out-Null
        $pacmanConf = Join-Path $etcDir 'pacman.conf'
        [IO.File]::WriteAllText($pacmanConf, "[options]`nArchitecture = auto`nSigLevel = Never`n", [Text.UTF8Encoding]::new($false))
        $previousMsysRoot = $script:MsysRoot
        $script:MsysRoot = $msysFakeRoot
        $script:msysCommands = [Collections.Generic.List[string]]::new()
        function Invoke-Msys([string]$Command, [switch]$AllowFailure) {
            $script:msysCommands.Add($Command)
            if ($AllowFailure) { return @{ ExitCode = 0; Output = '' } }
        }
        try {
            Sync-MsysTrust
            $updated = [IO.File]::ReadAllText($pacmanConf)
            Assert-True (-not ($updated -match '(?m)^SigLevel\s*=\s*Never\b')) 'SigLevel = Never must be restored'
            Assert-True ($updated -match '(?m)^SigLevel\s*=\s*Required DatabaseOptional\b') 'SigLevel must be restored to Required DatabaseOptional'
            Assert-True ($script:msysCommands.Contains('pacman-key --init && pacman-key --populate msys2')) 'pacman-key initialization was skipped'
        } finally {
            $script:MsysRoot = $previousMsysRoot
        }
    }

    Invoke-Scenario 'SID validation and MSYS2 root lookup reject unsafe values and UNC paths' {
        Assert-True (Test-ValidUserSid 'S-1-5-21-100-200-300-1001') 'Valid SID was rejected'
        Assert-True (-not (Test-ValidUserSid 'original-sid')) 'Placeholder SID was accepted'
        Assert-True (-not (Test-ValidUserSid 'S-1-5\..\evil')) 'Malformed SID was accepted'
        $previousOverride = $env:NEO_AIRPLAY_MSYS2_ROOT
        $env:NEO_AIRPLAY_MSYS2_ROOT = '\\attacker\share\msys64'
        $rejected = $false
        try {
            $null = Get-MsysRoot
        } catch {
            $rejected = $_.Exception.Message.Contains('non-local')
        } finally {
            if ($null -eq $previousOverride) { Remove-Item Env:\NEO_AIRPLAY_MSYS2_ROOT -ErrorAction SilentlyContinue }
            else { $env:NEO_AIRPLAY_MSYS2_ROOT = $previousOverride }
        }
        Assert-True $rejected 'UNC NEO_AIRPLAY_MSYS2_ROOT must be rejected'
    }

    Invoke-Scenario 'Invoke-VerifiedDownload rejects non-HTTPS URLs, deletes mismatched files, and accepts valid checksums' {
        $downloadDir = Join-Path $root 'downloads'
        New-Item -ItemType Directory -Path $downloadDir -Force | Out-Null
        $destFile = Join-Path $downloadDir 'artifact.bin'
        $validHash = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
        $httpRejected = $false
        try {
            Invoke-VerifiedDownload 'http://example.com/artifact.bin' $destFile $validHash 'Insecure artifact'
        } catch {
            $httpRejected = $_.Exception.Message.Contains('non-HTTPS')
        }
        Assert-True $httpRejected 'HTTP URL must be rejected before network access'

        $script:verifiedDownloadPayload = [Text.Encoding]::UTF8.GetBytes('verified payload')
        $script:verifiedDownloadCalls = 0
        $expectedSha = ([BitConverter]::ToString(([Security.Cryptography.SHA256]::Create()).ComputeHash($script:verifiedDownloadPayload)) -replace '-', '').ToLowerInvariant()
        function Invoke-HttpsDownloadStream([string]$Uri, [string]$Destination, [string]$Label) {
            $script:verifiedDownloadCalls++
            [IO.File]::WriteAllBytes($Destination, $script:verifiedDownloadPayload)
        }
        $badHashRejected = $false
        $checksumError = ''
        try {
            Invoke-VerifiedDownload 'https://example.com/artifact.bin' $destFile ('0' * 64) 'Mismatched artifact'
        } catch {
            $checksumError = $_.Exception.Message
            $badHashRejected = $checksumError.Contains('SHA-256 verification failed')
        }
        Assert-True $badHashRejected "Checksum mismatch must throw SHA-256 error; actual error: $checksumError"
        Assert-True ($script:verifiedDownloadCalls -eq 1) 'Download mock must be called once for the mismatched checksum'
        Assert-True (-not (Test-Path -LiteralPath $destFile)) 'Failed download must not leave destination file'
        Assert-True (@(Get-ChildItem -LiteralPath $downloadDir -Filter '*.download.*').Count -eq 0) 'Failed download must clean temporary file'

        Invoke-VerifiedDownload 'https://example.com/artifact.bin' $destFile $expectedSha 'Valid artifact'
        Assert-True ($script:verifiedDownloadCalls -eq 2) 'Download mock must be called once for the valid checksum'
        Assert-True ([IO.File]::ReadAllText($destFile) -eq 'verified payload') 'Valid download did not write destination'
    }

    Invoke-Scenario 'Expand-ZipArchiveSafely blocks Zip Slip traversal and extracts valid entries' {
        Add-Type -AssemblyName System.IO.Compression
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zipDir = Join-Path $root 'zip-test'
        $extractDir = Join-Path $zipDir 'extract'
        New-Item -ItemType Directory -Path $zipDir -Force | Out-Null

        $evilZip = Join-Path $zipDir 'evil.zip'
        $zipStream = [IO.File]::Open($evilZip, [IO.FileMode]::Create)
        $archive = [IO.Compression.ZipArchive]::new($zipStream, [IO.Compression.ZipArchiveMode]::Create)
        $entry = $archive.CreateEntry('../../escaped.txt')
        $writer = [IO.StreamWriter]::new($entry.Open())
        $writer.Write('pwned')
        $writer.Dispose()
        $archive.Dispose()
        $zipStream.Dispose()

        $zipSlipBlocked = $false
        try {
            Expand-ZipArchiveSafely $evilZip $extractDir
        } catch {
            $zipSlipBlocked = $_.Exception.Message.Contains('escapes destination directory')
        }
        Assert-True $zipSlipBlocked 'Zip Slip traversal entry was not blocked'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $root 'escaped.txt'))) 'Zip Slip wrote outside destination'

        $goodZip = Join-Path $zipDir 'good.zip'
        $zipStream = [IO.File]::Open($goodZip, [IO.FileMode]::Create)
        $archive = [IO.Compression.ZipArchive]::new($zipStream, [IO.Compression.ZipArchiveMode]::Create)
        $entry = $archive.CreateEntry('bundle/tool.txt')
        $writer = [IO.StreamWriter]::new($entry.Open())
        $writer.Write('safe tool')
        $writer.Dispose()
        $archive.Dispose()
        $zipStream.Dispose()

        $goodExtract = Join-Path $zipDir 'good-extract'
        Expand-ZipArchiveSafely $goodZip $goodExtract
        Assert-True ([IO.File]::ReadAllText((Join-Path $goodExtract 'bundle/tool.txt')) -eq 'safe tool') 'Valid zip entry failed extraction'
    }

    Invoke-Scenario 'Progress replacement remains readable and check mode is read-only' {
        Set-InstallStage 'packages' 'Installing tools'
        Set-InstallStage 'uxplay-build' 'Building tools'
        $progress = [IO.File]::ReadAllText($ProgressFile) | ConvertFrom-Json
        Assert-True ($progress.pid -eq $PID -and $progress.stage -eq 'uxplay-build') 'Progress did not advance'
        $before = [IO.File]::ReadAllText($ProgressFile)
        $Check = $true
        Set-InstallStage 'checking' 'Read-only probe'
        Assert-True ([IO.File]::ReadAllText($ProgressFile) -ceq $before) 'Check mode changed progress records'
    }

    Invoke-Scenario 'Version/hash validators and SHA256SUMS manifest parser handle valid and malicious inputs' {
        Assert-True ((Test-ValidScrcpyVersion 'v5.0.1') -eq '5.0.1') 'Valid scrcpy version failed'
        $badScrcpy = $false
        try { $null = Test-ValidScrcpyVersion '5.0.1; calc.exe' } catch { $badScrcpy = $true }
        Assert-True $badScrcpy 'Malicious scrcpy version was not rejected'

        $uxVer = Resolve-ValidatedUxPlayRef 'v1.73.6'
        Assert-True ($uxVer.Version -eq '1.73.6' -and $uxVer.Ref -eq 'v1.73.6') 'Valid UxPlay version failed'
        $badUx = $false
        try { $null = Resolve-ValidatedUxPlayRef '../../etc/passwd' } catch { $badUx = $true }
        Assert-True $badUx 'Malicious UxPlay ref was not rejected'

        Assert-True (Test-ValidSha256 ('a' * 64)) 'Valid SHA256 rejected'
        Assert-True (-not (Test-ValidSha256 'short')) 'Invalid SHA256 accepted'

        $manifest = "16c3fc2068df64946f670e3504b4752073ebae47a416a45b46e79a5ab19c689b *./scrcpy-winarm64-v5.0.1.zip`nb12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5  scrcpy-win64-v5.0.1.zip"
        Assert-True ((Find-ScrcpyChecksumInManifest $manifest 'scrcpy-win64-v5.0.1.zip') -eq 'b12a2c4ee8be317422451fc7dcf8ee20a71b5ea7ef9ad73ddd825a25316227a5') 'Failed to parse win64 hash'
        Assert-True ((Find-ScrcpyChecksumInManifest $manifest 'scrcpy-winarm64-v5.0.1.zip') -eq '16c3fc2068df64946f670e3504b4752073ebae47a416a45b46e79a5ab19c689b') 'Failed to parse winarm64 hash'
    }

    Invoke-Scenario 'Assert-AllowedDownloadHost enforces HTTPS host allowlist and rejects credentials or ports' {
        Assert-AllowedDownloadHost 'https://github.com/FDH2/UxPlay/archive/v1.73.7.tar.gz' 'UxPlay'
        Assert-AllowedDownloadHost 'https://objects.githubusercontent.com/artifact.zip' 'Artifact'
        Assert-AllowedDownloadHost 'https://nodejs.org/dist/v24.22.2/node-v24.22.2-win-x64.zip' 'Node.js'
        foreach ($badUrl in @(
            'http://github.com/FDH2/UxPlay',
            'https://evil.example/payload.zip',
            'https://repo.msys2.org/distrib/msys2.exe',
            'https://user:pass@github.com/FDH2/UxPlay',
            'https://github.com:8443/FDH2/UxPlay'
        )) {
            $rejected = $false
            try { Assert-AllowedDownloadHost $badUrl 'Untrusted' } catch { $rejected = $true }
            Assert-True $rejected "Untrusted URL was accepted: $badUrl"
        }
    }

    Invoke-Scenario 'Copy-FileAtomicSecure and Test-LocalSafePath block unsafe targets' {
        $src = Join-Path $root 'src.bin'
        $dst = Join-Path $root 'dst-dir\dst.bin'
        [IO.File]::WriteAllText($src, 'secure-data')
        Copy-FileAtomicSecure $src $dst
        Assert-True ([IO.File]::ReadAllText($dst) -eq 'secure-data') 'Copy-FileAtomicSecure failed'

        Assert-True (-not (Test-LocalSafePath '\\server\share\msys64')) 'UNC path must be rejected'
        Assert-True (-not (Test-LocalSafePath '//server/share/msys64')) 'Forward-slash UNC path must be rejected'
        Assert-True (-not (Test-LocalSafePath 'relative\path')) 'Relative path must be rejected'
        Assert-True (Test-LocalSafePath (Join-Path $root 'msys64')) 'Local rooted path must be allowed'
        if ($env:OS -eq 'Windows_NT') {
            Assert-True (Test-LocalSafePath 'C:\msys64') 'Windows drive path must be allowed'
        }
    }

    Invoke-Scenario 'Expand-ZipArchiveSafely blocks rooted Windows paths and Assert-SafeTarArchive blocks Tar Slip' {
        Add-Type -AssemblyName System.IO.Compression
        $zipDir = Join-Path $root 'zip-drive-test'
        New-Item -ItemType Directory -Path $zipDir -Force | Out-Null
        $driveZip = Join-Path $zipDir 'drive.zip'
        $zipStream = [IO.File]::Open($driveZip, [IO.FileMode]::Create)
        $archive = [IO.Compression.ZipArchive]::new($zipStream, [IO.Compression.ZipArchiveMode]::Create)
        $entry = $archive.CreateEntry('C:\escaped.txt')
        $writer = [IO.StreamWriter]::new($entry.Open())
        $writer.Write('pwned')
        $writer.Dispose()
        $archive.Dispose()
        $zipStream.Dispose()

        $driveBlocked = $false
        try {
            Expand-ZipArchiveSafely $driveZip (Join-Path $zipDir 'out')
        } catch {
            $driveBlocked = $true
        }
        Assert-True $driveBlocked 'Rooted Windows ZIP entry must be blocked'

        function ConvertTo-MsysPath([string]$Path) { return $Path }
        function Invoke-Msys([string]$Command) {
            Assert-True ($Command.Contains('tar -tzf')) 'Tar check must list archive entries'
            Assert-True ($Command.Contains('tar -tvzf')) 'Tar check must inspect archive permissions/links'
            throw 'Archive entry escapes destination directory: ../../evil.txt'
        }
        $fakeArchive = Join-Path $zipDir 'fake.tar.gz'
        [IO.File]::WriteAllText($fakeArchive, 'stub')
        $tarBlocked = $false
        try { Assert-SafeTarArchive $fakeArchive } catch { $tarBlocked = $_.Exception.Message.Contains('escapes destination directory') }
        Assert-True $tarBlocked 'Assert-SafeTarArchive must propagate traversal failures'
    }

    Write-Host "$script:passed installer scenarios passed."
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force
}
