using System.Diagnostics;

namespace NeoAirPlay.Services;

internal interface IReceiverProcessFactory
{
    IReceiverProcess Create();
}

internal interface IReceiverProcess : IDisposable
{
    event Action<string?>? OutputReceived;
    event Action<string?>? ErrorReceived;
    event EventHandler? Exited;

    bool HasExited { get; }
    int ExitCode { get; }

    bool Start(ProcessStartInfo startInfo);
    void BeginOutputReadLine();
    void BeginErrorReadLine();
    bool CloseMainWindow();
    void Kill(bool entireProcessTree);
    Task WaitForExitAsync(CancellationToken cancellationToken);
}

internal sealed class SystemReceiverProcessFactory : IReceiverProcessFactory
{
    public IReceiverProcess Create() => new SystemReceiverProcess();
}

internal sealed class SystemReceiverProcess : IReceiverProcess
{
    private readonly Process _process = new() { EnableRaisingEvents = true };

    public SystemReceiverProcess()
    {
        _process.OutputDataReceived += (_, args) => OutputReceived?.Invoke(args.Data);
        _process.ErrorDataReceived += (_, args) => ErrorReceived?.Invoke(args.Data);
        _process.Exited += (_, args) => Exited?.Invoke(this, args);
    }

    public event Action<string?>? OutputReceived;
    public event Action<string?>? ErrorReceived;
    public event EventHandler? Exited;

    public bool HasExited => _process.HasExited;
    public int ExitCode => _process.ExitCode;

    public bool Start(ProcessStartInfo startInfo)
    {
        _process.StartInfo = startInfo;
        return _process.Start();
    }

    public void BeginOutputReadLine() => _process.BeginOutputReadLine();
    public void BeginErrorReadLine() => _process.BeginErrorReadLine();
    public bool CloseMainWindow() => _process.CloseMainWindow();
    public void Kill(bool entireProcessTree) => _process.Kill(entireProcessTree);
    public Task WaitForExitAsync(CancellationToken cancellationToken) => _process.WaitForExitAsync(cancellationToken);
    public void Dispose() => _process.Dispose();
}
