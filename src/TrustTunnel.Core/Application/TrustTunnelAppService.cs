using TrustTunnel.Core.Deeplinks;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Security;
using TrustTunnel.Core.Toml;

namespace TrustTunnel.Core.Application;

public sealed class TrustTunnelAppService
{
    private readonly ISecretStore _secretStore;
    private readonly DeeplinkParser _deeplinkParser = new();
    private readonly TrustTunnelTomlParser _tomlParser = new();

    public TrustTunnelAppService(ISecretStore secretStore)
    {
        _secretStore = secretStore;
    }

    public async Task<ServerProfile> ImportDeeplinkAsync(string link, CancellationToken cancellationToken = default)
    {
        var imported = _deeplinkParser.Parse(link);
        return await MaterializeSecretsAsync(imported, cancellationToken);
    }

    public async Task<ServerProfile> ImportTomlAsync(string toml, CancellationToken cancellationToken = default)
    {
        var imported = _tomlParser.Parse(toml);
        return await MaterializeSecretsAsync(imported, cancellationToken);
    }

    private async Task<ServerProfile> MaterializeSecretsAsync(ImportedProfile imported, CancellationToken cancellationToken)
    {
        var profile = imported.Profile;
        var scope = $"profile/{profile.Id}";
        var passwordRef = string.IsNullOrWhiteSpace(imported.Secrets.Password)
            ? ""
            : await _secretStore.SaveAsync(scope, "password", imported.Secrets.Password, cancellationToken);
        var clientRandomRef = string.IsNullOrWhiteSpace(imported.Secrets.ClientRandom)
            ? ""
            : await _secretStore.SaveAsync(scope, "client_random", imported.Secrets.ClientRandom, cancellationToken);
        var socksPasswordRef = string.IsNullOrWhiteSpace(imported.Secrets.SocksPassword)
            ? ""
            : await _secretStore.SaveAsync(scope, "socks_password", imported.Secrets.SocksPassword, cancellationToken);

        return profile with
        {
            Endpoint = profile.Endpoint with
            {
                PasswordSecretRef = passwordRef,
                ClientRandomSecretRef = clientRandomRef
            },
            Listener = profile.Listener with
            {
                Socks = profile.Listener.Socks with
                {
                    PasswordSecretRef = socksPasswordRef
                }
            }
        };
    }
}
