using TrustTunnel.Core.Deeplinks;
using TrustTunnel.Core.Diagnostics;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Platform;
using TrustTunnel.Core.Security;
using TrustTunnel.Core.State;
using TrustTunnel.Core.Toml;
using TrustTunnel.Core.Validation;
using System.Text;

var tests = new (string Name, Action Body)[]
{
    ("deeplink parser extracts bundled TrustTunnel vector", DeeplinkVector),
    ("deeplink parser honors TLV fields and display name", DeeplinkTlvFields),
    ("TOML parser imports endpoint password as secret candidate", TomlImport),
    ("TOML builder escapes secrets and arrays", TomlBuilder),
    ("TOML parser keeps routing and fallback toggles", TomlRoutingOptions),
    ("app service materializes SOCKS password secret", AppServiceSocksPassword),
    ("TOML parser supports legacy root DNS upstreams", TomlLegacyDns),
    ("validators catch invalid routes and DNS", Validators),
    ("native runtime probe reports bundled core paths", NativeRuntimeProbe),
    ("connection state machine rejects illegal transitions", StateMachine),
    ("logs redact tt links and secrets", LogRedaction)
};

var failed = 0;
foreach (var (name, body) in tests)
{
    try
    {
        body();
        Console.WriteLine($"PASS {name}");
    }
    catch (Exception ex)
    {
        failed++;
        Console.WriteLine($"FAIL {name}: {ex.Message}");
    }
}

return failed == 0 ? 0 : 1;

static void DeeplinkVector()
{
    var link = "tt://?AAEBARF0dC5oZWwyLm11bXVydS5ydQUGdHR1c2VyBiRZcmdua0o4V2pOV090MXdRVW5jYzllYWt5VU1nb3hjSVpZY0ICFXR0LmhlbDIubXVtdXJ1LnJ1OjQ0Mw";
    var imported = new DeeplinkParser().Parse(link);
    Equal("tt.hel2.mumuru.ru", imported.Profile.Endpoint.Hostname);
    Equal("ttuser", imported.Profile.Endpoint.Username);
    Equal("tt.hel2.mumuru.ru:443", imported.Profile.Endpoint.Addresses.Single());
    True(imported.Secrets.Password.Length > 8, "password should be captured as secret candidate");
}

static void DeeplinkTlvFields()
{
    var link = BuildDeeplink(
        (0x00, VarIntBytes(1)),
        (0x0C, StringBytes("Pretty Server")),
        (0x01, StringBytes("vpn.example.com")),
        (0x02, StringBytes("vpn.example.com:443")),
        (0x0D, StringArrayBytes("tls://1.1.1.1", "https://dns.google/dns-query")),
        (0x05, StringBytes("realuser")),
        (0x06, StringBytes("real-password")),
        (0x0B, StringBytes("aabbcc/ffffff")),
        (0x09, VarIntBytes(2)),
        (0x0A, new byte[] { 1 }));

    var imported = new DeeplinkParser().Parse(link);
    Equal("Pretty Server", imported.Profile.DisplayName);
    Equal("vpn.example.com", imported.Profile.Endpoint.Hostname);
    Equal("realuser", imported.Profile.Endpoint.Username);
    Equal("real-password", imported.Secrets.Password);
    Equal("aabbcc/ffffff", imported.Secrets.ClientRandom);
    Equal(UpstreamProtocol.Http3, imported.Profile.Endpoint.UpstreamProtocol);
    Equal(true, imported.Profile.Endpoint.AntiDpi);
    Equal("tls://1.1.1.1", imported.Profile.Endpoint.DnsUpstreams.First());
}

static void TomlImport()
{
    var toml = """
        loglevel = "info"
        vpn_mode = "general"
        killswitch_enabled = true
        killswitch_allow_ports = [22, 8080]
        post_quantum_group_enabled = true
        exclusions = ["*.example.com"]

        [endpoint]
        hostname = "vpn.example.com"
        custom_sni = "front.example.com"
        addresses = ["vpn.example.com:443"]
        has_ipv6 = true
        username = "user"
        password = "secret"
        client_random = "abcd"
        skip_verification = false
        certificate = ""
        upstream_protocol = "http2"
        upstream_fallback_protocol = "http3"
        anti_dpi = false
        dns_upstreams = ["tls://1.1.1.1"]

        [listener.tun]
        included_routes = ["0.0.0.0/0"]
        excluded_routes = ["10.0.0.0/8"]
        mtu_size = 1280
        change_system_dns = true
        """;

    var imported = new TrustTunnelTomlParser().Parse(toml);
    Equal("vpn.example.com", imported.Profile.Endpoint.Hostname);
    Equal("front.example.com", imported.Profile.Endpoint.CustomSni);
    Equal("secret", imported.Secrets.Password);
    Equal("abcd", imported.Secrets.ClientRandom);
    Equal(true, imported.Profile.Routing.KillSwitchEnabled);
    Equal(2, imported.Profile.Routing.KillSwitchAllowPorts.Count);
    Equal(UpstreamProtocol.Http3, imported.Profile.Endpoint.FallbackProtocol);
}

