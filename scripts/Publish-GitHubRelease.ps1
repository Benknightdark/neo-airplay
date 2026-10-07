#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$SourceSha,
    [Parameter(Mandatory)][string]$AssetsDirectory,
    [string]$Repository = 'Benknightdark/neo-airplay'
)

$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "Invalid stable application version: $Version"
}
if ($SourceSha -notmatch '^[0-9a-fA-F]{40}$') {
    throw "Invalid release source commit SHA: $SourceSha"
}
if ([string]::IsNullOrWhiteSpace($env:GH_TOKEN)) {
    throw 'GH_TOKEN is required to publish release assets and update version metadata.'
}
foreach ($command in @('git', 'gh')) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "The release runner is missing required command '$command'."
    }
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$versionMetadataDirectory = Join-Path $AssetsDirectory 'version-metadata'
$metadataFiles = @('package.json', 'Cargo.toml', 'Cargo.lock')
foreach ($name in $metadataFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $versionMetadataDirectory $name) -PathType Leaf)) {
        throw "Release version metadata is missing $name."
    }
}
$versionMetadata = Get-Content -LiteralPath (Join-Path $versionMetadataDirectory 'package.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($versionMetadata.version -ne $Version) {
    throw "Version metadata contains $($versionMetadata.version), expected $Version."
}

$targetSpecifications = @(
    @{ Key = 'windows-x86_64'; Directory = 'windows' },
    @{ Key = 'macos-aarch64'; Directory = 'macos-aarch64' },
    @{ Key = 'macos-x86_64'; Directory = 'macos-x86_64' },
    @{ Key = 'linux-x86_64'; Directory = 'linux-x86_64' }
)
$mergedTargets = [ordered]@{}
$enginePackagePaths = [Collections.Generic.List[string]]::new()
foreach ($specification in $targetSpecifications) {
    $sourceDirectory = Join-Path $AssetsDirectory $specification.Directory
    $settingsPath = Join-Path $sourceDirectory 'engine-package.json'
    if (-not (Test-Path -LiteralPath $settingsPath -PathType Leaf)) {
        throw "Missing engine package metadata for $($specification.Key)."
    }
    $settings = Get-Content -LiteralPath $settingsPath -Raw -Encoding UTF8 | ConvertFrom-Json -AsHashtable
    if ($settings.SchemaVersion -ne 2 -or
        $settings.Version -ne $Version -or
        $settings.ReleaseTag -ne "v$Version" -or
        $settings.ManifestName -ne 'engine-package-manifest.json') {
        throw "Engine package metadata for $($specification.Key) has an invalid schema or version."
    }
    $target = $settings.Targets[$specification.Key]
    if (-not $target -or [IO.Path]::GetFileName([string]$target.AssetName) -ne $target.AssetName) {
        throw "Engine package metadata has no safe asset entry for $($specification.Key)."
    }
    $archivePath = Join-Path $sourceDirectory $target.AssetName
    if (-not (Test-Path -LiteralPath $archivePath -PathType Leaf)) {
        throw "Engine package archive is missing for $($specification.Key): $archivePath"
    }
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not [string]::Equals($actualHash, [string]$target.Sha256, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Engine package SHA-256 does not match metadata for $($specification.Key)."
    }
    $mergedTargets[$specification.Key] = $target
    $enginePackagePaths.Add($archivePath)
}

$mergedSettings = [ordered]@{
    SchemaVersion = 2
    Version = $Version
    ReleaseTag = "v$Version"
    ManifestName = 'engine-package-manifest.json'
    Targets = $mergedTargets
}

