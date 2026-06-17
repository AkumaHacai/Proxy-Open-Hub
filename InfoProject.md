# Proxy Open Hub: Project Map And Rust Debug Guide

Proxy Open Hub is a modular Windows proxy/VPN hub. The project is migrating from
the old WPF/C# client to:

- Rust backend for core adapters, config import, secret handling, trusted core
  storage, process launch, logs, and security checks.
- Flutter desktop UI for the main window, compact/expanded modes, settings,
  logs, import dialogs, and future per-core screens.
- TrustTunnel as the first working core. sing-box, NaiveProxy, Xray-core, and
  Hysteria2 are planned optional modules.

The old WPF project and HTML references are archived under:

```text
backups/Old Files/2026-06-16-wpf-and-references/
```

The active UI is:

```text
apps/desktop_flutter
```

## Repository Map

```text
Cargo.toml                         Rust workspace
crates/poh_core                    core adapters, TrustTunnel parser, security policy
crates/poh_core_runner             runtime file materialization + secret substitution
crates/poh_core_session            low-level process launch helpers
crates/poh_core_store              trusted core install/download/verify store
crates/poh_cli                     CLI bridge used by Flutter
apps/desktop_flutter               Flutter desktop UI
core-registry/trusted-sources.json trusted source registry for optional cores
native/bundled/win-x64             bundled TrustTunnel runtime
scripts/check.ps1                  local verification
scripts/build-desktop.ps1          build Rust CLI + Flutter Windows app
scripts/run-desktop.ps1            run the built desktop app
```

## Rust Layer

### `poh_core`

This crate owns the adapter-facing model:

- `CoreRegistry` lists built-in adapters.
- `TrustTunnelAdapter` detects and imports `tt://` links and TrustTunnel TOML.
- `TrustTunnelTomlParser` reads endpoint, listener, routing, DNS, proxy, and
  secret-related fields.
- `TrustTunnelTomlBuilder` builds runtime TOML again from the internal profile.
- Security policies validate trusted sources, runtime paths, environment keys,
  pinned release data, installed manifests, and SHA-256 values.

Important: the generic `Profile` model does not store plaintext secrets.
Parsers return secret candidates separately. Desktop import converts them into
`secret://...` references and stores the real values in DPAPI-protected state.

### `poh_core_runner`

This crate materializes runtime files before launch:

- substitutes secrets through a resolver;
- writes generated configs only inside the approved runtime directory;
- rejects absolute paths, `..`, Windows backslash paths, unsafe env keys, and
  oversized generated files;
- returns redacted previews for UI/log display.

### `poh_core_session`

This crate contains process-launch primitives. It is intentionally low-level
right now: launch spec, executable checks, process start/wait, and redacted
output. The next architecture step is a higher-level `SessionManager` with one
active core session, readiness probes, stop timeout, watchdog, and rollback
hooks.

### `poh_core_store`

This crate is the managed core basket:

- validates trusted catalog entries;
- requires active installable GitHub sources to have `pinned_release`;
- stores installed versions under `cores/<core_id>/<version>/`;
- writes per-core `active.json`;
- supports single-file and zip/multifile artifacts;
- extracts zip files with zip-slip and duplicate-path guards;
- records hashes for installed files and verifies them later;
- downloads only pinned GitHub release assets and verifies SHA-256 before the
  bytes can become an install request.

Downloader rules:

- no "latest release" lookup;
- no download from unpinned or planned sources;
- URL is derived from `owner`, `repo`, `pinned_release.version`, and
  `pinned_release.asset_name`;
- bytes are size-limited;
- SHA-256 is checked before install staging;
- install UI is still disabled until real pins and descriptors are added.

### `poh_cli`

The Flutter app talks to Rust through this CLI. Useful commands:

```powershell
.\target\debug\poh_cli.exe list
.\target\debug\poh_cli.exe sources
.\target\debug\poh_cli.exe detect "<profile text>"
.\target\debug\poh_cli.exe core-list-installed
.\target\debug\poh_cli.exe core-download-plan <core-id>
.\target\debug\poh_cli.exe core-install <core-id> <executable-relative-path>
.\target\debug\poh_cli.exe desktop-import-profile C:\path\to\profile.toml
.\target\debug\poh_cli.exe desktop-session-plan <desktop-state.json> <profile-id>
.\target\debug\poh_cli.exe desktop-session-start <desktop-state.json> <profile-id>
.\target\debug\poh_cli.exe desktop-session-status
.\target\debug\poh_cli.exe desktop-session-log
.\target\debug\poh_cli.exe desktop-session-stop
```

