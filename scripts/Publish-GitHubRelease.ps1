[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Version,

    [Parameter(Mandatory)]
    [string]$SourceSha,

    [Parameter(Mandatory)]
    [string]$AssetsDirectory,

    [string]$Repository = 'Benknightdark/neo-airplay'
)

$ErrorActionPreference = 'Stop'

if ($Version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw "無效的穩定版 App 版本：$Version"
}
if ($SourceSha -notmatch '^[0-9a-fA-F]{40}$') {
    throw "無效的 Release 來源 commit SHA：$SourceSha"
}
if ([string]::IsNullOrWhiteSpace($env:GH_TOKEN)) {
    throw '缺少 GH_TOKEN；請確認 GitHub App installation token 已建立。'
}
if (-not (Get-Command git -ErrorAction SilentlyContinue) -or -not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'Release runner 需要安裝 git 與 GitHub CLI (gh)。'
}

$projectRoot = Split-Path -Parent $PSScriptRoot
$projectRelativePath = 'src/NeoAirPlay.App/NeoAirPlay.App.csproj'
$settingsRelativePath = 'src/NeoAirPlay.App/engine-package.json'
$projectPath = Join-Path $projectRoot $projectRelativePath.Replace('/', '\')
$settingsPath = Join-Path $projectRoot $settingsRelativePath.Replace('/', '\')
$metadataSettingsPath = Join-Path $AssetsDirectory 'engine-package.json'
$tag = "v$Version"
$appArchive = Join-Path $AssetsDirectory "NeoAirPlay-$Version-win-x64.zip"
$appSidecar = "$appArchive.sha256"
$engineArchive = Join-Path $AssetsDirectory 'neo-airplay-engine-win-x64.zip'
$engineSidecar = "$engineArchive.sha256"
$checksumFile = Join-Path $AssetsDirectory 'SHA256SUMS.txt'

foreach ($requiredPath in @($appArchive, $appSidecar, $engineArchive, $engineSidecar, $checksumFile, $metadataSettingsPath)) {
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "Release 套件缺少必要檔案：$requiredPath"
    }
}

$settings = Get-Content -LiteralPath $metadataSettingsPath -Raw | ConvertFrom-Json
$engineHash = (Get-FileHash -LiteralPath $engineArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($settings.Version -ne $Version -or
    $settings.ReleaseTag -ne $tag -or
    $settings.AssetName -ne 'neo-airplay-engine-win-x64.zip' -or
    $settings.Sha256 -ne $engineHash) {
    throw 'Release metadata 中的版本、tag 或引擎 SHA-256 與建置資產不一致。'
}

& git fetch origin main --tags
if ($LASTEXITCODE -ne 0) {
    throw '無法更新 main 分支與 Release tags。'
}

$remoteTag = & git ls-remote --refs origin "refs/tags/$tag"
if ($LASTEXITCODE -ne 0) {
    throw "無法查詢遠端 tag：$tag"
}
$tagExists = -not [string]::IsNullOrWhiteSpace(($remoteTag -join ''))
$localTag = @(git tag --list $tag)
if ($LASTEXITCODE -ne 0) {
    throw "無法查詢本機 Release tag：$tag"
}
$localTagExists = $localTag -contains $tag
$tagSourceSha = $null
$localTagObject = $null
if ($localTagExists) {
    $localTagObject = (& git rev-parse "refs/tags/$tag").Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($localTagObject)) {
        throw "無法解析本機 Release tag object：$tag"
    }
    $tagCommit = (& git rev-parse "refs/tags/$tag^{commit}").Trim()
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($tagCommit)) {
        throw "無法解析本機 Release tag：$tag"
    }
    $tagCommitMessage = & git show -s --format=%B $tagCommit | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "無法讀取 Release tag commit：$tag"
    }
    if ($tagCommitMessage -match '(?m)^Release-For: (?<sha>[0-9a-fA-F]{40})\s*$') {
        $tagSourceSha = $Matches.sha
    }
    if (-not $tagSourceSha) {
        throw "Release tag $tag 沒有可驗證的來源 commit 標記，拒絕覆寫。"
    }
    if ($tagSourceSha -ine $SourceSha) {
        throw "Release tag $tag 已保留給來源 commit $tagSourceSha，不能改成 $SourceSha。請重新執行該來源的 Release workflow 以取得下一個版本。"
    }
}
if ($tagExists -and -not $localTagExists) {
    throw "遠端有 Release tag $tag，但無法在本機驗證 tag commit。"
}
if ($tagExists -and $localTagObject -ine (($remoteTag[0] -split '\s+')[0])) {
    throw "本機與遠端 Release tag $tag 不一致；拒絕清除或覆寫遠端 tag。"
}

