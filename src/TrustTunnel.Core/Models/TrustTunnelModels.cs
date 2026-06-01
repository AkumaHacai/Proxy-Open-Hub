namespace TrustTunnel.Core.Models;

public enum ListenerMode
{
    Tun,
    Socks
}

public enum RoutingMode
{
    General,
    Selective
}

public enum UpstreamProtocol
{
    Http2,
    Http3
}

public enum LogLevel
{
    Info,
    Debug,
    Trace
}

public sealed record EndpointConfig
{
    public string Hostname { get; init; } = "";
    public string CustomSni { get; init; } = "";
    public IReadOnlyList<string> Addresses { get; init; } = Array.Empty<string>();
    public bool HasIpv6 { get; init; } = true;
    public string Username { get; init; } = "";
    public string PasswordSecretRef { get; init; } = "";
    public string ClientRandomSecretRef { get; init; } = "";
    public bool SkipVerification { get; init; }
    public string CertificatePem { get; init; } = "";
    public UpstreamProtocol UpstreamProtocol { get; init; } = UpstreamProtocol.Http2;
    public UpstreamProtocol? FallbackProtocol { get; init; }
    public bool AntiDpi { get; init; }
    public bool PostQuantumGroupEnabled { get; init; } = true;
    public IReadOnlyList<string> DnsUpstreams { get; init; } = Array.Empty<string>();
}

public sealed record TunConfig
{
    public string BoundIf { get; init; } = "";
    public IReadOnlyList<string> IncludedRoutes { get; init; } = new[] { "0.0.0.0/0", "2000::/3" };
    public IReadOnlyList<string> ExcludedRoutes { get; init; } = new[] { "0.0.0.0/8", "10.0.0.0/8", "169.254.0.0/16", "172.16.0.0/12", "192.168.0.0/16", "224.0.0.0/3" };
    public int MtuSize { get; init; } = 1280;
    public int TcpRecvBufSize { get; init; }
    public int TcpSendBufSize { get; init; }
    public bool ChangeSystemDns { get; init; } = true;
    public string DeviceName { get; init; } = "";
    public bool UseExisting { get; init; }
}

public sealed record SocksConfig
{
    public string Address { get; init; } = "127.0.0.1:1080";
    public string Username { get; init; } = "";
    public string PasswordSecretRef { get; init; } = "";
    public bool AllowLanAccess { get; init; }
    public string HttpProxyAddress { get; init; } = "";
    public bool HttpProxyAllowLanAccess { get; init; }
}

public sealed record ListenerConfig
{
    public ListenerMode Mode { get; init; } = ListenerMode.Tun;
    public TunConfig Tun { get; init; } = new();
    public SocksConfig Socks { get; init; } = new();
}

public sealed record RoutingProfile
{
    public string Id { get; init; } = "default";
    public string Name { get; init; } = "Default";
    public RoutingMode Mode { get; init; } = RoutingMode.General;
    public IReadOnlyList<string> Exclusions { get; init; } = Array.Empty<string>();
    public bool KillSwitchEnabled { get; init; } = true;
    public IReadOnlyList<int> KillSwitchAllowPorts { get; init; } = Array.Empty<int>();
    public string Description { get; init; } = "Route all traffic through VPN";

    public override string ToString() => Name;
}

public sealed record ServerProfile
{
    public string Id { get; init; } = Guid.NewGuid().ToString("n");
    public string DisplayName { get; init; } = "";
    public EndpointConfig Endpoint { get; init; } = new();
    public string RoutingProfileId { get; init; } = "default";
    public RoutingProfile Routing { get; init; } = new();
    public ListenerConfig Listener { get; init; } = new();
    public bool ServiceModeEnabled { get; init; }
    public bool AutoConnect { get; init; }
    public string CountryCode { get; init; } = "";
    public string CountryName { get; init; } = "";
    public SpeedTestResult? TestResult { get; init; }
    public bool HasTestResult => TestResult is not null;
    public DateTimeOffset CreatedAt { get; init; } = DateTimeOffset.UtcNow;
    public DateTimeOffset UpdatedAt { get; init; } = DateTimeOffset.UtcNow;
}

public sealed record SpeedTestResult(
    string CountryCode,
    int PingMs,
    double DownloadMbps,
    double UploadMbps);

public sealed record TrustTunnelConfig
{
    public LogLevel LogLevel { get; init; } = LogLevel.Info;
    public RoutingProfile Routing { get; init; } = new();
    public EndpointConfig Endpoint { get; init; } = new();
    public ListenerConfig Listener { get; init; } = new();
}

public sealed record SecretCandidate(
    string Password = "",
    string ClientRandom = "",
    string SocksPassword = "");

public sealed record ImportedProfile(
    ServerProfile Profile,
    SecretCandidate Secrets,
    IReadOnlyList<string> Warnings);
