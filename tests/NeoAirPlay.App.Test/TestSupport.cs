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

internal sealed class FakeAndroidMirrorEngine : IAndroidMirrorEngine
{
    public ReceiverState State { get; private set; } = ReceiverState.Stopped;
    public string StatusMessage { get; private set; } = "請連接 Android 裝置後重新整理清單。";
    public IReadOnlyList<AndroidDevice> Devices { get; private set; } = Array.Empty<AndroidDevice>();
    public IReadOnlyList<AndroidDevice> AvailableDevices { get; set; } = Array.Empty<AndroidDevice>();
    public List<string> StartedSerials { get; } = [];
    public int StopCount { get; private set; }
    public event EventHandler? StatusChanged;

    public Task<IReadOnlyList<AndroidDevice>> RefreshDevicesAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        Devices = AvailableDevices;
        StatusMessage = Devices.Count == 0 ? "目前沒有可投影的 Android 裝置。" : $"找到 {Devices.Count} 台 Android 裝置。";
        StatusChanged?.Invoke(this, EventArgs.Empty);
        return Task.FromResult(Devices);
    }

    public Task StartAsync(string deviceSerial, bool fullscreen = false, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        StartedSerials.Add(deviceSerial);
        State = ReceiverState.Running;
        StatusMessage = $"正在投影 Android 裝置「{deviceSerial}」。";
        StatusChanged?.Invoke(this, EventArgs.Empty);
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        StopCount++;
        State = ReceiverState.Stopped;
        StatusMessage = "Android 投影已停止。";
        StatusChanged?.Invoke(this, EventArgs.Empty);
        return Task.CompletedTask;
    }

    public ValueTask DisposeAsync() => ValueTask.CompletedTask;
}
