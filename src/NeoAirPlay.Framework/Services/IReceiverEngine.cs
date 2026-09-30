using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public interface IReceiverEngine : IAsyncDisposable
{
    ReceiverState State { get; }
    string StatusMessage { get; }
    event EventHandler? StatusChanged;
    Task StartAsync(ReceiverOptions options, CancellationToken cancellationToken = default);
    Task StopAsync(CancellationToken cancellationToken = default);
}
