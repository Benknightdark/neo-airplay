using NeoAirPlay.Models;
using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Framework.Test;

[Trait("Category", "Unit")]
public sealed class FileAppLoggerTests
{
    [Fact]
    public async Task WriteAsync_AppendsSingleLineAndRaisesEntryEvent()
    {
        using var temporaryDirectory = new TemporaryDirectory();
        var logger = new FileAppLogger(System.IO.Path.Combine(temporaryDirectory.Path, "logs"));
        AppLogEntry? receivedEntry = null;
        logger.EntryWritten += (_, entry) => receivedEntry = entry;

        await logger.WriteAsync(
            LogLevel.Information,
            " first\r\nsecond ",
            TestContext.Current.CancellationToken);

        Assert.NotNull(receivedEntry);
        Assert.Equal("first\r\nsecond", receivedEntry.Message);
        var logFile = Directory.GetFiles(logger.LogDirectory, "*.log").Single();
        var contents = await File.ReadAllTextAsync(logFile, TestContext.Current.CancellationToken);
        Assert.Contains("first  second", contents);
        Assert.DoesNotContain("first\r\nsecond", contents);
    }
}
