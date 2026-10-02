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

        try
        {
            viewModel.SetEngineSetupStatus("正在檢查投影引擎…", isInProgress: true);
            var result = await Task.Run(
                () => bootstrapper.PrepareAsync(progress.Report, cancellationToken),
                cancellationToken);

            if (result.RequiresElevation)
            {
                var inspector = new BonjourServiceInspector();
                var setupBonjour = inspector.TryGetInstalledRuntimeDirectory(out _)
                    && inspector.GetRuntime(out _) is null;
                var description = setupBonjour
                    ? "引擎安裝目錄需要系統管理員權限；同意後也會將已安裝的 Bonjour Service 切換至受保護的 Program Files 共用目錄。"
                    : "引擎安裝目錄需要系統管理員權限，才能放入已驗證的投影引擎檔案。";
                var answer = MessageBox.Show(
                    $"{description}\n\nBonjour 原服務路徑會先備份；服務執行檔會放在受保護的系統共用目錄。是否繼續？Windows 接著會顯示系統管理員權限提示。",
                    "Neo AirPlay 引擎設定",
                    MessageBoxButton.YesNo,
                    MessageBoxImage.Information);
                if (answer != MessageBoxResult.Yes)
                {
                    await logger.WriteAsync(LogLevel.Information, "使用者取消引擎管理員安裝。", cancellationToken);
                    viewModel.SetEngineSetupStatus("引擎檔案已下載；授權後才能安裝到此目錄。", isInProgress: false);
                    return false;
                }

                var elevatedExitCode = await LaunchElevatedSetupAsync(
                    result.CachedArchivePath,
                    setupBonjour,
                    cancellationToken);
                if (elevatedExitCode is null)
                {
                    viewModel.SetEngineSetupStatus("已取消系統管理員權限要求；下次啟動時會再試一次。", isInProgress: false);
                    await logger.WriteAsync(LogLevel.Information, "使用者取消 Windows UAC 權限要求。", cancellationToken);
                    return false;
                }
                if (elevatedExitCode != 0)
                {
                    var failure = $"系統管理員引擎安裝失敗（結束代碼 {elevatedExitCode}）。請查看診斷記錄。";
                    viewModel.SetEngineSetupStatus(failure, isInProgress: false);
                    await logger.WriteAsync(LogLevel.Error, failure, cancellationToken);
                    return false;
                }

                if (!bootstrapper.IsRuntimeComplete())
                {
                    const string failure = "管理員安裝完成，但仍有引擎檔案缺少。請查看診斷記錄。";
                    viewModel.SetEngineSetupStatus(failure, isInProgress: false);
                    await logger.WriteAsync(LogLevel.Error, failure, cancellationToken);
                    return false;
                }
            }
            else if (!result.IsReady)
            {
                viewModel.SetEngineSetupStatus(result.Message, isInProgress: false);
                return false;
            }

            var bonjourReady = await EnsureBonjourAsync(logger, viewModel, cancellationToken);
            if (!bonjourReady)
            {
                return false;
            }

            viewModel.SetEngineSetupStatus(string.Empty, isInProgress: false);
            await logger.WriteAsync(LogLevel.Information, "投影引擎已準備完成。", cancellationToken);
            return true;
        }
        catch (OperationCanceledException)
        {
            viewModel.SetEngineSetupStatus("引擎準備已取消；下次啟動時會再試一次。", isInProgress: false);
            return false;
        }
        catch (Exception exception)
        {
            var message = $"引擎準備失敗：{exception.Message}";
            viewModel.SetEngineSetupStatus(message, isInProgress: false);
            await logger.WriteAsync(LogLevel.Error, message, cancellationToken);
            return false;
        }
    }

    private static async Task<bool> EnsureBonjourAsync(
        IAppLogger logger,
        MainViewModel viewModel,
        CancellationToken cancellationToken)
    {
        var inspector = new BonjourServiceInspector();
        if (inspector.GetRuntime(out _) is not null)
        {
            return true;
        }

        if (!inspector.TryGetInstalledRuntimeDirectory(out _))
        {
            var answer = MessageBox.Show(
                $"AirPlay 需要 Bonjour Service，但這台電腦找不到可用的 Bonjour 安裝。請先依 Apple 官方說明安裝，再重新啟動 Neo AirPlay。\n\n{BonjourSupportUrl}\n\n要現在開啟 Apple 支援頁面嗎？Android 投影仍可使用。",
                "需要 Bonjour",
                MessageBoxButton.YesNo,
                MessageBoxImage.Information);
            if (answer == MessageBoxResult.Yes)
            {
                Process.Start(new ProcessStartInfo(BonjourSupportUrl) { UseShellExecute = true });
            }

            const string message = "Android 引擎已就緒；AirPlay 需要先安裝 Bonjour Service。";
            viewModel.SetEngineSetupStatus(message, isInProgress: false);
            await logger.WriteAsync(LogLevel.Warning, message, cancellationToken);
            return false;
        }

        var confirmation = MessageBox.Show(
            "AirPlay 需要將 Bonjour Service 指向受保護的 Program Files 共用目錄。這會安裝 Bonjour 元件、備份原路徑並啟動服務；Windows 接著會要求系統管理員權限。是否繼續？\n\nAndroid 投影不受影響。",
            "啟用 Bonjour Service",
            MessageBoxButton.YesNo,
            MessageBoxImage.Information);
        if (confirmation != MessageBoxResult.Yes)
        {
            const string declinedMessage = "AirPlay 尚未啟用 Bonjour Service；Android 投影仍可使用。";
            viewModel.SetEngineSetupStatus(declinedMessage, isInProgress: false);
            await logger.WriteAsync(LogLevel.Information, declinedMessage, cancellationToken);
            return false;
        }

        var exitCode = await LaunchElevatedSetupAsync(archivePath: null, configureBonjour: true, cancellationToken);
        if (exitCode is null)
        {
            const string canceledMessage = "已取消 Bonjour 管理員權限要求；AirPlay 尚未啟用。";
            viewModel.SetEngineSetupStatus(canceledMessage, isInProgress: false);
            await logger.WriteAsync(LogLevel.Information, canceledMessage, cancellationToken);
            return false;
        }
        var runtime = inspector.GetRuntime(out var problem);
        if (exitCode != 0 || runtime is null)
        {
            var failure = $"Bonjour Service 啟用失敗：{problem ?? $"管理員程序結束代碼 {exitCode}"}";
            viewModel.SetEngineSetupStatus(failure, isInProgress: false);
            await logger.WriteAsync(LogLevel.Error, failure, cancellationToken);
            return false;
        }

        await logger.WriteAsync(LogLevel.Information, "Bonjour Service 已啟用。", cancellationToken);
        return true;
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
