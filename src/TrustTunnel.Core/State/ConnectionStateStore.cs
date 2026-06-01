namespace TrustTunnel.Core.State;

public enum ConnectionPhase
{
    Idle,
    Preparing,
    PermissionRequired,
    Connecting,
    Authenticating,
    Connected,
    Reconnecting,
    Disconnecting,
    Disconnected,
    Error
}

public sealed record ConnectionSnapshot(
    ConnectionPhase Phase,
    string ServerName = "",
    string Message = "",
    DateTimeOffset ChangedAt = default);

public sealed class ConnectionStateStore
{
    private static readonly Dictionary<ConnectionPhase, ConnectionPhase[]> Allowed = new()
    {
        [ConnectionPhase.Idle] = new[] { ConnectionPhase.Preparing, ConnectionPhase.Connecting, ConnectionPhase.Disconnected, ConnectionPhase.Error },
        [ConnectionPhase.Preparing] = new[] { ConnectionPhase.PermissionRequired, ConnectionPhase.Connecting, ConnectionPhase.Error },
        [ConnectionPhase.PermissionRequired] = new[] { ConnectionPhase.Preparing, ConnectionPhase.Connecting, ConnectionPhase.Error, ConnectionPhase.Idle },
        [ConnectionPhase.Connecting] = new[] { ConnectionPhase.Authenticating, ConnectionPhase.Connected, ConnectionPhase.Disconnecting, ConnectionPhase.Error },
        [ConnectionPhase.Authenticating] = new[] { ConnectionPhase.Connected, ConnectionPhase.Error },
        [ConnectionPhase.Connected] = new[] { ConnectionPhase.Reconnecting, ConnectionPhase.Disconnecting, ConnectionPhase.Error },
        [ConnectionPhase.Reconnecting] = new[] { ConnectionPhase.Connected, ConnectionPhase.Disconnecting, ConnectionPhase.Error },
        [ConnectionPhase.Disconnecting] = new[] { ConnectionPhase.Disconnected, ConnectionPhase.Error },
        [ConnectionPhase.Disconnected] = new[] { ConnectionPhase.Preparing, ConnectionPhase.Connecting, ConnectionPhase.Idle },
        [ConnectionPhase.Error] = new[] { ConnectionPhase.Idle, ConnectionPhase.Preparing, ConnectionPhase.Disconnected }
    };

    public ConnectionSnapshot Current { get; private set; } = new(ConnectionPhase.Idle, ChangedAt: DateTimeOffset.UtcNow);
    public event EventHandler<ConnectionSnapshot>? Changed;

    public void TransitionTo(ConnectionPhase next, string message = "", string serverName = "")
    {
        if (!Allowed[Current.Phase].Contains(next))
        {
            throw new InvalidOperationException($"Invalid connection transition: {Current.Phase} -> {next}");
        }

        Current = new ConnectionSnapshot(next, string.IsNullOrWhiteSpace(serverName) ? Current.ServerName : serverName, message, DateTimeOffset.UtcNow);
        Changed?.Invoke(this, Current);
    }
}
