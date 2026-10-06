using System.IO.Compression;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;
using NeoAirPlay.Models;
using NeoAirPlay.Services;

namespace NeoAirPlay;

internal sealed class EnginePackageBootstrapper
{
    private const long MaximumArchiveSize = 1_500_000_000;
    private const long MaximumExtractedSize = 1_500_000_000;
    private const string Repository = "Benknightdark/neo-airplay";
    private const string EngineAssetName = "neo-airplay-engine-win-x64.zip";
    private const string EngineManifestName = "engine-package-manifest.json";
    private static readonly Regex VersionPattern = new(
        @"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$",
        RegexOptions.CultureInvariant | RegexOptions.Compiled);
    private static readonly JsonSerializerOptions JsonOptions = new() { PropertyNameCaseInsensitive = true };

    private readonly string _applicationDirectory;
    private readonly string _engineDirectory;
    private readonly string _settingsPath;
    private readonly string _packageCacheDirectory;
    private readonly HttpClient _httpClient;
    private readonly IAppLogger _logger;

    internal EnginePackageBootstrapper(
        string applicationDirectory,
        string packageCacheDirectory,
        HttpClient httpClient,
        IAppLogger logger)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(applicationDirectory);
        ArgumentException.ThrowIfNullOrWhiteSpace(packageCacheDirectory);
        ArgumentNullException.ThrowIfNull(httpClient);
        ArgumentNullException.ThrowIfNull(logger);

