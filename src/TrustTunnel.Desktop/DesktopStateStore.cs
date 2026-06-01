using System.IO;
using System.Text.Json;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Desktop;

public sealed record PersistedDesktopState
{
    public AppSettings Settings { get; init; } = new();
    public List<ServerProfile> Profiles { get; init; } = new();
    public List<RoutingProfile> RoutingProfiles { get; init; } = new();
    public Dictionary<string, string> Secrets { get; init; } = new(StringComparer.Ordinal);
}

public sealed class DesktopStateStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };

    public string FilePath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "TrustTunnel",
        "desktop-state.json");

    public PersistedDesktopState Load()
    {
        try
        {
            if (!File.Exists(FilePath))
            {
                return new PersistedDesktopState();
            }

            var json = File.ReadAllText(FilePath);
            return JsonSerializer.Deserialize<PersistedDesktopState>(json, JsonOptions) ?? new PersistedDesktopState();
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or JsonException)
        {
            return new PersistedDesktopState();
        }
    }

    public void Save(PersistedDesktopState state)
    {
        var directory = Path.GetDirectoryName(FilePath);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var json = JsonSerializer.Serialize(state, JsonOptions);
        File.WriteAllText(FilePath, json);
    }
}
