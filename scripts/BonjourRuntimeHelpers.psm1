function Resolve-ServiceExecutablePath {
    [CmdletBinding()]
    param([AllowNull()][string]$ImagePath)

    if ([string]::IsNullOrWhiteSpace($ImagePath)) { return $null }
    $trimmedPath = [Environment]::ExpandEnvironmentVariables($ImagePath.Trim())
    if ($trimmedPath.StartsWith('"')) {
        $endQuote = $trimmedPath.IndexOf('"', 1)
        if ($endQuote -lt 2) { return $null }
        return $trimmedPath.Substring(1, $endQuote - 1)
    }

    $match = [regex]::Match($trimmedPath, '^.+?\.exe', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if ($match.Success) { return $match.Value }
    return $null
}

function Get-PeMachineType {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$Path)

    $stream = $null
    $reader = $null
    try {
        $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ($stream.Length -lt 0x40) { return $null }

        $reader = [IO.BinaryReader]::new($stream, [Text.Encoding]::ASCII, $true)
        $stream.Position = 0x3C
        $peOffset = $reader.ReadUInt32()
        if ($peOffset -gt ($stream.Length - 6)) { return $null }

        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne [uint32]0x00004550) { return $null }
        return [uint16]$reader.ReadUInt16()
    }
    catch {
        return $null
    }
    finally {
        if ($reader) { $reader.Dispose() }
        if ($stream) { $stream.Dispose() }
    }
}

function Test-PathUnderRoot {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Root
    )

    if ([string]::IsNullOrWhiteSpace($Root)) { return $false }
    try {
        $fullPath = [IO.Path]::GetFullPath($Path)
        $normalizedRoot = [IO.Path]::GetFullPath($Root).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        return $fullPath.StartsWith($normalizedRoot, [StringComparison]::OrdinalIgnoreCase)
    }
    catch {
        return $false
    }
}

function Test-UsableBonjourExecutablePath {
    [CmdletBinding()]
    param([AllowNull()][string]$ImagePath)

    $executable = Resolve-ServiceExecutablePath $ImagePath
    if ([string]::IsNullOrWhiteSpace($executable) -or
        [IO.Path]::GetFileName($executable) -ine 'mDNSResponder.exe' -or
        -not (Test-Path -LiteralPath $executable -PathType Leaf)) {
        return $false
    }

    try {
        $responder = Get-Item -LiteralPath $executable
        $machine = Get-PeMachineType -Path $executable
        return $responder.VersionInfo.ProductName -eq 'Bonjour' -and
            -not [string]::IsNullOrWhiteSpace($responder.VersionInfo.FileVersion) -and
            $machine -in @([uint16]0x014C, [uint16]0x8664)
    }
    catch {
        return $false
    }
}

function Test-TrustedBonjourDnsSdPath {
    [CmdletBinding()]
    param(
        [AllowNull()][string]$Path,
        [AllowNull()][string]$ResponderPath
    )

    if ([string]::IsNullOrWhiteSpace($Path) -or
        [IO.Path]::GetFileName($Path) -ine 'dnssd.dll' -or
        -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $false
    }

    $programFiles = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
    $system32 = Join-Path $env:SystemRoot 'System32'
    if (-not (Test-PathUnderRoot -Path $Path -Root $programFiles) -and
        -not (Test-PathUnderRoot -Path $Path -Root $system32)) {
        return $false
    }

    try {
        $dnsSd = Get-Item -LiteralPath $Path
        if ($dnsSd.VersionInfo.ProductName -ne 'Bonjour' -or
            [string]::IsNullOrWhiteSpace($dnsSd.VersionInfo.FileVersion) -or
            (Get-PeMachineType -Path $Path) -ne [uint16]0x8664) {
            return $false
        }

        if (-not [string]::IsNullOrWhiteSpace($ResponderPath)) {
            $executable = Resolve-ServiceExecutablePath $ResponderPath
            if (-not (Test-UsableBonjourExecutablePath $executable)) { return $false }
            $responder = Get-Item -LiteralPath $executable
            if ($responder.VersionInfo.FileVersion -ne $dnsSd.VersionInfo.FileVersion) { return $false }
        }

        return $true
    }
    catch {
        return $false
    }
}

