using System.IO.Compression;
using System.Net;
using System.Security.Cryptography;
using System.Text.Json;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Tests.App;

[Trait("Category", "Unit")]
public sealed class EnginePackageBootstrapperTests
{
    [Fact]
    public async Task PrepareAsync_WhenRuntimeIsComplete_DoesNotDownload()
    {
        using var fixture = new EnginePackageFixture();
        fixture.CreateValidPackage();
        fixture.CreateInstalledRuntime();
        using var handler = new FakeHttpMessageHandler(_ => throw new InvalidOperationException("Unexpected download."));
        using var httpClient = new HttpClient(handler);
        var bootstrapper = fixture.CreateBootstrapper(httpClient);

        var result = await bootstrapper.PrepareAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.True(result.IsReady);
        Assert.Equal(0, handler.RequestCount);
    }

    [Fact]
    public async Task PrepareAsync_WhenOnlySharedFilesAreMissing_RestoresThemWithoutReplacingEngines()
    {
        using var fixture = new EnginePackageFixture();
        var archive = fixture.CreateValidPackage();
        fixture.CreateInstalledRuntime();
        var airPlayExecutable = Path.Combine(fixture.EngineDirectory, "uxplay.exe");
        File.WriteAllText(airPlayExecutable, "keep existing engine");
        File.Delete(Path.Combine(fixture.EngineDirectory, "licenses", "notice.txt"));
        using var handler = new FakeHttpMessageHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new ByteArrayContent(archive)
        });
        using var httpClient = new HttpClient(handler);
        var bootstrapper = fixture.CreateBootstrapper(httpClient);

        var result = await bootstrapper.PrepareAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.True(result.IsReady, result.Message);
        Assert.Equal("keep existing engine", await File.ReadAllTextAsync(airPlayExecutable, TestContext.Current.CancellationToken));
        Assert.Equal(
            "license",
            await File.ReadAllTextAsync(
                Path.Combine(fixture.EngineDirectory, "licenses", "notice.txt"),
                TestContext.Current.CancellationToken));
        Assert.Equal(1, handler.RequestCount);
    }

    [Fact]
    public async Task PrepareAsync_WhenFilesAreMissing_DownloadsAndInstallsPackage()
    {
        using var fixture = new EnginePackageFixture();
        var archive = fixture.CreateValidPackage();
        using var handler = new FakeHttpMessageHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new ByteArrayContent(archive)
        });
        using var httpClient = new HttpClient(handler);
        var bootstrapper = fixture.CreateBootstrapper(httpClient);

        var result = await bootstrapper.PrepareAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.True(result.IsReady, result.Message);
        Assert.Equal(1, handler.RequestCount);
        Assert.True(File.Exists(Path.Combine(fixture.EngineDirectory, "uxplay.exe")));
        Assert.True(File.Exists(Path.Combine(fixture.EngineDirectory, "android", "scrcpy.exe")));
        Assert.True(File.Exists(Path.Combine(fixture.EngineDirectory, "licenses", "notice.txt")));
    }

    [Fact]
    public async Task PrepareAsync_WhenOuterHashDoesNotMatch_RejectsPackageWithoutInstallingFiles()
    {
        using var fixture = new EnginePackageFixture();
        var archive = fixture.CreateValidPackage(expectedHashOverride: new string('0', 64));
        using var handler = new FakeHttpMessageHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new ByteArrayContent(archive)
        });
        using var httpClient = new HttpClient(handler);
        var bootstrapper = fixture.CreateBootstrapper(httpClient);

        var result = await bootstrapper.PrepareAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.False(result.IsReady);
        Assert.Contains("SHA-256", result.Message, StringComparison.OrdinalIgnoreCase);
        Assert.False(File.Exists(Path.Combine(fixture.EngineDirectory, "uxplay.exe")));
    }

    [Fact]
    public async Task PrepareAsync_WhenVersionContainsPathCharacters_RejectsSettingsBeforeDownload()
    {
        using var fixture = new EnginePackageFixture();
        fixture.CreateValidPackage();
        var settings = JsonSerializer.Deserialize<EnginePackageSettings>(
            await File.ReadAllTextAsync(fixture.SettingsPath, TestContext.Current.CancellationToken))!;
        await File.WriteAllTextAsync(
            fixture.SettingsPath,
            JsonSerializer.Serialize(settings with { Version = "../outside", ReleaseTag = "v../outside" }),
            TestContext.Current.CancellationToken);
        using var handler = new FakeHttpMessageHandler(_ => throw new InvalidOperationException("Unexpected download."));
        using var httpClient = new HttpClient(handler);
        var bootstrapper = fixture.CreateBootstrapper(httpClient);

        var result = await bootstrapper.PrepareAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.False(result.IsReady);
        Assert.Equal(0, handler.RequestCount);
        Assert.Contains("欄位不完整或無效", result.Message);
    }

    [Fact]
    public async Task PrepareAsync_WhenArchiveContainsPathTraversal_RejectsPackage()
    {
        using var fixture = new EnginePackageFixture();
        var archive = fixture.CreateValidPackage(includeTraversalEntry: true);
        using var handler = new FakeHttpMessageHandler(_ => new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new ByteArrayContent(archive)
        });
        using var httpClient = new HttpClient(handler);
        var bootstrapper = fixture.CreateBootstrapper(httpClient);

        var result = await bootstrapper.PrepareAsync(cancellationToken: TestContext.Current.CancellationToken);

        Assert.False(result.IsReady);
        Assert.Contains("不安全路徑", result.Message);
        Assert.False(File.Exists(Path.Combine(fixture.RootDirectory, "evil.txt")));
        Assert.False(File.Exists(Path.Combine(fixture.EngineDirectory, "uxplay.exe")));
    }

    private sealed class EnginePackageFixture : IDisposable
    {
        private static readonly Dictionary<string, byte[]> PackageFiles = new(StringComparer.OrdinalIgnoreCase)
        {
            ["engine/uxplay.exe"] = "uxplay"u8.ToArray(),
            ["engine/runtime/bin/gstreamer.dll"] = "gst runtime"u8.ToArray(),
            ["engine/runtime/lib/gstreamer-1.0/plugin.dll"] = "gst plugin"u8.ToArray(),
            ["engine/android/scrcpy.exe"] = "scrcpy"u8.ToArray(),
            ["engine/android/adb.exe"] = "adb"u8.ToArray(),
            ["engine/licenses/notice.txt"] = "license"u8.ToArray()
        };

        private readonly string _packageSettingsPath;

        public EnginePackageFixture()
        {
            RootDirectory = Path.Combine(Path.GetTempPath(), "NeoAirPlay.EnginePackage.Test", Guid.NewGuid().ToString("N"));
            ApplicationDirectory = Path.Combine(RootDirectory, "App");
            PackageCacheDirectory = Path.Combine(RootDirectory, "Cache");
            EngineDirectory = Path.Combine(ApplicationDirectory, "engine");
            _packageSettingsPath = Path.Combine(ApplicationDirectory, "engine-package.json");
            Directory.CreateDirectory(ApplicationDirectory);
        }

        public string RootDirectory { get; }
        public string ApplicationDirectory { get; }
        public string PackageCacheDirectory { get; }
        public string EngineDirectory { get; }
        public string SettingsPath => _packageSettingsPath;

        public byte[] CreateValidPackage(string? expectedHashOverride = null, bool includeTraversalEntry = false)
        {
            using var archiveStream = new MemoryStream();
            using (var archive = new ZipArchive(archiveStream, ZipArchiveMode.Create, leaveOpen: true))
            {
                var entries = new List<EngineRuntimePackageFile>();
                foreach (var (path, contents) in PackageFiles)
                {
                    AddArchiveFile(archive, path, contents);
                    entries.Add(new EngineRuntimePackageFile(
                        path,
                        contents.Length,
                        Convert.ToHexString(SHA256.HashData(contents)).ToLowerInvariant()));
                }

                if (includeTraversalEntry)
                {
                    AddArchiveFile(archive, "../evil.txt", "outside"u8.ToArray());
                }

                var manifest = new EngineRuntimePackageManifest(
                    "0.1.0",
                    ["engine/uxplay.exe", "engine/runtime/bin/gstreamer.dll", "engine/runtime/lib/gstreamer-1.0/plugin.dll"],
                    ["engine/android/adb.exe", "engine/android/scrcpy.exe"],
                    ["engine/licenses/notice.txt"],
                    [.. entries]);
                AddArchiveFile(archive, "engine-package-manifest.json", JsonSerializer.SerializeToUtf8Bytes(manifest));
            }

            var archiveBytes = archiveStream.ToArray();
            var settings = new EnginePackageSettings(
                "0.1.0",
                "v0.1.0",
                "neo-airplay-engine-win-x64.zip",
                expectedHashOverride ?? Convert.ToHexString(SHA256.HashData(archiveBytes)).ToLowerInvariant(),
                "engine-package-manifest.json",
                ["engine/uxplay.exe", "engine/runtime/bin/gstreamer.dll", "engine/runtime/lib/gstreamer-1.0/plugin.dll"],
                ["engine/android/adb.exe", "engine/android/scrcpy.exe"],
                ["engine/licenses/notice.txt"]);
            File.WriteAllText(_packageSettingsPath, JsonSerializer.Serialize(settings));
            return archiveBytes;
        }

        public void CreateInstalledRuntime()
        {
            foreach (var (path, contents) in PackageFiles)
            {
                var relative = path["engine/".Length..].Replace('/', Path.DirectorySeparatorChar);
                var destination = Path.Combine(EngineDirectory, relative);
                Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
                File.WriteAllBytes(destination, contents);
            }
        }

        public EnginePackageBootstrapper CreateBootstrapper(HttpClient httpClient) =>
            new(ApplicationDirectory, PackageCacheDirectory, httpClient, new FakeAppLogger());

        public void Dispose()
        {
            if (Directory.Exists(RootDirectory))
            {
                Directory.Delete(RootDirectory, recursive: true);
            }
        }

        private static void AddArchiveFile(ZipArchive archive, string path, byte[] contents)
        {
            var entry = archive.CreateEntry(path, CompressionLevel.Optimal);
            using var stream = entry.Open();
            stream.Write(contents);
        }
    }

    private sealed class FakeHttpMessageHandler(Func<HttpRequestMessage, HttpResponseMessage> respond) : HttpMessageHandler
    {
        public int RequestCount { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            RequestCount++;
            return Task.FromResult(respond(request));
        }
    }
}
