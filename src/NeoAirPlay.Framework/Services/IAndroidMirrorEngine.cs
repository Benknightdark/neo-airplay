using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public interface IAndroidMirrorEngine : IAsyncDisposable
{
    ReceiverState State { get; }
    string StatusMessage { get; }
    IReadOnlyList<AndroidDevice> Devices { get; }
    event EventHandler? StatusChanged;
    Task<IReadOnlyList<AndroidDevice>> RefreshDevicesAsync(CancellationToken cancellationToken = default);
    Task StartAsync(string deviceSerial, bool fullscreen = false, CancellationToken cancellationToken = default);
    Task StopAsync(CancellationToken cancellationToken = default);
}