function Get-BonjourSourceDnsSdPaths {
    [CmdletBinding()]
    param()

    $programFiles = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
    $paths = Get-BonjourSourceExecutablePaths |
        Where-Object { Test-PathUnderRoot -Path $_ -Root $programFiles } |
        ForEach-Object { Join-Path (Split-Path -Parent $_) 'dnssd.dll' }

    $paths += Join-Path (Get-BonjourRuntimeDirectory) 'dnssd.dll'
    $paths += Join-Path (Join-Path $env:SystemRoot 'System32') 'dnssd.dll'
    $paths | Select-Object -Unique
}

function Get-CompatibleBonjourDnsSdPath {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$ResponderPath)

    $executable = Resolve-ServiceExecutablePath $ResponderPath
    if (-not (Test-TrustedBonjourExecutablePath $executable)) { return $null }

    $candidates = @(
        (Join-Path (Split-Path -Parent $executable) 'dnssd.dll')
        (Get-BonjourSourceDnsSdPaths)
    ) | Select-Object -Unique
    foreach ($candidate in $candidates) {
        if (Test-TrustedBonjourDnsSdPath -Path $candidate -ResponderPath $executable) {
            return $candidate
        }
    }

    return $null
}

function Test-UsableBonjourRuntimePair {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$ResponderPath,
        [Parameter(Mandatory)][string]$DnsSdPath
    )

    (Test-UsableBonjourExecutablePath $ResponderPath) -and
        (Test-TrustedBonjourDnsSdPath -Path $DnsSdPath -ResponderPath $ResponderPath)
}

function Test-TrustedBonjourExecutablePath {
    [CmdletBinding()]
    param([AllowNull()][string]$ImagePath)

    if (-not (Test-UsableBonjourExecutablePath $ImagePath)) {
        return $false
    }

    $executable = [IO.Path]::GetFullPath((Resolve-ServiceExecutablePath $ImagePath))
    $trustedRoots = @(
        [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
        [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)
    ) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

    foreach ($root in $trustedRoots) {
        $normalizedRoot = [IO.Path]::GetFullPath($root).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        if ($executable.StartsWith($normalizedRoot, [StringComparison]::OrdinalIgnoreCase)) {
            return $true
        }
    }

    return $false
}

function Get-BonjourServiceSettings {
    [CmdletBinding()]
    param()

    $settingsPath = Join-Path $PSScriptRoot 'BonjourServiceSettings.json'
    if (-not (Test-Path -LiteralPath $settingsPath -PathType Leaf)) {
        throw "找不到 Bonjour 執行元件設定檔：$settingsPath"
    }

    Get-Content -LiteralPath $settingsPath -Raw -Encoding UTF8 | ConvertFrom-Json
}

function Get-BonjourMissingCompatibleX64DnsSdMessage {
    [CmdletBinding()]
    param()

    $message = [string](Get-BonjourServiceSettings).MissingCompatibleX64DnsSdMessage
    if ([string]::IsNullOrWhiteSpace($message)) {
        throw 'Bonjour 執行元件設定中沒有 MissingCompatibleX64DnsSdMessage。'
    }

    $message
}

function Resolve-BonjourSettingsPath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)][string]$PropertyName,
        [Parameter(Mandatory)][string]$BaseDirectory
    )

    $settings = Get-BonjourServiceSettings
    $relativePath = [string]$settings.$PropertyName
    $segments = $relativePath -split '[\\/]'
    $invalidSegments = @($segments | Where-Object { $_ -notmatch '^[A-Za-z0-9._-]+$' -or $_ -in @('', '.', '..') })
    if ([string]::IsNullOrWhiteSpace($relativePath) -or
        [IO.Path]::IsPathRooted($relativePath) -or
        $invalidSegments.Count -gt 0) {
        throw "Bonjour 執行元件設定中的 $PropertyName 不是安全的相對路徑。"
    }

    Join-Path $BaseDirectory $relativePath
}

function Get-BonjourRuntimeDirectory {
    [CmdletBinding()]
    param()

    $programFiles = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
    Resolve-BonjourSettingsPath 'ProgramFilesRelativeDirectory' $programFiles
}

