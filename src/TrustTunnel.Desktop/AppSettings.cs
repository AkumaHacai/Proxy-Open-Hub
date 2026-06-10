namespace TrustTunnel.Desktop;

public enum SystemProxyMode
{
    Off,
    Socks5
}

public enum AppLanguage
{
    System,
    English,
    Russian,
    Chinese,
    Persian
}

public sealed record AppSettings
{
    public AppearanceSettings Appearance { get; init; } = new();
    public AppLanguage Language { get; init; } = AppLanguage.System;
    public string PingHost { get; init; } = "8.8.8.8";
    public string HttpsTestUrl { get; init; } = "https://www.google.com/generate_204";
    public TimeSpan DiagnosticsTimeout { get; init; } = TimeSpan.FromSeconds(5);
    public bool DefaultSocksAllowLan { get; init; }
    public string DefaultSocksAddress { get; init; } = "127.0.0.1:1080";
    public SystemProxyMode SystemProxyMode { get; init; } = SystemProxyMode.Off;
    public bool EnableHttpProxyOptions { get; init; } = true;
    public string DefaultHttpProxyAddress { get; init; } = "127.0.0.1:8080";
    public string MainWindowMode { get; init; } = "Expanded";
}
