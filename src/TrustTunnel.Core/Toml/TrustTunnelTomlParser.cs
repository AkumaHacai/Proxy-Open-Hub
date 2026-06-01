using TrustTunnel.Core.Models;

namespace TrustTunnel.Core.Toml;

public sealed class TrustTunnelTomlParser
{
    public ImportedProfile Parse(string toml)
    {
        var values = ParseFlat(toml);
        var endpoint = new EndpointConfig
        {
            Hostname = Get(values, "endpoint.hostname"),
            CustomSni = Get(values, "endpoint.custom_sni"),
            Addresses = GetArray(values, "endpoint.addresses"),
            HasIpv6 = GetBool(values, "endpoint.has_ipv6", true),
            Username = Get(values, "endpoint.username"),
            PasswordSecretRef = "",
            ClientRandomSecretRef = "",
            SkipVerification = GetBool(values, "endpoint.skip_verification", false),
            CertificatePem = Get(values, "endpoint.certificate"),
            UpstreamProtocol = Get(values, "endpoint.upstream_protocol") == "http3" ? UpstreamProtocol.Http3 : UpstreamProtocol.Http2,
            FallbackProtocol = Get(values, "endpoint.upstream_fallback_protocol") switch
            {
                "http2" => UpstreamProtocol.Http2,
                "http3" => UpstreamProtocol.Http3,
                _ => null
            },
            AntiDpi = GetBool(values, "endpoint.anti_dpi", false),
            PostQuantumGroupEnabled = GetBool(values, "post_quantum_group_enabled", true),
            DnsUpstreams = GetArray(values, "endpoint.dns_upstreams", GetArray(values, "dns_upstreams"))
        };

        var listenerMode = values.ContainsKey("listener.socks.address") ? ListenerMode.Socks : ListenerMode.Tun;
        var listener = new ListenerConfig
        {
            Mode = listenerMode,
            Tun = new TunConfig
            {
                BoundIf = Get(values, "listener.tun.bound_if"),
                IncludedRoutes = GetArray(values, "listener.tun.included_routes", new[] { "0.0.0.0/0", "2000::/3" }),
                ExcludedRoutes = GetArray(values, "listener.tun.excluded_routes", new[] { "0.0.0.0/8", "10.0.0.0/8", "169.254.0.0/16", "172.16.0.0/12", "192.168.0.0/16", "224.0.0.0/3" }),
                MtuSize = GetInt(values, "listener.tun.mtu_size", 1280),
                TcpRecvBufSize = GetInt(values, "listener.tun.tcp_recv_buf_size", 0),
                TcpSendBufSize = GetInt(values, "listener.tun.tcp_send_buf_size", 0),
                ChangeSystemDns = GetBool(values, "listener.tun.change_system_dns", true),
                DeviceName = Get(values, "listener.tun.device_name"),
                UseExisting = GetBool(values, "listener.tun.use_existing", false)
            },
            Socks = new SocksConfig
            {
                Address = Get(values, "listener.socks.address", "127.0.0.1:1080"),
                Username = Get(values, "listener.socks.username"),
                AllowLanAccess = GetBool(values, "listener.socks.allow_lan_access", false),
                HttpProxyAddress = Get(values, "listener.socks.http_proxy_address"),
                HttpProxyAllowLanAccess = GetBool(values, "listener.socks.http_proxy_allow_lan_access", false)
            }
        };

        var profile = new ServerProfile
        {
            DisplayName = endpoint.Hostname,
            Endpoint = endpoint,
            Routing = new RoutingProfile
            {
                Mode = Get(values, "vpn_mode") == "selective" ? RoutingMode.Selective : RoutingMode.General,
                KillSwitchEnabled = GetBool(values, "killswitch_enabled", true),
                KillSwitchAllowPorts = GetIntArray(values, "killswitch_allow_ports"),
                Exclusions = GetArray(values, "exclusions")
            },
            Listener = listener
        };

        return new ImportedProfile(
            profile,
            new SecretCandidate(
                Get(values, "endpoint.password"),
                Get(values, "endpoint.client_random"),
                Get(values, "listener.socks.password")),
            Array.Empty<string>());
    }

    private static Dictionary<string, string> ParseFlat(string toml)
    {
        var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        var section = "";
        foreach (var raw in toml.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None))
        {
            var line = StripComment(raw).Trim();
            if (line.Length == 0)
            {
                continue;
            }

            if (line.StartsWith("[", StringComparison.Ordinal) && line.EndsWith("]", StringComparison.Ordinal))
            {
                section = line[1..^1].Trim();
                continue;
            }

            var separator = line.IndexOf('=');
            if (separator < 1)
            {
                continue;
            }

            var key = line[..separator].Trim();
            var value = line[(separator + 1)..].Trim();
            values[string.IsNullOrWhiteSpace(section) ? key : $"{section}.{key}"] = value;
        }

        return values;
    }

    private static string StripComment(string line)
    {
        var inString = false;
        for (var i = 0; i < line.Length; i++)
        {
            if (line[i] == '"' && (i == 0 || line[i - 1] != '\\'))
            {
                inString = !inString;
            }

            if (!inString && line[i] == '#')
            {
                return line[..i];
            }
        }

        return line;
    }

    private static string Get(Dictionary<string, string> values, string key, string fallback = "")
    {
        return values.TryGetValue(key, out var value) ? Unquote(value) : fallback;
    }

    private static int GetInt(Dictionary<string, string> values, string key, int fallback)
    {
        return values.TryGetValue(key, out var value) && int.TryParse(value, out var parsed) ? parsed : fallback;
    }

    private static bool GetBool(Dictionary<string, string> values, string key, bool fallback)
    {
        return values.TryGetValue(key, out var value) && bool.TryParse(value, out var parsed) ? parsed : fallback;
    }

    private static IReadOnlyList<string> GetArray(Dictionary<string, string> values, string key, IReadOnlyList<string>? fallback = null)
    {
        if (!values.TryGetValue(key, out var value))
        {
            return fallback ?? Array.Empty<string>();
        }

        value = value.Trim();
        if (!value.StartsWith("[", StringComparison.Ordinal) || !value.EndsWith("]", StringComparison.Ordinal))
        {
            return Array.Empty<string>();
        }

        return value[1..^1]
            .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Select(Unquote)
            .ToArray();
    }

    private static IReadOnlyList<int> GetIntArray(Dictionary<string, string> values, string key)
    {
        if (!values.TryGetValue(key, out var value))
        {
            return Array.Empty<int>();
        }

        value = value.Trim();
        if (!value.StartsWith("[", StringComparison.Ordinal) || !value.EndsWith("]", StringComparison.Ordinal))
        {
            return Array.Empty<int>();
        }

        return value[1..^1]
            .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Select(item => int.TryParse(item, out var port) ? port : 0)
            .Where(port => port > 0)
            .ToArray();
    }

    private static string Unquote(string value)
    {
        value = value.Trim();
        if (value.Length >= 2 && value[0] == '"' && value[^1] == '"')
        {
            value = value[1..^1];
        }

        return value.Replace("\\n", "\n", StringComparison.Ordinal)
            .Replace("\\r", "\r", StringComparison.Ordinal)
            .Replace("\\\"", "\"", StringComparison.Ordinal)
            .Replace("\\\\", "\\", StringComparison.Ordinal);
    }
}
