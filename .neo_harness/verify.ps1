[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$solutionPath = Join-Path $projectRoot 'NeoAirPlay.slnx'

& dotnet build $solutionPath
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& dotnet test $solutionPath --no-build --filter "Category=Unit"
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

exit 0
