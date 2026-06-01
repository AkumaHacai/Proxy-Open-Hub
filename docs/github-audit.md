# GitHub Audit Notes

Checked on 2026-06-01:

- `TrustTunnel/TrustTunnel`
- `TrustTunnel/TrustTunnelClient`
- `TrustTunnel/TrustTunnelFlutterClient`
- public search for `TrustTunnel C#`, `TrustTunnel WPF`, `trusttunnel_client C#`

Findings applied to this GUI:

- Official Windows adapter is not a C# app; it is a small native wrapper shaped like `start/stop` with TOML input.
- Windows TUN mode requires both Administrator privileges and `wintun.dll` in the DLL search path.
- `dns_upstreams` was moved under `[endpoint]`, but root-level `dns_upstreams` must still be imported for old configs.
- Newer TUN defaults include `0.0.0.0/8`, `169.254.0.0/16`, and `224.0.0.0/3` in excluded routes.
- `custom_sni` replaced the old `hostname|sni` style.
- `device_name`, `use_existing`, `tcp_recv_buf_size`, and `tcp_send_buf_size` are current client settings and are exposed in Advanced options.
- Important bug-fix areas to avoid repeating later in the native bridge: certificate verification errors on Windows, socket protection / outbound interface detection, DNS forwarding socket password, ping timeout semantics, QUIC routing by VPN mode, and endpoint/service config path validation.

Not found:

- A maintained public C# TrustTunnel GUI/client repository. The public community GUI referenced by the official README is a Python wrapper, which this project intentionally avoids as a runtime.
