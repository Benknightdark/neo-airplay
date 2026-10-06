using System.ComponentModel;
using System.Diagnostics;
using System.Net.Http;
using System.Security.Principal;
using System.Threading;
using System.Windows;
using Application = System.Windows.Application;
using MessageBox = System.Windows.MessageBox;
using NeoAirPlay.Models;
using NeoAirPlay.Services;
using NeoAirPlay.ViewModels;

namespace NeoAirPlay;

public partial class App : Application
{
    private static readonly HttpClient EngineHttpClient = new() { Timeout = TimeSpan.FromMinutes(30) };
    private const string BonjourSupportUrl = "https://support.apple.com/zh-tw/106380";

    private Mutex? _instanceMutex;
    private bool _ownsMutex;
    private UxPlayProcessEngine? _engine;
    private ScrcpyAndroidMirrorEngine? _androidEngine;
    private CancellationTokenSource? _startupCancellation;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        AppDataPaths dataPaths;
        try
        {
            dataPaths = AppDataPaths.FromCommandLine(e.Args);
        }
        catch (ArgumentException exception)
        {
            MessageBox.Show(
                exception.Message,
                "Neo AirPlay",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown();
            return;
        }

        if (e.Args.Contains("--elevated-engine-setup", StringComparer.OrdinalIgnoreCase))
        {
            var helperExitCode = await RunElevatedSetupModeAsync(e.Args, dataPaths);
            Shutdown(helperExitCode);
            return;
        }

        _instanceMutex = new Mutex(initiallyOwned: true, dataPaths.MutexName, out _ownsMutex);
        if (!_ownsMutex)
        {
            MessageBox.Show(
                "Neo AirPlay 已經在系統匣中執行。",
                "Neo AirPlay",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
            Shutdown();
            return;
        }

        try
        {
            var logger = new FileAppLogger(dataPaths.LogDirectory);
            var settingsStore = new JsonSettingsStore(logger, dataPaths.SettingsPath);
            var startupRegistration = new StartupRegistration();
            var settings = await settingsStore.LoadAsync();
            var engine = new UxPlayProcessEngine(
                logger,
                gstreamerRegistryPath: dataPaths.GStreamerRegistryPath);
            var androidEngine = new ScrcpyAndroidMirrorEngine(logger);
            _engine = engine;
            _androidEngine = androidEngine;

            var viewModel = new MainViewModel(
                engine,
                androidEngine,
                logger,
                settingsStore,
                startupRegistration,
                settings);
            var window = new MainWindow(viewModel, closeOnWindowClose: dataPaths.IsE2eProfile);
            MainWindow = window;
            window.Show();

            if (e.Args.Contains("--background", StringComparer.OrdinalIgnoreCase))
            {
                window.HideToTray();
            }

            if (!dataPaths.IsE2eProfile)
            {
                _startupCancellation = new CancellationTokenSource();
                var progress = new Progress<string>(message => viewModel.SetEngineSetupStatus(message, isInProgress: true));
                if (e.Args.Contains("--start", StringComparer.OrdinalIgnoreCase))
                {
                    if (await PrepareEnginesAsync(dataPaths, logger, viewModel, progress, _startupCancellation.Token))
                    {
                        await viewModel.StartReceiverAsync();
                    }
                }
                else
                {
                    _ = PrepareEnginesAsync(dataPaths, logger, viewModel, progress, _startupCancellation.Token);
                }
            }
            else if (e.Args.Contains("--start", StringComparer.OrdinalIgnoreCase))
            {
                await viewModel.StartReceiverAsync();
            }
        }
        catch (Exception exception)
        {
            MessageBox.Show(
                $"Neo AirPlay 無法啟動。\n\n{exception.Message}",
                "Neo AirPlay",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown();
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _startupCancellation?.Cancel();
        _startupCancellation?.Dispose();
        _engine?.Dispose();
        _androidEngine?.Dispose();
        if (_ownsMutex)
        {
            _instanceMutex?.ReleaseMutex();
        }

        _instanceMutex?.Dispose();
        base.OnExit(e);
    }

    private static async Task<bool> PrepareEnginesAsync(
        AppDataPaths dataPaths,
        IAppLogger logger,
        MainViewModel viewModel,
        IProgress<string> progress,
        CancellationToken cancellationToken)
    {
        var applicationDirectory = AppContext.BaseDirectory;
        var bootstrapper = new EnginePackageBootstrapper(
            applicationDirectory,
            dataPaths.EnginePackageCacheDirectory,
            EngineHttpClient,
            logger);

        viewModel.SetEngineSetupStatus("正在檢查 Bonjour Service…", isInProgress: true);
        var bonjourCoordinator = new BonjourStartupCoordinator(
            new BonjourStartupInspector(),
            token => LaunchElevatedSetupAsync(archivePath: null, configureBonjour: true, token),
            ShowMissingBonjour,
            ShowBonjourElevationReason);
        BonjourStartupResult bonjourResult;
        try
        {
            bonjourResult = await bonjourCoordinator.EnsureReadyAsync(logger, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            viewModel.SetEngineSetupStatus("Bonjour 準備已取消；下次啟動時會再試一次。", isInProgress: false);
            return false;
        }
        catch (Exception exception)
        {
            var message = $"Bonjour Service 檢查失敗：{exception.Message}";
            await logger.WriteAsync(LogLevel.Error, message, cancellationToken);
            bonjourResult = new BonjourStartupResult(IsReady: false, message);
        }

        if (cancellationToken.IsCancellationRequested)
        {
            viewModel.SetEngineSetupStatus("啟動準備已取消；下次啟動時會再試一次。", isInProgress: false);
            return false;
        }

        viewModel.SetEngineSetupStatus("正在檢查投影引擎…", isInProgress: true);
        var engineReady = false;
        string? engineMessage = null;
        try
        {
            var result = await Task.Run(
                () => bootstrapper.PrepareAsync(progress.Report, cancellationToken),
                cancellationToken);

            if (result.RequiresElevation)
            {
                var answer = MessageBox.Show(
                    "引擎安裝目錄需要系統管理員權限，才能放入已驗證的投影引擎檔案。是否繼續？Windows 接著會顯示系統管理員權限提示。",
                    "Neo AirPlay 引擎設定",
                    MessageBoxButton.YesNo,
                    MessageBoxImage.Information);
                if (answer != MessageBoxResult.Yes)
                {
                    engineMessage = "引擎檔案已下載；授權後才能安裝到此目錄。";
                    await logger.WriteAsync(LogLevel.Information, "使用者取消引擎管理員安裝。", cancellationToken);
                }
                else
                {
                    var elevatedExitCode = await LaunchElevatedSetupAsync(
                        result.CachedArchivePath,
                        configureBonjour: false,
                        cancellationToken);
                    if (elevatedExitCode is null)
                    {
                        engineMessage = "已取消系統管理員權限要求；引擎檔案尚未安裝。";
                        await logger.WriteAsync(LogLevel.Information, "使用者取消 Windows UAC 權限要求。", cancellationToken);
                    }
                    else if (elevatedExitCode != 0)
                    {
                        engineMessage = $"系統管理員引擎安裝失敗（結束代碼 {elevatedExitCode}）。請查看診斷記錄。";
                        await logger.WriteAsync(LogLevel.Error, engineMessage, cancellationToken);
                    }
                    else if (!bootstrapper.IsRuntimeComplete())
                    {
                        engineMessage = "管理員安裝完成，但仍有引擎檔案缺少。請查看診斷記錄。";
                        await logger.WriteAsync(LogLevel.Error, engineMessage, cancellationToken);
                    }
                    else
                    {
                        engineReady = true;
                    }
                }
            }
            else if (result.IsReady)
            {
                engineReady = true;
            }
            else
            {
                engineMessage = result.Message;
                await logger.WriteAsync(LogLevel.Error, engineMessage, cancellationToken);
            }
        }
        catch (OperationCanceledException)
        {
            engineMessage = "引擎準備已取消；下次啟動時會再試一次。";
        }
        catch (Exception exception)
        {
            engineMessage = $"引擎準備失敗：{exception.Message}";
            await logger.WriteAsync(LogLevel.Error, engineMessage, cancellationToken);
        }

        var statusMessages = new[]
        {
            bonjourResult.IsReady ? null : bonjourResult.Message,
            engineReady ? null : engineMessage
        }.Where(message => !string.IsNullOrWhiteSpace(message));
        viewModel.SetEngineSetupStatus(string.Join(Environment.NewLine, statusMessages), isInProgress: false);

        if (bonjourResult.IsReady && engineReady)
        {
            await logger.WriteAsync(LogLevel.Information, "投影引擎已準備完成。", cancellationToken);
            return true;
        }

        return false;
    }

    private static void ShowMissingBonjour(string? sourceProblem)
    {
        var problem = string.IsNullOrWhiteSpace(sourceProblem)
            ? "AirPlay 需要 Bonjour Service，但找不到可用的 Bonjour 安裝檔案。"
            : sourceProblem;
        var answer = MessageBox.Show(
            $"{problem}\n\n{BonjourSupportUrl}\n\n要現在開啟 Apple 支援頁面嗎？Android 投影仍可使用。",
            "需要 Bonjour",
            MessageBoxButton.YesNo,
            MessageBoxImage.Information);
        if (answer == MessageBoxResult.Yes)
        {
            try
            {
                Process.Start(new ProcessStartInfo(BonjourSupportUrl) { UseShellExecute = true });
            }
            catch (Exception exception)
            {
                MessageBox.Show(
                    $"無法開啟 Apple 支援頁面：{exception.Message}\n\n{BonjourSupportUrl}",
                    "需要 Bonjour",
                    MessageBoxButton.OK,
                    MessageBoxImage.Warning);
            }
        }
    }

    private static void ShowBonjourElevationReason()
    {
        MessageBox.Show(
            "AirPlay 需要啟動或修復 Bonjour Service。Neo AirPlay 會使用可信任的本機 Bonjour 檔案，並將服務放在受保護的 Program Files 共用目錄。Windows 接著會顯示系統管理員權限提示。",
            "啟用 Bonjour Service",
            MessageBoxButton.OK,
            MessageBoxImage.Information);
    }

    private static async Task<int?> LaunchElevatedSetupAsync(
        string? archivePath,
        bool configureBonjour,
        CancellationToken cancellationToken)
    {
        var executablePath = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(executablePath))
        {
            throw new InvalidOperationException("無法找到 Neo AirPlay 執行檔，不能要求系統管理員權限。");
        }

        var startInfo = new ProcessStartInfo(executablePath)
        {
            WorkingDirectory = AppContext.BaseDirectory,
            UseShellExecute = true,
            Verb = "runas"
        };
        startInfo.ArgumentList.Add("--elevated-engine-setup");
        if (!string.IsNullOrWhiteSpace(archivePath))
        {
            startInfo.ArgumentList.Add("--engine-package-archive");
            startInfo.ArgumentList.Add(Path.GetFullPath(archivePath));
        }
        if (configureBonjour)
        {
            startInfo.ArgumentList.Add("--enable-bonjour");
        }

        try
        {
            using var process = Process.Start(startInfo);
            if (process is null)
            {
                return 1;
            }

            await process.WaitForExitAsync(cancellationToken);
            return process.ExitCode;
        }
        catch (Win32Exception exception) when (exception.NativeErrorCode == 1223)
        {
            return null;
        }
    }

    private static async Task<int> RunElevatedSetupModeAsync(IReadOnlyList<string> arguments, AppDataPaths dataPaths)
    {
        var logger = new FileAppLogger(dataPaths.LogDirectory);
        try
        {
            if (!IsRunningAsAdministrator())
            {
                throw new UnauthorizedAccessException("引擎設定 helper 必須由 Windows UAC 以系統管理員身分啟動。");
            }

            var archiveIndex = IndexOfArgument(arguments, "--engine-package-archive");
            if (archiveIndex >= 0)
            {
                if (archiveIndex + 1 >= arguments.Count)
                {
                    throw new ArgumentException("--engine-package-archive 需要指定 ZIP 檔案路徑。");
                }

                var bootstrapper = new EnginePackageBootstrapper(
                    AppContext.BaseDirectory,
                    dataPaths.EnginePackageCacheDirectory,
                    EngineHttpClient,
                    logger);
                var installResult = await bootstrapper.InstallArchiveAsync(arguments[archiveIndex + 1]);
                if (!installResult.IsReady)
                {
                    throw new IOException(installResult.Message);
                }
            }

            if (arguments.Contains("--enable-bonjour", StringComparer.OrdinalIgnoreCase))
            {
                await RunBonjourScriptsAsync(logger);
            }

            return 0;
        }
        catch (Exception exception)
        {
            await logger.WriteAsync(LogLevel.Error, $"系統管理員設定失敗：{exception.Message}");
            return 1;
        }
    }

    private static async Task RunBonjourScriptsAsync(IAppLogger logger)
    {
        var scriptsDirectory = Path.Combine(AppContext.BaseDirectory, "scripts");
        await RunPowerShellScriptAsync(Path.Combine(scriptsDirectory, "Stage-BonjourRuntime.ps1"), logger);
        await RunPowerShellScriptAsync(Path.Combine(scriptsDirectory, "Install-BonjourService.ps1"), logger);
    }

    private static async Task RunPowerShellScriptAsync(string scriptPath, IAppLogger logger)
    {
        if (!File.Exists(scriptPath))
        {
            throw new FileNotFoundException("找不到 Bonjour 管理腳本。", scriptPath);
        }

        var powerShellPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.System),
            "WindowsPowerShell",
            "v1.0",
            "powershell.exe");
        var startInfo = new ProcessStartInfo(powerShellPath)
        {
            WorkingDirectory = AppContext.BaseDirectory,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        startInfo.ArgumentList.Add("-NoLogo");
        startInfo.ArgumentList.Add("-NoProfile");
        startInfo.ArgumentList.Add("-NonInteractive");
        startInfo.ArgumentList.Add("-ExecutionPolicy");
        startInfo.ArgumentList.Add("Bypass");
        startInfo.ArgumentList.Add("-File");
        startInfo.ArgumentList.Add(scriptPath);

        using var process = Process.Start(startInfo)
            ?? throw new InvalidOperationException("無法啟動 Bonjour 管理腳本。");
        var standardOutputTask = process.StandardOutput.ReadToEndAsync();
        var standardErrorTask = process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();
        var standardOutput = await standardOutputTask;
        var standardError = await standardErrorTask;
        if (!string.IsNullOrWhiteSpace(standardOutput))
        {
            await logger.WriteAsync(LogLevel.Information, standardOutput.Trim());
        }
        if (!string.IsNullOrWhiteSpace(standardError))
        {
            await logger.WriteAsync(LogLevel.Warning, standardError.Trim());
        }
        if (process.ExitCode != 0)
        {
            throw new IOException($"{Path.GetFileName(scriptPath)} 失敗，結束代碼 {process.ExitCode}。");
        }
    }

    private static bool IsRunningAsAdministrator()
    {
        using var identity = WindowsIdentity.GetCurrent();
        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
    }

    private static int IndexOfArgument(IReadOnlyList<string> arguments, string expected) =>
        Enumerable.Range(0, arguments.Count)
            .FirstOrDefault(index => string.Equals(arguments[index], expected, StringComparison.OrdinalIgnoreCase), -1);
}
