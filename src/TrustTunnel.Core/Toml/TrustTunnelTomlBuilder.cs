using System.Text;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Core.Toml;

public sealed class TrustTunnelTomlBuilder
{
    public string Build(TrustTunnelConfig config, string endpointPassword, string clientRandom = "", string socksPassword = "")
    {
        var builder = new StringBuilder();
        Line(builder, "loglevel", ToToml(config.LogLevel));
        Line(builder, "vpn_mode", ToToml(config.Routing.Mode));
        Line(builder, "killswitch_enabled", config.Routing.KillSwitchEnabled);
        Line(builder, "killswitch_allow_ports", config.Routing.KillSwitchAllowPorts);
        Line(builder, "post_quantum_group_enabled", config.Endpoint.PostQuantumGroupEnabled);
        Line(builder, "exclusions", config.Routing.Exclusions);

        builder.AppendLine();
        builder.AppendLine("[endpoint]");
        Line(builder, "hostname", config.Endpoint.Hostname);
        if (!string.IsNullOrWhiteSpace(config.Endpoint.CustomSni))
        {
            Line(builder, "custom_sni", config.Endpoint.CustomSni);
        }

        Line(builder, "addresses", config.Endpoint.Addresses);
        Line(builder, "has_ipv6", config.Endpoint.HasIpv6);
        Line(builder, "username", config.Endpoint.Username);
        Line(builder, "password", endpointPassword);
        Line(builder, "client_random", clientRandom);
        Line(builder, "skip_verification", config.Endpoint.SkipVerification);
        Line(builder, "certificate", config.Endpoint.CertificatePem);
        Line(builder, "dns_upstreams", config.Endpoint.DnsUpstreams);
        Line(builder, "upstream_protocol", ToToml(config.Endpoint.UpstreamProtocol));
        if (config.Endpoint.FallbackProtocol is { } fallbackProtocol)
        {
            Line(builder, "upstream_fallback_protocol", ToToml(fallbackProtocol));
        }

        Line(builder, "anti_dpi", config.Endpoint.AntiDpi);

        builder.AppendLine();
        if (config.Listener.Mode == ListenerMode.Tun)
        {
            builder.AppendLine("[listener.tun]");
            Line(builder, "bound_if", config.Listener.Tun.BoundIf);
            Line(builder, "included_routes", config.Listener.Tun.IncludedRoutes);
            Line(builder, "excluded_routes", config.Listener.Tun.ExcludedRoutes);
            Line(builder, "mtu_size", config.Listener.Tun.MtuSize);
            Line(builder, "tcp_recv_buf_size", config.Listener.Tun.TcpRecvBufSize);
            Line(builder, "tcp_send_buf_size", config.Listener.Tun.TcpSendBufSize);
            Line(builder, "change_system_dns", config.Listener.Tun.ChangeSystemDns);
            Line(builder, "device_name", config.Listener.Tun.DeviceName);
            Line(builder, "use_existing", config.Listener.Tun.UseExisting);
        }
        else
        {
            builder.AppendLine("[listener.socks]");
            Line(builder, "address", config.Listener.Socks.Address);
            if (!string.IsNullOrWhiteSpace(config.Listener.Socks.Username))
            {
                Line(builder, "username", config.Listener.Socks.Username);
            }

            if (!string.IsNullOrWhiteSpace(socksPassword))
            {
                Line(builder, "password", socksPassword);
            }

            if (config.Listener.Socks.AllowLanAccess)
            {
                Line(builder, "allow_lan_access", true);
            }

            if (!string.IsNullOrWhiteSpace(config.Listener.Socks.HttpProxyAddress))
            {
                Line(builder, "http_proxy_address", config.Listener.Socks.HttpProxyAddress);
                Line(builder, "http_proxy_allow_lan_access", config.Listener.Socks.HttpProxyAllowLanAccess);
            }
        }

        return builder.ToString();
    }

    private static string ToToml(LogLevel value) => value.ToString().ToLowerInvariant();
    private static string ToToml(RoutingMode value) => value.ToString().ToLowerInvariant();
    private static string ToToml(UpstreamProtocol value) => value == UpstreamProtocol.Http2 ? "http2" : "http3";

    private static void Line(StringBuilder builder, string key, string value) => builder.Append(key).Append(" = ").Append('"').Append(Escape(value)).AppendLine("\"");
    private static void Line(StringBuilder builder, string key, bool value) => builder.Append(key).Append(" = ").AppendLine(value ? "true" : "false");
    private static void Line(StringBuilder builder, string key, int value) => builder.Append(key).Append(" = ").AppendLine(value.ToString(System.Globalization.CultureInfo.InvariantCulture));
    private static void Line(StringBuilder builder, string key, IEnumerable<int> values) => builder.Append(key).Append(" = [").Append(string.Join(", ", values)).AppendLine("]");
    private static void Line(StringBuilder builder, string key, IEnumerable<string> values) => builder.Append(key).Append(" = [").Append(string.Join(", ", values.Select(v => $"\"{Escape(v)}\""))).AppendLine("]");

    private static string Escape(string value)
    {
        return value.Replace("\\", "\\\\", StringComparison.Ordinal)
            .Replace("\"", "\\\"", StringComparison.Ordinal)
            .Replace("\r", "\\r", StringComparison.Ordinal)
            .Replace("\n", "\\n", StringComparison.Ordinal);
    }
}
