using System.Net.NetworkInformation;
using System.Windows.Threading;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Desktop;

public sealed record TrafficMetricsSnapshot(double DownloadMbps, double UploadMbps);

public sealed class TrafficMetricsService : IDisposable
{
    private static readonly StringComparer InterfaceNameComparer = StringComparer.OrdinalIgnoreCase;

    private readonly DispatcherTimer _timer;
    private readonly List<NetworkInterface> _interfaces = new();
    private ServerProfile? _profile;
    private string _profileId = "";
    private long _lastReceivedBytes;
    private long _lastSentBytes;
    private DateTimeOffset _lastSampleAt;
    private int _refreshTicksRemaining;
    private bool _hasBaseline;

    public TrafficMetricsService()
    {
        _timer = new DispatcherTimer
        {
            Interval = TimeSpan.FromSeconds(1)
        };
        _timer.Tick += HandleTimerTick;
    }

    public event EventHandler<TrafficMetricsSnapshot>? Updated;

    public void Start(ServerProfile profile)
    {
        if (_timer.IsEnabled && InterfaceNameComparer.Equals(_profileId, profile.Id))
        {
            RefreshInterfaces(profile);
            return;
        }

        _profile = profile;
        _profileId = profile.Id;
        _refreshTicksRemaining = 12;
        RefreshInterfaces(profile);
        PrimeBaseline();
        _timer.Start();
    }

    public void Stop()
    {
        _timer.Stop();
        _profile = null;
        _profileId = "";
        _interfaces.Clear();
        _refreshTicksRemaining = 0;
        _hasBaseline = false;
        _lastReceivedBytes = 0;
        _lastSentBytes = 0;
        Updated?.Invoke(this, new TrafficMetricsSnapshot(0, 0));
    }

    public void Dispose()
    {
        _timer.Stop();
        _timer.Tick -= HandleTimerTick;
    }

    private void HandleTimerTick(object? sender, EventArgs e)
    {
        if (_profile is { } profile && _refreshTicksRemaining > 0)
        {
            RefreshInterfaces(profile);
            _refreshTicksRemaining--;
        }

        var now = DateTimeOffset.UtcNow;
        var (receivedBytes, sentBytes) = ReadByteTotals();

        if (!_hasBaseline)
        {
            SetBaseline(now, receivedBytes, sentBytes);
            Updated?.Invoke(this, new TrafficMetricsSnapshot(0, 0));
            return;
        }

        var elapsedSeconds = Math.Max((now - _lastSampleAt).TotalSeconds, 0.001);
        var downloadMbps = ToMbps(receivedBytes - _lastReceivedBytes, elapsedSeconds);
        var uploadMbps = ToMbps(sentBytes - _lastSentBytes, elapsedSeconds);

        SetBaseline(now, receivedBytes, sentBytes);
        Updated?.Invoke(this, new TrafficMetricsSnapshot(downloadMbps, uploadMbps));
    }

    private void RefreshInterfaces(ServerProfile profile)
    {
        var nextInterfaces = ResolveInterfaces(profile);
        if (SameInterfaces(_interfaces, nextInterfaces))
        {
            return;
        }

        _interfaces.Clear();
        _interfaces.AddRange(nextInterfaces);
        _hasBaseline = false;
    }

    private void PrimeBaseline()
    {
        var (receivedBytes, sentBytes) = ReadByteTotals();
        SetBaseline(DateTimeOffset.UtcNow, receivedBytes, sentBytes);
        Updated?.Invoke(this, new TrafficMetricsSnapshot(0, 0));
    }

    private void SetBaseline(DateTimeOffset sampleAt, long receivedBytes, long sentBytes)
    {
        _lastSampleAt = sampleAt;
        _lastReceivedBytes = receivedBytes;
        _lastSentBytes = sentBytes;
        _hasBaseline = true;
    }

    private (long ReceivedBytes, long SentBytes) ReadByteTotals()
    {
        long receivedBytes = 0;
        long sentBytes = 0;

        foreach (var networkInterface in _interfaces)
        {
            try
            {
                var statistics = networkInterface.GetIPv4Statistics();
                receivedBytes += statistics.BytesReceived;
                sentBytes += statistics.BytesSent;
            }
            catch (NetworkInformationException)
            {
            }
            catch (PlatformNotSupportedException)
            {
            }
        }

        return (receivedBytes, sentBytes);
    }

    private static IReadOnlyList<NetworkInterface> ResolveInterfaces(ServerProfile profile)
    {
        var interfaces = NetworkInterface.GetAllNetworkInterfaces();
        if (profile.Listener.Mode != ListenerMode.Tun)
        {
            return ActiveNonLoopbackInterfaces(interfaces);
        }

        var deviceName = profile.Listener.Tun.DeviceName.Trim();

        if (deviceName.Length > 0)
        {
            var byDeviceName = interfaces
                .Where(networkInterface => MatchesName(networkInterface, deviceName))
                .ToArray();
            if (byDeviceName.Length > 0)
            {
                return byDeviceName;
            }
        }

        var trustTunnelInterfaces = interfaces
            .Where(networkInterface => MatchesName(networkInterface, "TrustTunnel") || MatchesName(networkInterface, "Wintun"))
            .ToArray();
        if (trustTunnelInterfaces.Length > 0)
        {
            return trustTunnelInterfaces;
        }

        return ActiveNonLoopbackInterfaces(interfaces);
    }

    private static NetworkInterface[] ActiveNonLoopbackInterfaces(IEnumerable<NetworkInterface> interfaces)
    {
        return interfaces
            .Where(networkInterface => networkInterface.OperationalStatus == OperationalStatus.Up)
            .Where(networkInterface => networkInterface.NetworkInterfaceType != NetworkInterfaceType.Loopback)
            .ToArray();
    }

    private static bool MatchesName(NetworkInterface networkInterface, string value)
    {
        return networkInterface.Name.Contains(value, StringComparison.OrdinalIgnoreCase)
            || networkInterface.Description.Contains(value, StringComparison.OrdinalIgnoreCase);
    }

    private static bool SameInterfaces(IReadOnlyList<NetworkInterface> current, IReadOnlyList<NetworkInterface> next)
    {
        return current.Count == next.Count
            && current.Select(networkInterface => networkInterface.Id)
                .SequenceEqual(next.Select(networkInterface => networkInterface.Id), StringComparer.OrdinalIgnoreCase);
    }

    private static double ToMbps(long byteDelta, double elapsedSeconds)
    {
        if (byteDelta <= 0)
        {
            return 0;
        }

        return byteDelta * 8d / 1_000_000d / elapsedSeconds;
    }
}
