[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'desktop/src-tauri/resources/setup/install.ps1') -Development
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
$managedNode = Join-Path $env:LOCALAPPDATA 'NeoAirPlay/node-v22.23.3'
$env:Path = "$managedNode;$env:USERPROFILE\.cargo\bin;$env:USERPROFILE\.bun\bin;$env:Path"
Push-Location (Join-Path $PSScriptRoot 'desktop')
try {
    & bun install --frozen-lockfile --silent
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
finally { Pop-Location }
Write-Host 'Development environment is ready. Open a new terminal, then run: cd desktop; bun run tauri dev'
