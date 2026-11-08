[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('dev', 'release', 'bundle')]
    [string]$Mode,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
)

$ErrorActionPreference = 'Stop'
if ($Mode -eq 'bundle' -and $Arguments.Count -gt 0) {
    [Console]::Error.WriteLine('The bundle launcher does not accept additional arguments.')
    exit 2
}
$setupScript = Join-Path $PSScriptRoot 'setup.ps1'
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $setupScript
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$desktopDirectory = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$homeDirectory = $env:USERPROFILE
$nodeVersion = (Get-Content -LiteralPath (Join-Path $desktopDirectory '.node-version') -Raw).Trim()
$managedNode = Join-Path $env:LOCALAPPDATA "NeoAirPlay\node-v$nodeVersion"
$developmentPaths = @(
    $managedNode,
    (Join-Path $homeDirectory '.cargo\bin'),
    (Join-Path $homeDirectory '.bun\bin'),
    [Environment]::GetEnvironmentVariable('Path', 'Machine'),
    [Environment]::GetEnvironmentVariable('Path', 'User'),
    $env:Path
) | Where-Object { $_ }
$env:Path = ($developmentPaths | Select-Object -Unique) -join ';'
$env:NEO_AIRPLAY_SETUP_COMPLETE = '1'

Push-Location $desktopDirectory
try {
    switch ($Mode) {
        'dev' { & bun run tauri dev @Arguments }
        'release' { & bun run release @Arguments }
        'bundle' { & bun run bundle @Arguments }
    }
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
