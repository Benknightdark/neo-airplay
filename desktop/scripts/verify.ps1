[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$desktopRoot = Split-Path -Parent $PSScriptRoot

if ($env:OS -eq 'Windows_NT') {
    $nodeVersionFile = Join-Path $desktopRoot '.node-version'
    $pinnedNode = if (Test-Path -LiteralPath $nodeVersionFile -PathType Leaf) { (Get-Content -LiteralPath $nodeVersionFile -Raw).Trim() } else { '' }
    $priorityPaths = @()
    if ($pinnedNode -and $env:LOCALAPPDATA) {
        $managedNode = Join-Path $env:LOCALAPPDATA "NeoAirPlay\node-v$pinnedNode"
        if (Test-Path -LiteralPath $managedNode -PathType Container) { $priorityPaths += $managedNode }
    }
    if ($env:USERPROFILE) {
        foreach ($subPath in @('.cargo\bin', '.bun\bin')) {
            $candidate = Join-Path $env:USERPROFILE $subPath
            if (Test-Path -LiteralPath $candidate -PathType Container) { $priorityPaths += $candidate }
        }
    }
    if ($priorityPaths.Count -gt 0) {
        $env:Path = (($priorityPaths + @($env:Path -split ';')) | Where-Object { $_ } | Select-Object -Unique) -join ';'
    }
}

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
