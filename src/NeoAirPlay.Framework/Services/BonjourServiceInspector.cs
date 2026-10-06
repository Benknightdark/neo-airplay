using System.Diagnostics;
using System.Buffers.Binary;
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
    private const ushort PeMachineI386 = 0x014C;
    private const ushort PeMachineAmd64 = 0x8664;
    private const uint PeSignature = 0x00004550;
    private const int DosHeaderPeOffset = 0x3C;

    public BonjourRuntime? GetRuntime(out string? problem)
    {
        problem = null;
        var runtimeDirectory = GetBonjourRuntimeDirectory();
        var expectedResponder = Path.Combine(runtimeDirectory, "mDNSResponder.exe");
        var expectedDnsSd = Path.Combine(runtimeDirectory, "dnssd.dll");

        if (!IsCompatibleBonjourRuntimePair(expectedResponder, expectedDnsSd))
        {
            problem = "系統尚未備妥 Neo AirPlay 的 Bonjour 元件。請重新啟動 App，並在提示時允許啟用 Bonjour Service。";
            return null;
        }

        using var serviceKey = Registry.LocalMachine.OpenSubKey(ServiceRegistryPath);
        if (serviceKey is null)
        {
            problem = "找不到 Windows 的 Bonjour Service。請先安裝 Bonjour，或讓 Neo AirPlay 使用可信任的本機 Bonjour 檔案建立服務。";
            return null;
        }

        var registeredPath = serviceKey.GetValue("ImagePath") as string;
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
            var startValue = serviceKey.GetValue("Start");
            if (startValue is not null && Convert.ToInt32(startValue) == 4)
            {
                problem = "Bonjour Service 在 Windows 中為停用狀態；Neo AirPlay 不會自動變更這項設定。";
            }
            else
            {
                problem = "Bonjour Service 尚未執行。";
            }

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

    internal static string GetBonjourMissingCompatibleX64DnsSdMessage()
    {
        using var stream = typeof(BonjourServiceInspector).Assembly
            .GetManifestResourceStream(BonjourServiceSettingsResourceName)
            ?? throw new InvalidOperationException("BonjourServiceSettings.json is missing from the Framework assembly.");
        using var document = JsonDocument.Parse(stream);
        if (!document.RootElement.TryGetProperty("MissingCompatibleX64DnsSdMessage", out var messageElement)
            || messageElement.ValueKind != JsonValueKind.String
            || string.IsNullOrWhiteSpace(messageElement.GetString()))
        {
            throw new InvalidDataException("BonjourServiceSettings.json is missing MissingCompatibleX64DnsSdMessage.");
        }

        return messageElement.GetString()!;
    }

    internal static IReadOnlyList<string> GetBonjourSourceExecutablePaths()
    {
        using var stream = typeof(BonjourServiceInspector).Assembly
            .GetManifestResourceStream(BonjourServiceSettingsResourceName)
            ?? throw new InvalidOperationException("BonjourServiceSettings.json is missing from the Framework assembly.");
        using var document = JsonDocument.Parse(stream);
        var paths = GetConfiguredSourceExecutablePaths(
                document.RootElement,
                "ProgramFilesRelativeSourceExecutablePaths",
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles))
            .Concat(GetConfiguredSourceExecutablePaths(
                document.RootElement,
                "ProgramFilesX86RelativeSourceExecutablePaths",
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86)))
            .ToArray();
        if (paths.Length == 0)
        {
            throw new InvalidDataException("BonjourServiceSettings.json has no Bonjour source executable paths.");
        }

        return paths;
    }

    internal static IReadOnlyList<string> GetBonjourSourceDnsSdPaths()
    {
        var programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        var dnsSdPaths = GetBonjourSourceExecutablePaths()
            .Where(path => IsUnderRoot(path, programFiles))
            .Select(path => Path.Combine(Path.GetDirectoryName(path)!, "dnssd.dll"))
            .Append(Path.Combine(GetBonjourRuntimeDirectory(), "dnssd.dll"))
            .Append(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "dnssd.dll"));

        return dnsSdPaths.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
    }

    private static IEnumerable<string> GetConfiguredSourceExecutablePaths(
        JsonElement root,
        string propertyName,
        string baseDirectory)
    {
        if (string.IsNullOrWhiteSpace(baseDirectory))
        {
            return [];
        }

        if (!root.TryGetProperty(propertyName, out var pathsElement)
            || pathsElement.ValueKind != JsonValueKind.Array)
        {
            throw new InvalidDataException($"BonjourServiceSettings.json is missing {propertyName}.");
        }

        return pathsElement.EnumerateArray()
            .Select(element => element.ValueKind == JsonValueKind.String ? element.GetString() : null)
            .Select(relativePath => Path.Combine(baseDirectory, ValidateRelativePath(relativePath, propertyName)))
            .ToArray();
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

        return ValidateRelativePath(pathElement.GetString(), propertyName);
    }

    private static string ValidateRelativePath(string? relativePath, string propertyName)
    {
        if (string.IsNullOrWhiteSpace(relativePath) || Path.IsPathRooted(relativePath))
        {
            throw new InvalidDataException($"BonjourServiceSettings.json contains an invalid relative path for {propertyName}.");
        }

        var segments = relativePath.Split(['\\', '/'], StringSplitOptions.None);
        if (segments.Any(segment => string.IsNullOrEmpty(segment)
                || segment is "." or ".."
                || segment.Any(character => !char.IsAsciiLetterOrDigit(character) && character is not ('.' or '_' or '-' or ' '))))
        {
            throw new InvalidDataException($"BonjourServiceSettings.json contains an unsafe relative path for {propertyName}.");
        }

        return segments.Aggregate(string.Empty, (path, segment) =>
            path.Length == 0 ? segment : Path.Combine(path, segment));
    }

    internal bool TryGetInstalledRuntimeDirectory(out string? directory) =>
        TryGetInstalledRuntimeDirectory(out directory, out _);

    internal bool TryGetInstalledRuntimeDirectory(out string? directory, out string? problem)
    {
        directory = null;
        problem = null;
        using var serviceKey = Registry.LocalMachine.OpenSubKey(ServiceRegistryPath);
        var executablePath = ExtractExecutablePath(serviceKey?.GetValue("ImagePath") as string);

        var candidates = new List<string?>
        {
            executablePath,
            ReadBackedUpExecutablePath(),
            Path.Combine(GetBonjourRuntimeDirectory(), "mDNSResponder.exe")
        };
        candidates.AddRange(GetBonjourSourceExecutablePaths());

        var foundBonjourResponder = false;
        var dnsSdCandidates = GetBonjourSourceDnsSdPaths();
        foreach (var candidate in candidates
            .Where(path => !string.IsNullOrWhiteSpace(path))
            .Distinct(StringComparer.OrdinalIgnoreCase))
        {
            if (!TryGetBonjourResponderVersion(candidate, out _))
            {
                continue;
            }

            foundBonjourResponder = true;
            var candidateDnsSdPaths = dnsSdCandidates
                .Prepend(Path.Combine(Path.GetDirectoryName(candidate)!, "dnssd.dll"));
            var dnsSdPath = candidateDnsSdPaths.FirstOrDefault(path =>
                IsCompatibleBonjourRuntimePair(candidate!, path));
            if (dnsSdPath is null)
            {
                continue;
            }

            directory = Path.GetDirectoryName(candidate);
            return !string.IsNullOrWhiteSpace(directory);
        }

        problem = foundBonjourResponder
            ? GetBonjourMissingCompatibleX64DnsSdMessage()
            : "找不到可信任且有效的 Bonjour Service 執行檔。請依 Apple 官方說明安裝 Bonjour，再重新啟動 Neo AirPlay。";
        return false;
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

    private static bool TryGetBonjourResponderVersion(string? executablePath, out string? fileVersion)
    {
        fileVersion = null;
        if (string.IsNullOrWhiteSpace(executablePath)
            || !string.Equals(Path.GetFileName(executablePath), "mDNSResponder.exe", StringComparison.OrdinalIgnoreCase)
            || !File.Exists(executablePath)
            || !IsUnderTrustedExecutableRoot(executablePath))
        {
            return false;
        }

        try
        {
            var version = FileVersionInfo.GetVersionInfo(executablePath);
            if (!string.Equals(version.ProductName, "Bonjour", StringComparison.Ordinal)
                || string.IsNullOrWhiteSpace(version.FileVersion)
                || !TryGetPeMachineType(executablePath, out var machine)
                || machine is not (PeMachineI386 or PeMachineAmd64))
            {
                return false;
            }

            fileVersion = version.FileVersion;
            return true;
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return false;
        }
    }

    internal static bool IsCompatibleBonjourRuntimePair(
        string executablePath,
        string dnsSdPath)
    {
        if (!TryGetBonjourResponderVersion(executablePath, out _)
            || !File.Exists(dnsSdPath)
            || !IsUnderTrustedDnsSdRoot(dnsSdPath))
        {
            return false;
        }

        try
        {
            var responderVersionInfo = FileVersionInfo.GetVersionInfo(executablePath);
            var dnsSdVersion = FileVersionInfo.GetVersionInfo(dnsSdPath);
            return TryGetPeMachineType(executablePath, out var responderMachine)
                && TryGetPeMachineType(dnsSdPath, out var dnsSdMachine)
                && IsCompatibleBonjourRuntimeMetadata(
                    responderVersionInfo.ProductName,
                    responderVersionInfo.FileVersion,
                    responderMachine,
                    dnsSdVersion.ProductName,
                    dnsSdVersion.FileVersion,
                    dnsSdMachine);
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return false;
        }
    }

    internal static bool IsCompatibleBonjourRuntimeMetadata(
        string? responderProduct,
        string? responderVersion,
        ushort responderMachine,
        string? dnsSdProduct,
        string? dnsSdVersion,
        ushort dnsSdMachine) =>
        string.Equals(responderProduct, "Bonjour", StringComparison.Ordinal)
        && string.Equals(dnsSdProduct, "Bonjour", StringComparison.Ordinal)
        && !string.IsNullOrWhiteSpace(responderVersion)
        && string.Equals(responderVersion, dnsSdVersion, StringComparison.Ordinal)
        && responderMachine is PeMachineI386 or PeMachineAmd64
        && dnsSdMachine == PeMachineAmd64;

    internal static ushort? ReadPeMachineType(Stream stream)
    {
        if (!stream.CanSeek)
        {
            return null;
        }

        var originalPosition = stream.Position;
        try
        {
            if (stream.Length < DosHeaderPeOffset + sizeof(uint))
            {
                return null;
            }

            Span<byte> offsetBytes = stackalloc byte[sizeof(uint)];
            stream.Position = DosHeaderPeOffset;
            if (!ReadExactly(stream, offsetBytes))
            {
                return null;
            }

            var peOffset = BinaryPrimitives.ReadUInt32LittleEndian(offsetBytes);
            if (peOffset > stream.Length - sizeof(uint) - sizeof(ushort))
            {
                return null;
            }

            Span<byte> signatureAndMachine = stackalloc byte[sizeof(uint) + sizeof(ushort)];
            stream.Position = peOffset;
            if (!ReadExactly(stream, signatureAndMachine)
                || BinaryPrimitives.ReadUInt32LittleEndian(signatureAndMachine) != PeSignature)
            {
                return null;
            }

            return BinaryPrimitives.ReadUInt16LittleEndian(signatureAndMachine[sizeof(uint)..]);
        }
        finally
        {
            stream.Position = originalPosition;
        }
    }

    private static bool ReadExactly(Stream stream, Span<byte> destination)
    {
        var totalRead = 0;
        while (totalRead < destination.Length)
        {
            var bytesRead = stream.Read(destination[totalRead..]);
            if (bytesRead == 0)
            {
                return false;
            }

            totalRead += bytesRead;
        }

        return true;
    }

    private static bool TryGetPeMachineType(string path, out ushort machine)
    {
        machine = 0;
        try
        {
            using var stream = File.OpenRead(path);
            var result = ReadPeMachineType(stream);
            if (result is null)
            {
                return false;
            }

            machine = result.Value;
            return true;
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException or ArgumentException)
        {
            return false;
        }
    }

    private static bool IsUnderTrustedExecutableRoot(string executablePath) =>
        IsUnderRoot(executablePath, Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles))
        || IsUnderRoot(executablePath, Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86));

    private static bool IsUnderTrustedDnsSdRoot(string dnsSdPath) =>
        IsUnderRoot(dnsSdPath, Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles))
        || IsUnderRoot(dnsSdPath, Environment.GetFolderPath(Environment.SpecialFolder.System));

    private static bool IsUnderRoot(string path, string root)
    {
        if (string.IsNullOrWhiteSpace(root))
        {
            return false;
        }

        try
        {
            var fullPath = Path.GetFullPath(path);
            var normalizedRoot = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
            return fullPath.StartsWith(normalizedRoot, StringComparison.OrdinalIgnoreCase);
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
