using System.ComponentModel;
using System.Drawing;
using System.Windows;
using Application = System.Windows.Application;
using Forms = System.Windows.Forms;
using NeoAirPlay.ViewModels;

namespace NeoAirPlay;

public partial class MainWindow : Window
{
    private readonly MainViewModel _viewModel;
    private readonly Forms.NotifyIcon _trayIcon;
    private readonly bool _closeOnWindowClose;
    private bool _allowClose;
    private bool _exitInProgress;

    public MainWindow(MainViewModel viewModel, bool closeOnWindowClose = false)
    {
        _viewModel = viewModel;
        _closeOnWindowClose = closeOnWindowClose;
        DataContext = viewModel;
        InitializeComponent();

        var trayMenu = new Forms.ContextMenuStrip();
        var openItem = new Forms.ToolStripMenuItem("開啟 Neo AirPlay");
        openItem.Click += (_, _) => ShowFromTray();
        var startItem = new Forms.ToolStripMenuItem("啟動接收器");
        startItem.Click += (_, _) => _viewModel.StartCommand.Execute(null);
        var stopItem = new Forms.ToolStripMenuItem("停止接收器");
        stopItem.Click += (_, _) => _viewModel.StopCommand.Execute(null);
        var exitItem = new Forms.ToolStripMenuItem("結束");
        exitItem.Click += (_, _) => _viewModel.ExitCommand.Execute(null);
        trayMenu.Items.Add(openItem);
        trayMenu.Items.Add(new Forms.ToolStripSeparator());
        trayMenu.Items.Add(startItem);
        trayMenu.Items.Add(stopItem);
        trayMenu.Items.Add(new Forms.ToolStripSeparator());
        trayMenu.Items.Add(exitItem);

        _trayIcon = new Forms.NotifyIcon
        {
            Icon = SystemIcons.Application,
            Text = "Neo AirPlay",
            ContextMenuStrip = trayMenu,
            Visible = true
        };
        _trayIcon.MouseDoubleClick += (_, args) =>
        {
            if (args.Button == Forms.MouseButtons.Left)
            {
                ShowFromTray();
            }
        };

        _viewModel.ExitRequested += OnExitRequested;
        _viewModel.MinimizeRequested += OnMinimizeRequested;
        Closing += OnClosing;
        Closed += OnClosed;
    }

    public void HideToTray() => Hide();

    private void ShowFromTray()
    {
        Show();
        if (WindowState == WindowState.Minimized)
        {
            WindowState = WindowState.Normal;
        }

        Activate();
    }

    private void OnMinimizeRequested(object? sender, EventArgs e) => HideToTray();

    private async void OnExitRequested(object? sender, EventArgs e)
    {
        if (_exitInProgress)
        {
            return;
        }

        _exitInProgress = true;
        try
        {
            await _viewModel.StopReceiverAsync();
        }
        catch (Exception)
        {
            // App shutdown must proceed even if the engine could not stop cleanly.
        }
        finally
        {
            _allowClose = true;
            Close();
            Application.Current.Shutdown();
        }
    }

    private void OnClosing(object? sender, CancelEventArgs e)
    {
        if (_allowClose)
        {
            return;
        }

        if (_closeOnWindowClose)
        {
            e.Cancel = true;
            if (!_exitInProgress)
            {
                _viewModel.ExitCommand.Execute(null);
            }

            return;
        }

        e.Cancel = true;
        HideToTray();
    }

    private void OnClosed(object? sender, EventArgs e)
    {
        _viewModel.ExitRequested -= OnExitRequested;
        _viewModel.MinimizeRequested -= OnMinimizeRequested;
        _trayIcon.Visible = false;
        _trayIcon.Dispose();
    }
}
