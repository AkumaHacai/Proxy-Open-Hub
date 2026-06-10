# Notices

Proxy Open Hub is an independent desktop client/hub. It is not the official TrustTunnel application.

## Project License

Proxy Open Hub source code is licensed under the Apache License 2.0. See `LICENSE.txt`.

## Third-Party Native Components

This repository may include or load native components that are not authored by Proxy Open Hub:

- TrustTunnel / vpn_easy native core and CLI fallback.
- Wintun driver/runtime from WireGuard LLC.
- Future optional cores, such as sing-box, NaiveProxy, Xray-core, and Hysteria2.

Each native component keeps its own upstream license and release terms. When binaries are bundled, keep the corresponding license file beside the binary. Current bundled examples:

- `native/bundled/win-x64/TRUSTTUNNEL_LICENSE.txt`
- `native/bundled/win-x64/WINTUN_LICENSE.txt`

Do not remove upstream copyright, license, or attribution notices.
