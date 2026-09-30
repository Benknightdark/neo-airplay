using System.Text;
using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public sealed class FileAppLogger : IAppLogger
{
    private readonly SemaphoreSlim _writeLock = new(1, 1);

    public FileAppLogger()
    {
        LogDirectory = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "NeoAirPlay",
            "Logs");
    }

    public string LogDirectory { get; }

    public event EventHandler<AppLogEntry>? EntryWritten;

    public Task InfoAsync(string message, CancellationToken cancellationToken = default) =>
        WriteAsync(LogLevel.Information, message, cancellationToken);

    public Task WarningAsync(string message, CancellationToken cancellationToken = default) =>
        WriteAsync(LogLevel.Warning, message, cancellationToken);

    public Task ErrorAsync(string message, CancellationToken cancellationToken = default) =>
        WriteAsync(LogLevel.Error, message, cancellationToken);

    public async Task WriteAsync(LogLevel level, string message, CancellationToken cancellationToken = default)
    {
        var entry = new AppLogEntry(DateTimeOffset.Now, level, message.Trim());
        var filePath = Path.Combine(LogDirectory, $"{entry.Timestamp:yyyy-MM-dd}.log");
        var safeMessage = entry.Message.Replace("\r", " ").Replace("\n", " ");
        var line = $"[{entry.Timestamp:yyyy-MM-dd HH:mm:ss.fff zzz}] [{level}] {safeMessage}{Environment.NewLine}";

        await _writeLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            Directory.CreateDirectory(LogDirectory);
            await File.AppendAllTextAsync(filePath, line, Encoding.UTF8, cancellationToken).ConfigureAwait(false);
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException)
        {
            // Keep the in-app diagnostic stream available even when the log file cannot be written.
        }
        finally
        {
            _writeLock.Release();
        }

        EntryWritten?.Invoke(this, entry);
    }
}
