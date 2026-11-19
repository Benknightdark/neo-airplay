[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$systemRoot = if ($env:SystemRoot -and ($env:SystemRoot -match '^[a-zA-Z]:[\\/]')) { $env:SystemRoot } else { 'C:\Windows' }
$powershellExecutable = Join-Path $systemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
& $powershellExecutable -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'install.ps1') -Development
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
$nodeVersion = (Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\.node-version') -Raw).Trim()
$managedNode = Join-Path $env:LOCALAPPDATA "NeoAirPlay/node-v$nodeVersion"
$env:Path = "$managedNode;$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.bun\bin;$env:Path"
Push-Location (Join-Path $PSScriptRoot '..')
try {
    & bun install --frozen-lockfile --silent
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally { Pop-Location }
Write-Host 'Development environment is ready.'
