[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$solutionPath = Join-Path $projectRoot 'NeoAirPlay.slnx'

& dotnet build $solutionPath
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$windowsPowerShell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
if (-not (Test-Path -LiteralPath $windowsPowerShell -PathType Leaf)) {
    throw "Windows PowerShell 5.1 was not found at '$windowsPowerShell'."
}

$bonjourCompatibilityTests = Join-Path $projectRoot 'tests\scripts\BonjourPowerShellCompatibility.Tests.ps1'
$appScriptsDirectory = Join-Path $projectRoot 'src\NeoAirPlay.App\bin\Debug\net10.0-windows\win-x64\scripts'
& $windowsPowerShell -NoProfile -NonInteractive -File $bonjourCompatibilityTests -RepositoryRoot $projectRoot -AppScriptsDirectories $appScriptsDirectory
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& dotnet test $solutionPath --no-build --filter "Category=Unit"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$versionScriptTests = Join-Path $projectRoot 'tests\scripts\Set-AppVersion.Tests.ps1'
& $versionScriptTests
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

exit 0
