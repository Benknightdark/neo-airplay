using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Threading;
using Application = System.Windows.Application;
using Clipboard = System.Windows.Clipboard;
using NeoAirPlay.Commands;
using NeoAirPlay.Models;
using NeoAirPlay.Services;

namespace NeoAirPlay.ViewModels;

public sealed class MainViewModel : INotifyPropertyChanged
{
    private readonly IReceiverEngine _engine;
    private readonly IAppLogger _logger;
    private readonly ISettingsStore _settingsStore;
    private readonly IStartupRegistration _startupRegistration;
    private ReceiverSettings _savedSettings;
    private ReceiverState _state;
    private string _statusMessage = string.Empty;
    private string _receiverName;
    private string _selectedRenderer;
    private bool _fullscreen;
    private bool _startOnLogin;
    private bool _settingsDirty;
    private bool _settingsApplyPending;
    private string _currentPage = "Home";
    private string _alertMessage = string.Empty;

    public MainViewModel(
        IReceiverEngine engine,
        IAppLogger logger,
        ISettingsStore settingsStore,
        IStartupRegistration startupRegistration,
        ReceiverSettings settings)
    {
        _engine = engine;
        _logger = logger;
        _settingsStore = settingsStore;
        _startupRegistration = startupRegistration;
        _savedSettings = settings;
        _state = engine.State;
        _statusMessage = engine.StatusMessage;
        _receiverName = settings.ReceiverName;
        _selectedRenderer = settings.Renderer;
        _fullscreen = settings.Fullscreen;
        _startOnLogin = settings.StartOnLogin;
        RendererOptions = ["Auto", "D3D11", "D3D12"];

        StartCommand = new AsyncRelayCommand(StartReceiverAsync, CanStart);
        StopCommand = new AsyncRelayCommand(StopReceiverAsync, CanStop);
        SaveSettingsCommand = new AsyncRelayCommand(SaveSettingsAsync, () => SettingsDirty);
        ApplySettingsCommand = new AsyncRelayCommand(ApplyPendingSettingsAsync, CanApplySettings);
        NavigateCommand = new RelayCommand(parameter =>
        {
            if (parameter is string page && page is "Home" or "Settings" or "Diagnostics")
            {
                CurrentPage = page;
            }
        });
        CopyReceiverNameCommand = new RelayCommand(_ => CopyReceiverName());
        OpenLogsCommand = new RelayCommand(_ => OpenLogDirectory());
        ExitCommand = new RelayCommand(_ => ExitRequested?.Invoke(this, EventArgs.Empty));
        MinimizeCommand = new RelayCommand(_ => MinimizeRequested?.Invoke(this, EventArgs.Empty));
        DismissAlertCommand = new RelayCommand(_ => AlertMessage = string.Empty);

        Subscribe(StartCommand);
        Subscribe(StopCommand);
        Subscribe(SaveSettingsCommand);
        Subscribe(ApplySettingsCommand);

        _engine.StatusChanged += OnEngineStatusChanged;
        _logger.EntryWritten += OnLogEntryWritten;
        _ = _logger.WriteAsync(LogLevel.Information, "Neo AirPlay 已啟動。");
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    public event EventHandler? ExitRequested;
    public event EventHandler? MinimizeRequested;

    public AsyncRelayCommand StartCommand { get; }
    public AsyncRelayCommand StopCommand { get; }
    public AsyncRelayCommand SaveSettingsCommand { get; }
    public AsyncRelayCommand ApplySettingsCommand { get; }
    public RelayCommand NavigateCommand { get; }
    public RelayCommand CopyReceiverNameCommand { get; }
    public RelayCommand OpenLogsCommand { get; }
    public RelayCommand ExitCommand { get; }
    public RelayCommand MinimizeCommand { get; }
    public RelayCommand DismissAlertCommand { get; }
    public ObservableCollection<AppLogEntry> Logs { get; } = [];
    public IReadOnlyList<string> RendererOptions { get; }

    public ReceiverState State
    {
        get => _state;
        private set
        {
            if (SetProperty(ref _state, value))
            {
                OnPropertyChanged(nameof(StateLabel));
                OnPropertyChanged(nameof(StatusTitle));
                OnPropertyChanged(nameof(StatusDescription));
                OnPropertyChanged(nameof(StatusAccent));
                OnPropertyChanged(nameof(IsReceiverRunning));
                OnPropertyChanged(nameof(IsReceiverStopped));
                NotifyCommands();
            }
        }
    }

    public string StatusMessage
    {
        get => _statusMessage;
        private set
        {
            if (SetProperty(ref _statusMessage, value))
            {
                OnPropertyChanged(nameof(StatusDescription));
            }
        }
    }

    public string ReceiverName
    {
        get => _receiverName;
        set
        {
            if (SetProperty(ref _receiverName, value))
            {
                UpdateSettingsDirty();
            }
        }
    }

    public string SelectedRenderer
    {
        get => _selectedRenderer;
        set
        {
            if (SetProperty(ref _selectedRenderer, value))
            {
                UpdateSettingsDirty();
            }
        }
    }

    public bool Fullscreen
    {
        get => _fullscreen;
        set
        {
            if (SetProperty(ref _fullscreen, value))
            {
                UpdateSettingsDirty();
            }
        }
    }

    public bool StartOnLogin
    {
        get => _startOnLogin;
        set
        {
            if (SetProperty(ref _startOnLogin, value))
            {
                UpdateSettingsDirty();
            }
        }
    }

    public bool SettingsDirty
    {
        get => _settingsDirty;
        private set
        {
            if (SetProperty(ref _settingsDirty, value))
            {
                SaveSettingsCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool SettingsApplyPending
    {
        get => _settingsApplyPending;
        private set
        {
            if (SetProperty(ref _settingsApplyPending, value))
            {
                OnPropertyChanged(nameof(ShowApplyWarning));
                ApplySettingsCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool ShowApplyWarning => SettingsApplyPending && State == ReceiverState.Running;

    public string CurrentPage
    {
        get => _currentPage;
        private set
        {
            if (SetProperty(ref _currentPage, value))
            {
                OnPropertyChanged(nameof(CurrentPageLabel));
                OnPropertyChanged(nameof(IsHomePage));
                OnPropertyChanged(nameof(IsSettingsPage));
                OnPropertyChanged(nameof(IsDiagnosticsPage));
            }
        }
    }

    public bool IsHomePage => CurrentPage == "Home";
    public bool IsSettingsPage => CurrentPage == "Settings";
    public bool IsDiagnosticsPage => CurrentPage == "Diagnostics";
    public string CurrentPageLabel => CurrentPage switch
    {
        "Settings" => "設定",
        "Diagnostics" => "診斷記錄",
        _ => "總覽"
    };
    public bool IsReceiverRunning => State == ReceiverState.Running;
    public bool IsReceiverStopped => State is ReceiverState.Stopped or ReceiverState.Faulted or ReceiverState.DependencyMissing;

    public string AlertMessage
    {
        get => _alertMessage;
        private set
        {
            if (SetProperty(ref _alertMessage, value))
            {
                OnPropertyChanged(nameof(HasAlert));
            }
        }
    }

    public bool HasAlert => !string.IsNullOrWhiteSpace(AlertMessage);

    public string StateLabel => State switch
    {
        ReceiverState.Starting => "正在啟動",
        ReceiverState.Running => "接收器已啟動",
        ReceiverState.Stopping => "正在停止",
        ReceiverState.Faulted => "啟動失敗",
        ReceiverState.DependencyMissing => "需要設定引擎",
        _ => "尚未啟動"
    };

    public string StatusTitle => State switch
    {
        ReceiverState.Starting => "正在準備接收器",
        ReceiverState.Running => "AirPlay 接收器已啟動",
        ReceiverState.Stopping => "正在停止接收器",
        ReceiverState.Faulted => "接收器遇到問題",
        ReceiverState.DependencyMissing => "尚未備妥必要元件",
        _ => "準備好接收畫面與音訊"
    };

    public string StatusDescription => State switch
    {
        ReceiverState.Starting => "正在啟動 UxPlay 與影音元件，請稍候。",
        ReceiverState.Running => $"在 Apple 裝置的 AirPlay 清單中選擇「{ReceiverName}」。影音會顯示在獨立播放視窗。",
        ReceiverState.Stopping => "正在關閉接收程序。",
        ReceiverState.Faulted or ReceiverState.DependencyMissing => StatusMessage,
        _ => "啟動後，iPhone、iPad 或 Mac 就能將畫面或音訊傳送到這台電腦。"
    };

    public string StatusAccent => State switch
    {
        ReceiverState.Running => "#17A773",
        ReceiverState.Starting or ReceiverState.Stopping => "#D78016",
        ReceiverState.Faulted or ReceiverState.DependencyMissing => "#D34444",
        _ => "#8A99AD"
    };

    public string SettingsHint => State == ReceiverState.Running
        ? "變更接收器名稱或畫面選項後，儲存並重新啟動接收器即可套用。"
        : "設定會在下次啟動接收器時生效。";

    public async Task StartReceiverAsync()
    {
        AlertMessage = string.Empty;
        await _engine.StartAsync(ToOptions(CurrentSettings()));
        State = _engine.State;
        StatusMessage = _engine.StatusMessage;
        if (State == ReceiverState.Running)
        {
            SettingsApplyPending = false;
        }
        if (State is ReceiverState.Faulted or ReceiverState.DependencyMissing)
        {
            AlertMessage = StatusMessage;
        }
    }

    public async Task StopReceiverAsync()
    {
        AlertMessage = string.Empty;
        await _engine.StopAsync();
        State = _engine.State;
        StatusMessage = _engine.StatusMessage;
    }

    private async Task SaveSettingsAsync()
    {
        AlertMessage = string.Empty;
        if (string.IsNullOrWhiteSpace(ReceiverName))
        {
            AlertMessage = "請輸入 AirPlay 顯示名稱。";
            return;
        }

        var updatedSettings = CurrentSettings() with { ReceiverName = ReceiverName.Trim() };
        var engineOptionsChanged = ToOptions(updatedSettings) != ToOptions(_savedSettings);
        var oldStartOnLogin = _savedSettings.StartOnLogin;

        try
        {
            _startupRegistration.SetEnabled(updatedSettings.StartOnLogin);
            await _settingsStore.SaveAsync(updatedSettings);
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException or InvalidOperationException or System.Security.SecurityException)
        {
            try
            {
                _startupRegistration.SetEnabled(oldStartOnLogin);
            }
            catch (Exception rollbackException) when (rollbackException is IOException or UnauthorizedAccessException or InvalidOperationException or System.Security.SecurityException)
            {
                await _logger.WriteAsync(LogLevel.Error, $"還原登入啟動設定失敗：{rollbackException.Message}");
            }

            AlertMessage = $"無法儲存設定：{exception.Message}";
            await _logger.WriteAsync(LogLevel.Error, AlertMessage);
            return;
        }

        _savedSettings = updatedSettings;
        ReceiverName = updatedSettings.ReceiverName;
        SettingsDirty = false;
        SettingsApplyPending = engineOptionsChanged && State == ReceiverState.Running;
        await _logger.WriteAsync(LogLevel.Information, "設定已儲存。");
        AlertMessage = SettingsApplyPending
            ? "設定已儲存。接收器仍在使用舊設定，請按「重新啟動並套用」。"
            : "設定已儲存。";
    }

    private async Task ApplyPendingSettingsAsync()
    {
        if (!SettingsApplyPending)
        {
            return;
        }

        AlertMessage = string.Empty;
        await _engine.StopAsync();
        await _engine.StartAsync(ToOptions(_savedSettings));
        State = _engine.State;
        StatusMessage = _engine.StatusMessage;
        SettingsApplyPending = State != ReceiverState.Running;
        if (State is ReceiverState.Faulted or ReceiverState.DependencyMissing)
        {
            AlertMessage = StatusMessage;
        }
    }

    private ReceiverSettings CurrentSettings() => new()
    {
        ReceiverName = ReceiverName,
        Renderer = SelectedRenderer,
        Fullscreen = Fullscreen,
        StartOnLogin = StartOnLogin
    };

    private static ReceiverOptions ToOptions(ReceiverSettings settings) =>
        new(settings.ReceiverName, settings.Renderer, settings.Fullscreen);

    private void UpdateSettingsDirty()
    {
        SettingsDirty = CurrentSettings() != _savedSettings;
        if (SettingsDirty)
        {
            AlertMessage = string.Empty;
        }
    }

    private bool CanStart() => State is ReceiverState.Stopped or ReceiverState.Faulted or ReceiverState.DependencyMissing;
    private bool CanStop() => State is ReceiverState.Starting or ReceiverState.Running;
    private bool CanApplySettings() => SettingsApplyPending && State == ReceiverState.Running;

    private void OnEngineStatusChanged(object? sender, EventArgs e)
    {
        var dispatcher = Application.Current?.Dispatcher;
        if (dispatcher is null || dispatcher.CheckAccess())
        {
            RefreshEngineStatus();
            return;
        }

        dispatcher.BeginInvoke(DispatcherPriority.DataBind, RefreshEngineStatus);
    }

    private void RefreshEngineStatus()
    {
        State = _engine.State;
        StatusMessage = _engine.StatusMessage;
        if (State is ReceiverState.Faulted or ReceiverState.DependencyMissing)
        {
            AlertMessage = StatusMessage;
        }

        OnPropertyChanged(nameof(ShowApplyWarning));
        OnPropertyChanged(nameof(SettingsHint));
        NotifyCommands();
    }

    private void OnLogEntryWritten(object? sender, AppLogEntry entry)
    {
        var dispatcher = Application.Current?.Dispatcher;
        if (dispatcher is null || dispatcher.CheckAccess())
        {
            AddLogEntry(entry);
            return;
        }

        dispatcher.BeginInvoke(DispatcherPriority.Background, () => AddLogEntry(entry));
    }

    private void AddLogEntry(AppLogEntry entry)
    {
        Logs.Insert(0, entry);
        while (Logs.Count > 300)
        {
            Logs.RemoveAt(Logs.Count - 1);
        }
    }

    private void CopyReceiverName()
    {
        try
        {
            Clipboard.SetText(ReceiverName);
            AlertMessage = "AirPlay 名稱已複製。";
        }
        catch (System.Runtime.InteropServices.ExternalException)
        {
            AlertMessage = "目前無法使用剪貼簿，請稍後再試。";
        }
    }

    private void OpenLogDirectory()
    {
        try
        {
            Directory.CreateDirectory(_logger.LogDirectory);
            Process.Start(new ProcessStartInfo
            {
                FileName = _logger.LogDirectory,
                UseShellExecute = true
            });
        }
        catch (Exception exception) when (exception is IOException or InvalidOperationException or System.ComponentModel.Win32Exception or UnauthorizedAccessException)
        {
            AlertMessage = $"無法開啟記錄資料夾：{exception.Message}";
        }
    }

    private void Subscribe(AsyncRelayCommand command)
    {
        command.ExecutionFailed += exception =>
        {
            AlertMessage = exception.Message;
            _ = _logger.WriteAsync(LogLevel.Error, exception.ToString());
        };
    }

    private void NotifyCommands()
    {
        StartCommand.NotifyCanExecuteChanged();
        StopCommand.NotifyCanExecuteChanged();
        ApplySettingsCommand.NotifyCanExecuteChanged();
        OnPropertyChanged(nameof(ShowApplyWarning));
        OnPropertyChanged(nameof(SettingsHint));
    }

    private bool SetProperty<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }

        field = value;
        OnPropertyChanged(propertyName);
        return true;
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null) =>
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
}
