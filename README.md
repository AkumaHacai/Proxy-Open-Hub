![Proxy Open Hub logo](logo/png/POH.png)

# Proxy Open Hub

![Platform](https://img.shields.io/badge/platform-Windows-blue)
![Framework](https://img.shields.io/badge/framework-.NET_10-purple)
![License](https://img.shields.io/badge/license-Apache_2.0-green)
![Status](https://img.shields.io/badge/status-alpha-orange)
![Current core](https://img.shields.io/badge/current_core-TrustTunnel-0f766e)

> Windows desktop proxy/VPN client for TrustTunnel. Planned adapters: sing-box, Xray-core, Hysteria2 and NaiveProxy.
>
> Russian main README: [README.md](README_ru.md)

Proxy Open Hub is an independent Windows client built with WPF and .NET.

In practice, this is still a **TrustTunnel-first** project. The current working path is simple: profile import, typed settings, config generation, TrustTunnel core launch, diagnostics, TUN / SOCKS modes, and local system proxy control.

This is not the official TrustTunnel client. It is a separate desktop shell. It is not a real universal hub yet. The goal is still that: one UI for multiple cores and multiple profile formats.

## What this project is

The app is meant to put the usual scattered pieces into one desktop client:

- profile import;
- typed settings storage;
- config generation;
- local core launch;
- listener mode switch;
- diagnostics and basic telemetry.

The idea is simple: one Windows desktop client, not a pile of unrelated launchers and configs.

## Project status

Status: **alpha**.

Only the TrustTunnel path is actually usable right now. Everything else is planned work and should not be presented as already supported.

Current problems:

- secret persistence is not hardened enough yet;
- there is no signed installer and no proper update flow;
- there is no stable release pipeline with ready binaries;
- there is no real multi-core adapter layer;
- documentation is still thin;
- UI screenshots are still missing.

## Cores

| Core | Status | Notes |
|---|---|---|
| TrustTunnel | working alpha | current working core |
| sing-box | planned | adapter, config generator and launcher are still needed |
| Xray-core | planned | adapter and profile model are still needed |
| Hysteria2 | planned | runtime adapter is still needed |
| NaiveProxy | planned | runtime adapter is still needed |

Short and honest: this is not a multi-core client yet. Right now it is a TrustTunnel GUI with a multi-core direction.

## What already exists

- WPF UI with compact and expanded modes.
- Tray behavior.
- `tt://` link import.
- Manual server creation.
- TOML import / preview / tools.
- Typed endpoint, listener, routing and server profile models.
- Routing presets.
- Validation for host:port, DNS upstreams, CIDR, MTU and certificate fields.
- SOCKS5 credential generation.
- System proxy toggle for SOCKS5.
- Live network counters.
- Server diagnostics.
- Localization: English, Russian, Chinese, Persian.

## What is still missing

- Core selector inside profiles.
- A shared adapter layer for several cores.
- Safe persistent secret storage using DPAPI or Windows Credential Manager.
- Signed portable / release builds.
- Proper per-core documentation.
- JSON / YAML / URL imports for sing-box, Xray and other cores.
- Screenshots, GIFs and a short demo.

## Roadmap

### Near term

- [ ] Add GitHub description, topics and social preview.
- [ ] Publish the first `v0.1.0-alpha` release.
- [ ] Fix the install/readme path for normal users.
- [ ] Add real UI screenshots.

### Architecture

- [ ] Introduce `CoreKind`.
- [ ] Introduce a shared `IProxyCoreAdapter`.
- [ ] Move TrustTunnel runtime code into a dedicated adapter.
- [ ] Remove direct UI coupling to the TrustTunnel controller.
- [ ] Add core binary manifests: version, source, SHA256, license.

### New cores

- [ ] sing-box
- [ ] Xray-core
- [ ] Hysteria2
- [ ] NaiveProxy

### Before a stable release

- [ ] DPAPI / Windows Credential Manager for secrets.
- [ ] Signed installer.
- [ ] Trusted binary verification.
- [ ] QA for TUN, SOCKS5, HTTP proxy, service mode and tray logic.
- [ ] Release notes and changelog.

## Installation

There are no stable prebuilt binaries yet. When they exist, they will be published in `Releases`.

Right now the practical option is to build from source.

### Requirements

- Windows 10 or Windows 11
- .NET SDK 10
- TrustTunnel native files for real connections

### Build

```powershell
dotnet restore --configfile .\NuGet.Config
dotnet build .\TrustTunnelGuiClient.sln --no-restore
dotnet run --project .\src\TrustTunnel.Desktop\TrustTunnel.Desktop.csproj --no-build
dotnet format .\TrustTunnelGuiClient.sln --verify-no-changes --no-restore
```

### Desktop binary

```text
ProxyOpenHub.exe
```

## Native files

The TrustTunnel runtime can be placed in `native/bundled/win-x64` or next to the built desktop executable.

Preferred runtime:

- `vpn_easy.dll`
- `wintun.dll`
- `vpn_easy_service.exe`

CLI fallback:

- `trusttunnel_client.exe`
- `wintun.dll`

Planned future binaries:

- `sing-box.exe`
- `xray.exe`
- `hysteria.exe`
- `naive.exe`

## UI

Screenshots are not in the repository yet. This still needs to be fixed.

The first screenshot set should show:

- expanded mode;
- compact mode;
- TOML tools;
- routing profiles;
- diagnostics;
- live metrics.

## Documentation

- Russian main README: [README.md](README.md)
- Releases: [GitHub Releases](https://github.com/AkumaHacai/Proxy-Open-Hub/releases)
- Issues: [GitHub Issues](https://github.com/AkumaHacai/Proxy-Open-Hub/issues)

## Hosting

The links below are partner / referral links. If you use them, the project may receive a small benefit. This block does not affect the code, roadmap or technical decisions.

- AEZA — budget VPS, useful for quick tests: [aeza.net](https://aeza.net/?ref=520655)
- JustHost — useful if you need RU locations, including Moscow and Saint Petersburg: [justhost.ru](https://justhost.ru/?ref=286200)
- Serv.Host — another low-cost VPS option: [serv.host](https://serv.host/?from=24960)

## License

Proxy Open Hub source code is distributed under the Apache License 2.0. See `LICENSE.txt`.

Native cores and external binaries keep their own licenses. See `NOTICE.md` and the license files shipped next to the related binaries.

## Disclaimer

Proxy Open Hub is an independent project. It is not affiliated with TrustTunnel, sing-box, Xray-core, Hysteria2, NaiveProxy or Wintun.
