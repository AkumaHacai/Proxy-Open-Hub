# Proxy Open Hub

![Proxy Open Hub logo](logo/png/POH.png)

Proxy Open Hub is a Windows desktop proxy/VPN hub built with WPF and .NET. The current working core is the official TrustTunnel Windows core, and the UI is being prepared for optional cores such as sing-box, NaiveProxy, Xray-core, and Hysteria2.

The project is not the official TrustTunnel client. It is an independent desktop shell that imports profiles, stores typed settings, generates TrustTunnel TOML, and launches the bundled/installed native core.

## Current Status

Alpha. The main TrustTunnel workflow is usable, but the project still needs hardening before a public stable release:

- stronger persistent secret storage through Windows Credential Manager or DPAPI;
- signed installer/update flow;
- full manual QA for TUN, SOCKS5, HTTP proxy, service mode, tray behavior, and system proxy behavior;
- trusted download/install flow for optional future cores;
- broader UI translation review for Russian, English, Chinese, and Persian.

## Implemented

- Modern WPF interface with expanded and compact modes.
- Proxy Open Hub application icon, taskbar icon, tray icon, and branded logo assets.
- Tray behavior: close button hides the window, tray menu can reopen or exit.
- `tt://?...` TrustTunnel profile import.
- TOML import, TOML editor, copy/save workflow with sensitive-data preview toggle.
- Manual server creation.
- Typed config model for endpoint, listener, TUN, SOCKS5, routing, server profiles, and app settings.
- Routing profiles with simple presets for local network and RU bypass rules.
- Validators for host:port, DNS upstreams, CIDR routes, exclusions, MTU, and unsafe certificate settings.
- Secret store abstraction; current desktop implementation keeps MVP secrets in the local app state.
- SOCKS5 credentials are generated automatically when missing.
- System proxy toggle for SOCKS5 profiles.
- Connection state machine and redacted logs.
- Live network metrics from OS counters while connected.
- Server diagnostics for native files, TUN prerequisites, ping, and HTTPS access checks.
- Localization: English, Russian, Chinese, Persian, with system-language default on first launch.
- Native runtime loader for `vpn_easy.dll`, service mode via `vpn_easy_service.exe`, and CLI fallback via `trusttunnel_client.exe`.

## Native Core Files

Official native files can be placed in `native/bundled/win-x64` or copied next to the built desktop executable.

Preferred ABI integration:

- `vpn_easy.dll`
- `wintun.dll` for TUN mode
- `vpn_easy_service.exe` for service mode

Release-package CLI fallback:

- `trusttunnel_client.exe`
- `wintun.dll` for TUN mode

More details: `docs/native-core-integration.md`.

## Commands

```powershell
dotnet restore --configfile .\NuGet.Config
dotnet build .\TrustTunnelGuiClient.sln --no-restore
dotnet run --project .\tests\TrustTunnel.Tests\TrustTunnel.Tests.csproj --no-build
dotnet format .\TrustTunnelGuiClient.sln --verify-no-changes --no-restore
```

The desktop binary is emitted as `ProxyOpenHub.exe`.

## License

Proxy Open Hub source code is licensed under the Apache License 2.0. See `LICENSE.txt`.

Bundled native components and future downloadable cores keep their own licenses. See `NOTICE.md` and the license files shipped beside each native binary.
