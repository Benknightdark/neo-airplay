using System.Diagnostics;
using FlaUI.Core;
using FlaUI.Core.AutomationElements;
using FlaUI.UIA3;
using Xunit;
using FlaUIApplication = FlaUI.Core.Application;

namespace NeoAirPlay.Tests.App;

[Trait("Category", "HardwareE2E")]
[Collection("HardwareE2E")]
public sealed class AndroidProjectionUiHardwareEndToEndTests
{
    [Fact]
    public async Task DashboardStartsUsbProjectionWhileAirPlayIsRunning()
    {
        var solutionRoot = FindSolutionRoot(AppContext.BaseDirectory);
        var configuration = FindBuildConfiguration(AppContext.BaseDirectory);
        var appDirectory = Path.Combine(
            solutionRoot,
            "src",
            "NeoAirPlay.App",
            "bin",
            configuration,
            "net10.0-windows",
            "win-x64");
        var executablePath = Path.Combine(appDirectory, "NeoAirPlay.exe");
        var adbPath = Path.Combine(appDirectory, "engine", "android", "adb.exe");
        Assert.True(File.Exists(executablePath), $"App executable was not found: {executablePath}");
        Assert.True(File.Exists(adbPath), $"Bundled ADB was not found: {adbPath}");
        Assert.False(IsProcessRunning("uxplay"), "Stop any existing AirPlay receiver before running this hardware E2E.");

        var cancellationToken = TestContext.Current.CancellationToken;
        var usbSerial = await FindAuthorizedUsbSerialAsync(adbPath, cancellationToken);
        Assert.False(string.IsNullOrWhiteSpace(usbSerial), "Connect and authorize one USB Android device first.");

        var profileDirectory = Path.Combine(
            Path.GetTempPath(),
            "NeoAirPlay.Android.HardwareE2E",
            Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(profileDirectory);

        FlaUIApplication? app = null;
        try
        {
            var startInfo = new ProcessStartInfo(executablePath)
            {
                WorkingDirectory = appDirectory,
                UseShellExecute = false
            };
            startInfo.ArgumentList.Add("--e2e-profile");
            startInfo.ArgumentList.Add(profileDirectory);
            startInfo.ArgumentList.Add("--start");
            app = FlaUIApplication.Launch(startInfo);

            using var automation = new UIA3Automation();
            var window = app.GetMainWindow(automation, TimeSpan.FromSeconds(20));
            Assert.NotNull(window);
            Assert.Equal("Neo AirPlay", window.Title);
            Assert.True(
                await WaitUntilAsync(() => IsProcessRunning("uxplay"), TimeSpan.FromSeconds(20), cancellationToken),
                "The AirPlay receiver did not start in the test App.");

            var deviceSelector = window.FindFirstDescendant(
                condition => condition.ByAutomationId("AndroidDeviceSelector"))?.AsComboBox();
            var refreshButton = window.FindFirstDescendant(
                condition => condition.ByAutomationId("RefreshAndroidDevices"))?.AsButton();
            Assert.NotNull(deviceSelector);
            Assert.NotNull(refreshButton);
            refreshButton.Invoke();

            Assert.True(
                await WaitUntilAsync(
                    () => deviceSelector.Items.Any(item => item.Name.Contains(usbSerial!, StringComparison.Ordinal)),
                    TimeSpan.FromSeconds(20),
                    cancellationToken),
                $"The USB device {usbSerial} was not listed by the Dashboard.");

            var usbDeviceIndex = Array.FindIndex(
                deviceSelector.Items,
                item => item.Name.Contains(usbSerial!, StringComparison.Ordinal));
            Assert.True(usbDeviceIndex >= 0, $"The USB device {usbSerial} was not selectable.");
            deviceSelector.Select(usbDeviceIndex);

            Assert.True(
                await WaitUntilAsync(
                    () => window.FindFirstDescendant(condition => condition.ByAutomationId("StartAndroidMirror"))?.AsButton().IsEnabled == true,
                    TimeSpan.FromSeconds(10),
                    cancellationToken),
                "The Dashboard did not enable Android projection for the authorized USB device.");

            window.FindFirstDescendant(condition => condition.ByAutomationId("StartAndroidMirror"))?.AsButton().Invoke();

            Assert.True(
                await WaitUntilAsync(
                    () => window.FindFirstDescendant(condition => condition.ByAutomationId("AndroidStateLabel"))?.Name == "Android 正在投影",
                    TimeSpan.FromSeconds(15),
                    cancellationToken),
                "The Dashboard did not report Android projection as running.");
            Assert.True(
                await WaitForScrcpyWindowAsync(usbSerial!, TimeSpan.FromSeconds(15), cancellationToken),
                $"scrcpy did not show a projection window for USB device {usbSerial}.");

            var stopButton = window.FindFirstDescendant(
                condition => condition.ByAutomationId("StopAndroidMirror"))?.AsButton();
            Assert.NotNull(stopButton);
            stopButton.Invoke();

            Assert.True(
                await WaitUntilAsync(
                    () => window.FindFirstDescendant(condition => condition.ByAutomationId("AndroidStateLabel"))?.Name == "Android 尚未投影",
                    TimeSpan.FromSeconds(15),
                    cancellationToken),
                "The Dashboard did not report Android projection as stopped.");
            Assert.True(IsProcessRunning("uxplay"), "Stopping Android also stopped the AirPlay receiver.");
            Assert.True(app.Close(), "The App did not close cleanly after stopping Android projection.");
            Assert.True(app.HasExited);
        }
        finally
        {
            if (app is not null)
            {
                if (!app.HasExited)
                {
                    try
                    {
                        app.Close();
                    }
                    catch (Exception)
                    {
                        app.Kill();
                    }
                }

                if (!app.HasExited)
                {
                    app.Kill();
                }

                app.Dispose();
            }

            if (!string.IsNullOrWhiteSpace(usbSerial))
            {
                StopScrcpyWindow(usbSerial);
            }

            if (Directory.Exists(profileDirectory))
            {
                Directory.Delete(profileDirectory, recursive: true);
            }
        }
    }

    private static async Task<string?> FindAuthorizedUsbSerialAsync(
        string adbPath,
        CancellationToken cancellationToken)
    {
        var startInfo = new ProcessStartInfo(adbPath)
        {
            WorkingDirectory = Path.GetDirectoryName(adbPath)!,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true
        };
        startInfo.ArgumentList.Add("devices");
        startInfo.ArgumentList.Add("-l");

        using var process = Process.Start(startInfo)
            ?? throw new InvalidOperationException("Could not start bundled ADB.");
        var outputTask = process.StandardOutput.ReadToEndAsync(cancellationToken);
        var errorTask = process.StandardError.ReadToEndAsync(cancellationToken);
        await process.WaitForExitAsync(cancellationToken)
            .WaitAsync(TimeSpan.FromSeconds(15), cancellationToken);
        var output = await outputTask;
        var error = await errorTask;
        Assert.True(process.ExitCode == 0, $"ADB device discovery failed: {error}");

        return output.Split(["\r\n", "\n"], StringSplitOptions.RemoveEmptyEntries)
            .Select(line => line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries))
            .Where(columns => columns.Length >= 2
                && columns[1] == "device"
                && !columns[0].Contains(':'))
            .Select(columns => columns[0])
            .FirstOrDefault();
    }

