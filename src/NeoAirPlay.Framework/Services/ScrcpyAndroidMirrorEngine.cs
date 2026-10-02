using System.ComponentModel;
using System.Diagnostics;
using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public sealed class ScrcpyAndroidMirrorEngine : IAndroidMirrorEngine, IDisposable
{
    private static readonly TimeSpan AdbCommandTimeout = TimeSpan.FromSeconds(15);
    private const string RequiredFilesResourceName = "NeoAirPlay.Services.scrcpy-runtime-files.txt";
    private static readonly string[]? RequiredFiles = LoadRequiredFiles();

    private readonly SemaphoreSlim _lifecycleLock = new(1, 1);
    private readonly IAppLogger _logger;
    private readonly IReceiverProcessFactory _processFactory;
    private readonly IAndroidCommandRunner _commandRunner;
    private readonly string _engineDirectory;
    private IReceiverProcess? _process;
    private bool _stopRequested;
    private IReadOnlyList<AndroidDevice> _devices = Array.Empty<AndroidDevice>();

    public ScrcpyAndroidMirrorEngine(IAppLogger logger, string? engineDirectory = null)
        : this(
            logger,
            new SystemReceiverProcessFactory(),
            new SystemAndroidCommandRunner(),
            engineDirectory ?? Path.Combine(AppContext.BaseDirectory, "engine", "android"))
    {
    }

    internal ScrcpyAndroidMirrorEngine(
        IAppLogger logger,
        IReceiverProcessFactory processFactory,
        IAndroidCommandRunner commandRunner,
        string engineDirectory)
    {
        ArgumentNullException.ThrowIfNull(logger);
        ArgumentNullException.ThrowIfNull(processFactory);
        ArgumentNullException.ThrowIfNull(commandRunner);
        ArgumentException.ThrowIfNullOrWhiteSpace(engineDirectory);

        _logger = logger;
        _processFactory = processFactory;
        _commandRunner = commandRunner;
        _engineDirectory = Path.GetFullPath(engineDirectory);
    }

    public ReceiverState State { get; private set; } = ReceiverState.Stopped;
    public string StatusMessage { get; private set; } = "請連接 Android 裝置後重新整理清單。";
    public IReadOnlyList<AndroidDevice> Devices => _devices;
    public event EventHandler? StatusChanged;
    internal static IReadOnlyList<string> RequiredRuntimeFiles =>
        RequiredFiles is null ? Array.Empty<string>() : Array.AsReadOnly(RequiredFiles);

    public async Task<IReadOnlyList<AndroidDevice>> RefreshDevicesAsync(CancellationToken cancellationToken = default)
    {
        await _lifecycleLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            var issue = GetRuntimeIssue();
            if (issue is not null)
            {
                _devices = Array.Empty<AndroidDevice>();
                SetDeviceRefreshFailureStatus(ReceiverState.DependencyMissing, issue);
                await _logger.WriteAsync(LogLevel.Error, issue, cancellationToken);
                return _devices;
            }

            try
            {
                var result = await _commandRunner.RunAsync(
                    CreateAdbStartInfo(Path.Combine(_engineDirectory, "adb.exe"), _engineDirectory),
                    AdbCommandTimeout,
                    cancellationToken);

                if (result.ExitCode != 0)
                {
                    var detail = FirstNonEmptyLine(result.StandardError, result.StandardOutput);
                    var message = string.IsNullOrWhiteSpace(detail)
                        ? $"ADB 裝置查詢失敗，結束代碼：{result.ExitCode}。"
                        : $"ADB 裝置查詢失敗：{detail}";
                    _devices = Array.Empty<AndroidDevice>();
                    SetDeviceRefreshFailureStatus(ReceiverState.Faulted, message);
                    await _logger.WriteAsync(LogLevel.Error, message, cancellationToken);
                    return _devices;
                }

                _devices = ParseAdbDevices(result.StandardOutput);
                if (_process is { HasExited: false })
                {
                    SetStatus(ReceiverState.Running, "Android 投影仍在執行；裝置清單已更新。");
                }
                else
                {
                    var message = GetDiscoveryMessage(_devices);
                    SetStatus(ReceiverState.Stopped, message);
                    await _logger.WriteAsync(LogLevel.Information, message, cancellationToken);
                }

                return _devices;
            }
            catch (Exception exception) when (exception is InvalidOperationException or Win32Exception or IOException or UnauthorizedAccessException or TimeoutException)
            {
                var message = $"無法查詢 Android 裝置：{exception.Message}";
                _devices = Array.Empty<AndroidDevice>();
                SetDeviceRefreshFailureStatus(ReceiverState.Faulted, message);
                await _logger.WriteAsync(LogLevel.Error, message, cancellationToken);
                return _devices;
            }
        }
        finally
        {
            _lifecycleLock.Release();
        }
    }

    public async Task StartAsync(
        string deviceSerial,
        bool fullscreen = false,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(deviceSerial);
        await _lifecycleLock.WaitAsync(cancellationToken);
        try
        {
            if (_process is { HasExited: false })
            {
                return;
            }

            if (_process is not null)
            {
                _process.Dispose();
                _process = null;
            }

            SetStatus(ReceiverState.Starting, "正在啟動 Android 投影。請留意手機上的偵錯授權提示。 ");
            var issue = GetRuntimeIssue();
            if (issue is not null)
            {
                SetStatus(ReceiverState.DependencyMissing, issue);
                await _logger.WriteAsync(LogLevel.Error, issue, cancellationToken);
                return;
            }

            var device = _devices.FirstOrDefault(candidate =>
                candidate.IsReady && string.Equals(candidate.Serial, deviceSerial, StringComparison.Ordinal));
            if (device is null)
            {
                const string message = "找不到可用的 Android 裝置。請重新整理清單，並確認手機已授權 USB 偵錯。";
                SetStatus(ReceiverState.Faulted, message);
                await _logger.WriteAsync(LogLevel.Warning, message, cancellationToken);
                return;
            }

            var process = _processFactory.Create();
            process.OutputReceived += message => LogProcessMessage(LogLevel.Information, message);
            process.ErrorReceived += message => LogProcessMessage(LogLevel.Warning, message);
            process.Exited += (_, _) => OnProcessExited(process);

            _stopRequested = false;
            bool started;
            try
            {
                started = process.Start(CreateScrcpyStartInfo(
                    Path.Combine(_engineDirectory, "scrcpy.exe"),
                    _engineDirectory,
                    device.Serial,
                    fullscreen));
            }
            catch
            {
                process.Dispose();
                throw;
            }

            if (!started)
            {
                process.Dispose();
                const string message = "Windows 無法啟動 scrcpy。";
                SetStatus(ReceiverState.Faulted, message);
                await _logger.WriteAsync(LogLevel.Error, message, cancellationToken);
                return;
            }

            _process = process;
            process.BeginOutputReadLine();
            process.BeginErrorReadLine();
            SetStatus(ReceiverState.Running, $"正在投影 Android 裝置「{device.DisplayName}」。");
            await _logger.WriteAsync(LogLevel.Information, $"scrcpy 已啟動，裝置序號為 {device.Serial}。", cancellationToken);
        }
        catch (Exception exception) when (exception is InvalidOperationException or Win32Exception or IOException or UnauthorizedAccessException)
        {
            var message = $"Android 投影啟動失敗：{exception.Message}";
            SetStatus(ReceiverState.Faulted, message);
            await _logger.WriteAsync(LogLevel.Error, message, cancellationToken);
        }
        finally
        {
            _lifecycleLock.Release();
        }
    }

    public async Task StopAsync(CancellationToken cancellationToken = default)
    {
        await _lifecycleLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            var process = _process;
            if (process is null || process.HasExited)
            {
                _process = null;
                process?.Dispose();
                if (State != ReceiverState.DependencyMissing)
                {
                    SetStatus(ReceiverState.Stopped, "Android 投影已停止。");
                }

                return;
            }

            _stopRequested = true;
            SetStatus(ReceiverState.Stopping, "正在停止 Android 投影。 ");
            try
            {
                if (!process.CloseMainWindow())
                {
                    process.Kill(entireProcessTree: true);
                }

                try
                {
                    await process.WaitForExitAsync(cancellationToken)
                        .WaitAsync(TimeSpan.FromSeconds(5), cancellationToken)
                        .ConfigureAwait(false);
                }
                catch (TimeoutException)
                {
                    process.Kill(entireProcessTree: true);
                    await process.WaitForExitAsync(cancellationToken).ConfigureAwait(false);
                }
            }
            catch (InvalidOperationException)
            {
                // The process can exit between the state check and the close request.
            }
            catch (Win32Exception exception)
            {
                await _logger.WriteAsync(
                    LogLevel.Warning,
                    $"無法正常停止 scrcpy，已嘗試結束程序：{exception.Message}",
                    cancellationToken).ConfigureAwait(false);
                if (!process.HasExited)
                {
                    process.Kill(entireProcessTree: true);
                    await process.WaitForExitAsync(cancellationToken).ConfigureAwait(false);
                }
            }
            finally
            {
                process.Dispose();
                _process = null;
                SetStatus(ReceiverState.Stopped, "Android 投影已停止。");
            }

            await _logger.WriteAsync(LogLevel.Information, "scrcpy 已停止。", cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _lifecycleLock.Release();
        }
    }

    public void Dispose()
    {
        try
        {
            StopAsync().GetAwaiter().GetResult();
        }
        catch (Exception exception) when (exception is InvalidOperationException or Win32Exception)
        {
            _ = _logger.WriteAsync(LogLevel.Error, $"關閉時停止 scrcpy 失敗：{exception.Message}");
        }
    }

    public async ValueTask DisposeAsync()
    {
        await StopAsync();
    }

    internal static IReadOnlyList<AndroidDevice> ParseAdbDevices(string output)
    {
        ArgumentNullException.ThrowIfNull(output);
        var devices = new List<AndroidDevice>();
        using var reader = new StringReader(output);
        while (reader.ReadLine() is { } line)
        {
            var columns = line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
            if (columns.Length < 2
                || columns[0].Equals("List", StringComparison.OrdinalIgnoreCase)
                || columns[0].StartsWith('*'))
            {
                continue;
            }

            var status = columns[1] switch
            {
                "device" => AndroidDeviceStatus.Ready,
                "unauthorized" => AndroidDeviceStatus.Unauthorized,
                "offline" => AndroidDeviceStatus.Offline,
                _ => AndroidDeviceStatus.Unknown
            };
            var model = columns.Skip(2)
                .FirstOrDefault(column => column.StartsWith("model:", StringComparison.OrdinalIgnoreCase))?
                .Substring("model:".Length)
                .Replace('_', ' ');

            devices.Add(new AndroidDevice(columns[0], string.IsNullOrWhiteSpace(model) ? columns[0] : model, status));
        }

        return devices;
    }

    internal static ProcessStartInfo CreateAdbStartInfo(string executablePath, string engineDirectory)
    {
        var startInfo = CreateStartInfo(executablePath, engineDirectory);
        startInfo.ArgumentList.Add("devices");
        startInfo.ArgumentList.Add("-l");
        startInfo.RedirectStandardOutput = true;
        startInfo.RedirectStandardError = true;
        startInfo.CreateNoWindow = true;
        return startInfo;
    }

    internal static ProcessStartInfo CreateScrcpyStartInfo(
        string executablePath,
        string engineDirectory,
        string deviceSerial,
        bool fullscreen)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(deviceSerial);
        var startInfo = CreateStartInfo(executablePath, engineDirectory);
        startInfo.ArgumentList.Add("--serial");
        startInfo.ArgumentList.Add(deviceSerial);
        startInfo.ArgumentList.Add("--window-title");
        startInfo.ArgumentList.Add($"Neo AirPlay - Android ({deviceSerial})");
        if (fullscreen)
        {
            startInfo.ArgumentList.Add("--fullscreen");
        }

        startInfo.RedirectStandardOutput = true;
        startInfo.RedirectStandardError = true;
        startInfo.CreateNoWindow = true;
        return startInfo;
    }

    private string? GetRuntimeIssue()
    {
        var requiredFiles = RequiredFiles;
        if (requiredFiles is not { Length: > 0 })
        {
            return "Android 投影元件檔案清單未隨程式提供。請重新建置應用程式。";
        }

        var missingFiles = requiredFiles
            .Where(file => !File.Exists(Path.Combine(_engineDirectory, file)))
            .ToArray();
        if (missingFiles.Length == 0)
        {
            return null;
        }

        return $"Android 投影元件不完整，缺少：{string.Join("、", missingFiles)}。請執行 scripts\\Stage-ScrcpyRuntime.ps1。";
    }

    private static string[]? LoadRequiredFiles()
    {
        using var stream = typeof(ScrcpyAndroidMirrorEngine).Assembly.GetManifestResourceStream(RequiredFilesResourceName);
        if (stream is null)
        {
            return null;
        }

        using var reader = new StreamReader(stream);
        var files = reader.ReadToEnd()
            .Split(["\r\n", "\n"], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (files.Length == 0
            || files.Any(file => !string.Equals(Path.GetFileName(file), file, StringComparison.Ordinal))
            || files.Distinct(StringComparer.OrdinalIgnoreCase).Count() != files.Length)
        {
            return null;
        }

        return files;
    }

    private void SetDeviceRefreshFailureStatus(ReceiverState failureState, string message)
    {
        if (_process is { HasExited: false })
        {
            SetStatus(ReceiverState.Running, $"Android 投影仍在執行，但無法更新裝置清單：{message}");
            return;
        }

        SetStatus(failureState, message);
    }

    private static ProcessStartInfo CreateStartInfo(string executablePath, string engineDirectory)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = executablePath,
            WorkingDirectory = engineDirectory,
            UseShellExecute = false
        };
        var currentPath = Environment.GetEnvironmentVariable("PATH") ?? string.Empty;
        startInfo.Environment["PATH"] = string.Join(
            Path.PathSeparator,
            new[] { engineDirectory, currentPath }.Where(value => !string.IsNullOrWhiteSpace(value)));
        return startInfo;
    }

    private static string GetDiscoveryMessage(IReadOnlyList<AndroidDevice> devices)
    {
        var readyCount = devices.Count(device => device.IsReady);
        if (readyCount > 0)
        {
            return $"找到 {readyCount} 台可投影的 Android 裝置。";
        }

        if (devices.Any(device => device.Status == AndroidDeviceStatus.Unauthorized))
        {
            return "Android 裝置尚未授權。請解鎖手機並允許 USB 偵錯。";
        }

        if (devices.Any(device => device.Status == AndroidDeviceStatus.Offline))
        {
            return "ADB 找到離線裝置。請重新連接 USB，或確認手機與電腦的無線偵錯連線。";
        }

        return "目前沒有可投影的 Android 裝置。請連接 USB，或連上已配對的無線偵錯裝置。";
    }

    private static string FirstNonEmptyLine(params string[] values) =>
        values.SelectMany(value => value.Split(["\r\n", "\n"], StringSplitOptions.RemoveEmptyEntries))
            .Select(line => line.Trim())
            .FirstOrDefault(line => line.Length > 0) ?? string.Empty;

    private void LogProcessMessage(LogLevel level, string? message)
    {
        if (!string.IsNullOrWhiteSpace(message))
        {
            _ = _logger.WriteAsync(level, $"scrcpy: {message}");
        }
    }

    private async void OnProcessExited(IReceiverProcess process)
    {
        try
        {
            await _lifecycleLock.WaitAsync();
            try
            {
                if (!ReferenceEquals(_process, process) || _stopRequested)
                {
                    return;
                }

                int exitCode;
                try
                {
                    exitCode = process.ExitCode;
                }
                catch (InvalidOperationException)
                {
                    exitCode = 0;
                }

                _process = null;
                process.Dispose();
                var state = exitCode == 0 ? ReceiverState.Stopped : ReceiverState.Faulted;
                var message = exitCode == 0
                    ? "Android 投影視窗已關閉。"
                    : $"scrcpy 已結束，結束代碼：{exitCode}。請檢查手機授權與診斷記錄。";
                SetStatus(state, message);
                await _logger.WriteAsync(
                    exitCode == 0 ? LogLevel.Information : LogLevel.Error,
                    message);
            }
            finally
            {
                _lifecycleLock.Release();
            }
        }
        catch (ObjectDisposedException)
        {
            // Application shutdown may dispose the lifecycle lock after the process exits.
        }
    }

    private void SetStatus(ReceiverState state, string message)
    {
        if (State == state && StatusMessage == message)
        {
            return;
        }

        State = state;
        StatusMessage = message;
        StatusChanged?.Invoke(this, EventArgs.Empty);
    }
}
