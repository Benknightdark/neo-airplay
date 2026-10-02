using System.Diagnostics;
using NeoAirPlay.Models;
using NeoAirPlay.Services;

namespace NeoAirPlay.Framework.Test;

internal sealed class TemporaryDirectory : IDisposable
{
    public TemporaryDirectory()
    {
        Path = System.IO.Path.Combine(
            System.IO.Path.GetTempPath(),
            "NeoAirPlay.Framework.Test",
            Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public string Path { get; }

    public void Dispose()
    {
        if (Directory.Exists(Path))
        {
            Directory.Delete(Path, recursive: true);
        }
    }
}

internal sealed class TestLogger : IAppLogger
{
    public string LogDirectory => string.Empty;
    public List<(LogLevel Level, string Message)> Entries { get; } = [];
    public event EventHandler<AppLogEntry>? EntryWritten;

    public Task WriteAsync(LogLevel level, string message, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        Entries.Add((level, message));
        EntryWritten?.Invoke(this, new AppLogEntry(DateTimeOffset.Now, level, message));
        return Task.CompletedTask;
    }
}

internal sealed class FakeBonjourRuntimeInspector(BonjourRuntime? runtime, string? problem = null)
    : IBonjourRuntimeInspector
{
    public BonjourRuntime? GetRuntime(out string? issue)
    {
        issue = problem;
        return runtime;
    }
}

internal sealed class FakeReceiverProcessFactory : IReceiverProcessFactory
{
    public FakeReceiverProcess Process { get; } = new();
    public IReceiverProcess Create() => Process;
}

internal sealed class FakeReceiverProcess : IReceiverProcess
{
    public event Action<string?>? OutputReceived;
    public event Action<string?>? ErrorReceived;
    public event EventHandler? Exited;

    public bool HasExited { get; private set; }
    public int ExitCode { get; set; }
    public bool StartResult { get; set; } = true;
    public bool CloseMainWindowResult { get; set; } = true;
    public bool WasDisposed { get; private set; }
    public bool WasKilled { get; private set; }
    public ProcessStartInfo? StartInfo { get; private set; }

    public bool Start(ProcessStartInfo startInfo)
    {
        StartInfo = startInfo;
        HasExited = !StartResult;
        return StartResult;
    }

    public void BeginOutputReadLine() { }
    public void BeginErrorReadLine() { }
    public bool CloseMainWindow() => CloseMainWindowResult;

    public void Kill(bool entireProcessTree)
    {
        WasKilled = true;
        HasExited = true;
    }

    public Task WaitForExitAsync(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        HasExited = true;
        return Task.CompletedTask;
    }

    public void EmitOutput(string? message) => OutputReceived?.Invoke(message);
    public void EmitError(string? message) => ErrorReceived?.Invoke(message);

    public void Exit(int exitCode)
    {
        ExitCode = exitCode;
        HasExited = true;
        Exited?.Invoke(this, EventArgs.Empty);
    }

    public void Dispose() => WasDisposed = true;
}

internal sealed class FakeAndroidCommandRunner : IAndroidCommandRunner
{
    public AndroidCommandResult Result { get; set; } = new(0, "List of devices attached\n", string.Empty);
    public Exception? Error { get; set; }
    public ProcessStartInfo? StartInfo { get; private set; }

    public Task<AndroidCommandResult> RunAsync(
        ProcessStartInfo startInfo,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        StartInfo = startInfo;
        return Error is null ? Task.FromResult(Result) : Task.FromException<AndroidCommandResult>(Error);
    }
}
