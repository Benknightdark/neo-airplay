[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RepositoryRoot,
    [Parameter(Mandatory)][string[]]$AppScriptsDirectories
)

$ErrorActionPreference = 'Stop'
$sourceScriptsDirectory = Join-Path (Resolve-Path -LiteralPath $RepositoryRoot).Path 'scripts'
$scriptDirectories = @($sourceScriptsDirectory) + @($AppScriptsDirectories)
$scriptNames = @(
    'BonjourRuntimeHelpers.psm1',
    'Stage-BonjourRuntime.ps1',
    'Install-BonjourService.ps1',
    'Restore-BonjourService.ps1'
)

foreach ($directory in $scriptDirectories) {
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
        throw "Bonjour script directory was not found: $directory"
    }

    foreach ($scriptName in $scriptNames) {
        $scriptPath = Join-Path $directory $scriptName
        if (-not (Test-Path -LiteralPath $scriptPath -PathType Leaf)) {
            throw "Bonjour script was not found: $scriptPath"
        }

        $tokens = $null
        $parseErrors = $null
        [System.Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$parseErrors) > $null
        if ($parseErrors.Count -gt 0) {
            $details = ($parseErrors | ForEach-Object { "Line $($_.Extent.StartLineNumber): $($_.Message)" }) -join [Environment]::NewLine
            throw "Windows PowerShell 5.1 could not parse '$scriptPath':$([Environment]::NewLine)$details"
        }
    }

    $settingsPath = Join-Path $directory 'BonjourServiceSettings.json'
    if (-not (Test-Path -LiteralPath $settingsPath -PathType Leaf)) {
        throw "Bonjour settings file was not found: $settingsPath"
    }

    $expectedMessage = [string](Get-Content -LiteralPath $settingsPath -Raw -Encoding UTF8 | ConvertFrom-Json).MissingCompatibleX64DnsSdMessage
    $helperPath = Join-Path $directory 'BonjourRuntimeHelpers.psm1'
    Remove-Module -Name BonjourRuntimeHelpers -ErrorAction SilentlyContinue
    Import-Module -Name $helperPath -Force -ErrorAction Stop
    $actualMessage = [string](Get-BonjourMissingCompatibleX64DnsSdMessage)
    if ($actualMessage -cne $expectedMessage) {
        throw "Bonjour settings message was not decoded as UTF-8 from '$settingsPath'."
    }
    Remove-Module -Name BonjourRuntimeHelpers -ErrorAction SilentlyContinue
}

Write-Host 'Windows PowerShell 5.1 Bonjour compatibility checks passed.'
