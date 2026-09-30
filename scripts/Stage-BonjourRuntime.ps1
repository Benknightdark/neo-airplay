[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$destinationDirectory = Join-Path $projectRoot 'engine\bonjour'
$service = Get-CimInstance Win32_Service -Filter "Name='Bonjour Service'"
if (-not $service) {
    throw '找不到 Windows 的 Bonjour Service。請先安裝 Bonjour，或手動將 mDNSResponder.exe 與 dnssd.dll 放入 engine\bonjour。'
}

$imagePath = $service.PathName.Trim()
if ($imagePath.StartsWith('"')) {
    $endQuote = $imagePath.IndexOf('"', 1)
    if ($endQuote -lt 2) { throw "無法解析 Bonjour Service 路徑：$imagePath" }
    $sourceExecutable = $imagePath.Substring(1, $endQuote - 1)
}
else {
    $match = [regex]::Match($imagePath, '^.+?\.exe', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    if (-not $match.Success) { throw "無法解析 Bonjour Service 路徑：$imagePath" }
    $sourceExecutable = $match.Value
}

$sourceDirectory = Split-Path -Parent $sourceExecutable
$sourceFiles = @('mDNSResponder.exe', 'dnssd.dll') | ForEach-Object {
    $path = Join-Path $sourceDirectory $_
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Bonjour 執行檔目錄缺少必要檔案：$path"
    }

    $item = Get-Item -LiteralPath $path
    if ($item.VersionInfo.ProductName -ne 'Bonjour') {
        throw "拒絕複製非 Bonjour 元件：$path"
    }

    [pscustomobject]@{
        Source = $path
        Name = $_
        Version = $item.VersionInfo.FileVersion
        Product = $item.VersionInfo.ProductName
    }
}

$versions = @($sourceFiles.Version | Select-Object -Unique)
if ($versions.Count -ne 1) {
    throw "mDNSResponder.exe 與 dnssd.dll 版本不一致：$($versions -join ', ')"
}

New-Item -ItemType Directory -Path $destinationDirectory -Force | Out-Null
foreach ($file in $sourceFiles) {
    $destination = Join-Path $destinationDirectory $file.Name
    if ([IO.Path]::GetFullPath($file.Source) -ine [IO.Path]::GetFullPath($destination)) {
        Copy-Item -LiteralPath $file.Source -Destination $destination -Force
    }
}

$manifest = [ordered]@{
    Product = 'Bonjour'
    FileVersion = $versions[0]
    Files = @($sourceFiles | ForEach-Object {
        [ordered]@{
            Name = $_.Name
            Sha256 = (Get-FileHash -LiteralPath (Join-Path $destinationDirectory $_.Name) -Algorithm SHA256).Hash
        }
    })
}
$manifestPath = Join-Path $destinationDirectory 'runtime-manifest.json'
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $manifestPath -Encoding utf8

Write-Host "已將 Bonjour $($versions[0]) 的服務元件放入：$destinationDirectory"
Write-Host '僅複製 mDNSResponder.exe 與 dnssd.dll；沒有複製 UxPlay wrapper 或影音執行檔。'
