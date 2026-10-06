using NeoAirPlay.Models;
using NeoAirPlay.Services;

namespace NeoAirPlay.Services;

internal interface IBonjourStartupInspector
{
    BonjourRuntime? GetRuntime(out string? problem);
    bool TryGetInstalledRuntimeDirectory(out string? directory, out string? problem);
}

internal sealed class BonjourStartupInspector : IBonjourStartupInspector
{
    private readonly BonjourServiceInspector _inspector = new();

    public BonjourRuntime? GetRuntime(out string? problem) => _inspector.GetRuntime(out problem);

    public bool TryGetInstalledRuntimeDirectory(out string? directory, out string? problem) =>
        _inspector.TryGetInstalledRuntimeDirectory(out directory, out problem);
}

internal sealed record BonjourStartupResult(bool IsReady, string? Message = null);

internal sealed class BonjourStartupCoordinator(
    IBonjourStartupInspector inspector,
    Func<CancellationToken, Task<int?>> runElevatedSetup,
    Action<string?> showMissingBonjour,
    Action showElevationReason)
{
    public async Task<BonjourStartupResult> EnsureReadyAsync(
        IAppLogger logger,
        CancellationToken cancellationToken)
    {
        if (inspector.GetRuntime(out _) is not null)
        {
            return new BonjourStartupResult(IsReady: true);
        }

        if (!inspector.TryGetInstalledRuntimeDirectory(out _, out var sourceProblem))
        {
            showMissingBonjour(sourceProblem);
            var message = sourceProblem
                ?? "AirPlay 尚未啟用：找不到可信任的 Bonjour 安裝檔案，請先依 Apple 官方說明安裝 Bonjour。";
            await logger.WriteAsync(LogLevel.Warning, message, cancellationToken);
            return new BonjourStartupResult(IsReady: false, message);
        }

        showElevationReason();

        int? exitCode;
        try
        {
            exitCode = await runElevatedSetup(cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            var failure = $"Bonjour Service 啟用失敗：{exception.Message}";
            await logger.WriteAsync(LogLevel.Error, failure, cancellationToken);
            return new BonjourStartupResult(IsReady: false, failure);
        }

        if (exitCode is null)
        {
            const string message = "Bonjour Service 尚未啟用：已取消系統管理員權限要求。";
            await logger.WriteAsync(LogLevel.Information, message, cancellationToken);
            return new BonjourStartupResult(IsReady: false, message);
        }

        var runtime = inspector.GetRuntime(out var problem);
        if (exitCode != 0 || runtime is null)
        {
            var failure = $"Bonjour Service 啟用失敗：{problem ?? $"管理員程序結束代碼 {exitCode}"}";
            await logger.WriteAsync(LogLevel.Error, failure, cancellationToken);
            return new BonjourStartupResult(IsReady: false, failure);
        }

        await logger.WriteAsync(LogLevel.Information, "Bonjour Service 已啟用。", cancellationToken);
        return new BonjourStartupResult(IsReady: true);
    }
}
