#Requires -Version 7.4
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$profileDirectory = Join-Path $temporaryRoot ("neo-airplay-e2e-" + [guid]::NewGuid().ToString('N'))
$binaryName = if ($IsWindows) { 'neo-airplay.exe' } else { 'neo-airplay' }
$appBinary = Join-Path $projectRoot "target/debug/$binaryName"
$env:VITE_TAURI_E2E = 'true'
$env:NEO_E2E_PROFILE = $profileDirectory

try {
    Push-Location $projectRoot
    try {
        & bun tauri build --debug --no-bundle --features e2e --config src-tauri/tauri.e2e.conf.json
        if ($LASTEXITCODE -ne 0) {
            throw "Tauri E2E application build failed with exit code $LASTEXITCODE."
        }

        if (-not (Test-Path -LiteralPath $appBinary -PathType Leaf)) {
            throw "Tauri E2E application binary was not produced: $appBinary"
        }

        & bun run e2e
        if ($LASTEXITCODE -ne 0) {
            throw "WebdriverIO E2E run failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    Remove-Item Env:VITE_TAURI_E2E -ErrorAction SilentlyContinue
    Remove-Item Env:NEO_E2E_PROFILE -ErrorAction SilentlyContinue
    $resolvedProfile = [IO.Path]::GetFullPath($profileDirectory)
    $temporaryPrefix = $temporaryRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if ($resolvedProfile.StartsWith($temporaryPrefix, [StringComparison]::Ordinal) -and
        (Test-Path -LiteralPath $resolvedProfile -PathType Container)) {
        Remove-Item -LiteralPath $resolvedProfile -Recurse -Force
    }
}
