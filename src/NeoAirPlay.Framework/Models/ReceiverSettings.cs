namespace NeoAirPlay.Models;

public sealed record ReceiverSettings
{
    public string ReceiverName { get; init; } = "Neo AirPlay";
    public string Renderer { get; init; } = "Auto";
    public bool Fullscreen { get; init; }
    public bool StartOnLogin { get; init; }
}

public sealed record ReceiverOptions(
    string ReceiverName,
    string Renderer,
    bool Fullscreen);

public enum ReceiverState
{
    Stopped,
    Starting,
    Running,
    Stopping,
    Faulted,
    DependencyMissing
}

public enum LogLevel
{
    Information,
    Warning,
    Error
}

public sealed record AppLogEntry(DateTimeOffset Timestamp, LogLevel Level, string Message)
{
    public string DisplayTime => Timestamp.ToLocalTime().ToString("HH:mm:ss");
    public string DisplayLevel => Level switch
    {
        LogLevel.Warning => "注意",
        LogLevel.Error => "錯誤",
        _ => "資訊"
    };
}
