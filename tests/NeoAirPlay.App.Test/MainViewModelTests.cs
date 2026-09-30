using NeoAirPlay.Models;
using NeoAirPlay.ViewModels;
using Xunit;

namespace NeoAirPlay.Tests.App;

[Trait("Category", "Unit")]
public sealed class MainViewModelTests
{
    [Fact]
    public async Task SaveSettingsAsync_WithBlankReceiverName_ShowsValidationAndDoesNotPersist()
    {
        var settingsStore = new FakeSettingsStore(new ReceiverSettings());
        var startupRegistration = new FakeStartupRegistration();
        var viewModel = CreateViewModel(settingsStore, startupRegistration);
        viewModel.ReceiverName = "   ";

        await viewModel.SaveSettingsCommand.ExecuteAsync();

        Assert.Equal(0, settingsStore.SaveCount);
        Assert.Empty(startupRegistration.Calls);
        Assert.Equal("請輸入 AirPlay 顯示名稱。", viewModel.AlertMessage);
    }

    [Fact]
    public async Task SaveSettingsAsync_TrimsReceiverNameAndUpdatesStartupRegistration()
    {
        var settingsStore = new FakeSettingsStore(new ReceiverSettings());
        var startupRegistration = new FakeStartupRegistration();
        var viewModel = CreateViewModel(settingsStore, startupRegistration);
        viewModel.ReceiverName = "  Living Room  ";
        viewModel.StartOnLogin = true;

        await viewModel.SaveSettingsCommand.ExecuteAsync();

        Assert.Equal("Living Room", settingsStore.SavedSettings.ReceiverName);
        Assert.True(settingsStore.SavedSettings.StartOnLogin);
        Assert.True(startupRegistration.Enabled);
        Assert.Equal([true], startupRegistration.Calls);
        Assert.False(viewModel.SettingsDirty);
    }

    [Fact]
    public async Task SaveSettingsAsync_WhenStoreFails_RestoresPreviousStartupRegistration()
    {
        var settingsStore = new FakeSettingsStore(new ReceiverSettings())
        {
            SaveError = new IOException("disk unavailable")
        };
        var startupRegistration = new FakeStartupRegistration();
        var logger = new FakeAppLogger();
        var viewModel = CreateViewModel(settingsStore, startupRegistration, logger);
        viewModel.StartOnLogin = true;

        await viewModel.SaveSettingsCommand.ExecuteAsync();

        Assert.Equal([true, false], startupRegistration.Calls);
        Assert.False(startupRegistration.Enabled);
        Assert.StartsWith("無法儲存設定：", viewModel.AlertMessage);
        Assert.Contains(logger.Entries, entry => entry.Level == LogLevel.Error);
    }

    [Fact]
    public async Task ApplySettingsAsync_RestartsRunningReceiverWithSavedOptions()
    {
        var engine = new FakeReceiverEngine();
        await engine.StartAsync(
            new ReceiverOptions("Neo AirPlay", "Auto", false),
            TestContext.Current.CancellationToken);
        var settingsStore = new FakeSettingsStore(new ReceiverSettings());
        var viewModel = CreateViewModel(settingsStore, new FakeStartupRegistration(), engine: engine);
        viewModel.SelectedRenderer = "D3D11";

        await viewModel.SaveSettingsCommand.ExecuteAsync();
        Assert.True(viewModel.SettingsApplyPending);

        await viewModel.ApplySettingsCommand.ExecuteAsync();

        Assert.Equal(1, engine.StopCount);
        Assert.Equal("D3D11", engine.StartedOptions[^1].Renderer);
        Assert.Equal(ReceiverState.Running, viewModel.State);
        Assert.False(viewModel.SettingsApplyPending);
    }

    private static MainViewModel CreateViewModel(
        FakeSettingsStore settingsStore,
        FakeStartupRegistration startupRegistration,
        FakeAppLogger? logger = null,
        FakeReceiverEngine? engine = null)
    {
        return new MainViewModel(
            engine ?? new FakeReceiverEngine(),
            logger ?? new FakeAppLogger(),
            settingsStore,
            startupRegistration,
            settingsStore.SavedSettings);
    }
}
