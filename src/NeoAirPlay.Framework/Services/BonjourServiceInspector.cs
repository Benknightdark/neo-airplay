using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Win32;

namespace NeoAirPlay.Services;

public sealed class BonjourServiceInspector : IBonjourRuntimeInspector
{
    private const string BonjourServiceSettingsResourceName = "NeoAirPlay.BonjourServiceSettings.json";
    private const string BonjourServiceName = "Bonjour Service";
    private const string ServiceRegistryPath = @"SYSTEM\CurrentControlSet\Services\" + BonjourServiceName;
    private const uint ServiceManagerConnect = 0x0001;
    private const uint ServiceQueryStatus = 0x0004;
    private const uint ServiceRunningState = 0x00000004;
    private const int ServiceStatusProcessInfo = 0;

    public BonjourRuntime? GetRuntime(out string? problem)
    {
        problem = null;
        var runtimeDirectory = GetBonjourRuntimeDirectory();
        var expectedResponder = Path.Combine(runtimeDirectory, "mDNSResponder.exe");
        var expectedDnsSd = Path.Combine(runtimeDirectory, "dnssd.dll");

        if (!File.Exists(expectedResponder) || !File.Exists(expectedDnsSd))
        {
            problem = "系統尚未備妥 Neo AirPlay 的 Bonjour 元件。請重新啟動 App，並在提示時允許啟用 Bonjour Service。";
            return null;
        }

        using var serviceKey = Registry.LocalMachine.OpenSubKey(ServiceRegistryPath);
        var registeredPath = serviceKey?.GetValue("ImagePath") as string;
        var normalizedRegisteredPath = ExtractExecutablePath(registeredPath);
        if (string.IsNullOrWhiteSpace(normalizedRegisteredPath)
            || !PathEquals(normalizedRegisteredPath, expectedResponder))
        {
            problem = $"Bonjour Service 尚未指向 {expectedResponder}。請重新啟動 App，並在提示時允許啟用 Bonjour Service。";
            return null;
        }

        if (!TryQueryBonjourServiceStatus(out var serviceStatus, out var errorCode))
        {
            problem = $"無法查詢 Bonjour Service 狀態（Windows 錯誤碼 {errorCode}）。";
            return null;
        }

        if (serviceStatus.CurrentState != ServiceRunningState || serviceStatus.ProcessId == 0)
        {
            problem = "Bonjour Service 尚未執行。請從 Windows 服務管理員啟動它。";
            return null;
        }

        return new BonjourRuntime(runtimeDirectory);
    }

