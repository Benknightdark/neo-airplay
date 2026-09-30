using System.Security.Cryptography;
using System.Text;

namespace NeoAirPlay;

internal sealed record AppDataPaths(
    string SettingsPath,
    string LogDirectory,
    string GStreamerRegistryPath,
    string MutexName,
    bool IsE2eProfile)
{
    public static AppDataPaths FromCommandLine(IReadOnlyList<string> arguments)
    {
        var profileArgumentIndex = -1;
        for (var index = 0; index < arguments.Count; index++)
        {
            if (string.Equals(arguments[index], "--e2e-profile", StringComparison.OrdinalIgnoreCase))
            {
                profileArgumentIndex = index;
                break;
            }
        }

        if (profileArgumentIndex < 0)
        {
            return CreateDefault();
        }

        if (profileArgumentIndex + 1 >= arguments.Count
            || string.IsNullOrWhiteSpace(arguments[profileArgumentIndex + 1]))
        {
            throw new ArgumentException("--e2e-profile 需要指定暫存資料夾路徑。", nameof(arguments));
        }

        var profileDirectory = Path.GetFullPath(arguments[profileArgumentIndex + 1]);
        var roamingRoot = Path.Combine(profileDirectory, "roaming", "NeoAirPlay");
        var localRoot = Path.Combine(profileDirectory, "local", "NeoAirPlay");
        var profileHash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(profileDirectory)));

        return new AppDataPaths(
            Path.Combine(roamingRoot, "settings.json"),
            Path.Combine(localRoot, "Logs"),
            Path.Combine(localRoot, "gstreamer-registry.bin"),
            $"Local\\NeoAirPlay.E2E.{profileHash}",
            IsE2eProfile: true);
    }

    private static AppDataPaths CreateDefault()
    {
        var roamingRoot = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "NeoAirPlay");
        var localRoot = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "NeoAirPlay");

        return new AppDataPaths(
            Path.Combine(roamingRoot, "settings.json"),
            Path.Combine(localRoot, "Logs"),
            Path.Combine(localRoot, "gstreamer-registry.bin"),
            "Local\\NeoAirPlay.SingleInstance",
            IsE2eProfile: false);
    }
}
