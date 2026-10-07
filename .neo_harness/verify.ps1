#Requires -Version 7.4
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    function Invoke-VerificationStep {
        param(
            [Parameter(Mandatory)][string]$Name,
            [Parameter(Mandatory)][scriptblock]$Command
        )

        Write-Host "==> $Name"
        $global:LASTEXITCODE = 0
        & $Command
        if ($LASTEXITCODE -ne 0) {
            throw "Verification step '$Name' failed with exit code $LASTEXITCODE."
        }
    }

    Invoke-VerificationStep 'bun install' { bun install --frozen-lockfile --silent }
    Invoke-VerificationStep 'frontend typecheck' { bun run typecheck }
    Invoke-VerificationStep 'frontend unit tests' { bun run test --reporter=dot }
    Invoke-VerificationStep 'frontend build' { bun run build --logLevel error }
    Invoke-VerificationStep 'cargo format' { cargo fmt --all --check --quiet }
    Invoke-VerificationStep 'cargo clippy' { cargo clippy --workspace --all-targets --locked --quiet -- -D warnings }
    Invoke-VerificationStep 'cargo tests' { cargo test --workspace --locked --quiet }

    if ($IsWindows) {
        $windowsPowerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        if (-not (Test-Path -LiteralPath $windowsPowerShell -PathType Leaf)) {
            throw "Windows PowerShell 5.1 was not found at '$windowsPowerShell'."
        }

        $bonjourCompatibilityTests = Join-Path $projectRoot 'tests\scripts\BonjourPowerShellCompatibility.Tests.ps1'
        $scriptsDirectory = Join-Path $projectRoot 'scripts'
        Invoke-VerificationStep 'Bonjour PowerShell 5.1 compatibility' {
            & $windowsPowerShell -NoProfile -NonInteractive -File $bonjourCompatibilityTests `
                -RepositoryRoot $projectRoot `
                -AppScriptsDirectories $scriptsDirectory
        }

        Invoke-VerificationStep '.NET solution build' { dotnet build (Join-Path $projectRoot 'NeoAirPlay.slnx') -v q --nologo }
        Invoke-VerificationStep '.NET unit tests' {
            dotnet test (Join-Path $projectRoot 'NeoAirPlay.slnx') --no-build -v q --nologo --filter 'Category=Unit'
        }
    }

    Invoke-VerificationStep 'Set-AppVersion smoke tests' {
        & (Join-Path $projectRoot 'tests/scripts/Set-AppVersion.Tests.ps1')
    }
}
finally {
    Pop-Location
}

Write-Host 'Verification passed.'