static void TomlBuilder()
{
    var config = new TrustTunnelConfig
    {
        Endpoint = new EndpointConfig
        {
            Hostname = "vpn.example.com",
            CustomSni = "front.example.com",
            Addresses = new[] { "vpn.example.com:443" },
            Username = "user",
            FallbackProtocol = UpstreamProtocol.Http2,
            DnsUpstreams = new[] { "tls://1.1.1.1" }
        }
    };

    var toml = new TrustTunnelTomlBuilder().Build(config, "s\"ecret");
    True(toml.Contains("password = \"s\\\"ecret\"", StringComparison.Ordinal), "password must be escaped");
    True(toml.Contains("addresses = [\"vpn.example.com:443\"]", StringComparison.Ordinal), "arrays must be rendered");
    True(toml.Contains("custom_sni = \"front.example.com\"", StringComparison.Ordinal), "custom SNI must be rendered");
    True(toml.Contains("upstream_fallback_protocol = \"http2\"", StringComparison.Ordinal), "fallback protocol must be rendered");
}

static void TomlRoutingOptions()
{
    var toml = """
        loglevel = "info"
        vpn_mode = "selective"
        killswitch_enabled = false
        killswitch_allow_ports = [53, 123]
        post_quantum_group_enabled = false
        exclusions = ["*.local", "192.168.0.0/16"]

        [endpoint]
        hostname = "vpn.example.com"
        addresses = ["vpn.example.com:443"]
        has_ipv6 = false
        username = "user"
        password = "secret"
        client_random = ""
        skip_verification = true
        certificate = ""
        upstream_protocol = "http3"
        upstream_fallback_protocol = "http2"
        anti_dpi = true
        dns_upstreams = ["https://dns.adguard.com/dns-query"]

        [listener.socks]
        address = "127.0.0.1:1080"
        allow_lan_access = true
        http_proxy_address = "0.0.0.0:8080"
        http_proxy_allow_lan_access = true
        """;

    var imported = new TrustTunnelTomlParser().Parse(toml);
    Equal(RoutingMode.Selective, imported.Profile.Routing.Mode);
    Equal(false, imported.Profile.Routing.KillSwitchEnabled);
    Equal(2, imported.Profile.Routing.KillSwitchAllowPorts.Count);
    Equal(false, imported.Profile.Endpoint.HasIpv6);
    Equal(true, imported.Profile.Endpoint.SkipVerification);
    Equal(true, imported.Profile.Endpoint.AntiDpi);
    Equal(UpstreamProtocol.Http3, imported.Profile.Endpoint.UpstreamProtocol);
    Equal(UpstreamProtocol.Http2, imported.Profile.Endpoint.FallbackProtocol);
    Equal(ListenerMode.Socks, imported.Profile.Listener.Mode);
    Equal(true, imported.Profile.Listener.Socks.AllowLanAccess);
    Equal("0.0.0.0:8080", imported.Profile.Listener.Socks.HttpProxyAddress);
    Equal(true, imported.Profile.Listener.Socks.HttpProxyAllowLanAccess);
}

static void AppServiceSocksPassword()
{
    var toml = """
        [endpoint]
        hostname = "vpn.example.com"
        addresses = ["vpn.example.com:443"]
        username = "user"
        password = "secret"

        [listener.socks]
        address = "127.0.0.1:1080"
        password = "socks-secret"
        """;

    var store = new InMemorySecretStore();
    var profile = new TrustTunnel.Core.Application.TrustTunnelAppService(store)
        .ImportTomlAsync(toml)
        .GetAwaiter()
        .GetResult();

    True(!string.IsNullOrWhiteSpace(profile.Listener.Socks.PasswordSecretRef), "SOCKS password ref should be materialized");
    Equal("socks-secret", store.ReadAsync(profile.Listener.Socks.PasswordSecretRef).GetAwaiter().GetResult());
}