        _applicationDirectory = Path.GetFullPath(applicationDirectory);
        _engineDirectory = Path.Combine(_applicationDirectory, "engine");
        _settingsPath = Path.Combine(_applicationDirectory, "engine-package.json");
        _packageCacheDirectory = Path.GetFullPath(packageCacheDirectory);
        _httpClient = httpClient;
        _logger = logger;
    }

    internal async Task<EngineProvisioningResult> PrepareAsync(
        Action<string>? reportStatus = null,
        CancellationToken cancellationToken = default)
    {
        string? cachedArchivePath = null;
        try
        {
            var settings = await LoadSettingsAsync(cancellationToken);
            var missingGroups = GetMissingGroups(settings);
            if (missingGroups.Count == 0)
            {
                return new EngineProvisioningResult(true, "引擎檔案已齊全。");
            }

            Directory.CreateDirectory(_packageCacheDirectory);
            var archivePath = Path.Combine(_packageCacheDirectory, $"{settings.Version}-{settings.AssetName}");
            cachedArchivePath = archivePath;
            if (!await IsArchiveValidAsync(archivePath, settings.Sha256, cancellationToken))
            {
                if (File.Exists(archivePath))
                {
                    File.Delete(archivePath);
                }

                reportStatus?.Invoke("正在背景下載投影引擎…");
                var downloadUri = new Uri(
                    $"https://github.com/{Repository}/releases/download/{Uri.EscapeDataString(settings.ReleaseTag)}/{Uri.EscapeDataString(settings.AssetName)}");
                await DownloadArchiveAsync(downloadUri, archivePath, cancellationToken);
                if (!await IsArchiveValidAsync(archivePath, settings.Sha256, cancellationToken))
                {
                    File.Delete(archivePath);
                    throw new InvalidDataException("引擎套件 SHA-256 驗證失敗，已刪除下載檔案。");
                }
            }

            if (!CanWriteApplicationDirectory())
            {
                return new EngineProvisioningResult(
                    false,
                    "引擎套件已下載。安裝目錄需要系統管理員權限。",
                    RequiresElevation: true,
                    CachedArchivePath: archivePath);
            }

            return await InstallArchiveAsync(archivePath, reportStatus, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (UnauthorizedAccessException exception) when (cachedArchivePath is not null && File.Exists(cachedArchivePath))
        {
            var message = $"引擎套件已下載，但安裝目錄需要系統管理員權限：{exception.Message}";
            await _logger.WriteAsync(LogLevel.Information, message, cancellationToken);
            return new EngineProvisioningResult(false, message, RequiresElevation: true, CachedArchivePath: cachedArchivePath);
        }
        catch (Exception exception) when (exception is IOException
            or UnauthorizedAccessException
            or HttpRequestException
            or InvalidDataException
            or JsonException
            or ArgumentException)
        {
            const string prefix = "引擎準備失敗：";
            await _logger.WriteAsync(LogLevel.Error, prefix + exception.Message, cancellationToken);
            return new EngineProvisioningResult(false, prefix + exception.Message);
        }
    }

    internal async Task<EngineProvisioningResult> InstallArchiveAsync(
        string archivePath,
        Action<string>? reportStatus = null,
        CancellationToken cancellationToken = default)
    {
        var settings = await LoadSettingsAsync(cancellationToken);
        await using var archiveStream = new FileStream(
            archivePath,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read,
            bufferSize: 128 * 1024,
            FileOptions.Asynchronous | FileOptions.SequentialScan);
        if (!await IsArchiveValidAsync(archiveStream, settings.Sha256, cancellationToken))
        {
            throw new InvalidDataException("待安裝的引擎套件 SHA-256 不符。");
        }

        var missingGroups = GetMissingGroups(settings);
        if (missingGroups.Count == 0)
        {
            return new EngineProvisioningResult(true, "引擎檔案已齊全。");
        }

        var stagingDirectory = Path.Combine(Path.GetTempPath(), "NeoAirPlay", "engine-stage-" + Guid.NewGuid().ToString("N"));
        try
        {
            reportStatus?.Invoke("正在驗證並安裝引擎檔案…");
            var packageManifest = await ExtractAndValidateArchiveAsync(archiveStream, stagingDirectory, settings, cancellationToken);
            var engineSource = Path.Combine(stagingDirectory, "engine");
            var filesToInstall = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

            if (missingGroups.Contains(EngineRuntimeGroup.AirPlay))
            {
                AddFiles(filesToInstall, settings.AirPlayFiles);
            }
            if (missingGroups.Contains(EngineRuntimeGroup.Android))
            {
                AddFiles(filesToInstall, settings.AndroidFiles);
            }
            AddFiles(filesToInstall, settings.SharedFiles);

            foreach (var relativePath in filesToInstall)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var normalizedPath = ToEngineRelativePath(relativePath);
                var source = ResolveChildPath(engineSource, normalizedPath);
                var destination = ResolveChildPath(_engineDirectory, normalizedPath);
                if (!File.Exists(source))
                {
                    throw new InvalidDataException($"引擎套件缺少檔案：{relativePath}");
                }

                var destinationDirectory = Path.GetDirectoryName(destination)
                    ?? throw new InvalidDataException($"無效的引擎路徑：{relativePath}");
                Directory.CreateDirectory(destinationDirectory);
                var temporaryFile = destination + "." + Guid.NewGuid().ToString("N") + ".tmp";
                try
                {
                    File.Copy(source, temporaryFile, overwrite: false);
                    File.Move(temporaryFile, destination, overwrite: true);
                }
                finally
                {
                    if (File.Exists(temporaryFile))
                    {
                        File.Delete(temporaryFile);
                    }
                }
            }

            var remainingGroups = GetMissingGroups(settings);
            if (remainingGroups.Count > 0)
            {
                throw new InvalidDataException(
                    $"引擎安裝後仍有缺少元件：{string.Join("、", remainingGroups)}。");
            }

            await _logger.WriteAsync(
                LogLevel.Information,
                $"已安裝 Neo AirPlay 引擎套件 {packageManifest.Version}。",
                cancellationToken);
            return new EngineProvisioningResult(true, "引擎檔案已準備完成。");
        }
        finally
        {
            if (Directory.Exists(stagingDirectory))
            {
                Directory.Delete(stagingDirectory, recursive: true);
            }
        }
    }

    internal bool IsRuntimeComplete()
    {
        if (!File.Exists(_settingsPath))
        {
            return false;
        }

        var json = File.ReadAllText(_settingsPath);
        var settings = JsonSerializer.Deserialize<EnginePackageSettings>(json, JsonOptions)
            ?? throw new InvalidDataException("engine-package.json 格式無效。");
        ValidateSettings(settings);
        return GetMissingGroups(settings).Count == 0;
    }

    private async Task<EnginePackageSettings> LoadSettingsAsync(CancellationToken cancellationToken)
    {
        if (!File.Exists(_settingsPath))
        {
            throw new FileNotFoundException("找不到引擎套件設定 engine-package.json。", _settingsPath);
        }

        var json = await File.ReadAllTextAsync(_settingsPath, cancellationToken);
        var settings = JsonSerializer.Deserialize<EnginePackageSettings>(json, JsonOptions)
            ?? throw new InvalidDataException("engine-package.json 格式無效。");
        ValidateSettings(settings);
        return settings;
    }

    private static void ValidateSettings(EnginePackageSettings settings)
    {
        if (string.IsNullOrWhiteSpace(settings.Version)
            || !VersionPattern.IsMatch(settings.Version)
            || !string.Equals(settings.ReleaseTag, $"v{settings.Version}", StringComparison.Ordinal)
            || !string.Equals(settings.AssetName, EngineAssetName, StringComparison.Ordinal)
            || !string.Equals(settings.ManifestName, EngineManifestName, StringComparison.Ordinal)
            || string.IsNullOrWhiteSpace(settings.Sha256)
            || settings.Sha256.Length != 64
            || settings.AirPlayFiles is not { Length: > 0 }
            || settings.AndroidFiles is not { Length: > 0 }
            || settings.SharedFiles is null
            || !settings.Sha256.All(Uri.IsHexDigit)
            || settings.AirPlayFiles.Any(string.IsNullOrWhiteSpace)
            || settings.AndroidFiles.Any(string.IsNullOrWhiteSpace)
            || settings.SharedFiles.Any(string.IsNullOrWhiteSpace))
        {
            throw new InvalidDataException("engine-package.json 欄位不完整或無效。");
        }

        var allPaths = settings.AirPlayFiles
            .Concat(settings.AndroidFiles)
            .Concat(settings.SharedFiles)
            .ToArray();
        if (allPaths.Any(path => !path.StartsWith("engine/", StringComparison.OrdinalIgnoreCase)
                || path.Contains("\\", StringComparison.Ordinal)
                || path.Contains(":", StringComparison.Ordinal)
                || path.Split('/').Any(part => part is "" or "." or "..")
                || path.StartsWith("engine/bonjour/", StringComparison.OrdinalIgnoreCase))
            || allPaths.Distinct(StringComparer.OrdinalIgnoreCase).Count() != allPaths.Length)
        {
            throw new InvalidDataException("engine-package.json 含有重複或不安全的檔案路徑。");
        }
    }

    private IReadOnlySet<EngineRuntimeGroup> GetMissingGroups(EnginePackageSettings settings)
    {
        var missing = new HashSet<EngineRuntimeGroup>();
        if (settings.AirPlayFiles.Any(path => !File.Exists(ResolveChildPath(_engineDirectory, ToEngineRelativePath(path)))))
        {
            missing.Add(EngineRuntimeGroup.AirPlay);
        }
        if (settings.AndroidFiles.Any(path => !File.Exists(ResolveChildPath(_engineDirectory, ToEngineRelativePath(path)))))
        {
            missing.Add(EngineRuntimeGroup.Android);
        }
        if (settings.SharedFiles.Any(path => !File.Exists(ResolveChildPath(_engineDirectory, ToEngineRelativePath(path)))))
        {
            missing.Add(EngineRuntimeGroup.Shared);
        }

        return missing;
    }

    private async Task DownloadArchiveAsync(Uri uri, string destinationPath, CancellationToken cancellationToken)
    {
        var temporaryPath = destinationPath + ".download";
        try
        {
            using var response = await _httpClient.GetAsync(uri, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
            if (response.StatusCode == HttpStatusCode.NotFound)
            {
                throw new HttpRequestException(
                    $"找不到引擎套件 Release 附件：{uri}。請依 README 發佈引擎套件。",
                    inner: null,
                    statusCode: response.StatusCode);
            }
            response.EnsureSuccessStatusCode();
            if (response.Content.Headers.ContentLength is > MaximumArchiveSize)
            {
                throw new InvalidDataException("引擎套件超過允許大小。");
            }

            await using var source = await response.Content.ReadAsStreamAsync(cancellationToken);
            await using (var destination = new FileStream(
                temporaryPath,
                FileMode.Create,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 128 * 1024,
                FileOptions.Asynchronous | FileOptions.SequentialScan))
            {
                var buffer = new byte[128 * 1024];
                long totalBytes = 0;
                while (true)
                {
                    var bytesRead = await source.ReadAsync(buffer, cancellationToken);
                    if (bytesRead == 0)
                    {
                        break;
                    }

                    totalBytes += bytesRead;
                    if (totalBytes > MaximumArchiveSize)
                    {
                        throw new InvalidDataException("引擎套件超過允許大小。");
                    }
                    await destination.WriteAsync(buffer.AsMemory(0, bytesRead), cancellationToken);
                }

                await destination.FlushAsync(cancellationToken);
            }

            File.Move(temporaryPath, destinationPath, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporaryPath))
            {
                File.Delete(temporaryPath);
            }
        }
    }

    private async Task<EngineRuntimePackageManifest> ExtractAndValidateArchiveAsync(
        Stream archiveStream,
        string stagingDirectory,
        EnginePackageSettings settings,
        CancellationToken cancellationToken)
    {
        Directory.CreateDirectory(stagingDirectory);
        archiveStream.Position = 0;
        using var archive = new ZipArchive(archiveStream, ZipArchiveMode.Read, leaveOpen: true);
        var extractedFiles = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        long extractedSize = 0;
        var manifestFound = false;

        foreach (var entry in archive.Entries)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (string.IsNullOrEmpty(entry.FullName))
            {
                throw new InvalidDataException("引擎套件含有空白路徑。");
            }

            var isDirectory = entry.FullName.EndsWith("/", StringComparison.Ordinal);
            var safeDestination = ResolveArchivePath(stagingDirectory, entry.FullName.TrimEnd('/'));
            if (isDirectory)
            {
                Directory.CreateDirectory(safeDestination);
                continue;
            }

            if (!extractedFiles.Add(entry.FullName))
            {
                throw new InvalidDataException($"引擎套件含有重複檔案：{entry.FullName}");
            }
            var unixFileType = (entry.ExternalAttributes >> 16) & 0xF000;
            if (unixFileType == 0xA000)
            {
                throw new InvalidDataException($"引擎套件不允許符號連結：{entry.FullName}");
            }

            extractedSize += entry.Length;
            if (extractedSize > MaximumExtractedSize)
            {
                throw new InvalidDataException("引擎套件解壓縮後超過允許大小。");
            }

            var destinationDirectory = Path.GetDirectoryName(safeDestination)
                ?? throw new InvalidDataException($"無效的套件路徑：{entry.FullName}");
            Directory.CreateDirectory(destinationDirectory);
            await using var source = entry.Open();
            await using var destination = new FileStream(
                safeDestination,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 128 * 1024,
                FileOptions.Asynchronous | FileOptions.SequentialScan);
            await source.CopyToAsync(destination, cancellationToken);

            if (string.Equals(entry.FullName, settings.ManifestName, StringComparison.OrdinalIgnoreCase))
            {
                manifestFound = true;
            }
        }

        if (!manifestFound)
        {
            throw new InvalidDataException($"引擎套件缺少 {settings.ManifestName}。");
        }

        var manifestPath = ResolveArchivePath(stagingDirectory, settings.ManifestName);
        var manifestJson = await File.ReadAllTextAsync(manifestPath, cancellationToken);
        var manifest = JsonSerializer.Deserialize<EngineRuntimePackageManifest>(manifestJson, JsonOptions)
            ?? throw new InvalidDataException("引擎套件 manifest 格式無效。");
        if (manifest.Files is null
            || manifest.AirPlayFiles is null
            || manifest.AndroidFiles is null
            || manifest.SharedFiles is null
            || !string.Equals(manifest.Version, settings.Version, StringComparison.Ordinal)
            || !SamePaths(manifest.AirPlayFiles, settings.AirPlayFiles)
            || !SamePaths(manifest.AndroidFiles, settings.AndroidFiles)
            || !SamePaths(manifest.SharedFiles, settings.SharedFiles))
        {
            throw new InvalidDataException("引擎套件 manifest 與 App 所需版本或檔案清單不符。");
        }

        var manifestFiles = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var file in manifest.Files)
        {
            if (file is null || string.IsNullOrWhiteSpace(file.Path))
            {
                throw new InvalidDataException("引擎套件 manifest 含有空白檔案項目。");
            }

            ValidateManifestPath(file.Path);
            if (!manifestFiles.Add(file.Path)
                || file.Length < 0
                || string.IsNullOrWhiteSpace(file.Sha256)
                || file.Sha256.Length != 64
                || !file.Sha256.All(Uri.IsHexDigit))
            {
                throw new InvalidDataException($"引擎套件 manifest 含有無效檔案項目：{file.Path}");
            }

            var path = ResolveArchivePath(stagingDirectory, file.Path);
            if (!File.Exists(path) || new FileInfo(path).Length != file.Length)
            {
                throw new InvalidDataException($"引擎套件檔案缺少或大小不符：{file.Path}");
            }

            await using var stream = File.OpenRead(path);
            var hash = Convert.ToHexString(await SHA256.HashDataAsync(stream, cancellationToken)).ToLowerInvariant();
            if (!string.Equals(hash, file.Sha256, StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException($"引擎套件檔案 SHA-256 不符：{file.Path}");
            }
        }

        var actualPackageFiles = extractedFiles
            .Where(path => !string.Equals(path, settings.ManifestName, StringComparison.OrdinalIgnoreCase))
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        if (!actualPackageFiles.SetEquals(manifestFiles))
        {
            throw new InvalidDataException("引擎套件檔案清單與 ZIP 內容不一致。");
        }

        var requiredFiles = settings.AirPlayFiles
            .Concat(settings.AndroidFiles)
            .Concat(settings.SharedFiles)
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        if (!requiredFiles.IsSubsetOf(manifestFiles))
        {
            throw new InvalidDataException("引擎套件 manifest 缺少 App 必要檔案。");
        }

        return manifest;
    }

    private static void ValidateManifestPath(string path)
    {
        if (string.IsNullOrWhiteSpace(path)
            || !path.StartsWith("engine/", StringComparison.OrdinalIgnoreCase)
            || path.Contains('\\')
            || path.Contains(':')
            || path.Split('/').Any(part => part is "" or "." or "..")
            || path.StartsWith("engine/bonjour/", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"引擎套件 manifest 含有不安全路徑：{path}");
        }
    }

    private static bool SamePaths(IReadOnlyList<string> left, IReadOnlyList<string> right) =>
        left.Count == right.Count
        && left.ToHashSet(StringComparer.OrdinalIgnoreCase).SetEquals(right);

    private static void AddFiles(ISet<string> destination, IReadOnlyList<string> paths)
    {
        foreach (var path in paths)
        {
            destination.Add(path);
        }
    }

    private static string ToEngineRelativePath(string archivePath) => archivePath["engine/".Length..];

    private static string ResolveArchivePath(string rootDirectory, string entryPath)
    {
        if (string.IsNullOrWhiteSpace(entryPath)
            || entryPath.StartsWith('/')
            || entryPath.Contains('\\')
            || entryPath.Contains(':')
            || entryPath.Split('/').Any(part => part is "" or "." or ".."))
        {
            throw new InvalidDataException($"引擎套件含有不安全路徑：{entryPath}");
        }

        return ResolveChildPath(rootDirectory, entryPath.Replace('/', Path.DirectorySeparatorChar));
    }

    private static string ResolveChildPath(string rootDirectory, string relativePath)
    {
        var fullRoot = Path.GetFullPath(rootDirectory).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        var fullPath = Path.GetFullPath(Path.Combine(fullRoot, relativePath));
        if (!fullPath.StartsWith(fullRoot, StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"引擎路徑超出目標目錄：{relativePath}");
        }

        return fullPath;
    }

    private async Task<bool> IsArchiveValidAsync(string archivePath, string expectedSha256, CancellationToken cancellationToken)
    {
        if (!File.Exists(archivePath))
        {
            return false;
        }

        await using var stream = new FileStream(
            archivePath,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read,
            bufferSize: 128 * 1024,
            FileOptions.Asynchronous | FileOptions.SequentialScan);
        return await IsArchiveValidAsync(stream, expectedSha256, cancellationToken);
    }

    private static async Task<bool> IsArchiveValidAsync(Stream stream, string expectedSha256, CancellationToken cancellationToken)
    {
        if (!stream.CanSeek || stream.Length == 0 || stream.Length > MaximumArchiveSize)
        {
            return false;
        }

        stream.Position = 0;
        var actualHash = Convert.ToHexString(await SHA256.HashDataAsync(stream, cancellationToken)).ToLowerInvariant();
        stream.Position = 0;
        return string.Equals(actualHash, expectedSha256, StringComparison.OrdinalIgnoreCase);
    }

    private bool CanWriteApplicationDirectory()
    {
        var probePath = Path.Combine(_applicationDirectory, ".neo-airplay-write-check-" + Guid.NewGuid().ToString("N"));
        try
        {
            using (File.Create(probePath)) { }
            return true;
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException)
        {
            return false;
        }
        finally
        {
            if (File.Exists(probePath))
            {
                File.Delete(probePath);
            }
        }
    }

    private enum EngineRuntimeGroup
    {
        AirPlay,
        Android,
        Shared
    }
}

internal sealed record EngineProvisioningResult(
    bool IsReady,
    string Message,
    bool RequiresElevation = false,
    string? CachedArchivePath = null);

internal sealed record EnginePackageSettings(
    string Version,
    string ReleaseTag,
    string AssetName,
    string Sha256,
    string ManifestName,
    string[] AirPlayFiles,
    string[] AndroidFiles,
    string[] SharedFiles);

internal sealed record EngineRuntimePackageManifest(
    string Version,
    string[] AirPlayFiles,
    string[] AndroidFiles,
    string[] SharedFiles,
    EngineRuntimePackageFile[] Files);

internal sealed record EngineRuntimePackageFile(string Path, long Length, string Sha256);
