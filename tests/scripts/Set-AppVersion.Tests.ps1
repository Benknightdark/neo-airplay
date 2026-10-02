$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$versionScript = Join-Path $projectRoot 'scripts\Set-AppVersion.ps1'
$temporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ('NeoAirPlay.VersionTest.' + [guid]::NewGuid().ToString('N'))
$temporaryProject = Join-Path $temporaryDirectory 'NeoAirPlay.App.csproj'
New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null

try {
    $fixture = @'
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <Version>0.1.0</Version>
  </PropertyGroup>
</Project>
'@
    [IO.File]::WriteAllText($temporaryProject, $fixture, [Text.UTF8Encoding]::new($false))

    $nextVersion = (& $versionScript -ProjectPath $temporaryProject).Trim()
    if ($nextVersion -ne '0.1.1') {
        throw "預期 patch 升級結果為 0.1.1，實際為 $nextVersion"
    }

    [xml]$updatedProject = [IO.File]::ReadAllText($temporaryProject)
    if ($updatedProject.Project.PropertyGroup.Version -ne '0.1.1') {
        throw 'patch 升級後的 csproj 版本不正確。'
    }

    $pinnedVersion = (& $versionScript -ProjectPath $temporaryProject -Version '2.3.4').Trim()
    if ($pinnedVersion -ne '2.3.4') {
        throw "預期指定版本結果為 2.3.4，實際為 $pinnedVersion"
    }

    $beforeInvalidVersion = [IO.File]::ReadAllText($temporaryProject)
    try {
        & $versionScript -ProjectPath $temporaryProject -Version '../invalid' | Out-Null
        throw '無效的指定版本未被拒絕。'
    }
    catch {
        if ($_.Exception.Message -eq '無效的指定版本未被拒絕。') {
            throw
        }
    }

    if ([IO.File]::ReadAllText($temporaryProject) -ne $beforeInvalidVersion) {
        throw '拒絕無效版本時不應修改 csproj。'
    }

    Write-Host 'Set-AppVersion smoke tests passed.'
}
finally {
    if (Test-Path -LiteralPath $temporaryDirectory -PathType Container) {
        $resolvedTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
        $resolvedTestDirectory = [IO.Path]::GetFullPath($temporaryDirectory)
        if (-not $resolvedTestDirectory.StartsWith($resolvedTempRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw "拒絕清理暫存目錄外的路徑：$resolvedTestDirectory"
        }

        Remove-Item -LiteralPath $resolvedTestDirectory -Recurse -Force
    }
}
