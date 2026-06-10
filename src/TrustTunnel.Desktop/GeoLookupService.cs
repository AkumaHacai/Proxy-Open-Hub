using System.Net;
using System.Net.Http;
using System.Text.Json;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Desktop;

public sealed class GeoLookupService : IDisposable
{
    private readonly HttpClient _httpClient = new() { Timeout = TimeSpan.FromSeconds(4) };
    private readonly Dictionary<string, GeoResult> _cache = new(StringComparer.OrdinalIgnoreCase);
    private bool _disposed;

    public async Task<GeoResult?> ResolveAsync(ServerProfile profile, CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);

        var host = profile.Endpoint.Addresses.FirstOrDefault()?.Split(':')[0].Trim('[', ']');
        if (string.IsNullOrWhiteSpace(host))
        {
            host = profile.Endpoint.Hostname;
        }

        if (string.IsNullOrWhiteSpace(host))
        {
            return null;
        }

        if (_cache.TryGetValue(host, out var cached))
        {
            return cached;
        }

        var ip = IPAddress.TryParse(host, out var parsed)
            ? parsed.ToString()
            : (await Dns.GetHostAddressesAsync(host, cancellationToken)).FirstOrDefault()?.ToString();
        if (string.IsNullOrWhiteSpace(ip))
        {
            return null;
        }

        if (_cache.TryGetValue(ip, out cached))
        {
            return cached;
        }

        var url = $"http://ip-api.com/json/{ip}?fields=countryCode,country,query";
        var json = await _httpClient.GetStringAsync(url, cancellationToken);
        var result = JsonSerializer.Deserialize<GeoResult>(json, new JsonSerializerOptions { PropertyNameCaseInsensitive = true });
        if (result is null || string.IsNullOrWhiteSpace(result.CountryCode))
        {
            return null;
        }

        _cache[host] = result;
        _cache[ip] = result;
        return result;
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _httpClient.Dispose();
        _cache.Clear();
        _disposed = true;
    }
}

public sealed record GeoResult(string CountryCode, string Country, string Query);
