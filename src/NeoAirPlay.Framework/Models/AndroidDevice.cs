namespace NeoAirPlay.Models;

public enum AndroidDeviceStatus
{
    Ready,
    Unauthorized,
    Offline,
    Unknown
}

public sealed record AndroidDevice(string Serial, string Model, AndroidDeviceStatus Status)
{
    public bool IsReady => Status == AndroidDeviceStatus.Ready;

    public string DisplayName
    {
        get
        {
            var state = Status switch
            {
                AndroidDeviceStatus.Ready => "已連線",
                AndroidDeviceStatus.Unauthorized => "等待手機授權",
                AndroidDeviceStatus.Offline => "離線",
                _ => "狀態未知"
            };

            return string.Equals(Model, Serial, StringComparison.OrdinalIgnoreCase)
                ? $"{Serial} · {state}"
                : $"{Model} ({Serial}) · {state}";
        }
    }
}