$flattenedDirectory = Join-Path $AssetsDirectory "publish-$Version"
if (Test-Path -LiteralPath $flattenedDirectory -PathType Container) {
    $assetsRoot = [IO.Path]::GetFullPath($AssetsDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    $resolvedFlattened = [IO.Path]::GetFullPath($flattenedDirectory)
    if (-not $resolvedFlattened.StartsWith($assetsRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove a publish directory outside the release asset root: $resolvedFlattened"
    }
    Remove-Item -LiteralPath $resolvedFlattened -Recurse -Force
}
New-Item -ItemType Directory -Path $flattenedDirectory -Force | Out-Null
$releaseAssets = [Collections.Generic.List[string]]::new()
function Add-ReleaseAsset {
    param([Parameter(Mandatory)][string]$SourcePath, [Parameter(Mandatory)][string]$AssetName)
    if (-not (Test-Path -LiteralPath $SourcePath -PathType Leaf)) {
        throw "Required release asset is missing: $SourcePath"
    }
    if ([IO.Path]::GetFileName($AssetName) -ne $AssetName) {
        throw "Invalid release asset name: $AssetName"
    }
    $destination = Join-Path $flattenedDirectory $AssetName
    Copy-Item -LiteralPath $SourcePath -Destination $destination -Force
    $releaseAssets.Add($destination)
}

$applicationAssets = @(
    @{ Directory = 'windows'; Name = "NeoAirPlay-$Version-windows-x86_64-setup.exe" },
    @{ Directory = 'windows'; Name = "NeoAirPlay-$Version-windows-x86_64.zip" },
    @{ Directory = 'macos-aarch64'; Name = "NeoAirPlay-$Version-macos-aarch64.dmg" },
    @{ Directory = 'macos-x86_64'; Name = "NeoAirPlay-$Version-macos-x86_64.dmg" },
    @{ Directory = 'linux-x86_64'; Name = "NeoAirPlay-$Version-linux-x86_64.appimage" },
    @{ Directory = 'linux-x86_64'; Name = "NeoAirPlay-$Version-linux-x86_64.deb" }
)
foreach ($asset in $applicationAssets) {
    Add-ReleaseAsset -SourcePath (Join-Path (Join-Path $AssetsDirectory $asset.Directory) $asset.Name) -AssetName $asset.Name
}
foreach ($index in 0..($targetSpecifications.Count - 1)) {
    $specification = $targetSpecifications[$index]
    $archiveName = [IO.Path]::GetFileName($enginePackagePaths[$index])
    Add-ReleaseAsset -SourcePath $enginePackagePaths[$index] -AssetName $archiveName
}
foreach ($helper in @('install-uxplay-macos.sh', 'install-uxplay-linux.sh', 'uxplay-release.json')) {
    Add-ReleaseAsset -SourcePath (Join-Path (Join-Path $AssetsDirectory 'windows') $helper) -AssetName $helper
}
$mergedSettingsPath = Join-Path $flattenedDirectory 'engine-package.json'
[IO.File]::WriteAllText(
    $mergedSettingsPath,
    ($mergedSettings | ConvertTo-Json -Depth 12),
    [Text.UTF8Encoding]::new($false))
$releaseAssets.Add($mergedSettingsPath)

$applicationReleaseAssets = $releaseAssets.ToArray()
foreach ($assetPath in $applicationReleaseAssets) {
    if ([IO.Path]::GetExtension($assetPath).ToLowerInvariant() -in @('.exe', '.zip', '.dmg', '.appimage', '.deb', '.sh', '.json')) {
        $name = [IO.Path]::GetFileName($assetPath)
        $hash = (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText("$assetPath.sha256", "$hash  $name$([Environment]::NewLine)", [Text.ASCIIEncoding]::new())
        $releaseAssets.Add("$assetPath.sha256")
    }
}
$checksums = @(
    $releaseAssets |
        Where-Object { $_ -notlike '*.sha256' } |
        Sort-Object { [IO.Path]::GetFileName($_) } |
        ForEach-Object {
            $name = [IO.Path]::GetFileName($_)
            $hash = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
            "$hash  $name"
        }
)
$checksumPath = Join-Path $flattenedDirectory 'SHA256SUMS.txt'
[IO.File]::WriteAllLines($checksumPath, $checksums, [Text.ASCIIEncoding]::new())
$releaseAssets.Add($checksumPath)

$projectRelativePaths = @('package.json', 'Cargo.toml', 'Cargo.lock', 'src-tauri/engine-package.json')

Push-Location $projectRoot
try {
    & git fetch origin main --tags
    if ($LASTEXITCODE -ne 0) { throw 'Could not fetch main and release tags.' }
    $existingReleaseCommit = (& git log origin/main --format=%H --grep="Release-For: $SourceSha" -1).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not look up existing release metadata commits.' }
    $mainPackageJson = & git show 'origin/main:package.json' | Out-String
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the current main application version.' }
    $mainVersion = [string]($mainPackageJson | ConvertFrom-Json).version
    if ([version]$mainVersion -gt [version]$Version) {
        throw "main has advanced to $mainVersion; refusing to write older release metadata $Version."
    }
    if ($existingReleaseCommit) {
        $existingPackageJson = & git show "${existingReleaseCommit}:package.json" | Out-String
        if ($LASTEXITCODE -ne 0) { throw 'Could not read the existing release metadata version.' }
        $existingVersion = [string]($existingPackageJson | ConvertFrom-Json).version
        if ($existingVersion -ne $Version) {
            throw "Source commit already reserves version $existingVersion, not $Version."
        }
    }

    & git switch --detach origin/main
    if ($LASTEXITCODE -ne 0) { throw 'Could not check out the latest main commit.' }
    Copy-Item -LiteralPath $mergedSettingsPath -Destination (Join-Path $projectRoot 'src-tauri/engine-package.json') -Force
    foreach ($name in $metadataFiles) {
        Copy-Item -LiteralPath (Join-Path $versionMetadataDirectory $name) -Destination (Join-Path $projectRoot $name) -Force
    }
    foreach ($relativePath in $projectRelativePaths) {
        $sourcePath = Join-Path $projectRoot $relativePath.Replace('/', [IO.Path]::DirectorySeparatorChar)
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "Release metadata source file is missing: $sourcePath"
        }
    }
    & git add -- @projectRelativePaths
    if ($LASTEXITCODE -ne 0) { throw 'Could not stage release version metadata.' }
    & git diff --cached --quiet -- @projectRelativePaths
    $metadataChanged = $LASTEXITCODE -eq 1
    if ($LASTEXITCODE -notin @(0, 1)) { throw 'Could not inspect staged release metadata.' }

    if (-not $existingReleaseCommit -or $metadataChanged) {
        & git config user.name 'github-actions[bot]'
        & git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
        $commitSubject = if ($existingReleaseCommit) { "chore(release): refresh v$Version [skip ci]" } else { "chore(release): v$Version [skip ci]" }
        & git commit --allow-empty -m $commitSubject -m "Release-For: $SourceSha"
        if ($LASTEXITCODE -ne 0) { throw 'Could not commit release version metadata.' }
        & git push origin 'HEAD:main'
        if ($LASTEXITCODE -ne 0) { throw 'Could not push release metadata to main.' }
        $metadataCommit = (& git rev-parse HEAD).Trim()
    }
    else {
        $metadataCommit = $existingReleaseCommit
    }

    $tag = "v$Version"
    $remoteTag = & git ls-remote --refs origin "refs/tags/$tag"
    if ($LASTEXITCODE -ne 0) { throw "Could not query remote tag $tag." }
    $tagExists = -not [string]::IsNullOrWhiteSpace(($remoteTag -join ''))
    $tagCanBeReused = $false
    if ($tagExists) {
        & git fetch origin "refs/tags/${tag}:refs/tags/${tag}"
        if ($LASTEXITCODE -ne 0) { throw "Could not fetch remote tag $tag for source verification." }
    }

    $releaseView = & gh release view $tag --repo $Repository --json isDraft 2>$null
    $releaseViewExit = $LASTEXITCODE
    if ($releaseViewExit -eq 0) {
        $existingRelease = $releaseView | ConvertFrom-Json
        $tagCommit = (& git rev-parse "refs/tags/$tag^{commit}").Trim()
        if ($LASTEXITCODE -ne 0 -or -not $tagCommit) { throw "Could not resolve release tag $tag." }
        $tagMessage = & git show -s --format=%B $tagCommit | Out-String
        if ($LASTEXITCODE -ne 0) { throw "Could not inspect release tag commit $tag." }
        if ($tagMessage -notmatch '(?m)^Release-For: (?<sha>[0-9a-fA-F]{40})\s*$' -or $Matches.sha -ine $SourceSha) {
            throw "Release tag $tag is not reserved for source commit $SourceSha."
        }
        if (-not $existingRelease.isDraft) {
            Write-Host "GitHub Release $tag is already published for source commit $SourceSha."
            return
        }
        & gh release delete $tag --repo $Repository --cleanup-tag --yes
        if ($LASTEXITCODE -ne 0) { throw "Could not remove the draft Release $tag." }
        & git tag --delete $tag
        if ($LASTEXITCODE -ne 0) { throw "Could not remove local draft tag $tag." }
        $tagExists = $false
    }
    elseif ($releaseViewExit -ne 1) {
        throw "Could not inspect GitHub Release $tag (gh exit code $releaseViewExit)."
    }
    elseif ($tagExists) {
        $tagCommit = (& git rev-parse "refs/tags/$tag^{commit}").Trim()
        if ($LASTEXITCODE -ne 0 -or -not $tagCommit) { throw "Could not resolve orphaned release tag $tag." }
        $tagMessage = & git show -s --format=%B $tagCommit | Out-String
        if ($LASTEXITCODE -ne 0) { throw "Could not inspect orphaned release tag $tag." }
        if ($tagMessage -notmatch '(?m)^Release-For: (?<sha>[0-9a-fA-F]{40})\s*$' -or
            $Matches.sha -ine $SourceSha -or
            $tagCommit -ine $metadataCommit) {
            throw "Remote tag $tag exists without a GitHub Release and does not point to this exact source metadata commit."
        }
        $tagCanBeReused = $true
    }

    $localTag = @(git tag --list $tag)
    if ($LASTEXITCODE -ne 0) { throw "Could not inspect local tag $tag." }
    if (($localTag -contains $tag) -and -not $tagCanBeReused) {
        $localTagCommit = (& git rev-parse "refs/tags/$tag^{commit}").Trim()
        if ($LASTEXITCODE -ne 0) { throw "Could not inspect local tag commit $tag." }
        $localTagMessage = & git show -s --format=%B $localTagCommit | Out-String
        if ($LASTEXITCODE -ne 0 -or $localTagMessage -notmatch '(?m)^Release-For: (?<sha>[0-9a-fA-F]{40})\s*$' -or $Matches.sha -ine $SourceSha) {
            throw "Local tag $tag belongs to another source commit; refusing to replace it."
        }
        & git tag --delete $tag
        if ($LASTEXITCODE -ne 0) { throw "Could not remove stale local tag $tag." }
    }
    if (-not $tagCanBeReused) {
        & git tag -a $tag $metadataCommit -m "Neo AirPlay $tag"
        if ($LASTEXITCODE -ne 0) { throw "Could not create release tag $tag." }
        & git push origin "refs/tags/$tag"
        if ($LASTEXITCODE -ne 0) { throw "Could not push release tag $tag." }
    }

    $developRefs = & git ls-remote --heads origin refs/heads/develop
    if ($LASTEXITCODE -eq 0 -and $developRefs) {
        & git fetch origin develop
        if ($LASTEXITCODE -ne 0) { throw 'Could not fetch develop for metadata synchronization.' }
        & git switch --detach origin/develop
        if ($LASTEXITCODE -ne 0) { throw 'Could not check out develop for metadata synchronization.' }
        & git restore "--source=$metadataCommit" --staged --worktree -- @projectRelativePaths
        if ($LASTEXITCODE -ne 0) { throw 'Could not synchronize release metadata to develop.' }
        & git diff --cached --quiet -- @projectRelativePaths
        if ($LASTEXITCODE -eq 1) {
            & git config user.name 'github-actions[bot]'
            & git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
            & git commit -m "chore(release): sync v$Version [skip ci]"
            if ($LASTEXITCODE -ne 0) { throw 'Could not commit release metadata synchronization to develop.' }
            & git push origin 'HEAD:develop'
            if ($LASTEXITCODE -ne 0) { throw 'Could not push release metadata synchronization to develop.' }
        }
        elseif ($LASTEXITCODE -ne 0) {
            throw 'Could not inspect develop release metadata changes.'
        }
    }
    elseif ($LASTEXITCODE -ne 0) {
        throw 'Could not query the remote develop branch.'
    }

    $releaseArguments = @('release', 'create', $tag) + @($releaseAssets) + @(
        '--repo', $Repository,
        '--title', "Neo AirPlay $tag",
        '--generate-notes',
        '--draft',
        '--target', $metadataCommit
    )
    & gh @releaseArguments
    if ($LASTEXITCODE -ne 0) { throw "Could not create draft GitHub Release $tag." }
    & gh release edit $tag --repo $Repository --draft=false
    if ($LASTEXITCODE -ne 0) { throw "Could not publish GitHub Release $tag." }
}
finally {
    Pop-Location
}

Write-Host "Published Neo AirPlay v$Version."