$releaseViewExitCode = 1
$existingReleaseDraft = $null
$existingReleaseJson = & gh release view $tag --repo $Repository --json id,isDraft 2>$null
$releaseViewExitCode = $LASTEXITCODE
if ($releaseViewExitCode -eq 0) {
    $existingReleaseDraft = [bool](($existingReleaseJson | ConvertFrom-Json).isDraft)
    if (-not $existingReleaseDraft) {
        if ($tagSourceSha -ine $SourceSha) {
            throw "GitHub Release $tag 已發布，但無法確認它屬於來源 commit $SourceSha；為避免把不同執行檔錯誤標記為同一版本，已停止。"
        }
        Write-Host "GitHub Release $tag 已由來源 commit $SourceSha 發布；略過重複發佈。"
        return
    }

    if ($tagExists -and $tagSourceSha -ine $SourceSha) {
        throw "GitHub draft Release $tag 不屬於來源 commit $SourceSha；為避免覆寫其他建置，已停止。"
    }
    & gh release delete $tag --repo $Repository --cleanup-tag --yes
    if ($LASTEXITCODE -ne 0) {
        throw "無法清除未發布的 draft Release：$tag"
    }
    $tagExists = $false
}
elseif ($releaseViewExitCode -ne 1) {
    throw "查詢 GitHub Release 失敗，結束代碼：$releaseViewExitCode"
}

if ($tagExists) {
    & git push origin --delete $tag
    if ($LASTEXITCODE -ne 0) {
        throw "無法清除未發布 Release 的舊 tag：$tag"
    }
}
if ($localTagExists) {
    & git tag --delete $tag
    if ($LASTEXITCODE -ne 0) {
        throw "無法清除本機未發布 Release tag：$tag"
    }
}

& git switch --detach origin/main
if ($LASTEXITCODE -ne 0) {
    throw '無法切換到最新 main commit。'
}

$projectVersion = [string]([xml](Get-Content -LiteralPath $projectPath -Raw)).Project.PropertyGroup.Version
if ($projectVersion -match '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$' -and
    [version]$projectVersion -gt [version]$Version) {
    throw "main 已升到 $projectVersion，拒絕以舊版本 $Version 覆寫。"
}

& (Join-Path $PSScriptRoot 'Set-AppVersion.ps1') -ProjectPath $projectPath -Version $Version | Out-Null
Copy-Item -LiteralPath $metadataSettingsPath -Destination $settingsPath -Force

$existingReleaseCommit = (& git log origin/main --format=%H --grep="Release-For: $SourceSha" -1).Trim()
if ($LASTEXITCODE -ne 0) {
    throw '無法查詢 main 上的 Release metadata commit。'
}