`core-install` currently needs `<executable-relative-path>` because
`CoreLaunchDescriptor` is not implemented yet. After descriptors exist, this
will be known per core.

## Desktop State And Secrets

Current state path:

```text
%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json
```

Legacy state path that may be migrated:

```text
%LOCALAPPDATA%\TrustTunnel\desktop-state.json
```

Import flow:

1. Rust parses `tt://` or TOML.
2. Rust creates a desktop profile with secret references.
3. Real password/client-random values are protected with Windows DPAPI.
4. Legacy plaintext `Secrets` are migrated into `ProtectedSecrets` on load.
5. Flutter reloads state and shows the imported server.

## Debug Commands

Full project check:

```powershell
.\scripts\check.ps1
```

Rust only:

```powershell
cargo fmt --all --check
cargo test --workspace
cargo build -p poh_cli
```

Build and run desktop:

```powershell
.\scripts\build-desktop.ps1
.\scripts\run-desktop.ps1
```

Release exe after build:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe
```

The app must be able to find:

```text
apps/desktop_flutter/build/windows/x64/runner/Release/poh_cli.exe
```

Override CLI path for local debugging:

```powershell
$env:POH_CLI_PATH = "C:\Users\mirot\Documents\TT gui\target\debug\poh_cli.exe"
```

## Safe Import Debug

Use a temporary `%LOCALAPPDATA%` to test import without touching real profiles:

```powershell
$temp = Join-Path $env:TEMP ("poh-debug-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $temp | Out-Null
$old = $env:LOCALAPPDATA
$env:LOCALAPPDATA = $temp

.\target\debug\poh_cli.exe desktop-import-profile C:\path\to\profile.toml
Get-Content "$temp\ProxyOpenHub\desktop-state.json"

$env:LOCALAPPDATA = $old
Remove-Item -Recurse -Force $temp
```

## TrustTunnel Runtime Debug

Bundled runtime:

```text
native/bundled/win-x64/trusttunnel_client.exe
native/bundled/win-x64/wintun.dll
```

Session commands:

```powershell
.\target\debug\poh_cli.exe desktop-session-plan "%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json" <profile-id>
.\target\debug\poh_cli.exe desktop-session-start "%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json" <profile-id>
.\target\debug\poh_cli.exe desktop-session-status
.\target\debug\poh_cli.exe desktop-session-log
.\target\debug\poh_cli.exe desktop-session-stop
```

For local development only, an alternate TrustTunnel binary can be tested with
`POH_DEV=1` or a debug build. The binary still must match the pinned SHA-256:

```powershell
$env:POH_DEV = "1"
$env:POH_TRUSTTUNNEL_CORE_PATH = "C:\path\to\trusttunnel_client.exe"
```

## Current Status

Ready:

- Rust workspace builds and tests.
- TrustTunnel TOML/deeplink import works.
- UTF-8 BOM TOML files are accepted.
- Flutter Add Server calls Rust import.
- Flutter loads real profiles from `desktop-state.json`.
- Connect/Disconnect starts/stops the real bundled TrustTunnel core.
- Logs and redacted previews flow through Rust.
- Live network metrics read OS counters while connected.
- DPAPI `ProtectedSecrets` are used for imported secrets.
- Import preview blocks risky TLS/LAN settings until confirmed.
- Trusted core store supports active version tracking, zip artifacts, pinned
  archive SHA, installed file hashes, and pinned GitHub download planning.

Still in progress:

- `SessionManager` with a strict state machine and readiness probes.
- `CoreLaunchDescriptor` for per-core executable/config/log knowledge.
- TrustTunnel migration from app-local bundle into managed core store.
- NaiveProxy/sing-box/Xray/Hysteria adapters and install UI.
- Installer, tray behavior, and release packaging.
