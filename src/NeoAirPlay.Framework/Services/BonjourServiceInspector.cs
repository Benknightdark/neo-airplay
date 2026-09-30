using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace NeoAirPlay.Services;

public sealed class BonjourServiceInspector : IBonjourRuntimeInspector
{
    private const string BonjourServiceName = "Bonjour Service";
    private const string ServiceRegistryPath = @"SYSTEM\CurrentControlSet\Services\" + BonjourServiceName;
    private const uint ServiceManagerConnect = 0x0001;
    private const uint ServiceQueryStatus = 0x0004;
    private const uint ServiceRunningState = 0x00000004;
    private const int ServiceStatusProcessInfo = 0;

    public BonjourRuntime? GetProjectRuntime(string engineDirectory, out string? problem)
    {
        problem = null;
        var projectBonjourDirectory = ResolveProjectBonjourDirectory(engineDirectory);
        var expectedResponder = Path.Combine(projectBonjourDirectory, "mDNSResponder.exe");
        var expectedDnsSd = Path.Combine(projectBonjourDirectory, "dnssd.dll");

        if (!File.Exists(expectedResponder) || !File.Exists(expectedDnsSd))
        {
            problem = "專案內尚未備妥 Bonjour 元件。請先完成 scripts\\build-engine.ps1。";
            return null;
        }

        using var serviceKey = Registry.LocalMachine.OpenSubKey(ServiceRegistryPath);
        var registeredPath = serviceKey?.GetValue("ImagePath") as string;
        var normalizedRegisteredPath = ExtractExecutablePath(registeredPath);
        if (string.IsNullOrWhiteSpace(normalizedRegisteredPath)
            || !PathEquals(normalizedRegisteredPath, expectedResponder))
        {
            problem = "Bonjour Service 尚未指向本專案的 engine\\bonjour\\mDNSResponder.exe。請執行 scripts\\Install-BonjourService.ps1。";
            return null;
        }

        if (!TryQueryBonjourServiceStatus(out var serviceStatus, out var errorCode))
        {
            problem = $"無法查詢 Bonjour Service 狀態（Windows 錯誤碼 {errorCode}）。";
            return null;
        }

        if (serviceStatus.CurrentState != ServiceRunningState || serviceStatus.ProcessId == 0)
        {
            problem = "專案內的 Bonjour Service 尚未執行。請從 Windows 服務管理員啟動它。";
            return null;
        }

        return new BonjourRuntime(projectBonjourDirectory);
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

    private static string ResolveProjectBonjourDirectory(string engineDirectory)
    {
        for (var current = new DirectoryInfo(Path.GetFullPath(engineDirectory)); current is not null; current = current.Parent)
        {
            var solutionMarker = Path.Combine(current.FullName, "NeoAirPlay.slnx");
            var projectRuntime = Path.Combine(current.FullName, "engine", "bonjour");
            if (File.Exists(solutionMarker) && Directory.Exists(projectRuntime))
            {
                return projectRuntime;
            }
        }

        return Path.Combine(engineDirectory, "bonjour");
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
