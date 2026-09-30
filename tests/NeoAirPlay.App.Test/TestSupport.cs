using NeoAirPlay.Models;
using NeoAirPlay.Services;

namespace NeoAirPlay.Tests.App;

internal sealed class FakeAppLogger : IAppLogger
{
    public string LogDirectory => Path.GetTempPath();
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

internal sealed class FakeSettingsStore(ReceiverSettings initialSettings) : ISettingsStore
{
    public ReceiverSettings SavedSettings { get; private set; } = initialSettings;
    public Exception? SaveError { get; set; }
    public int SaveCount { get; private set; }

    public Task<ReceiverSettings> LoadAsync(CancellationToken cancellationToken = default) =>
        Task.FromResult(SavedSettings);

    public Task SaveAsync(ReceiverSettings settings, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        SaveCount++;
        if (SaveError is not null)
        {
            return Task.FromException(SaveError);
        }

        SavedSettings = settings;
        return Task.CompletedTask;
    }
}

internal sealed class FakeStartupRegistration(bool initialValue = false) : IStartupRegistration
{
    public bool Enabled { get; private set; } = initialValue;
    public List<bool> Calls { get; } = [];
    public Exception? SetError { get; set; }

    public void SetEnabled(bool enabled)
    {
        Calls.Add(enabled);
        if (SetError is not null)
        {
            throw SetError;
        }

        Enabled = enabled;
    }
}

internal sealed class FakeReceiverEngine : IReceiverEngine
{
    public ReceiverState State { get; private set; } = ReceiverState.Stopped;
    public string StatusMessage { get; private set; } = "接收器尚未啟動。";
    public List<ReceiverOptions> StartedOptions { get; } = [];
    public int StopCount { get; private set; }
    public event EventHandler? StatusChanged;

    public Task StartAsync(ReceiverOptions options, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        StartedOptions.Add(options);
        State = ReceiverState.Running;
        StatusMessage = $"正在以「{options.ReceiverName}」接收 AirPlay。";
        StatusChanged?.Invoke(this, EventArgs.Empty);
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        StopCount++;
        State = ReceiverState.Stopped;
        StatusMessage = "接收器已停止。";
        StatusChanged?.Invoke(this, EventArgs.Empty);
        return Task.CompletedTask;
    }

    public ValueTask DisposeAsync() => ValueTask.CompletedTask;
}
