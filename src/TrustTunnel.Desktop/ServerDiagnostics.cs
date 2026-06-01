using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Net.NetworkInformation;
using System.Security.Principal;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Platform;

namespace TrustTunnel.Desktop;

public sealed record ServerDiagnosticResult(
    string Name,
    bool Success,
    string Message,
    TimeSpan Elapsed);

public static class ServerDiagnostics
{
    private const int UnknownPing = -1;

    public static async Task<IReadOnlyList<ServerDiagnosticResult>> RunAsync(ServerProfile profile, AppSettings settings, CancellationToken cancellationToken = default)
    {
        var results = new List<ServerDiagnosticResult>
        {
            NativeCorePrerequisites(profile),
            WindowsTunPrerequisites(profile),
            await PingAsync(profile, settings, cancellationToken),
            await HttpsAsync(settings, cancellationToken)
        };

        return results;
    }

    public static async Task<ServerDiagnosticResult> PingAsync(ServerProfile profile, AppSettings settings, CancellationToken cancellationToken = default)
    {
        var host = string.IsNullOrWhiteSpace(settings.PingHost) ? profile.Endpoint.Hostname : settings.PingHost;
        var stopwatch = Stopwatch.StartNew();
        try
        {
            using var ping = new Ping();
            var reply = await ping.SendPingAsync(host, settings.DiagnosticsTimeout);
            stopwatch.Stop();
            return reply.Status == IPStatus.Success
                ? new ServerDiagnosticResult("Ping", true, $"{host}: {reply.RoundtripTime} ms", stopwatch.Elapsed)
                : new ServerDiagnosticResult("Ping", false, $"{host}: {reply.Status}", stopwatch.Elapsed);
        }
        catch (Exception ex) when (ex is PingException or InvalidOperationException)
        {
            stopwatch.Stop();
            return new ServerDiagnosticResult("Ping", false, $"{host}: {ex.Message}", stopwatch.Elapsed);
        }
    }

    public static async Task<SpeedTestResult> ConnectionTestAsync(ServerProfile profile, AppSettings settings, CancellationToken cancellationToken = default)
    {
        var pingMs = await PingEndpointAsync(profile, settings, cancellationToken);
        var downloadMbps = await DownloadProbeAsync(settings, cancellationToken);
        var countryCode = string.IsNullOrWhiteSpace(profile.CountryCode)
            ? GuessCountryCode(profile.Endpoint.Hostname)
            : profile.CountryCode;
        return new SpeedTestResult(countryCode, pingMs, downloadMbps, 0);
    }

    private static async Task<int> PingEndpointAsync(ServerProfile profile, AppSettings settings, CancellationToken cancellationToken)
    {
        try
        {
            using var ping = new Ping();
            var reply = await ping.SendPingAsync(profile.Endpoint.Hostname, settings.DiagnosticsTimeout);
            return reply.Status == IPStatus.Success ? (int)reply.RoundtripTime : UnknownPing;
        }
        catch (Exception ex) when (ex is PingException or InvalidOperationException)
        {
            return UnknownPing;
        }
    }

    private static async Task<double> DownloadProbeAsync(AppSettings settings, CancellationToken cancellationToken)
    {
        var stopwatch = Stopwatch.StartNew();
        try
        {
            using var cts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            cts.CancelAfter(settings.DiagnosticsTimeout);
            using var client = new HttpClient { Timeout = settings.DiagnosticsTimeout };
            using var response = await client.GetAsync(settings.HttpsTestUrl, HttpCompletionOption.ResponseHeadersRead, cts.Token);
            if (!response.IsSuccessStatusCode)
            {
                return 0;
            }

            var bytes = await response.Content.ReadAsByteArrayAsync(cts.Token);
            stopwatch.Stop();
            return stopwatch.Elapsed.TotalSeconds <= 0 || bytes.Length == 0
                ? 0
                : bytes.Length * 8d / stopwatch.Elapsed.TotalSeconds / 1_000_000d;
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or UriFormatException)
        {
            return 0;
        }
    }

