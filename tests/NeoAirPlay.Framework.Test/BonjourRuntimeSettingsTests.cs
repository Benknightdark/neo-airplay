using System.Buffers.Binary;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Tests.Framework;

[Trait("Category", "Unit")]
public sealed class BonjourRuntimeSettingsTests
{
    [Fact]
    public void GetBonjourRuntimeDirectory_UsesTheEmbeddedSharedPathSetting()
    {
        var expected = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            "NeoAirPlay",
            "Bonjour");

        Assert.Equal(expected, BonjourServiceInspector.GetBonjourRuntimeDirectory());
    }

    [Fact]
    public void GetBonjourServiceBackupPath_UsesTheEmbeddedSharedPathSetting()
    {
        var expected = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData),
            "NeoAirPlay",
            "BonjourServiceBackup.json");

        Assert.Equal(expected, BonjourServiceInspector.GetBonjourServiceBackupPath());
    }

    [Fact]
    public void GetBonjourMissingCompatibleX64DnsSdMessage_UsesTheEmbeddedSharedMessage()
    {
        Assert.Equal(
            "已找到 Bonjour Service 執行檔，但沒有找到相同版本的 x64 DNS-SD 元件（dnssd.dll）。",
            BonjourServiceInspector.GetBonjourMissingCompatibleX64DnsSdMessage());
    }

    [Fact]
    public void GetBonjourSourceExecutablePaths_UsesEmbeddedProgramFilesPaths()
    {
        var programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        var programFilesX86 = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86);
        var expected = new[]
        {
            Path.Combine(programFiles, "Bonjour", "mDNSResponder.exe"),
            Path.Combine(programFiles, "Bonjour Print Services", "mDNSResponder.exe"),
            Path.Combine(programFilesX86, "Bonjour", "mDNSResponder.exe"),
            Path.Combine(programFilesX86, "Bonjour Print Services", "mDNSResponder.exe")
        };

        Assert.Equal(expected, BonjourServiceInspector.GetBonjourSourceExecutablePaths());
    }

    [Fact]
    public void GetBonjourSourceDnsSdPaths_UsesX64ProgramFilesAndSystem32Candidates()
    {
        var programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        var systemDirectory = Environment.GetFolderPath(Environment.SpecialFolder.System);
        var expected = new[]
        {
            Path.Combine(programFiles, "Bonjour", "dnssd.dll"),
            Path.Combine(programFiles, "Bonjour Print Services", "dnssd.dll"),
            Path.Combine(programFiles, "NeoAirPlay", "Bonjour", "dnssd.dll"),
            Path.Combine(systemDirectory, "dnssd.dll")
        };

        Assert.Equal(expected, BonjourServiceInspector.GetBonjourSourceDnsSdPaths());
    }

    [Fact]
    public void IsCompatibleBonjourRuntimeMetadata_AcceptsX86ResponderWithMatchingX64DnsSd()
    {
        Assert.True(BonjourServiceInspector.IsCompatibleBonjourRuntimeMetadata(
            "Bonjour", "2.0.2.0", 0x014C,
            "Bonjour", "2.0.2.0", 0x8664));
    }

    [Theory]
    [InlineData("Bonjour", "2.0.2.0", 0x014C, "Bonjour", "2.0.2.0", 0x014C)]
    [InlineData("Bonjour", "2.0.2.0", 0x014C, "Bonjour", "3.0.0.0", 0x8664)]
    [InlineData("Bonjour", "2.0.2.0", 0x014C, "Other", "2.0.2.0", 0x8664)]
    [InlineData("Bonjour", "2.0.2.0", 0x01C4, "Bonjour", "2.0.2.0", 0x8664)]
    public void IsCompatibleBonjourRuntimeMetadata_RejectsWrongArchitectureOrProductVersion(
        string responderProduct,
        string responderVersion,
        int responderMachine,
        string dnsSdProduct,
        string dnsSdVersion,
        int dnsSdMachine)
    {
        Assert.False(BonjourServiceInspector.IsCompatibleBonjourRuntimeMetadata(
            responderProduct,
            responderVersion,
            (ushort)responderMachine,
            dnsSdProduct,
            dnsSdVersion,
            (ushort)dnsSdMachine));
    }

    [Theory]
    [InlineData(0x014C)]
    [InlineData(0x8664)]
    public void ReadPeMachineType_ReturnsMachineFromPeHeader(int expectedMachine)
    {
        using var stream = new MemoryStream(CreatePeImage((ushort)expectedMachine));

        Assert.Equal((ushort)expectedMachine, BonjourServiceInspector.ReadPeMachineType(stream));
    }

    [Fact]
    public void ReadPeMachineType_RejectsInvalidPeSignature()
    {
        var image = CreatePeImage(0x8664);
        image[0x40] = 0x58;
        using var stream = new MemoryStream(image);

        Assert.Null(BonjourServiceInspector.ReadPeMachineType(stream));
    }

    private static byte[] CreatePeImage(ushort machine)
    {
        var image = new byte[0x80];
        image[0] = (byte)'M';
        image[1] = (byte)'Z';
        BinaryPrimitives.WriteUInt32LittleEndian(image.AsSpan(0x3C), 0x40);
        BinaryPrimitives.WriteUInt32LittleEndian(image.AsSpan(0x40), 0x00004550);
        BinaryPrimitives.WriteUInt16LittleEndian(image.AsSpan(0x44), machine);
        return image;
    }
}
