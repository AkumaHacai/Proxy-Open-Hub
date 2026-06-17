# Proxy Open Hub

![Proxy Open Hub logo](logo/proxy-open-hub-horizontal.svg)

Proxy Open Hub is a modular Windows proxy/VPN hub. The active migration target is:

- Rust backend for core adapters, trusted sources, config materialization, process lifecycle, secrets, logs, and security checks.
- Flutter desktop UI for the main client, settings, logs, and future per-core screens.
- Bundled TrustTunnel CLI core as the first working runtime.

The project is not the official TrustTunnel client. It is an independent desktop shell that currently supports TrustTunnel profiles and is being prepared for optional cores such as sing-box, NaiveProxy, Xray-core, and Hysteria2.

## Active Project Layout

These are the folders you usually need:

```text
Cargo.toml                         Rust workspace root
crates/                            Rust backend crates
  poh_cli/                         CLI bridge used by Flutter
  poh_core/                        Core adapters, TrustTunnel parser/builder, security policy
  poh_core_runner/                 Runtime config materialization
  poh_core_session/                Process launch helpers
  poh_core_store/                  Trusted core install/verify store
apps/desktop_flutter/              Flutter desktop application
core-registry/trusted-sources.json Trusted core source registry
native/bundled/win-x64/            Bundled TrustTunnel runtime files
logo/                              Source logo assets
docs/                              Migration/security/native notes
backups/Old Files/                 Old WPF app, HTML references, archived duplicates
```

The old WPF/.NET project and HTML references were moved to:

```text
backups/Old Files/2026-06-16-wpf-and-references/
```

`.vs/` may remain in the root if Visual Studio has it locked. It is local IDE cache, not part of the active app.

## Quick Start

From the repository root:

```powershell
.\scripts\check.ps1
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

`build-desktop.ps1` builds both layers and copies `poh_cli.exe` beside the
Flutter executable so the Connect button can find the Rust backend.

## Built EXE

After a Flutter Windows build, run:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe
```

The Rust helper CLI is:

```text
target/debug/poh_cli.exe
```

The build script also copies it here:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/poh_cli.exe
```

Flutter finds `poh_cli.exe` beside the app or by walking parent folders. You can override it:

```powershell
$env:POH_CLI_PATH = "C:\Users\mirot\Documents\TT gui\target\debug\poh_cli.exe"
```

TrustTunnel bundled runtime is expected here:

```text
native/bundled/win-x64/trusttunnel_client.exe
native/bundled/win-x64/wintun.dll
```

For local debugging only, you can override the core path. This is blocked in normal runs; set `POH_DEV=1` or use a debug build, and the binary still has to match the pinned bundled SHA-256:

```powershell
$env:POH_TRUSTTUNNEL_CORE_PATH = "C:\path\to\trusttunnel_client.exe"
```

## Build And Check

Rust backend:

```powershell
C:\Users\mirot\.cargo\bin\cargo.exe fmt --all --check
C:\Users\mirot\.cargo\bin\cargo.exe test --workspace
C:\Users\mirot\.cargo\bin\cargo.exe build -p poh_cli
```

Flutter UI:

```powershell
cd .\apps\desktop_flutter
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

Quick backend session commands:

```powershell
.\target\debug\poh_cli.exe desktop-session-status
.\target\debug\poh_cli.exe desktop-session-log
```

Quick TrustTunnel import command:

```powershell
.\target\debug\poh_cli.exe desktop-import-profile C:\path\to\profile.toml
```

The Flutter Add Server button uses the same command through the bundled
`poh_cli.exe` and writes `%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json`.

`desktop-session-start` starts the real TrustTunnel core. Use it only when you are ready for a real local SOCKS/TUN runtime:

```powershell
.\target\debug\poh_cli.exe desktop-session-start <desktop-state.json> <profile-id>
.\target\debug\poh_cli.exe desktop-session-stop
```

## Current Status

Implemented in the Rust + Flutter path:

- Real saved-profile loading from `desktop-state.json`.
- TrustTunnel TOML session materialization with real saved secrets.
- Redacted runtime preview and logs.
- Real `trusttunnel_client.exe` start/stop/status lifecycle through Rust CLI.
- Flutter main UI with compact/expanded modes.
- Flutter settings shell wired into the main UI.
- Flutter logs shell wired into the main UI.
- Flutter Add Server import shell wired into the main UI.
- TrustTunnel TOML/tt-link import into `%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json`.
- Imported secrets are stored as DPAPI-protected `ProtectedSecrets`; legacy plaintext `Secrets` are migrated on load.
- Import preview requires confirmation for high-risk TLS and LAN listener settings before saving.
- TOML parser handles UTF-8 BOM files from Windows editors.
- Persistent app settings saved to `%LOCALAPPDATA%\ProxyOpenHub\app-settings.json`.
- Live network metrics service based on OS counters while connected.
- Combined PowerShell build/check/run scripts for the Rust + Flutter app.
- Trusted-source registry scaffold for future optional cores.
- Core store can list installed cores and track the active version per core.
- Core store supports zip/multifile artifacts with zip-slip guards and installed file hashes.
- Installable GitHub-release cores now require pinned release metadata before download UI can be enabled.

Still in progress:

- Exact TrustTunnel/Wintun adapter matching for live traffic.
- Log streaming instead of manual refresh.
- Full routing/profile editor migration from WPF to Flutter.
- Trusted download/update UI for sing-box, NaiveProxy, Xray-core, and Hysteria2.
- Installer, tray behavior, and packaging.

## License

Proxy Open Hub source code is licensed under the Apache License 2.0. See `LICENSE.txt`.

Bundled native components and future downloadable cores keep their own licenses. See `NOTICE.md` and license files shipped beside each native binary.