function Get-BonjourSourceExecutablePaths {
    [CmdletBinding()]
    param()

    $settings = Get-BonjourServiceSettings
    $locations = @(
        [pscustomobject]@{
            PropertyName = 'ProgramFilesRelativeSourceExecutablePaths'
            BaseDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
        }
        [pscustomobject]@{
            PropertyName = 'ProgramFilesX86RelativeSourceExecutablePaths'
            BaseDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)
        }
    )

    foreach ($location in $locations) {
        if ([string]::IsNullOrWhiteSpace($location.BaseDirectory)) { continue }
        $relativePaths = @($settings.($location.PropertyName))
        if ($relativePaths.Count -eq 0) {
            throw "Bonjour 執行元件設定中沒有 $($location.PropertyName)。"
        }

        foreach ($relativePath in $relativePaths) {
            $segments = ([string]$relativePath) -split '[\\/]'
            $invalidSegments = @($segments | Where-Object { $_ -notmatch '^[A-Za-z0-9._ -]+$' -or $_ -in @('', '.', '..') })
            if ([string]::IsNullOrWhiteSpace([string]$relativePath) -or
                [IO.Path]::IsPathRooted([string]$relativePath) -or
                $invalidSegments.Count -gt 0) {
                throw "Bonjour 執行元件設定包含不安全的 $($location.PropertyName) 路徑。"
            }

            Join-Path $location.BaseDirectory ([string]$relativePath)
        }
    }
}

function Get-BonjourServiceBackupPath {
    [CmdletBinding()]
    param()

    Resolve-BonjourSettingsPath 'ProgramDataRelativeBackupPath' $env:ProgramData
}

function Protect-BonjourServiceBackupDirectory {
    [CmdletBinding()]
    param()

    $backupPath = Get-BonjourServiceBackupPath
    $backupDirectory = Split-Path -Parent $backupPath
    New-Item -ItemType Directory -Path $backupDirectory -Force | Out-Null

    $directory = Get-Item -LiteralPath $backupDirectory -Force
    if (($directory.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'Bonjour Service 備份目錄不能是重新解析點。'
    }

    $backupName = Split-Path -Leaf $backupPath
    $allowedFiles = @($backupName, "$backupName.tmp")
    foreach ($item in Get-ChildItem -LiteralPath $backupDirectory -Force) {
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
            $item.PSIsContainer -or
            $item.Name -notin $allowedFiles) {
            throw "Bonjour Service 備份目錄包含非預期項目：$($item.Name)"
        }
    }

    $icaclsPath = Join-Path $env:SystemRoot 'System32\icacls.exe'
    & $icaclsPath $backupDirectory /setowner '*S-1-5-32-544' /T /C | Out-Null
    if ($LASTEXITCODE -ne 0) { throw '無法將 Bonjour 備份目錄擁有者設為系統管理員群組。' }

    & $icaclsPath $backupDirectory /reset /T /C | Out-Null
    if ($LASTEXITCODE -ne 0) { throw '無法重設 Bonjour 備份目錄的 ACL。' }

    & $icaclsPath $backupDirectory /inheritance:r /T /C | Out-Null
    if ($LASTEXITCODE -ne 0) { throw '無法移除 Bonjour 備份目錄的繼承 ACL。' }

    $directoryGrants = @(
        '*S-1-5-32-544:(OI)(CI)(F)'
        '*S-1-5-18:(OI)(CI)(F)'
        '*S-1-5-32-545:(OI)(CI)(RX)'
    )
    foreach ($grant in $directoryGrants) {
        & $icaclsPath $backupDirectory /grant:r $grant /T /C | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "無法限制 Bonjour 備份目錄權限：$grant"
        }
    }
}

Export-ModuleMember -Function Resolve-ServiceExecutablePath, Get-PeMachineType, Test-UsableBonjourExecutablePath, Test-TrustedBonjourExecutablePath, Test-TrustedBonjourDnsSdPath, Test-UsableBonjourRuntimePair, Get-CompatibleBonjourDnsSdPath, Get-BonjourRuntimeDirectory, Get-BonjourSourceExecutablePaths, Get-BonjourSourceDnsSdPaths, Get-BonjourMissingCompatibleX64DnsSdMessage, Get-BonjourServiceBackupPath, Protect-BonjourServiceBackupDirectory