    private static async Task<bool> WaitForScrcpyWindowAsync(
        string deviceSerial,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        var expectedTitle = $"Neo AirPlay - Android ({deviceSerial})";
        return await WaitUntilAsync(() =>
        {
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

            return false;
        }, timeout, cancellationToken);
    }

    private static void StopScrcpyWindow(string deviceSerial)
    {
        var expectedTitle = $"Neo AirPlay - Android ({deviceSerial})";
        foreach (var process in Process.GetProcessesByName("scrcpy"))
        {
            using (process)
            {
                if (process.HasExited || !string.Equals(process.MainWindowTitle, expectedTitle, StringComparison.Ordinal))
                {
                    continue;
                }

                if (!process.CloseMainWindow() || !process.WaitForExit(5_000))
                {
                    if (!process.HasExited)
                    {
                        process.Kill(entireProcessTree: true);
                        process.WaitForExit(5_000);
                    }
                }
            }
        }
    }

    private static bool IsProcessRunning(string processName)
    {
        foreach (var process in Process.GetProcessesByName(processName))
        {
            using (process)
            {
                if (!process.HasExited)
                {
                    return true;
                }
            }
        }

        return false;
    }

    private static async Task<bool> WaitUntilAsync(
        Func<bool> condition,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        var stopwatch = Stopwatch.StartNew();
        while (stopwatch.Elapsed < timeout)
        {
            cancellationToken.ThrowIfCancellationRequested();
            if (condition())
            {
                return true;
            }

            await Task.Delay(TimeSpan.FromMilliseconds(250), cancellationToken);
        }

        return condition();
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

    private static string FindBuildConfiguration(string startDirectory)
    {
        for (var current = new DirectoryInfo(startDirectory); current is not null; current = current.Parent)
        {
            if (current.Parent?.Name == "bin" && current.Name is "Debug" or "Release")
            {
                return current.Name;
            }
        }

        throw new DirectoryNotFoundException("Could not determine the test build configuration.");
    }
}
