using System.Security.Cryptography;

namespace TrustTunnel.Desktop;

internal static class SocksCredentialGenerator
{
    public static string Username()
    {
        return $"tt_{RandomNumberGenerator.GetHexString(6).ToLowerInvariant()}";
    }

    public static string Password()
    {
        return Convert.ToBase64String(RandomNumberGenerator.GetBytes(24))
            .TrimEnd('=')
            .Replace("+", "-", StringComparison.Ordinal)
            .Replace("/", "_", StringComparison.Ordinal);
    }
}
