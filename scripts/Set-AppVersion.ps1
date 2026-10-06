[CmdletBinding()]
param(
    [string]$Version,
    [string]$ProjectPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'src\NeoAirPlay.App\NeoAirPlay.App.csproj')
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $ProjectPath -PathType Leaf)) {
    throw "找不到 App 專案檔：$ProjectPath"
}

$projectContent = [IO.File]::ReadAllText($ProjectPath)
[xml]$projectDocument = $projectContent
$versionNodes = $projectDocument.SelectNodes('/Project/PropertyGroup/Version')
if ($versionNodes.Count -ne 1) {
    throw "App 專案必須且只能有一個 PropertyGroup/Version：$ProjectPath"
}

$currentVersion = $versionNodes[0].InnerText.Trim()
if ($currentVersion -notmatch '^(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)$') {
    throw "目前 App 版本不是穩定版三段 SemVer：$currentVersion"
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $patch = [long]::Parse($Matches.patch, [Globalization.CultureInfo]::InvariantCulture)
    if ($patch -eq [long]::MaxValue) {
        throw "App patch 版本已達上限：$currentVersion"
    }

    $Version = '{0}.{1}.{2}' -f $Matches.major, $Matches.minor, ($patch + 1)
}
elseif ($Version -notmatch '^((0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*))$') {
    throw "指定版本不是穩定版三段 SemVer：$Version"
}

$versionPattern = '(?s)(<Version>\s*)' + [regex]::Escape($currentVersion) + '(\s*</Version>)'
$versionMatch = [regex]::Match($projectContent, $versionPattern)
if (-not $versionMatch.Success) {
    throw "無法在 App 專案檔中唯一定位版本節點：$ProjectPath"
}

$replacement = [Text.RegularExpressions.MatchEvaluator]{
    param($match)
    $match.Groups[1].Value + $Version + $match.Groups[2].Value
}
$updatedContent = [regex]::Replace($projectContent, $versionPattern, $replacement, 1)
[IO.File]::WriteAllText($ProjectPath, $updatedContent, [Text.UTF8Encoding]::new($false))

Write-Output $Version
