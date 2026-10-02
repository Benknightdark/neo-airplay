using System.Diagnostics;
using FlaUI.Core;
using FlaUI.Core.AutomationElements;
using FlaUI.UIA3;
using Xunit;
using FlaUIApplication = FlaUI.Core.Application;

namespace NeoAirPlay.Tests.App;

[Trait("Category", "E2E")]
public sealed class AppEndToEndTests
{
    [Fact]
    public void AppLaunchesAndNavigatesBetweenPages()
    {
        var solutionRoot = FindSolutionRoot(AppContext.BaseDirectory);
        var configuration = FindBuildConfiguration(AppContext.BaseDirectory);
        var executablePath = Path.Combine(
            solutionRoot,
            "src",
            "NeoAirPlay.App",
            "bin",
            configuration,
            "net10.0-windows",
            "win-x64",
            "NeoAirPlay.exe");
        Assert.True(File.Exists(executablePath), $"App executable was not found: {executablePath}");

        var profileDirectory = Path.Combine(
            Path.GetTempPath(),
            "NeoAirPlay.App.Test",
            Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(profileDirectory);

        FlaUIApplication? app = null;
        try
        {
            var startInfo = new ProcessStartInfo(executablePath)
            {
                WorkingDirectory = Path.GetDirectoryName(executablePath)!,
                UseShellExecute = false
            };
            startInfo.ArgumentList.Add("--e2e-profile");
            startInfo.ArgumentList.Add(profileDirectory);
            app = FlaUIApplication.Launch(startInfo);

            using var automation = new UIA3Automation();
            var window = app.GetMainWindow(automation, TimeSpan.FromSeconds(20));
            Assert.NotNull(window);
            Assert.Equal("Neo AirPlay", window.Title);

            var androidState = window.FindFirstDescendant(condition => condition.ByAutomationId("AndroidStateLabel"));
            var androidDeviceSelector = window.FindFirstDescendant(condition => condition.ByAutomationId("AndroidDeviceSelector"));
            var refreshAndroidDevices = window.FindFirstDescendant(condition => condition.ByAutomationId("RefreshAndroidDevices"))?.AsButton();
            var startAndroidMirror = window.FindFirstDescendant(condition => condition.ByAutomationId("StartAndroidMirror"))?.AsButton();
            Assert.NotNull(androidState);
            Assert.NotNull(androidDeviceSelector);
            Assert.NotNull(refreshAndroidDevices);
            Assert.True(refreshAndroidDevices.IsEnabled);
            Assert.NotNull(startAndroidMirror);
            Assert.False(startAndroidMirror.IsEnabled);

            AssertNavigation(window, "NavigateSettings", "設定");
            AssertNavigation(window, "NavigateDiagnostics", "診斷記錄");
            AssertNavigation(window, "NavigateHome", "總覽");

            Assert.True(app.Close(), "The application did not close cleanly in E2E profile mode.");
            Assert.True(app.HasExited);
            Assert.True(Directory.Exists(Path.Combine(profileDirectory, "local", "NeoAirPlay", "Logs")));
        }
        finally
        {
            if (app is not null)
            {
                if (!app.HasExited)
                {
                    app.Kill();
                }

                app.Dispose();
            }

            if (Directory.Exists(profileDirectory))
            {
                Directory.Delete(profileDirectory, recursive: true);
            }
        }
    }

    private static void AssertNavigation(Window window, string automationId, string expectedHeading)
    {
        var button = window.FindFirstDescendant(condition => condition.ByAutomationId(automationId))?.AsButton();
        Assert.NotNull(button);
        button.Invoke();

        var heading = window.FindFirstDescendant(condition => condition.ByAutomationId("CurrentPageHeading"));
        Assert.NotNull(heading);
        Assert.Equal(expectedHeading, heading.Name);
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
