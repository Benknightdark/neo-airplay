using System.Diagnostics;
using NeoAirPlay.Models;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Tests.App;

[Trait("Category", "HardwareE2E")]
[Collection("HardwareE2E")]
public sealed class ReceiverEnginesHardwareEndToEndTests
{
    [Fact]
    public async Task AirPlayAndAndroidReceiversRunAtTheSameTime()
    {
        var solutionRoot = FindSolutionRoot(AppContext.BaseDirectory);
        var engineDirectory = Path.Combine(solutionRoot, "engine");
        var logDirectory = Path.Combine(Path.GetTempPath(), "NeoAirPlay.HardwareE2E", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(logDirectory);
        var logger = new FileAppLogger(logDirectory);
        var registryPath = Path.Combine(logDirectory, "gstreamer-registry.bin");
        await using var airPlayEngine = new UxPlayProcessEngine(logger, engineDirectory, registryPath);
        await using var androidEngine = new ScrcpyAndroidMirrorEngine(logger, Path.Combine(engineDirectory, "android"));

        try
        {
            await airPlayEngine.StartAsync(
                new ReceiverOptions("Neo AirPlay Hardware E2E", "Auto", false),
                TestContext.Current.CancellationToken);
            Assert.True(
                airPlayEngine.State == ReceiverState.Running,
                $"AirPlay receiver did not start: {airPlayEngine.StatusMessage}");

            var devices = await androidEngine.RefreshDevicesAsync(TestContext.Current.CancellationToken);
            var device = devices.FirstOrDefault(candidate => candidate.IsReady);
            Assert.True(
                device is not null,
                $"Connect and authorize an Android device first. ADB reported: {androidEngine.StatusMessage}");

            await androidEngine.StartAsync(device!.Serial, cancellationToken: TestContext.Current.CancellationToken);
            Assert.Equal(ReceiverState.Running, androidEngine.State);
            await Task.Delay(TimeSpan.FromSeconds(3), TestContext.Current.CancellationToken);
            Assert.Equal(ReceiverState.Running, androidEngine.State);
            Assert.True(
                await WaitForScrcpyWindowAsync(device.Serial, TestContext.Current.CancellationToken),
                $"scrcpy did not create a visible projection window for {device.Serial}.");

            await androidEngine.StopAsync(TestContext.Current.CancellationToken);

            Assert.Equal(ReceiverState.Stopped, androidEngine.State);
            Assert.Equal(ReceiverState.Running, airPlayEngine.State);
        }
        finally
        {
            await androidEngine.StopAsync(CancellationToken.None);
            await airPlayEngine.StopAsync(CancellationToken.None);
        }
    }

    private static async Task<bool> WaitForScrcpyWindowAsync(string deviceSerial, CancellationToken cancellationToken)
    {
        var expectedTitle = $"Neo AirPlay - Android ({deviceSerial})";
        for (var attempt = 0; attempt < 40; attempt++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            foreach (var process in Process.GetProcessesByName("scrcpy"))
            {
                using (process)
                {
                    if (!process.HasExited && string.Equals(process.MainWindowTitle, expectedTitle, StringComparison.Ordinal))
                    {
                        return true;
                    }
                }
            }

            await Task.Delay(TimeSpan.FromMilliseconds(250), cancellationToken);
        }

        return false;
    }

    private static string FindSolutionRoot(string startDirectory)
    {
        for (var current = new DirectoryInfo(startDirectory); current is not null; current = current.Parent)
        {
            if (File.Exists(Path.Combine(current.FullName, "NeoAirPlay.slnx")))
            {
                return current.FullName;
            }
        }

        throw new DirectoryNotFoundException("Could not find NeoAirPlay.slnx above the test output directory.");
    }
}