static void TomlLegacyDns()
{
    var toml = """
        loglevel = "info"
        vpn_mode = "general"
        killswitch_enabled = true
        post_quantum_group_enabled = true
        exclusions = []
        dns_upstreams = ["tls://9.9.9.9"]

        [endpoint]
        hostname = "vpn.example.com"
        addresses = ["vpn.example.com:443"]
        username = "user"
        password = "secret"
        upstream_protocol = "http2"

        [listener.tun]
        included_routes = ["0.0.0.0/0"]
        excluded_routes = ["10.0.0.0/8"]
        mtu_size = 1280
        """;

    var imported = new TrustTunnelTomlParser().Parse(toml);
    Equal("tls://9.9.9.9", imported.Profile.Endpoint.DnsUpstreams.Single());
}

static void Validators()
{
    True(TrustTunnelValidators.IsValidDnsUpstream("https://dns.adguard.com/dns-query"), "DoH should be valid");
    True(!TrustTunnelValidators.IsValidDnsUpstream("https://"), "broken DoH should be invalid");
    True(TrustTunnelValidators.IsValidExclusion("[::1]:443"), "IPv6 host:port should be valid");
    True(!TrustTunnelValidators.IsValidCidr("10.0.0.0/99"), "bad CIDR prefix should be invalid");
}

static void NativeRuntimeProbe()
{
    var directory = Path.Combine(Path.GetTempPath(), "trusttunnel-test-" + Guid.NewGuid().ToString("n"));
    var probe = NativeTrustTunnelRuntimeInfo.Probe(directory);
    Equal(Path.GetFullPath(directory), probe.BaseDirectory);
    True(!probe.VpnEasyDllFound, "temporary probe directory should not contain vpn_easy.dll");
    True(probe.VpnEasyDllPath.EndsWith(NativeTrustTunnelRuntimeInfo.VpnEasyDllName, StringComparison.OrdinalIgnoreCase), "probe should point at vpn_easy.dll");
    True(probe.CliExePath.EndsWith(NativeTrustTunnelRuntimeInfo.CliExeName, StringComparison.OrdinalIgnoreCase), "probe should point at trusttunnel_client.exe");
    True(probe.WintunDllPath.EndsWith(NativeTrustTunnelRuntimeInfo.WintunDllName, StringComparison.OrdinalIgnoreCase), "probe should point at wintun.dll");
}

static void StateMachine()
{
    var store = new ConnectionStateStore();
    store.TransitionTo(ConnectionPhase.Preparing);
    Throws<InvalidOperationException>(() => store.TransitionTo(ConnectionPhase.Connected));
}

static void LogRedaction()
{
    var redacted = RedactingLog.Redact("tt://?abc password=secret client_random=abcd");
    True(!redacted.Contains("secret", StringComparison.Ordinal), "password should be redacted");
    True(!redacted.Contains("tt://?abc", StringComparison.Ordinal), "link should be redacted");
}

static void True(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

static void Equal<T>(T expected, T actual)
{
    if (!EqualityComparer<T>.Default.Equals(expected, actual))
    {
        throw new InvalidOperationException($"Expected '{expected}', got '{actual}'.");
    }
}

static void Throws<TException>(Action action) where TException : Exception
{
    try
    {
        action();
    }
    catch (TException)
    {
        return;
    }

    throw new InvalidOperationException($"Expected {typeof(TException).Name}.");
}

static string BuildDeeplink(params (int Tag, byte[] Value)[] fields)
{
    using var stream = new MemoryStream();
    foreach (var (tag, value) in fields)
    {
        stream.Write(VarIntBytes((ulong)tag));
        stream.Write(VarIntBytes((ulong)value.Length));
        stream.Write(value);
    }

    return "tt://?" + Convert.ToBase64String(stream.ToArray()).TrimEnd('=').Replace('+', '-').Replace('/', '_');
}

static byte[] StringBytes(string value) => Encoding.UTF8.GetBytes(value);

static byte[] StringArrayBytes(params string[] values)
{
    using var stream = new MemoryStream();
    foreach (var value in values)
    {
        var bytes = StringBytes(value);
        stream.Write(VarIntBytes((ulong)bytes.Length));
        stream.Write(bytes);
    }

    return stream.ToArray();
}

static byte[] VarIntBytes(ulong value)
{
    if (value <= 63)
    {
        return new[] { (byte)value };
    }

    if (value <= 16383)
    {
        return new[] { (byte)(0x40 | (value >> 8)), (byte)value };
    }

    if (value <= 1073741823)
    {
        return new[] { (byte)(0x80 | (value >> 24)), (byte)(value >> 16), (byte)(value >> 8), (byte)value };
    }

    return new[]
    {
        (byte)(0xC0 | (value >> 56)),
        (byte)(value >> 48),
        (byte)(value >> 40),
        (byte)(value >> 32),
        (byte)(value >> 24),
        (byte)(value >> 16),
        (byte)(value >> 8),
        (byte)value
    };
}
