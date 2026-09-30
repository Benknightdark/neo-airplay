using System.Text.Json;
using NeoAirPlay.Models;

namespace NeoAirPlay.Services;

public sealed class JsonSettingsStore : ISettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNameCaseInsensitive = true
    };

    private readonly IAppLogger _logger;
    private readonly string _settingsPath;

    public JsonSettingsStore(IAppLogger logger, string? settingsPath = null)
    {
        ArgumentNullException.ThrowIfNull(logger);
        _logger = logger;
        _settingsPath = settingsPath ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "NeoAirPlay",
            "settings.json");
    }

    public async Task<ReceiverSettings> LoadAsync(CancellationToken cancellationToken = default)
    {
        if (!File.Exists(_settingsPath))
        {
            return new ReceiverSettings();
        }

        try
        {
            await using var stream = File.OpenRead(_settingsPath);
            var settings = await JsonSerializer.DeserializeAsync<ReceiverSettings>(
                stream,
                JsonOptions,
                cancellationToken);

            return Sanitize(settings ?? new ReceiverSettings());
        }
        catch (Exception exception) when (exception is IOException or JsonException or UnauthorizedAccessException)
        {
            await _logger.WriteAsync(
                LogLevel.Warning,
                $"設定檔無法讀取，已使用預設值。{exception.Message}",
                cancellationToken);
            return new ReceiverSettings();
        }
    }

    public async Task SaveAsync(ReceiverSettings settings, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(settings);
        var directory = Path.GetDirectoryName(_settingsPath)
            ?? throw new InvalidOperationException("設定目錄無法解析。");
        Directory.CreateDirectory(directory);

        var temporaryPath = $"{_settingsPath}.tmp";
        await using (var stream = new FileStream(
            temporaryPath,
            FileMode.Create,
            FileAccess.Write,
            FileShare.None,
            bufferSize: 4096,
            useAsync: true))
        {
            await JsonSerializer.SerializeAsync(stream, Sanitize(settings), JsonOptions, cancellationToken);
            await stream.FlushAsync(cancellationToken);
        }

        File.Move(temporaryPath, _settingsPath, overwrite: true);
    }

    private static ReceiverSettings Sanitize(ReceiverSettings settings)
    {
        var renderer = settings.Renderer is "D3D11" or "D3D12" ? settings.Renderer : "Auto";
        var name = string.IsNullOrWhiteSpace(settings.ReceiverName)
            ? "Neo AirPlay"
            : settings.ReceiverName.Trim();

        return settings with
        {
            ReceiverName = name,
            Renderer = renderer
        };
    }
}
