using System.Net;
using System.Text.RegularExpressions;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Core.Validation;

public static partial class TrustTunnelValidators
{
    public static ValidationReport Validate(ServerProfile profile, bool skipVerificationConfirmed = false)
    {
        return Validate(new TrustTunnelConfig
        {
            Endpoint = profile.Endpoint,
            Listener = profile.Listener,
            Routing = profile.Routing
        }, skipVerificationConfirmed);
    }

    public static ValidationReport Validate(TrustTunnelConfig config, bool skipVerificationConfirmed = false)
    {
        var report = new ValidationReport();
        ValidateEndpoint(config.Endpoint, report, skipVerificationConfirmed);
        foreach (var exclusion in config.Routing.Exclusions)
        {
            if (!IsValidExclusion(exclusion))
            {
                report.Error("routing.exclusion.invalid", $"Invalid routing exclusion: {exclusion}");
            }
        }

        foreach (var port in config.Routing.KillSwitchAllowPorts)
        {
            if (port is < 1 or > 65535)
            {
                report.Error("killswitch.port.invalid", $"Kill switch allow port is out of range: {port}");
            }
        }

        if (config.Listener.Mode == ListenerMode.Tun)
        {
            ValidateTun(config.Listener.Tun, report);
        }
        else
        {
            ValidateHostPort(config.Listener.Socks.Address, "socks.address", report);
        }

        return report;
    }

    public static bool IsValidDnsUpstream(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return false;
        }

        if (value.StartsWith("sdns://", StringComparison.OrdinalIgnoreCase))
        {
            return value.Length > "sdns://".Length;
        }

        if (Uri.TryCreate(value, UriKind.Absolute, out var uri))
        {
            return uri.Scheme is "tcp" or "tls" or "https" or "quic" && !string.IsNullOrWhiteSpace(uri.Host);
        }

        return TrySplitHostPort(value, out var host, out var port) && IsHostLike(host) && port is >= 1 and <= 65535;
    }

    public static bool IsValidCidr(string value)
    {
        var parts = value.Split('/');
        if (parts.Length != 2 || !IPAddress.TryParse(parts[0], out var address) || !int.TryParse(parts[1], out var prefix))
        {
            return false;
        }

        return address.AddressFamily switch
        {
            System.Net.Sockets.AddressFamily.InterNetwork => prefix is >= 0 and <= 32,
            System.Net.Sockets.AddressFamily.InterNetworkV6 => prefix is >= 0 and <= 128,
            _ => false
        };
    }

    public static bool IsValidExclusion(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return false;
        }

        if (value.StartsWith("*:", StringComparison.Ordinal) &&
            int.TryParse(value[2..], out var wildcardPort))
        {
            return wildcardPort is >= 1 and <= 65535;
        }

        if (value.Contains('/'))
        {
            return IsValidCidr(value);
        }

        if (IPAddress.TryParse(value.Trim('[', ']'), out _))
        {
            return true;
        }

        if (TrySplitHostPort(value, out var host, out var port))
        {
            return IsHostLike(host.Trim('[', ']')) && port is >= 1 and <= 65535;
        }

        return DomainPattern().IsMatch(value);
    }

    private static void ValidateEndpoint(EndpointConfig endpoint, ValidationReport report, bool skipVerificationConfirmed)
    {
        if (string.IsNullOrWhiteSpace(endpoint.Hostname))
        {
            report.Error("endpoint.hostname.required", "Hostname/SNI is required.");
        }

        if (!string.IsNullOrWhiteSpace(endpoint.CustomSni) && endpoint.CustomSni.Contains('|', StringComparison.Ordinal))
        {
            report.Error("endpoint.custom_sni.invalid", "Custom SNI must not use the legacy hostname|sni syntax.");
        }

        if (endpoint.Addresses.Count == 0)
        {
            report.Error("endpoint.addresses.required", "At least one endpoint address is required.");
        }

        foreach (var address in endpoint.Addresses)
        {
            ValidateHostPort(address, "endpoint.address", report);
        }

        if (string.IsNullOrWhiteSpace(endpoint.Username))
        {
            report.Error("endpoint.username.required", "Username is required.");
        }

        if (string.IsNullOrWhiteSpace(endpoint.PasswordSecretRef))
        {
            report.Error("endpoint.password.required", "Password must be stored in secure storage before connecting.");
        }

        foreach (var upstream in endpoint.DnsUpstreams)
        {
            if (!IsValidDnsUpstream(upstream))
            {
                report.Error("endpoint.dns.invalid", $"Invalid DNS upstream: {upstream}");
            }
        }

        if (endpoint.SkipVerification && !skipVerificationConfirmed)
        {
            report.Warning("endpoint.skip_verification.confirm", "Certificate verification is disabled and requires explicit confirmation.");
        }

        if (!string.IsNullOrWhiteSpace(endpoint.CertificatePem) &&
            !endpoint.CertificatePem.Contains("BEGIN CERTIFICATE", StringComparison.Ordinal))
        {
            report.Error("endpoint.certificate.invalid", "Certificate PEM is invalid or incomplete.");
        }
    }

    private static void ValidateTun(TunConfig tun, ValidationReport report)
    {
        foreach (var route in tun.IncludedRoutes.Concat(tun.ExcludedRoutes))
        {
            if (!IsValidCidr(route))
            {
                report.Error("tun.route.invalid", $"Invalid route CIDR: {route}");
            }
        }

        if (tun.MtuSize is < 576 or > 9000)
        {
            report.Error("tun.mtu.invalid", "MTU must be between 576 and 9000.");
        }
    }

    private static void ValidateHostPort(string value, string codePrefix, ValidationReport report)
    {
        if (!TrySplitHostPort(value, out var host, out var port) || !IsHostLike(host) || port is < 1 or > 65535)
        {
            report.Error($"{codePrefix}.invalid", $"Address must be host:port with port 1-65535: {value}");
        }
    }

    private static bool TrySplitHostPort(string value, out string host, out int port)
    {
        host = "";
        port = 0;
        if (string.IsNullOrWhiteSpace(value))
        {
            return false;
        }

        if (value.StartsWith("[", StringComparison.Ordinal))
        {
            var close = value.IndexOf(']');
            if (close < 0 || close + 2 > value.Length || value[close + 1] != ':')
            {
                return false;
            }

            host = value[1..close];
            return int.TryParse(value[(close + 2)..], out port);
        }

        var separator = value.LastIndexOf(':');
        if (separator <= 0 || separator == value.Length - 1)
        {
            return false;
        }

        host = value[..separator];
        return int.TryParse(value[(separator + 1)..], out port);
    }

    private static bool IsHostLike(string host)
    {
        return IPAddress.TryParse(host, out _) || DomainPattern().IsMatch(host);
    }

    [GeneratedRegex(@"^(\*\.)?([a-zA-Z0-9-]+\.)*[a-zA-Z0-9-]+$")]
    private static partial Regex DomainPattern();
}
