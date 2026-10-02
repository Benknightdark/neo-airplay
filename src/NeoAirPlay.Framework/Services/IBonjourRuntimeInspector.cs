namespace NeoAirPlay.Services;

internal interface IBonjourRuntimeInspector
{
    BonjourRuntime? GetRuntime(out string? problem);
}
