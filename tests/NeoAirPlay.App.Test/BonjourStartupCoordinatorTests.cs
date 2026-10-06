using NeoAirPlay.Services;
using Xunit;

namespace NeoAirPlay.Tests.App;

[Trait("Category", "Unit")]
public sealed class BonjourStartupCoordinatorTests
{
    [Fact]
    public async Task EnsureReadyAsync_WhenServiceIsRunning_DoesNotRequestElevation()
    {
        var inspector = new FakeBonjourStartupInspector(
            hasInstalledRuntime: true,
            runtimes: [new BonjourRuntime("C:\\Program Files\\NeoAirPlay\\Bonjour")]);
        var elevatedCalls = 0;
        var promptCalls = 0;
        var coordinator = CreateCoordinator(
            inspector,
            _ =>
            {
                elevatedCalls++;
                return Task.FromResult<int?>(0);
            },
            _ => { },
            () => promptCalls++);

        var result = await coordinator.EnsureReadyAsync(new FakeAppLogger(), CancellationToken.None);

        Assert.True(result.IsReady);
        Assert.Equal(0, elevatedCalls);
        Assert.Equal(0, promptCalls);
    }

    [Fact]
    public async Task EnsureReadyAsync_WhenServiceIsStopped_RequestsElevationAndVerifiesItStarted()
    {
        var runtime = new BonjourRuntime("C:\\Program Files\\NeoAirPlay\\Bonjour");
        var inspector = new FakeBonjourStartupInspector(hasInstalledRuntime: true, runtimes: [null, runtime]);
        var elevatedCalls = 0;
        var promptCalls = 0;
        var coordinator = CreateCoordinator(
            inspector,
            _ =>
            {
                elevatedCalls++;
                return Task.FromResult<int?>(0);
            },
            _ => { },
            () => promptCalls++);

        var result = await coordinator.EnsureReadyAsync(new FakeAppLogger(), CancellationToken.None);

        Assert.True(result.IsReady);
        Assert.Equal(1, elevatedCalls);
        Assert.Equal(1, promptCalls);
    }

    [Fact]
    public async Task EnsureReadyAsync_WhenNoTrustedFilesExist_OffersAppleInstallWithoutElevation()
    {
        var inspector = new FakeBonjourStartupInspector(hasInstalledRuntime: false, runtimes: [null]);
        var elevatedCalls = 0;
        var missingPrompts = 0;
        var coordinator = new BonjourStartupCoordinator(
            inspector,
            _ =>
            {
                elevatedCalls++;
                return Task.FromResult<int?>(0);
            },
            _ => missingPrompts++,
            () => throw new InvalidOperationException("Unexpected elevation prompt."));

        var logger = new FakeAppLogger();
        var result = await coordinator.EnsureReadyAsync(logger, CancellationToken.None);

        Assert.False(result.IsReady);
        Assert.Contains("Apple 官方說明", result.Message!);
        Assert.Equal(1, missingPrompts);
        Assert.Equal(0, elevatedCalls);
        Assert.Contains(logger.Entries, entry => entry.Level == NeoAirPlay.Models.LogLevel.Warning);
    }

    [Fact]
    public async Task EnsureReadyAsync_WhenX64DnsSdIsMissing_ShowsSpecificDiagnosticWithoutElevation()
    {
        const string sourceProblem = "已找到 Bonjour Service 執行檔，但沒有找到相同版本的 x64 DNS-SD 元件（dnssd.dll）。";
        var inspector = new FakeBonjourStartupInspector(
            hasInstalledRuntime: false,
            sourceProblem: sourceProblem,
            runtimes: [null]);
        string? displayedProblem = null;
        var coordinator = new BonjourStartupCoordinator(
            inspector,
            _ => Task.FromResult<int?>(0),
            problem => displayedProblem = problem,
            () => throw new InvalidOperationException("Unexpected elevation prompt."));

        var result = await coordinator.EnsureReadyAsync(new FakeAppLogger(), CancellationToken.None);

        Assert.False(result.IsReady);
        Assert.Contains("x64 DNS-SD", result.Message!);
        Assert.Equal(sourceProblem, displayedProblem);
    }

    [Fact]
    public async Task EnsureReadyAsync_WhenUserCancelsUac_ReportsBonjourUnavailable()
    {
        var inspector = new FakeBonjourStartupInspector(hasInstalledRuntime: true, runtimes: [null]);
        var coordinator = CreateCoordinator(
            inspector,
            _ => Task.FromResult<int?>(null),
            _ => { });

        var result = await coordinator.EnsureReadyAsync(new FakeAppLogger(), CancellationToken.None);

        Assert.False(result.IsReady);
        Assert.Contains("已取消", result.Message!);
    }

    [Fact]
    public async Task EnsureReadyAsync_WhenElevatedSetupFails_ReportsInspectorProblem()
    {
        var inspector = new FakeBonjourStartupInspector(
            hasInstalledRuntime: true,
            runtimeProblem: "Bonjour Service 目前為停用狀態。",
            runtimes: [null]);
        var coordinator = CreateCoordinator(
            inspector,
            _ => Task.FromResult<int?>(1),
            _ => { });

        var result = await coordinator.EnsureReadyAsync(new FakeAppLogger(), CancellationToken.None);

        Assert.False(result.IsReady);
        Assert.Contains("停用狀態", result.Message!);
    }

    private static BonjourStartupCoordinator CreateCoordinator(
        IBonjourStartupInspector inspector,
        Func<CancellationToken, Task<int?>> runElevated,
        Action<string?> showMissing,
        Action? showElevationReason = null)
    {
        return new BonjourStartupCoordinator(
            inspector,
            runElevated,
            showMissing,
            showElevationReason ?? (() => { }));
    }

    private sealed class FakeBonjourStartupInspector : IBonjourStartupInspector
    {
        private readonly Queue<BonjourRuntime?> _runtimes;
        private readonly string? _runtimeProblem;
        private readonly string? _sourceProblem;

        public FakeBonjourStartupInspector(
            bool hasInstalledRuntime,
            IEnumerable<BonjourRuntime?>? runtimes = null,
            string? runtimeProblem = null,
            string? sourceProblem = null)
        {
            HasInstalledRuntime = hasInstalledRuntime;
            _runtimeProblem = runtimeProblem;
            _sourceProblem = sourceProblem;
            _runtimes = new Queue<BonjourRuntime?>(runtimes ?? []);
        }

        private bool HasInstalledRuntime { get; }

        public BonjourRuntime? GetRuntime(out string? problem)
        {
            problem = _runtimeProblem;
            return _runtimes.Count > 0 ? _runtimes.Dequeue() : null;
        }

        public bool TryGetInstalledRuntimeDirectory(out string? directory, out string? problem)
        {
            directory = HasInstalledRuntime ? "C:\\Program Files\\Bonjour" : null;
            problem = _sourceProblem;
            return HasInstalledRuntime;
        }
    }
}
