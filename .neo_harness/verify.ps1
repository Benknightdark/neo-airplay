[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$desktopRoot = Join-Path $projectRoot 'desktop'

if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
    throw 'Bun is required. Install the version documented in README.md.'
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust and Cargo are required. Install the stable toolchain documented in README.md.'
}

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    throw 'Node.js 22 is required for frontend unit tests. Install the version documented in README.md.'
}

$nodeVersion = (& node --version).Trim()
if ($LASTEXITCODE -ne 0 -or $nodeVersion -notmatch '^v22\.' -or [version]$nodeVersion.TrimStart('v') -lt [version]'22.22.2') {
    throw "Node.js 22.22.2 or newer within the 22.x line is required for frontend unit tests. Found '$nodeVersion'."
}

Push-Location $desktopRoot
try {
    & bun run --silent verify
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}

exit 0