    private static string GuessCountryCode(string hostname)
    {
        var lower = hostname.ToLowerInvariant();
        if (lower.Contains("hel") || lower.EndsWith(".fi", StringComparison.Ordinal))
        {
            return "FI";
        }

        if (lower.Contains("us") || lower.EndsWith(".us", StringComparison.Ordinal))
        {
            return "US";
        }

        if (lower.Contains("ru") || lower.EndsWith(".ru", StringComparison.Ordinal))
        {
            return "RU";
        }

        if (lower.Contains("de") || lower.EndsWith(".de", StringComparison.Ordinal))
        {
            return "DE";
        }

        if (lower.Contains("nl") || lower.EndsWith(".nl", StringComparison.Ordinal))
        {
            return "NL";
        }

        return "US";
    }

    public static ServerDiagnosticResult WindowsTunPrerequisites(ServerProfile profile)
    {
        if (!OperatingSystem.IsWindows() || profile.Listener.Mode != ListenerMode.Tun)
        {
            return new ServerDiagnosticResult("TUN prerequisites", true, "Not required for this profile.", TimeSpan.Zero);
        }

        var admin = IsAdministrator();
        var wintunPath = Path.Combine(AppContext.BaseDirectory, "wintun.dll");
        var wintun = File.Exists(wintunPath);
        var ok = profile.ServiceModeEnabled ? wintun : admin && wintun;
        var adminNote = profile.ServiceModeEnabled ? "admin needed only for first service install" : "admin required for direct TUN";
        var message = $"Admin: {(admin ? "yes" : "no")} ({adminNote}); wintun.dll: {(wintun ? wintunPath : "missing")}";
        return new ServerDiagnosticResult("TUN prerequisites", ok, message, TimeSpan.Zero);
    }

    public static ServerDiagnosticResult NativeCorePrerequisites(ServerProfile profile)
    {
        var probe = NativeTrustTunnelRuntimeInfo.Probe(AppContext.BaseDirectory);
        var needsService = profile.ServiceModeEnabled;
        var needsWintun = OperatingSystem.IsWindows() && profile.Listener.Mode == ListenerMode.Tun;
        var hasRunnableCore = needsService ? probe.VpnEasyDllFound : probe.VpnEasyDllFound || probe.CliExeFound;
        var ok = hasRunnableCore
            && (!needsService || probe.ServiceExeFound)
            && (!needsWintun || probe.WintunDllFound);
        var message = $"vpn_easy.dll: {(probe.VpnEasyDllFound ? probe.VpnEasyDllPath : "missing")}; "
            + $"trusttunnel_client.exe: {(probe.CliExeFound ? probe.CliExePath : (needsService ? "not enough for service mode" : "missing"))}; "
            + $"wintun.dll: {(probe.WintunDllFound ? probe.WintunDllPath : (needsWintun ? "missing" : "not required"))}; "
            + $"vpn_easy_service.exe: {(probe.ServiceExeFound ? probe.ServiceExePath : (needsService ? "missing" : "not required"))}";
        return new ServerDiagnosticResult("Native core", ok, message, TimeSpan.Zero);
    }

    public static async Task<ServerDiagnosticResult> HttpsAsync(AppSettings settings, CancellationToken cancellationToken = default)
    {
        var stopwatch = Stopwatch.StartNew();
        try
        {
            using var cts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            cts.CancelAfter(settings.DiagnosticsTimeout);
            using var client = new HttpClient { Timeout = settings.DiagnosticsTimeout };
            using var response = await client.GetAsync(settings.HttpsTestUrl, cts.Token);
            stopwatch.Stop();
            return new ServerDiagnosticResult("HTTPS", response.IsSuccessStatusCode, $"{settings.HttpsTestUrl}: {(int)response.StatusCode} {response.ReasonPhrase}", stopwatch.Elapsed);
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or UriFormatException)
        {
            stopwatch.Stop();
            return new ServerDiagnosticResult("HTTPS", false, $"{settings.HttpsTestUrl}: {ex.Message}", stopwatch.Elapsed);
        }
    }

    private static bool IsAdministrator()
    {
        if (!OperatingSystem.IsWindows())
        {
            return Environment.UserName == "root";
        }

        using var identity = WindowsIdentity.GetCurrent();
        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
    }
}
