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
}
