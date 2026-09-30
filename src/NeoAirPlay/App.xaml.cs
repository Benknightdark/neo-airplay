using System.Threading;
using System.Windows;
using Application = System.Windows.Application;
using MessageBox = System.Windows.MessageBox;
using NeoAirPlay.Services;
using NeoAirPlay.ViewModels;

namespace NeoAirPlay;

public partial class App : Application
{
    private Mutex? _instanceMutex;
    private bool _ownsMutex;
    private UxPlayProcessEngine? _engine;

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        _instanceMutex = new Mutex(initiallyOwned: true, "Local\\NeoAirPlay.SingleInstance", out _ownsMutex);
        if (!_ownsMutex)
        {
            MessageBox.Show(
                "Neo AirPlay 已經在系統匣中執行。",
                "Neo AirPlay",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
            Shutdown();
            return;
        }

        try
        {
            var logger = new FileAppLogger();
            var settingsStore = new JsonSettingsStore(logger);
            var startupRegistration = new StartupRegistration();
            var settings = await settingsStore.LoadAsync();
            var engine = new UxPlayProcessEngine(logger);
            _engine = engine;

            var viewModel = new MainViewModel(
                engine,
                logger,
                settingsStore,
                startupRegistration,
                settings);
            var window = new MainWindow(viewModel);
            MainWindow = window;
            window.Show();

            if (e.Args.Contains("--background", StringComparer.OrdinalIgnoreCase))
            {
                window.HideToTray();
            }

            if (e.Args.Contains("--start", StringComparer.OrdinalIgnoreCase))
            {
                await viewModel.StartReceiverAsync();
            }
        }
        catch (Exception exception)
        {
            MessageBox.Show(
                $"Neo AirPlay 無法啟動。\n\n{exception.Message}",
                "Neo AirPlay",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            Shutdown();
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _engine?.Dispose();
        if (_ownsMutex)
        {
            _instanceMutex?.ReleaseMutex();
        }

        _instanceMutex?.Dispose();
        base.OnExit(e);
    }
}
