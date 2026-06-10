using System.Runtime.InteropServices;
using Microsoft.Win32;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Desktop;

internal sealed class WindowsSystemProxyService : IDisposable
{
    private const string InternetSettingsKey = @"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    private const int InternetOptionRefresh = 37;
    private const int InternetOptionSettingsChanged = 39;

    private RegistrySnapshot? _snapshot;
    private string _appliedProxyServer = "";

    public void Apply(SystemProxyMode mode, ServerProfile profile)
    {
        if (!OperatingSystem.IsWindows() || mode == SystemProxyMode.Off || profile.Listener.Mode != ListenerMode.Socks)
        {
            Clear();
            return;
        }

        var endpoint = NormalizeLoopbackEndpoint(profile.Listener.Socks.Address);
        if (endpoint.Length == 0)
        {
            Clear();
            return;
        }

        var proxyServer = $"socks={endpoint}";
        if (StringComparer.Ordinal.Equals(_appliedProxyServer, proxyServer))
        {
            return;
        }

        using var key = Registry.CurrentUser.CreateSubKey(InternetSettingsKey, writable: true)
            ?? throw new InvalidOperationException("Unable to open Windows internet settings.");

        _snapshot ??= RegistrySnapshot.Capture(key);
        key.SetValue("ProxyEnable", 1, RegistryValueKind.DWord);
        key.SetValue("ProxyServer", proxyServer, RegistryValueKind.String);
        key.SetValue("ProxyOverride", "<local>", RegistryValueKind.String);
        _appliedProxyServer = proxyServer;
        NotifySettingsChanged();
    }

    public void Clear()
    {
        if (!OperatingSystem.IsWindows() || _snapshot is null)
        {
            return;
        }

        using var key = Registry.CurrentUser.CreateSubKey(InternetSettingsKey, writable: true);
        _snapshot.Restore(key);
        _snapshot = null;
        _appliedProxyServer = "";
        NotifySettingsChanged();
    }

    public void Dispose() => Clear();

    private static string NormalizeLoopbackEndpoint(string address)
    {
        var trimmed = address.Trim();
        if (trimmed.Length == 0)
        {
            return "";
        }

        var separator = trimmed.LastIndexOf(':');
        if (separator < 0 || separator == trimmed.Length - 1)
        {
            return "";
        }

        var host = trimmed[..separator].Trim().Trim('[', ']');
        var port = trimmed[(separator + 1)..].Trim();
        if (!int.TryParse(port, out var parsedPort) || parsedPort is <= 0 or > 65535)
        {
            return "";
        }

        host = host switch
        {
            "" or "0.0.0.0" or "::" => "127.0.0.1",
            _ => host
        };

        return $"{host}:{parsedPort}";
    }

    private static void NotifySettingsChanged()
    {
        _ = InternetSetOption(IntPtr.Zero, InternetOptionSettingsChanged, IntPtr.Zero, 0);
        _ = InternetSetOption(IntPtr.Zero, InternetOptionRefresh, IntPtr.Zero, 0);
    }

    [DllImport("wininet.dll", SetLastError = true)]
    private static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int dwBufferLength);

    private sealed record RegistrySnapshot(
        object? ProxyEnable,
        object? ProxyServer,
        object? ProxyOverride)
    {
        public static RegistrySnapshot Capture(RegistryKey key)
        {
            return new RegistrySnapshot(
                key.GetValue("ProxyEnable"),
                key.GetValue("ProxyServer"),
                key.GetValue("ProxyOverride"));
        }

        public void Restore(RegistryKey? key)
        {
            if (key is null)
            {
                return;
            }

            RestoreValue(key, "ProxyEnable", ProxyEnable, RegistryValueKind.DWord);
            RestoreValue(key, "ProxyServer", ProxyServer, RegistryValueKind.String);
            RestoreValue(key, "ProxyOverride", ProxyOverride, RegistryValueKind.String);
        }

        private static void RestoreValue(RegistryKey key, string name, object? value, RegistryValueKind kind)
        {
            if (value is null)
            {
                key.DeleteValue(name, throwOnMissingValue: false);
                return;
            }

            key.SetValue(name, value, kind);
        }
    }
}
