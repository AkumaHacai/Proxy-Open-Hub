# Architecture

The UI is deliberately thin. It can import profiles, select profiles, show redacted generated TOML, request connect/disconnect, and render state/logs.

Core layers:

```text
TrustTunnel.Core.Models
TrustTunnel.Core.Validation
TrustTunnel.Core.Toml
TrustTunnel.Core.Deeplinks
TrustTunnel.Core.Security
TrustTunnel.Core.State
TrustTunnel.Core.Platform
TrustTunnel.Core.Application
```

The platform layer is a facade. It is where the future TrustTunnel native bridge belongs. The UI must not create TUN devices, apply DNS, change routes, or parse core output.

The official TrustTunnel CLI documentation says the client accepts TOML config, supports `[endpoint]`, `[listener.tun]`, and `[listener.socks]`, and on Windows requires `wintun.dll` for tunnel mode. The MVP follows that contract but keeps native startup behind `IVpnController`.
