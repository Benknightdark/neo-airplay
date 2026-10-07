$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$versionScript = Join-Path $projectRoot 'scripts/Set-AppVersion.ps1'
$temporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ('NeoAirPlay.VersionTest.' + [guid]::NewGuid().ToString('N'))
$temporarySource = Join-Path $temporaryDirectory 'src-tauri'
New-Item -ItemType Directory -Path $temporarySource -Force | Out-Null

try {
    $packageFixture = @'
{
  "name": "neo-airplay",
  "version": "0.1.0",
  "dependencies": {}
}
'@
    $cargoFixture = @'
[workspace]
members = ["src-tauri"]

[workspace.package]
version = "0.1.0"
edition = "2024"
'@
    $engineFixture = @'
{
  "SchemaVersion": 2,
  "Version": "0.1.0",
  "ReleaseTag": "v0.1.0",
  "ManifestName": "engine-package-manifest.json",
  "Targets": {
    "windows-x86_64": {
      "AssetName": "neo-airplay-engine-windows-x86_64.zip",
      "Sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "AirPlayFiles": [],
      "AndroidFiles": ["engine/android/scrcpy.exe"],
      "SharedFiles": []
    }
  }
}
'@
    [IO.File]::WriteAllText((Join-Path $temporaryDirectory 'package.json'), $packageFixture, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $temporaryDirectory 'Cargo.toml'), $cargoFixture, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $temporarySource 'engine-package.json'), $engineFixture, [Text.UTF8Encoding]::new($false))

    $nextVersion = (& $versionScript -ProjectRoot $temporaryDirectory -SkipLockUpdate).Trim()
    if ($nextVersion -ne '0.1.1') {
        throw "Expected patch version 0.1.1, received $nextVersion"
    }
    $package = Get-Content -LiteralPath (Join-Path $temporaryDirectory 'package.json') -Raw | ConvertFrom-Json
    $cargo = Get-Content -LiteralPath (Join-Path $temporaryDirectory 'Cargo.toml') -Raw
    $engine = Get-Content -LiteralPath (Join-Path $temporarySource 'engine-package.json') -Raw | ConvertFrom-Json
    if ($package.version -ne '0.1.1' -or $cargo -notmatch '(?m)^version = "0\.1\.1"$') {
        throw 'package.json and Cargo.toml were not updated together.'
    }
    if ($engine.Version -ne '0.1.1' -or $engine.ReleaseTag -ne 'v0.1.1' -or @($engine.Targets.PSObject.Properties).Count -ne 0) {
        throw 'Engine metadata was not reset for the new release version.'
    }

    $pinnedVersion = (& $versionScript -ProjectRoot $temporaryDirectory -Version '2.3.4' -SkipLockUpdate).Trim()
    if ($pinnedVersion -ne '2.3.4') {
        throw "Expected requested version 2.3.4, received $pinnedVersion"
    }

    $beforeInvalidVersion = @(
        [IO.File]::ReadAllText((Join-Path $temporaryDirectory 'package.json'))
        [IO.File]::ReadAllText((Join-Path $temporaryDirectory 'Cargo.toml'))
        [IO.File]::ReadAllText((Join-Path $temporarySource 'engine-package.json'))
    )
    try {
        & $versionScript -ProjectRoot $temporaryDirectory -Version '../invalid' -SkipLockUpdate | Out-Null
        throw 'An invalid version was not rejected.'
    }
    catch {
        if ($_.Exception.Message -eq 'An invalid version was not rejected.') {
            throw
        }
    }
    $afterInvalidVersion = @(
        [IO.File]::ReadAllText((Join-Path $temporaryDirectory 'package.json'))
        [IO.File]::ReadAllText((Join-Path $temporaryDirectory 'Cargo.toml'))
        [IO.File]::ReadAllText((Join-Path $temporarySource 'engine-package.json'))
    )
    if (($beforeInvalidVersion -join [Environment]::NewLine) -cne ($afterInvalidVersion -join [Environment]::NewLine)) {
        throw 'Rejecting an invalid version must not modify version files.'
    }

    Write-Output 'Set-AppVersion smoke tests passed.'
}
finally {
    if (Test-Path -LiteralPath $temporaryDirectory -PathType Container) {
        $resolvedTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        $resolvedTestDirectory = [IO.Path]::GetFullPath($temporaryDirectory)
        if (-not $resolvedTestDirectory.StartsWith($resolvedTempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to clean a temporary directory outside the system temp root: $resolvedTestDirectory"
        }
        Remove-Item -LiteralPath $resolvedTestDirectory -Recurse -Force
    }
}
