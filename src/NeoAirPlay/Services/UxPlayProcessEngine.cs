using System.Diagnostics;
using System.ComponentModel;
using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public sealed class UxPlayProcessEngine(IAppLogger logger) : IReceiverEngine, IDisposable
{
    private readonly SemaphoreSlim _lifecycleLock = new(1, 1);
    private readonly BonjourServiceInspector _bonjourInspector = new();
    private Process? _process;
    private bool _stopRequested;

    public ReceiverState State { get; private set; } = ReceiverState.Stopped;
    public string StatusMessage { get; private set; } = "接收器尚未啟動。";

    public event EventHandler? StatusChanged;

    public async Task StartAsync(ReceiverOptions options, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(options);
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

            SetStatus(ReceiverState.Starting, "正在啟動 UxPlay。");
            var engineDirectory = Path.Combine(AppContext.BaseDirectory, "engine");
            var executablePath = Path.Combine(engineDirectory, "uxplay.exe");
            if (!File.Exists(executablePath))
            {
                const string message = "找不到 engine\\uxplay.exe。請先完成 scripts\\build-engine.ps1。";
                SetStatus(ReceiverState.DependencyMissing, message);
                await logger.WriteAsync(LogLevel.Error, message, cancellationToken);
                return;
            }

            var bonjour = _bonjourInspector.GetProjectRuntime(engineDirectory, out var problem);
            if (bonjour is null)
            {
                var message = problem ?? "Bonjour Service 不可用。";
                SetStatus(ReceiverState.DependencyMissing, message);
                await logger.WriteAsync(LogLevel.Error, message, cancellationToken);
                return;
            }

            var runtimeDirectory = Path.Combine(engineDirectory, "runtime", "bin");
            var pluginDirectory = Path.Combine(engineDirectory, "runtime", "lib", "gstreamer-1.0");
            if (!Directory.Exists(runtimeDirectory) || !Directory.Exists(pluginDirectory))
            {
                const string message = "找不到 engine\\runtime 的 GStreamer 元件。請先完成 scripts\\build-engine.ps1。";
                SetStatus(ReceiverState.DependencyMissing, message);
                await logger.WriteAsync(LogLevel.Error, message, cancellationToken);
                return;
            }

            var process = new Process
            {
                StartInfo = CreateStartInfo(
                    executablePath,
                    engineDirectory,
                    runtimeDirectory,
                    pluginDirectory,
                    bonjour.Directory,
                    options),
                EnableRaisingEvents = true
            };
            process.OutputDataReceived += (_, args) =>
            {
                if (!string.IsNullOrWhiteSpace(args.Data))
                {
                    _ = logger.WriteAsync(LogLevel.Information, $"UxPlay: {args.Data}");
                }
            };
            process.ErrorDataReceived += (_, args) =>
            {
                if (!string.IsNullOrWhiteSpace(args.Data))
                {
                    _ = logger.WriteAsync(LogLevel.Warning, $"UxPlay: {args.Data}");
                }
            };
            process.Exited += (_, _) => OnProcessExited(process);

            _stopRequested = false;
            bool started;
            try
            {
                started = process.Start();
            }
            catch
            {
                process.Dispose();
                throw;
            }

            if (!started)
            {
                process.Dispose();
                SetStatus(ReceiverState.Faulted, "Windows 無法啟動 UxPlay。");
                await logger.WriteAsync(LogLevel.Error, "Windows 無法啟動 UxPlay。", cancellationToken);
                return;
            }

            _process = process;
            process.BeginOutputReadLine();
            process.BeginErrorReadLine();
            SetStatus(ReceiverState.Running, $"正在以「{options.ReceiverName}」接收 AirPlay。");
            await logger.WriteAsync(LogLevel.Information, $"UxPlay 已啟動，接收器名稱為「{options.ReceiverName}」。", cancellationToken);
        }
        catch (Exception exception) when (exception is InvalidOperationException or Win32Exception or IOException or UnauthorizedAccessException)
        {
            SetStatus(ReceiverState.Faulted, $"UxPlay 啟動失敗：{exception.Message}");
            await logger.WriteAsync(LogLevel.Error, $"UxPlay 啟動失敗：{exception.Message}", cancellationToken);
        }
        finally
        {
            _lifecycleLock.Release();
        }
    }

    public async Task StopAsync(CancellationToken cancellationToken = default)
    {
        await _lifecycleLock.WaitAsync(cancellationToken);
        try
        {
            var process = _process;
            if (process is null || process.HasExited)
            {
                _process = null;
                process?.Dispose();
                if (State != ReceiverState.DependencyMissing)
                {
                    SetStatus(ReceiverState.Stopped, "接收器尚未啟動。");
                }

                return;
            }

            _stopRequested = true;
            SetStatus(ReceiverState.Stopping, "正在停止接收器。");
            try
            {
                if (!process.CloseMainWindow())
                {
                    process.Kill(entireProcessTree: true);
                }

                try
                {
                    await process.WaitForExitAsync(cancellationToken).WaitAsync(TimeSpan.FromSeconds(5), cancellationToken);
                }
                catch (TimeoutException)
                {
                    process.Kill(entireProcessTree: true);
                    await process.WaitForExitAsync(cancellationToken);
                }
            }
            catch (InvalidOperationException)
            {
                // The process can exit between the state check and the close request.
            }
            catch (System.ComponentModel.Win32Exception exception)
            {
                await logger.WriteAsync(LogLevel.Warning, $"無法正常停止 UxPlay，已嘗試結束程序：{exception.Message}", cancellationToken);
                if (!process.HasExited)
                {
                    process.Kill(entireProcessTree: true);
                    await process.WaitForExitAsync(cancellationToken);
                }
            }
            finally
            {
                process.Dispose();
                _process = null;
                SetStatus(ReceiverState.Stopped, "接收器已停止。");
            }

            await logger.WriteAsync(LogLevel.Information, "UxPlay 已停止。", cancellationToken);
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
        catch (Exception exception) when (exception is InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            _ = logger.WriteAsync(LogLevel.Error, $"關閉時停止 UxPlay 失敗：{exception.Message}");
        }

    }

    public async ValueTask DisposeAsync()
    {
        await StopAsync();
    }

    private static ProcessStartInfo CreateStartInfo(
        string executablePath,
        string engineDirectory,
        string runtimeDirectory,
        string pluginDirectory,
        string bonjourDirectory,
        ReceiverOptions options)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = executablePath,
            WorkingDirectory = engineDirectory,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };

        startInfo.ArgumentList.Add("-n");
        startInfo.ArgumentList.Add(options.ReceiverName);
        startInfo.ArgumentList.Add("-nh");
        if (options.Fullscreen)
        {
            startInfo.ArgumentList.Add("-fs");
        }

        var renderer = options.Renderer switch
        {
            "D3D11" => "d3d11videosink",
            "D3D12" => "d3d12videosink",
            _ => null
        };
        if (renderer is not null)
        {
            startInfo.ArgumentList.Add("-vs");
            startInfo.ArgumentList.Add(renderer);
        }

        var currentPath = Environment.GetEnvironmentVariable("PATH") ?? string.Empty;
        startInfo.Environment["PATH"] = string.Join(
            Path.PathSeparator,
            new[] { engineDirectory, runtimeDirectory, bonjourDirectory, currentPath }
                .Where(value => !string.IsNullOrWhiteSpace(value)));
        startInfo.Environment["GST_PLUGIN_PATH"] = pluginDirectory;
        startInfo.Environment["GST_PLUGIN_SYSTEM_PATH_1_0"] = pluginDirectory;
        startInfo.Environment["GST_REGISTRY"] = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "NeoAirPlay",
            "gstreamer-registry.bin");
        var scanner = Path.Combine(engineDirectory, "runtime", "libexec", "gstreamer-1.0", "gst-plugin-scanner.exe");
        if (File.Exists(scanner))
        {
            startInfo.Environment["GST_PLUGIN_SCANNER"] = scanner;
        }

        return startInfo;
    }

    private async void OnProcessExited(Process process)
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
                    ? "UxPlay 已結束。"
                    : $"UxPlay 已結束，結束代碼：{exitCode}。";
                SetStatus(state, message);
                await logger.WriteAsync(
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
            // Shutdown already disposed the lifecycle lock.
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
