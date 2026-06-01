# Release Plan

## MVP-1

- Import `tt://?...`
- Import TOML
- Manual server add
- Server list
- Edit / duplicate / delete server profiles
- Export generated TOML with a warning before copying secrets
- One-button connect/disconnect through `IVpnController`
- TUN mode validation
- SOCKS5 profile editing
- HTTP/2 config generation
- HTTP/3 and fallback transport fields
- DNS upstream validation
- Toggleable kill switch, DNS, IPv6, anti-DPI, post-quantum group, skip verification and TUN reuse fields
- Routing editor for general/selective modes, exclusions, included and excluded routes
- Logs with redaction
- Secure-storage abstraction

## Before Public Test

- Replace `InMemorySecretStore` with a durable OS secret store.
- Replace CLI fallback with `vpn_easy.dll` when the upstream Windows adapter is built locally.
- Bundle and verify `wintun.dll` in the installer.
- Add installer and elevation flow.
- Add integration tests against the real native bridge.