    internal static string GetBonjourRuntimeDirectory()
    {
        var relativeDirectory = GetBonjourRelativePath("ProgramFilesRelativeDirectory");
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), relativeDirectory);
    }

    internal static string GetBonjourServiceBackupPath()
    {
        var relativePath = GetBonjourRelativePath("ProgramDataRelativeBackupPath");
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), relativePath);
    }

    private static string GetBonjourRelativePath(string propertyName)
    {
        using var stream = typeof(BonjourServiceInspector).Assembly
            .GetManifestResourceStream(BonjourServiceSettingsResourceName)
            ?? throw new InvalidOperationException("BonjourServiceSettings.json is missing from the Framework assembly.");
        using var document = JsonDocument.Parse(stream);
        if (!document.RootElement.TryGetProperty(propertyName, out var pathElement)
            || pathElement.ValueKind != System.Text.Json.JsonValueKind.String)
        {
            throw new InvalidDataException($"BonjourServiceSettings.json is missing {propertyName}.");
        }

        var relativePath = pathElement.GetString();
        if (string.IsNullOrWhiteSpace(relativePath) || Path.IsPathRooted(relativePath))
        {
            throw new InvalidDataException($"BonjourServiceSettings.json contains an invalid relative path for {propertyName}.");
        }

        var segments = relativePath.Split(['\\', '/'], StringSplitOptions.None);
        if (segments.Any(segment => string.IsNullOrEmpty(segment)
                || segment is "." or ".."
                || segment.Any(character => !char.IsAsciiLetterOrDigit(character) && character is not ('.' or '_' or '-'))))
        {
            throw new InvalidDataException($"BonjourServiceSettings.json contains an unsafe relative path for {propertyName}.");
        }

        return segments.Aggregate(string.Empty, (path, segment) =>
            path.Length == 0 ? segment : Path.Combine(path, segment));
    }

    internal bool TryGetInstalledRuntimeDirectory(out string? directory)
    {
        directory = null;
        using var serviceKey = Registry.LocalMachine.OpenSubKey(ServiceRegistryPath);
        var executablePath = ExtractExecutablePath(serviceKey?.GetValue("ImagePath") as string);
        if (!HasBonjourRuntimeFiles(executablePath))
        {
            executablePath = ReadBackedUpExecutablePath();
        }

        if (!HasBonjourRuntimeFiles(executablePath))
        {
            return false;
        }

        var candidateDirectory = Path.GetDirectoryName(executablePath);
        if (string.IsNullOrWhiteSpace(candidateDirectory))
        {
            return false;
        }

        directory = candidateDirectory;
        return true;
    }

    private static string? ReadBackedUpExecutablePath()
    {
        var backupPath = GetBonjourServiceBackupPath();
        if (!File.Exists(backupPath))
        {
            return null;
        }

        try
        {
            using var document = System.Text.Json.JsonDocument.Parse(File.ReadAllText(backupPath));
            var root = document.RootElement;
            if (!root.TryGetProperty("ServiceName", out var serviceName)
                || !string.Equals(serviceName.GetString(), BonjourServiceName, StringComparison.Ordinal)
                || !root.TryGetProperty("OriginalImagePath", out var originalImagePath))
            {
                return null;
            }

            return ExtractExecutablePath(originalImagePath.GetString());
        }
        catch (Exception exception) when (exception is IOException
            or UnauthorizedAccessException
            or InvalidOperationException
            or System.Text.Json.JsonException)
        {
            return null;
        }
    }

    private static bool HasBonjourRuntimeFiles(string? executablePath)
    {
        if (string.IsNullOrWhiteSpace(executablePath) || !File.Exists(executablePath))
        {
            return false;
        }

        var directory = Path.GetDirectoryName(executablePath);
        if (string.IsNullOrWhiteSpace(directory))
        {
            return false;
        }

        if (!IsUnderTrustedRuntimeRoot(executablePath))
        {
            return false;
        }

        var responderPath = Path.Combine(directory, "mDNSResponder.exe");
        var dnsSdPath = Path.Combine(directory, "dnssd.dll");
        if (!File.Exists(responderPath) || !File.Exists(dnsSdPath))
        {
            return false;
        }

        try
        {
            var responderVersion = FileVersionInfo.GetVersionInfo(responderPath);
            var dnsSdVersion = FileVersionInfo.GetVersionInfo(dnsSdPath);
            return string.Equals(responderVersion.ProductName, "Bonjour", StringComparison.Ordinal)
                && string.Equals(dnsSdVersion.ProductName, "Bonjour", StringComparison.Ordinal)
                && string.Equals(responderVersion.FileVersion, dnsSdVersion.FileVersion, StringComparison.Ordinal);
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return false;
        }
    }

    private static bool IsUnderTrustedRuntimeRoot(string executablePath)
    {
        try
        {
            var fullExecutablePath = Path.GetFullPath(executablePath);
            var trustedRoots = new[]
            {
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles)
            };

            return trustedRoots
                .Where(root => !string.IsNullOrWhiteSpace(root))
                .Select(root => Path.GetFullPath(root!).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar)
                .Any(root => fullExecutablePath.StartsWith(root, StringComparison.OrdinalIgnoreCase));
        }
        catch (Exception exception) when (exception is ArgumentException or IOException or NotSupportedException)
        {
            return false;
        }
    }

    private static bool TryQueryBonjourServiceStatus(out ServiceStatusProcess serviceStatus, out int errorCode)
    {
        serviceStatus = default;
        errorCode = 0;

        var managerHandle = OpenSCManager(null, null, ServiceManagerConnect);
        if (managerHandle == IntPtr.Zero)
        {
            errorCode = Marshal.GetLastWin32Error();
            return false;
        }

        try
        {
            var serviceHandle = OpenService(managerHandle, BonjourServiceName, ServiceQueryStatus);
            if (serviceHandle == IntPtr.Zero)
            {
                errorCode = Marshal.GetLastWin32Error();
                return false;
            }

            try
            {
                var succeeded = QueryServiceStatusEx(
                    serviceHandle,
                    ServiceStatusProcessInfo,
                    out serviceStatus,
                    checked((uint)Marshal.SizeOf<ServiceStatusProcess>()),
                    out _);
                if (!succeeded)
                {
                    errorCode = Marshal.GetLastWin32Error();
                }

                return succeeded;
            }
            finally
            {
                _ = CloseServiceHandle(serviceHandle);
            }
        }
        finally
        {
            _ = CloseServiceHandle(managerHandle);
        }
    }

    private static string? ExtractExecutablePath(string? imagePath)
    {
        if (string.IsNullOrWhiteSpace(imagePath))
        {
            return null;
        }

        var trimmed = Environment.ExpandEnvironmentVariables(imagePath.Trim());
        if (trimmed.StartsWith('"'))
        {
            var closingQuote = trimmed.IndexOf('"', 1);
            return closingQuote > 1 ? trimmed[1..closingQuote] : null;
        }

        var executableSuffix = trimmed.IndexOf(".exe", StringComparison.OrdinalIgnoreCase);
        return executableSuffix >= 0 ? trimmed[..(executableSuffix + 4)] : trimmed;
    }

    private static bool PathEquals(string left, string right)
    {
        try
        {
            return string.Equals(
                Path.GetFullPath(left),
                Path.GetFullPath(right),
                StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception exception) when (exception is ArgumentException or IOException or NotSupportedException)
        {
            return false;
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ServiceStatusProcess
    {
        public uint ServiceType;
        public uint CurrentState;
        public uint ControlsAccepted;
        public uint Win32ExitCode;
        public uint ServiceSpecificExitCode;
        public uint CheckPoint;
        public uint WaitHint;
        public uint ProcessId;
        public uint ServiceFlags;
    }

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr OpenSCManager(string? machineName, string? databaseName, uint desiredAccess);

    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr OpenService(IntPtr managerHandle, string serviceName, uint desiredAccess);

    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool QueryServiceStatusEx(
        IntPtr serviceHandle,
        int infoLevel,
        out ServiceStatusProcess serviceStatus,
        uint bufferSize,
        out uint bytesNeeded);

    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CloseServiceHandle(IntPtr serviceHandle);
}

public sealed record BonjourRuntime(string Directory);
