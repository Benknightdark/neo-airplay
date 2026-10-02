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

    [Fact]
    public async Task AndroidProjectionRunsAlongsideAirPlayAndStopsIndependently()
    {
        var airPlayEngine = new FakeReceiverEngine();
        await airPlayEngine.StartAsync(
            new ReceiverOptions("Neo AirPlay", "Auto", false),
            TestContext.Current.CancellationToken);
        var androidEngine = new FakeAndroidMirrorEngine
        {
            AvailableDevices =
            [
                new AndroidDevice("usb-123", "Pixel 8", AndroidDeviceStatus.Ready),
                new AndroidDevice("usb-456", "Phone", AndroidDeviceStatus.Unauthorized)
            ]
        };
        var viewModel = CreateViewModel(
            new FakeSettingsStore(new ReceiverSettings()),
            new FakeStartupRegistration(),
            engine: airPlayEngine,
            androidEngine: androidEngine);

        await viewModel.RefreshAndroidDevicesCommand.ExecuteAsync();

        Assert.Equal(2, viewModel.AndroidDevices.Count);
        Assert.Equal("usb-123", viewModel.SelectedAndroidDevice?.Serial);
        Assert.True(viewModel.StartAndroidMirrorCommand.CanExecute(null));

        await viewModel.StartAndroidMirrorCommand.ExecuteAsync();

        Assert.Equal(ReceiverState.Running, airPlayEngine.State);
        Assert.Equal(ReceiverState.Running, viewModel.AndroidState);
        Assert.Equal(["usb-123"], androidEngine.StartedSerials);

        await viewModel.StopAndroidMirrorCommand.ExecuteAsync();

        Assert.Equal(ReceiverState.Running, airPlayEngine.State);
        Assert.Equal(ReceiverState.Stopped, viewModel.AndroidState);
        Assert.Equal(1, androidEngine.StopCount);
    }

    [Fact]
    public async Task EnginePreparationDisablesEngineCommandsAndRestoresThemWhenFinished()
    {
        var androidEngine = new FakeAndroidMirrorEngine
        {
            AvailableDevices = [new AndroidDevice("usb-123", "Pixel 8", AndroidDeviceStatus.Ready)]
        };
        var viewModel = CreateViewModel(
            new FakeSettingsStore(new ReceiverSettings()),
            new FakeStartupRegistration(),
            androidEngine: androidEngine);
        await viewModel.RefreshAndroidDevicesAsync();
        var refreshCommandInvalidations = 0;
        var startAndroidCommandInvalidations = 0;
        viewModel.RefreshAndroidDevicesCommand.CanExecuteChanged += (_, _) => refreshCommandInvalidations++;
        viewModel.StartAndroidMirrorCommand.CanExecuteChanged += (_, _) => startAndroidCommandInvalidations++;

        Assert.True(viewModel.StartCommand.CanExecute(null));
        Assert.True(viewModel.RefreshAndroidDevicesCommand.CanExecute(null));
        Assert.True(viewModel.StartAndroidMirrorCommand.CanExecute(null));

        viewModel.SetEngineSetupStatus("正在準備引擎", isInProgress: true);

        Assert.False(viewModel.StartCommand.CanExecute(null));
        Assert.False(viewModel.RefreshAndroidDevicesCommand.CanExecute(null));
        Assert.False(viewModel.StartAndroidMirrorCommand.CanExecute(null));
        Assert.Equal(1, refreshCommandInvalidations);
        Assert.Equal(1, startAndroidCommandInvalidations);

        viewModel.SetEngineSetupStatus("引擎已就緒");

        Assert.True(viewModel.StartCommand.CanExecute(null));
        Assert.True(viewModel.RefreshAndroidDevicesCommand.CanExecute(null));
        Assert.True(viewModel.StartAndroidMirrorCommand.CanExecute(null));
        Assert.Equal(2, refreshCommandInvalidations);
        Assert.Equal(2, startAndroidCommandInvalidations);
    }

    [Fact]
    public async Task StopAllReceiversAsyncStopsAirPlayAndAndroid()
    {
        var airPlayEngine = new FakeReceiverEngine();
        await airPlayEngine.StartAsync(
            new ReceiverOptions("Neo AirPlay", "Auto", false),
            TestContext.Current.CancellationToken);
        var androidEngine = new FakeAndroidMirrorEngine
        {
            AvailableDevices = [new AndroidDevice("usb-123", "Pixel 8", AndroidDeviceStatus.Ready)]
        };
        var viewModel = CreateViewModel(
            new FakeSettingsStore(new ReceiverSettings()),
            new FakeStartupRegistration(),
            engine: airPlayEngine,
            androidEngine: androidEngine);
        await viewModel.RefreshAndroidDevicesAsync();
        await viewModel.StartAndroidMirrorAsync();

        await viewModel.StopAllReceiversAsync();

        Assert.Equal(1, airPlayEngine.StopCount);
        Assert.Equal(1, androidEngine.StopCount);
        Assert.Equal(ReceiverState.Stopped, viewModel.State);
        Assert.Equal(ReceiverState.Stopped, viewModel.AndroidState);
    }

    private static MainViewModel CreateViewModel(
        FakeSettingsStore settingsStore,
        FakeStartupRegistration startupRegistration,
        FakeAppLogger? logger = null,
        FakeReceiverEngine? engine = null,
        FakeAndroidMirrorEngine? androidEngine = null)
    {
        return new MainViewModel(
            engine ?? new FakeReceiverEngine(),
            androidEngine ?? new FakeAndroidMirrorEngine(),
            logger ?? new FakeAppLogger(),
            settingsStore,
            startupRegistration,
            settingsStore.SavedSettings);
    }
}
