using NeoAirPlay.Models;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Framework.Test;

[Trait("Category", "Unit")]
public sealed class ScrcpyAndroidMirrorEngineTests
{
    [Fact]
    public void RequiredRuntimeFiles_LoadsUniqueFlatNamesFromSharedManifest()
    {
        var requiredFiles = ScrcpyAndroidMirrorEngine.RequiredRuntimeFiles;

        Assert.NotEmpty(requiredFiles);
        Assert.Equal(requiredFiles.Count, requiredFiles.Distinct(StringComparer.OrdinalIgnoreCase).Count());
        Assert.All(requiredFiles, file => Assert.Equal(Path.GetFileName(file), file));
    }

    [Fact]
    public async Task RefreshDevicesAsync_ParsesUsbWirelessAndUnavailableDevices()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(
                0,
                "List of devices attached\n"
                + "usb-123 device product:pixel model:Pixel_8 transport_id:1\n"
                + "unauthorized-456 unauthorized usb:1-1\n"
                + "192.168.1.20:5555 device product:pixel model:Pixel_8_Pro\n"
                + "offline-789 offline usb:1-2\n",
                string.Empty)
        };
        await using var engine = CreateEngine(engineDirectory, commandRunner);

        var devices = await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);

        Assert.Collection(
            devices,
            device =>
            {
                Assert.Equal("usb-123", device.Serial);
                Assert.Equal("Pixel 8", device.Model);
                Assert.Equal(AndroidDeviceStatus.Ready, device.Status);
            },
            device => Assert.Equal(AndroidDeviceStatus.Unauthorized, device.Status),
            device =>
            {
                Assert.Equal("192.168.1.20:5555", device.Serial);
                Assert.Equal(AndroidDeviceStatus.Ready, device.Status);
            },
            device => Assert.Equal(AndroidDeviceStatus.Offline, device.Status));
        Assert.Contains("2 台", engine.StatusMessage);
        Assert.Equal("devices", commandRunner.StartInfo?.ArgumentList[0]);
        Assert.Equal("-l", commandRunner.StartInfo?.ArgumentList[1]);
    }

    [Fact]
    public async Task StartAndStopAsync_UsesSelectedDeviceAndClosesScrcpyProcess()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(
                0,
                "List of devices attached\n192.168.1.20:5555 device model:Pixel_8\n",
                string.Empty)
        };
        var processFactory = new FakeReceiverProcessFactory();
        await using var engine = CreateEngine(engineDirectory, commandRunner, processFactory);

        await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);
        await engine.StartAsync("192.168.1.20:5555", fullscreen: true, TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Running, engine.State);
        var startInfo = Assert.IsType<System.Diagnostics.ProcessStartInfo>(processFactory.Process.StartInfo);
        Assert.Equal("scrcpy.exe", Path.GetFileName(startInfo.FileName));
        Assert.Equal(
            ["--serial", "192.168.1.20:5555", "--window-title", "Neo AirPlay - Android (192.168.1.20:5555)", "--fullscreen"],
            startInfo.ArgumentList);
        Assert.Contains(engineDirectory, startInfo.Environment["PATH"]);

        await engine.StopAsync(TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Stopped, engine.State);
        Assert.True(processFactory.Process.WasDisposed);
        Assert.False(processFactory.Process.WasKilled);
    }

    [Fact]
    public async Task RefreshDevicesAsync_WhenRequiredFilesAreMissing_ReportsDependencyMissing()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var processFactory = new FakeReceiverProcessFactory();
        var commandRunner = new FakeAndroidCommandRunner();
        await using var engine = CreateEngine(temporaryDirectory.Path, commandRunner, processFactory);

        var devices = await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);

        Assert.Empty(devices);
        Assert.Equal(ReceiverState.DependencyMissing, engine.State);
        Assert.Contains("scrcpy.exe", engine.StatusMessage);
        Assert.Null(commandRunner.StartInfo);
        Assert.Null(processFactory.Process.StartInfo);
    }

    [Fact]
    public async Task RefreshDevicesAsync_WhenAdbFails_ReportsActionableError()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(1, string.Empty, "ADB server is unavailable")
        };
        await using var engine = CreateEngine(engineDirectory, commandRunner);

        var devices = await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);

        Assert.Empty(devices);
        Assert.Equal(ReceiverState.Faulted, engine.State);
        Assert.Contains("ADB server is unavailable", engine.StatusMessage);
    }

    [Fact]
    public async Task RefreshDevicesAsync_WhenAdbFailsDuringProjection_LeavesProjectionRunningAndStoppable()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(0, "List of devices attached\nphone-123 device model:Pixel_8\n", string.Empty)
        };
        var processFactory = new FakeReceiverProcessFactory();
        await using var engine = CreateEngine(engineDirectory, commandRunner, processFactory);

        await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);
        await engine.StartAsync("phone-123", cancellationToken: TestContext.Current.CancellationToken);

        commandRunner.Result = new AndroidCommandResult(1, string.Empty, "ADB server is unavailable");
        var devices = await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);

        Assert.Empty(devices);
        Assert.Equal(ReceiverState.Running, engine.State);
        Assert.Contains("仍在執行", engine.StatusMessage);
        Assert.Contains("ADB server is unavailable", engine.StatusMessage);

        commandRunner.Result = new AndroidCommandResult(0, "List of devices attached\nphone-123 device model:Pixel_8\n", string.Empty);
        await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Running, engine.State);
        Assert.Contains("裝置清單已更新", engine.StatusMessage);

        await engine.StopAsync(TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Stopped, engine.State);
        Assert.True(processFactory.Process.WasDisposed);
        Assert.False(processFactory.Process.WasKilled);
    }

    [Fact]
    public async Task RefreshDevicesAsync_WhenRuntimeFilesDisappearDuringProjection_LeavesProjectionStoppable()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(0, "List of devices attached\nphone-123 device model:Pixel_8\n", string.Empty)
        };
        var processFactory = new FakeReceiverProcessFactory();
        await using var engine = CreateEngine(engineDirectory, commandRunner, processFactory);

        await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);
        await engine.StartAsync("phone-123", cancellationToken: TestContext.Current.CancellationToken);
        File.Delete(Path.Combine(engineDirectory, "adb.exe"));

        var devices = await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);

        Assert.Empty(devices);
        Assert.Equal(ReceiverState.Running, engine.State);
        Assert.Contains("仍在執行", engine.StatusMessage);
        Assert.Contains("adb.exe", engine.StatusMessage);

        await engine.StopAsync(TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Stopped, engine.State);
        Assert.True(processFactory.Process.WasDisposed);
        Assert.False(processFactory.Process.WasKilled);
    }

    [Fact]
    public async Task StartAsync_WhenDeviceWasNotAuthorized_DoesNotStartScrcpy()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(
                0,
                "List of devices attached\nphone-123 unauthorized usb:1-1\n",
                string.Empty)
        };
        var processFactory = new FakeReceiverProcessFactory();
        await using var engine = CreateEngine(engineDirectory, commandRunner, processFactory);

        await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);
        await engine.StartAsync("phone-123", cancellationToken: TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Faulted, engine.State);
        Assert.Contains("重新整理清單", engine.StatusMessage);
        Assert.Null(processFactory.Process.StartInfo);
    }

    [Fact]
    public async Task ProcessExitWithError_ChangesEngineStateToFaulted()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var engineDirectory = CreateEngineLayout(temporaryDirectory.Path);
        var commandRunner = new FakeAndroidCommandRunner
        {
            Result = new AndroidCommandResult(0, "List of devices attached\nphone-123 device model:Pixel_8\n", string.Empty)
        };
        var processFactory = new FakeReceiverProcessFactory();
        await using var engine = CreateEngine(engineDirectory, commandRunner, processFactory);
        var statusChanged = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        engine.StatusChanged += (_, _) =>
        {
            if (engine.State == ReceiverState.Faulted)
            {
                statusChanged.TrySetResult();
            }
        };

        await engine.RefreshDevicesAsync(TestContext.Current.CancellationToken);
        await engine.StartAsync("phone-123", cancellationToken: TestContext.Current.CancellationToken);
        processFactory.Process.Exit(exitCode: 1);
        await statusChanged.Task.WaitAsync(TimeSpan.FromSeconds(3), TestContext.Current.CancellationToken);

        Assert.Equal(ReceiverState.Faulted, engine.State);
        Assert.Contains("結束代碼：1", engine.StatusMessage);
    }

    private static ScrcpyAndroidMirrorEngine CreateEngine(
        string engineDirectory,
        FakeAndroidCommandRunner commandRunner,
        FakeReceiverProcessFactory? processFactory = null) =>
        new(new TestLogger(), processFactory ?? new FakeReceiverProcessFactory(), commandRunner, engineDirectory);

    private static string CreateEngineLayout(string root)
    {
        Directory.CreateDirectory(root);
        foreach (var file in ScrcpyAndroidMirrorEngine.RequiredRuntimeFiles)
        {
            File.WriteAllText(Path.Combine(root, file), string.Empty);
        }

        return root;
    }
}
