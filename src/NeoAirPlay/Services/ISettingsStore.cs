using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public interface ISettingsStore
{
    Task<ReceiverSettings> LoadAsync(CancellationToken cancellationToken = default);
    Task SaveAsync(ReceiverSettings settings, CancellationToken cancellationToken = default);
}
