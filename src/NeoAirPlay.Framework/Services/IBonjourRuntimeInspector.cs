namespace NeoAirPlay.Services;

internal interface IBonjourRuntimeInspector
{
    BonjourRuntime? GetProjectRuntime(string engineDirectory, out string? problem);
}