if ([string]::IsNullOrWhiteSpace($existingReleaseCommit)) {
    & git config user.name 'github-actions[bot]'
    & git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
    & git add -- $projectRelativePath $settingsRelativePath
    if ($LASTEXITCODE -ne 0) {
        throw '無法加入版本與引擎 Release metadata。'
    }

    & git commit --allow-empty -m "chore(release): $tag [skip ci]" -m "Release-For: $SourceSha"
    if ($LASTEXITCODE -ne 0) {
        throw '無法建立 Release metadata commit。'
    }

    & git push origin 'HEAD:main'
    if ($LASTEXITCODE -ne 0) {
        throw '推送 Release metadata 到 main 失敗；請確認 GitHub App 已獲 main ruleset bypass。'
    }
    $metadataCommit = (& git rev-parse HEAD).Trim()
}
else {
    $metadataCommit = $existingReleaseCommit
    $existingProjectVersion = [string]([xml](& git show "${metadataCommit}:$projectRelativePath" | Out-String)).Project.PropertyGroup.Version
    if ($existingProjectVersion -ne $Version) {
        throw "來源 commit 已有 Release metadata $existingProjectVersion，與本次套件版本 $Version 不一致。"
    }

    $existingSettings = & git show "${metadataCommit}:$settingsRelativePath" | Out-String
    if ($existingSettings.Trim() -ne (Get-Content -LiteralPath $settingsPath -Raw).Trim()) {
        & git config user.name 'github-actions[bot]'
        & git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
        & git add -- $settingsRelativePath
        & git commit -m "chore(release): refresh $tag [skip ci]" -m "Release-For: $SourceSha"
        if ($LASTEXITCODE -ne 0) {
            throw '無法更新未發布 Release 的引擎 SHA metadata。'
        }
        & git push origin 'HEAD:main'
        if ($LASTEXITCODE -ne 0) {
            throw '推送更新後的引擎 SHA metadata 到 main 失敗。'
        }
        $metadataCommit = (& git rev-parse HEAD).Trim()
    }
}

& git tag -a $tag $metadataCommit -m "Neo AirPlay $tag"
if ($LASTEXITCODE -ne 0) {
    throw "無法建立 Git tag：$tag"
}
& git push origin "refs/tags/$tag"
if ($LASTEXITCODE -ne 0) {
    throw "推送 Git tag 失敗：$tag"
}

& git fetch origin develop
if ($LASTEXITCODE -eq 0) {
    & git switch --detach origin/develop
    if ($LASTEXITCODE -ne 0) {
        throw '無法切換到 develop 以同步 Release metadata。'
    }

    & git restore --source=$metadataCommit --staged --worktree -- $projectRelativePath $settingsRelativePath
    if ($LASTEXITCODE -ne 0) {
        throw '無法同步版本與引擎設定到 develop。'
    }
    & git diff --cached --quiet -- $projectRelativePath $settingsRelativePath
    if ($LASTEXITCODE -eq 1) {
        & git config user.name 'github-actions[bot]'
        & git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
        & git commit -m "chore(release): sync $tag [skip ci]"
        if ($LASTEXITCODE -ne 0) {
            throw '無法建立 develop Release metadata 同步 commit。'
        }
        & git push origin 'HEAD:develop'
        if ($LASTEXITCODE -ne 0) {
            throw '推送 Release metadata 到 develop 失敗。'
        }
    }
    elseif ($LASTEXITCODE -ne 0) {
        throw '無法檢查 develop 上的 Release metadata 差異。'
    }
}
else {
    Write-Warning '遠端尚無 develop 分支；略過 Release metadata 同步。'
}

$releaseAssets = @($appArchive, $appSidecar, $engineArchive, $engineSidecar, $checksumFile)
$releaseArguments = @('release', 'create', $tag) + $releaseAssets + @(
    '--repo', $Repository,
    '--title', "Neo AirPlay $tag",
    '--generate-notes',
    '--draft',
    '--target', $metadataCommit
)
& gh @releaseArguments
if ($LASTEXITCODE -ne 0) {
    throw "建立 draft GitHub Release 失敗：$tag"
}

$release = (& gh api "repos/$Repository/releases/tags/$tag" | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0 -or -not $release.id) {
    throw "找不到新建立的 GitHub Release：$tag"
}
& gh release edit $tag --repo $Repository --draft=false
if ($LASTEXITCODE -ne 0) {
    throw "將 GitHub Release 發布失敗：$tag"
}

Write-Host "已發布 Neo AirPlay $tag。"
