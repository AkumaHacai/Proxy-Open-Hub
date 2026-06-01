namespace TrustTunnel.Core.Security;

public interface ISecretStore
{
    Task<string> SaveAsync(string scope, string name, string value, CancellationToken cancellationToken = default);
    Task<string?> ReadAsync(string secretRef, CancellationToken cancellationToken = default);
    Task DeleteAsync(string secretRef, CancellationToken cancellationToken = default);
}

public sealed class InMemorySecretStore : ISecretStore
{
    private readonly Dictionary<string, string> _secrets = new(StringComparer.Ordinal);

    public Task<string> SaveAsync(string scope, string name, string value, CancellationToken cancellationToken = default)
    {
        var secretRef = $"secret://trusttunnel/{scope.Trim('/')}/{name}";
        _secrets[secretRef] = value;
        return Task.FromResult(secretRef);
    }

    public Task<string?> ReadAsync(string secretRef, CancellationToken cancellationToken = default)
    {
        return Task.FromResult(_secrets.TryGetValue(secretRef, out var value) ? value : null);
    }

    public Task DeleteAsync(string secretRef, CancellationToken cancellationToken = default)
    {
        _secrets.Remove(secretRef);
        return Task.CompletedTask;
    }

    public IReadOnlyDictionary<string, string> Snapshot()
    {
        return new Dictionary<string, string>(_secrets, StringComparer.Ordinal);
    }

    public void Load(IReadOnlyDictionary<string, string>? secrets)
    {
        _secrets.Clear();
        if (secrets is null)
        {
            return;
        }

        foreach (var (key, value) in secrets)
        {
            if (!string.IsNullOrWhiteSpace(key))
            {
                _secrets[key] = value;
            }
        }
    }
}
