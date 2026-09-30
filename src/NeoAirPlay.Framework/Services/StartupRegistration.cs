using Microsoft.Win32;

namespace NeoAirPlay.Services;

public sealed class StartupRegistration : IStartupRegistration
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "NeoAirPlay";

    public void SetEnabled(bool enabled)
    {
        using var runKey = Registry.CurrentUser.CreateSubKey(RunKeyPath, writable: true)
            ?? throw new InvalidOperationException("無法開啟 Windows 登入啟動設定。");

        if (!enabled)
        {
            runKey.DeleteValue(ValueName, throwOnMissingValue: false);
            return;
        }

        var executablePath = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(executablePath) || !File.Exists(executablePath))
        {
            throw new InvalidOperationException("無法確認 Neo AirPlay 執行檔路徑。");
        }

        runKey.SetValue(ValueName, $"\"{executablePath}\" --background --start", RegistryValueKind.String);
    }
}
