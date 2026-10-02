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

function Test-UsableBonjourExecutablePath {
    [CmdletBinding()]
    param([AllowNull()][string]$ImagePath)

    $executable = Resolve-ServiceExecutablePath $ImagePath
    if ([string]::IsNullOrWhiteSpace($executable) -or -not (Test-Path -LiteralPath $executable -PathType Leaf)) {
        return $false
    }

    $directory = Split-Path -Parent $executable
    $dnsSdPath = Join-Path $directory 'dnssd.dll'
    if (-not (Test-Path -LiteralPath $dnsSdPath -PathType Leaf)) {
        return $false
    }

    try {
        $responder = Get-Item -LiteralPath $executable
        $dnsSd = Get-Item -LiteralPath $dnsSdPath
        return $responder.VersionInfo.ProductName -eq 'Bonjour' -and
            $dnsSd.VersionInfo.ProductName -eq 'Bonjour' -and
            $responder.VersionInfo.FileVersion -eq $dnsSd.VersionInfo.FileVersion
    }
    catch {
        return $false
    }
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

    Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
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

function Get-BonjourServiceBackupPath {
    [CmdletBinding()]
    param()

    Resolve-BonjourSettingsPath 'ProgramDataRelativeBackupPath' $env:ProgramData
}

Export-ModuleMember -Function Resolve-ServiceExecutablePath, Test-UsableBonjourExecutablePath, Test-TrustedBonjourExecutablePath, Get-BonjourRuntimeDirectory, Get-BonjourServiceBackupPath
