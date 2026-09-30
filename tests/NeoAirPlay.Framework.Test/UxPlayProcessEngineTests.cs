using NeoAirPlay.Models;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Framework.Test;

[Trait("Category", "Unit")]
public sealed class UxPlayProcessEngineTests
{
    [Fact]
    public async Task StartAsync_WhenExecutableIsMissing_SetsDependencyMissingWithoutStartingProcess()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var logger = new TestLogger();
        var processFactory = new FakeReceiverProcessFactory();
        var inspector = new FakeBonjourRuntimeInspector(null, "Bonjour unavailable");
        await using var engine = new UxPlayProcessEngine(
            logger,
            processFactory,
            inspector,
            temporaryDirectory.Path,
            System.IO.Path.Combine(temporaryDirectory.Path, "registry.bin"));

        await engine.StartAsync(
            new ReceiverOptions("Receiver", "Auto", false),
            TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.DependencyMissing, engine.State);
        Assert.Null(processFactory.Process.StartInfo);
        Assert.Contains(logger.Entries, entry => entry.Level == LogLevel.Error);
    }

    [Fact]
    public async Task StartAndStopAsync_UsesConfiguredArgumentsAndTransitionsState()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var bonjourDirectory = System.IO.Path.Combine(temporaryDirectory.Path, "bonjour");
        Directory.CreateDirectory(bonjourDirectory);
        var processFactory = new FakeReceiverProcessFactory();
        var logger = new TestLogger();
        await using var engine = new UxPlayProcessEngine(
            logger,
            processFactory,
            new FakeBonjourRuntimeInspector(new BonjourRuntime(bonjourDirectory)),
            engineDirectory,
            System.IO.Path.Combine(temporaryDirectory.Path, "local", "gstreamer-registry.bin"));

        await engine.StartAsync(
            new ReceiverOptions("Living Room", "D3D11", true),
            TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Running, engine.State);
        Assert.NotNull(processFactory.Process.StartInfo);
        Assert.Equal(
            ["-n", "Living Room", "-nh", "-fs", "-vs", "d3d11videosink"],
            processFactory.Process.StartInfo.ArgumentList);
        Assert.Equal(
            System.IO.Path.Combine(temporaryDirectory.Path, "local", "gstreamer-registry.bin"),
            processFactory.Process.StartInfo.Environment["GST_REGISTRY"]);

        await engine.StopAsync(TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Stopped, engine.State);
        Assert.True(processFactory.Process.WasDisposed);
        Assert.False(processFactory.Process.WasKilled);
    }

    [Fact]
    public async Task StartAsync_WhenBonjourIsUnavailable_ReportsMissingDependency()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var processFactory = new FakeReceiverProcessFactory();
        await using var engine = new UxPlayProcessEngine(
            new TestLogger(),
            processFactory,
            new FakeBonjourRuntimeInspector(null, "Bonjour unavailable"),
            engineDirectory,
            System.IO.Path.Combine(temporaryDirectory.Path, "registry.bin"));

        await engine.StartAsync(
            new ReceiverOptions("Receiver", "Auto", false),
            TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.DependencyMissing, engine.State);
        Assert.Contains("Bonjour unavailable", engine.StatusMessage);
        Assert.Null(processFactory.Process.StartInfo);
    }

    private static string CreateEngineLayout(string root)
    {
        var engineDirectory = System.IO.Path.Combine(root, "engine");
        File.WriteAllText(System.IO.Path.Combine(Directory.CreateDirectory(engineDirectory).FullName, "uxplay.exe"), string.Empty);
        Directory.CreateDirectory(System.IO.Path.Combine(engineDirectory, "runtime", "bin"));
        Directory.CreateDirectory(System.IO.Path.Combine(engineDirectory, "runtime", "lib", "gstreamer-1.0"));
        return engineDirectory;
    }
}
