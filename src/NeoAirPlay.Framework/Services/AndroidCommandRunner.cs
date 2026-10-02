using System.Diagnostics;

namespace NeoAirPlay.Services;

internal sealed record AndroidCommandResult(int ExitCode, string StandardOutput, string StandardError);

internal interface IAndroidCommandRunner
{
    Task<AndroidCommandResult> RunAsync(
        ProcessStartInfo startInfo,
        TimeSpan timeout,
        CancellationToken cancellationToken);
}

internal sealed class SystemAndroidCommandRunner : IAndroidCommandRunner
{
    public async Task<AndroidCommandResult> RunAsync(
        ProcessStartInfo startInfo,
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        using var process = new Process { StartInfo = startInfo };
        if (!process.Start())
        {
            throw new InvalidOperationException($"無法啟動 Android 命令：{startInfo.FileName}");
        }

        var standardOutputTask = process.StandardOutput.ReadToEndAsync(cancellationToken);
        var standardErrorTask = process.StandardError.ReadToEndAsync(cancellationToken);

        try
        {
            await process.WaitForExitAsync(cancellationToken).WaitAsync(timeout, cancellationToken);
        }
        catch (TimeoutException)
        {
            TryKill(process);
            await process.WaitForExitAsync(CancellationToken.None);
            throw new TimeoutException($"Android 命令逾時：{Path.GetFileName(startInfo.FileName)}");
        }
        catch (OperationCanceledException)
        {
            TryKill(process);
            await process.WaitForExitAsync(CancellationToken.None);
            throw;
        }

        var standardOutput = await standardOutputTask;
        var standardError = await standardErrorTask;
        return new AndroidCommandResult(process.ExitCode, standardOutput, standardError);
    }

    private static void TryKill(Process process)
    {
        try
        {
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
            }
        }
        catch (InvalidOperationException)
        {
            // The command can exit between the state check and the kill request.
        }
        catch (System.ComponentModel.Win32Exception)
        {
            // Preserve the original timeout or cancellation error.
        }
    }
}
