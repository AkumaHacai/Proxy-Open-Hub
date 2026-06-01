# TrustTunnel GUI Client

Windows MVP client shell for TrustTunnel.

This repository follows the architecture described in the project notes:

```text
WPF UI
  -> application service
  -> typed config model / validators
  -> secret storage abstraction
  -> TOML builder / parser
  -> VPN controller facade
  -> native TrustTunnel bridge
```

The desktop app talks to `IVpnController`; the bundled `NativeBridgeVpnController` validates profiles, resolves secrets into generated TOML, checks Windows TUN preconditions, and calls the official Windows `vpn_easy` ABI when the native files are placed next to the desktop executable. If the official release package only provides `trusttunnel_client.exe`, the controller can run that CLI core as a fallback process.

## Implemented

- Windows WPF app on .NET.
- `tt://?...` import module with a bundled TrustTunnel test vector.
- TOML import module for endpoint/listener settings.
- Typed config model for endpoint, listener, TUN, SOCKS, routing profile, server profile.
- Validators for host:port, DNS upstreams, CIDR routes, exclusions, MTU, certificate warning.
- TOML builder from typed model with string/array escaping.
- Secret storage abstraction; MVP uses in-memory storage so plaintext secrets are not written to local profile config.
- Connection state machine with guarded transitions.
- Redacting diagnostic log for `tt://`, password, client random and certificate fields.
- Profile management in the GUI: edit, validate, duplicate, delete, and export TOML.
- Persistent desktop state in `%LOCALAPPDATA%\TrustTunnel\desktop-state.json` for profiles, routing profiles, app settings, and MVP secrets.
- Toggle-driven GUI fields for boolean config options such as IPv6, anti-DPI, post-quantum group, skip certificate verification, kill switch, system DNS changes, and existing TUN reuse.
- Routing/DNS editor for split tunneling mode, exclusions, included/excluded routes, kill switch allow ports, DNS upstreams, MTU, and TCP buffers.
- Advanced editor for SOCKS5, TUN device options, client random, certificate PEM, and fallback transport.
- Native runtime loader for `vpn_easy.dll`, direct connect/disconnect, optional Windows service mode through `vpn_easy_service.exe`, and CLI fallback through `trusttunnel_client.exe`.
- Server diagnostics for bundled native core files, Windows TUN requirements, ping, and HTTPS checks.
- Console smoke tests without external test packages.

## Native Core Files

Copy the official Windows native files into the app output directory, for example `src\TrustTunnel.Desktop\bin\Debug\net10.0-windows`.

Preferred ABI integration:

- `vpn_easy.dll`
- `wintun.dll` for TUN mode
- `vpn_easy_service.exe` for service mode

Release-package CLI fallback:

- `trusttunnel_client.exe`
- `wintun.dll` for TUN mode

More details: `docs/native-core-integration.md`.

## Next Native Integration Steps

1. Replace the in-memory `ISecretStore` with Windows Credential Manager or DPAPI backed storage.
2. Add a repeatable upstream native build step once CMake/MSVC/Conan/Rust are available on the machine.
3. Add durable SQLite/profile repository.

## Commands

```powershell
dotnet restore --configfile .\NuGet.Config
dotnet build .\TrustTunnelGuiClient.sln --configfile .\NuGet.Config
dotnet run --project .\tests\TrustTunnel.Tests\TrustTunnel.Tests.csproj --no-restore
```
