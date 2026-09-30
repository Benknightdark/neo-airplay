using NeoAirPlay.Models;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Framework.Test;

[Trait("Category", "Unit")]
public sealed class JsonSettingsStoreTests
{
    [Fact]
    public async Task LoadAsync_WhenFileDoesNotExist_ReturnsDefaults()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var store = new JsonSettingsStore(new TestLogger(), System.IO.Path.Combine(temporaryDirectory.Path, "settings.json"));

        var settings = await store.LoadAsync(TestContext.Current.CancellationToken);

        Assert.Equal("Neo AirPlay", settings.ReceiverName);
        Assert.Equal("Auto", settings.Renderer);
        Assert.False(settings.Fullscreen);
        Assert.False(settings.StartOnLogin);
    }

    [Fact]
    public async Task SaveAndLoadAsync_SanitizesSettingsAndRoundTrips()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var settingsPath = System.IO.Path.Combine(temporaryDirectory.Path, "settings.json");
        var store = new JsonSettingsStore(new TestLogger(), settingsPath);

        await store.SaveAsync(new ReceiverSettings
        {
            ReceiverName = "  Living Room  ",
            Renderer = "invalid-renderer",
            Fullscreen = true,
            StartOnLogin = true
        }, TestContext.Current.CancellationToken);

        var settings = await store.LoadAsync(TestContext.Current.CancellationToken);

        Assert.Equal("Living Room", settings.ReceiverName);
        Assert.Equal("Auto", settings.Renderer);
        Assert.True(settings.Fullscreen);
        Assert.True(settings.StartOnLogin);
    }

    [Fact]
    public async Task LoadAsync_WhenJsonIsMalformed_ReturnsDefaultsAndLogsWarning()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var logger = new TestLogger();
        var settingsPath = System.IO.Path.Combine(temporaryDirectory.Path, "settings.json");
        await File.WriteAllTextAsync(settingsPath, "{", TestContext.Current.CancellationToken);
        var store = new JsonSettingsStore(logger, settingsPath);

        var settings = await store.LoadAsync(TestContext.Current.CancellationToken);

        Assert.Equal("Neo AirPlay", settings.ReceiverName);
        Assert.Contains(logger.Entries, entry => entry.Level == LogLevel.Warning);
    }
}
