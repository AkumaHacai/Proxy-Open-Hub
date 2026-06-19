# Proxy Open Hub

![Proxy Open Hub logo](logo/proxy-open-hub-horizontal.svg)

Proxy Open Hub is a modular Windows desktop hub for proxy and VPN cores. The
current application combines a Rust backend with a Flutter desktop UI and ships
with a bundled TrustTunnel runtime as the first working core.

This is not the official TrustTunnel client. It is an independent shell designed
to manage multiple cores behind one UI. TrustTunnel is the current packaged
runtime; sing-box, NaiveProxy, Xray-core, and Hysteria2 are represented in the
core model and UI while their trusted install/update flow is still being
hardened.

Russian README: [README_ru.md](README_ru.md)

## Portable Pre-release

The current Windows x64 portable package is:

```text
ProxyOpenHub-portable-win-x64-2026-06-19-fix1.zip
```

The archive contains:

```text
proxy_open_hub.exe          Flutter desktop app
poh_cli.exe                 Rust CLI bridge used by the UI
data/                       Flutter runtime data
native/bundled/win-x64/     bundled TrustTunnel client and Wintun DLL
README.txt                  portable package notes
```

To run it:

1. Download the zip from GitHub Releases.
2. Extract the whole `ProxyOpenHub-portable-win-x64` directory.
3. Run `proxy_open_hub.exe`.

Application state is stored in:

```text
%LOCALAPPDATA%\ProxyOpenHub\
```

Secrets are stored through Windows DPAPI and are tied to the current Windows
user. This build is a pre-release: expect unsigned-binary warnings, incomplete
installer/tray behavior, and manual update flow.

## Project Layout

```text
Cargo.toml                         Rust workspace root
crates/                            Rust backend crates
  poh_cli/                         CLI bridge used by Flutter
  poh_core/                        core adapters, import/parsing, security policy
  poh_core_runner/                 runtime config materialization
  poh_core_session/                process launch helpers and descriptors
  poh_core_store/                  trusted core install/verify store
apps/desktop_flutter/              Flutter desktop application
core-registry/trusted-sources.json trusted core source registry
native/bundled/win-x64/            bundled TrustTunnel runtime files
logo/                              source logo assets
docs/                              migration/security/native notes
backups/Old Files/                 old WPF app, references, archived duplicates
```

The old WPF/.NET project and HTML references are archived under
`backups/Old Files/`.

## Quick Start For Development

From the repository root:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

`build-desktop.ps1` builds the Rust CLI and the Flutter Windows app, then copies
`poh_cli.exe` beside `proxy_open_hub.exe` so the UI can find the backend.

Built desktop executable:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe
```

Bundled TrustTunnel runtime is expected here:

```text
native/bundled/win-x64/trusttunnel_client.exe
native/bundled/win-x64/wintun.dll
```

For local debugging, the backend path can be overridden:

```powershell
$env:POH_CLI_PATH = "C:\path\to\poh_cli.exe"
```

TrustTunnel core override is intentionally restricted to debug/dev runs and must
still match the pinned SHA-256 when the security policy requires it:

```powershell
$env:POH_DEV = "1"
$env:POH_TRUSTTUNNEL_CORE_PATH = "C:\path\to\trusttunnel_client.exe"
```

## Build And Check

Rust backend:

```powershell
C:\Users\mirot\.cargo\bin\cargo.exe fmt --all --check
C:\Users\mirot\.cargo\bin\cargo.exe clippy --workspace -- -D warnings
C:\Users\mirot\.cargo\bin\cargo.exe test --workspace
C:\Users\mirot\.cargo\bin\cargo.exe build -p poh_cli
```

Flutter UI:

```powershell
cd .\apps\desktop_flutter
C:\Users\mirot\devtools\flutter\bin\dart.bat format lib test
C:\Users\mirot\devtools\flutter\bin\flutter.bat analyze
C:\Users\mirot\devtools\flutter\bin\flutter.bat test
C:\Users\mirot\devtools\flutter\bin\flutter.bat build windows
```

Combined scripts:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1 -Build
```

## CLI Bridge

Flutter talks to Rust through `poh_cli.exe`. Important desktop commands:

```text
desktop-list-profiles <state-path>
desktop-core-schema <core_id>
desktop-core-modes <core_id>
desktop-validate-profile <state-path>    # JSON on stdin
desktop-update-profile <state-path>      # JSON on stdin
desktop-preview-profile <input-text-file|->
desktop-import-profile <input-text-file|->
desktop-session-plan <state-path> <profile-id>
desktop-session-start <state-path> <profile-id>
desktop-session-supervise <state-path> <profile-id>
desktop-session-stop
desktop-session-reset
desktop-session-status
desktop-session-log
```

Per-core route settings are stored in `desktop-state.json` through
`RoutePresetsByCore`, `ActiveRouteByCore`, and `ActiveModeByCore`.

## Current Status

Implemented in the Rust + Flutter path:

- Real desktop profile loading from `%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json`.
- TrustTunnel TOML and `tt://` import with DPAPI-protected secrets.
- NaiveProxy JSON/proxy URL import path and generic per-core profile model.
- Per-core profile list, active-core filtering, and core-aware accent colors.
- Schema-driven profile editor backed by `desktop-core-schema`,
  `desktop-validate-profile`, and `desktop-update-profile`.
- Per-core route modes and user route presets backed by the desktop-state
  contract.
- TrustTunnel session plan/start/stop/status/log lifecycle through Rust CLI.
- Supervised session launch path, lifecycle states, single-instance lock,
  readiness probes, and faulted-session reporting.
- Redacted runtime previews and log redaction.
- System proxy, DNS, route, firewall, and kill-switch rollback ledger.
- Flutter main UI with compact/expanded modes, settings, routes, logs, import,
  profile editing, live network metrics, and core tabs.
- Trusted-source registry and core store foundations for future downloadable
  cores.
- ZIP/multifile core artifact handling with zip-slip guards and installed file
  hashes.

Still in progress:

- Full trusted download/update UI for sing-box, NaiveProxy, Xray-core, and
  Hysteria2.
- Exact TrustTunnel/Wintun adapter matching for live traffic.
- Streaming logs instead of manual refresh.
- Per-core advanced settings pages.
- Installer, tray behavior, code signing, and automatic update flow.
- Long-lived service/watchdog behavior and broader integration tests.

## License

Proxy Open Hub source code is licensed under the Apache License 2.0. See
`LICENSE.txt`.

Bundled native components and future downloadable cores keep their own licenses.
See `NOTICE.md` and license files shipped beside each native binary.
