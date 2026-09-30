using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public interface IAppLogger
{
    string LogDirectory { get; }
    event EventHandler<AppLogEntry>? EntryWritten;
    Task WriteAsync(LogLevel level, string message, CancellationToken cancellationToken = default);
}
