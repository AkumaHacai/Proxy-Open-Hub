# Proxy Open Hub: Rust + Flutter Migration Plan

## Goal

Move Proxy Open Hub from a WPF-first prototype to a modular desktop hub where every network core owns its own capabilities, settings, import rules, runtime files, and update policy.

The application must not force TrustTunnel rules onto sing-box, NaiveProxy, Xray-core, Hysteria2, or future cores. The shared app layer should only manage identity, downloads, trust, lifecycle, UI state, diagnostics, logs, and safe user workflows.

## Current Backup

Current WPF project snapshot:

```text
C:\Users\mirot\Documents\TT gui\backups\ProxyOpenHub-WPF-backup-20260615-163305.zip
```

The archive excludes build output, git metadata, artifacts, and local IDE folders.

## Target Architecture

```text
proxy-open-hub/
  apps/
    desktop_flutter/          # Flutter desktop UI
  crates/
    poh_core/                 # shared domain: profiles, state, errors, events
    poh_core_runner/          # runtime config materialization and redaction
    poh_core_store/           # verified local core store, checksum, rollback
    poh_core_session/         # safe process launch for verified cores
    poh_cli/                  # development smoke CLI
    poh_ipc/                  # Flutter <-> Rust bridge
  core-registry/
    trusted-sources.json      # visible list of official sources
  data/
    profiles/                 # user profiles, one core_id per profile
    cores/                    # installed binaries by core/version/platform
```

## Core Adapter Contract

Every core is represented by an adapter. The adapter is the only layer that knows that core's config format and feature set.

Required adapter capabilities:

- `core_id`: stable id, for example `trusttunnel`, `sing-box`, `naiveproxy`.
- `display_name`, `protocol_colors`, `supported_platforms`.
- `detect_import(input)`: decides whether a link, TOML, JSON, YAML, or text belongs to this core.
- `parse_profile(input)`: produces a profile with `core_id`, human metadata, and core-owned config.
- `settings_schema()`: describes UI fields that appear only when the core is installed.
- `build_runtime_config(profile)`: writes the actual config used by the binary.
- `validate(profile)`: validates only rules that belong to the core.
- `start(profile)`, `stop()`, `status()`: lifecycle via shared runner.
- `diagnostics(profile)`: ping, HTTP check, binary presence, port checks, config dry run where supported.
- `migrate_config(old_version, new_version)`: optional per-core migration.

Shared app data should stay small:

```text
Profile {
  id,
  name,
  core_id,
  tags,
  last_used_at,
  ui_metadata,
  core_config
}
```

`core_config` is owned by the adapter. The main app does not try to normalize every core into one universal VPN model.

## Core Download And Trust Model

The app should show users where binaries come from before download.

Trusted source record:

```json
{
  "core_id": "sing-box",
  "display_name": "sing-box",
  "source_type": "github_release",
  "owner": "SagerNet",
  "repo": "sing-box",
  "license": "GPL-3.0-or-later",
  "homepage": "https://sing-box.sagernet.org/",
  "checksums": "required",
  "signature": "preferred",
  "enabled_by_default": false
}
```

Install flow:

1. User opens core selector.
2. App shows installed cores and trusted available cores.
3. User clicks install.
4. Rust updater downloads release metadata, artifact, checksum/signature.
5. Binary is extracted into `cores/{core_id}/{version}/{platform}`.
6. Adapter becomes active and its settings appear.
7. Profiles for that core become importable and runnable.

Update flow:

1. Check trusted source.
2. Download new version beside the old version.
3. Verify.
4. Switch installed version atomically.
5. Keep rollback version.

## Flutter UI Boundary

Flutter owns presentation only:

- main window expanded/compact;
- tabs for installed cores;
- profile list and per-core settings forms;
- import dialogs;
- logs and diagnostics views;
- theme, language, accessibility.

Rust owns behavior:

- downloads and updates;
- process/service lifecycle;
- config generation;
- secret storage;
- system proxy;
- TUN/admin checks;
- network counters;
- logs/events;
- validation.

Recommended IPC shape:

- Commands: `install_core`, `import_profile`, `start_profile`, `stop_profile`, `save_profile`, `run_diagnostics`.
- Event stream: `CoreInstallProgress`, `ProfileStatusChanged`, `TrafficMetrics`, `LogLine`, `DiagnosticsResult`.

This can start as local JSON-RPC over stdin/stdout or a localhost named pipe. FFI can come later if needed.

## What To Reuse From WPF Prototype

Keep as reference:

- import behavior for TrustTunnel links and TOML;
- typed TrustTunnel config model;
- validator test cases;
- redaction rules;
- routing presets idea;
- live metrics behavior;
- localization strings and product terminology;
- Proxy Open Hub branding and logo assets.

Rewrite:

- UI layout and windows;
- WPF-specific styles;
- WPF state store;
- one-size-fits-all settings model.

## Migration Stages

### Stage 0: Freeze Current WPF State

- Keep current GitHub branch as legacy WPF alpha.
- Keep the local backup zip.
- Add a new branch for the Rust + Flutter migration.

### Stage 1: Rust Workspace Skeleton

- Create Rust workspace under `crates/`.
- Add domain types: `CoreId`, `ProfileId`, `Profile`, `CoreManifest`, `CoreStatus`, `ConnectionState`.
- Add adapter trait and a fake demo adapter.
- Add JSON serialization tests.

Exit criteria: CLI can list fake cores and import a fake profile.

### Stage 2: TrustTunnel Adapter

- Port TrustTunnel import and TOML generation logic from C# to Rust.
- Preserve existing test vectors.
- Implement runtime config writer.
- Implement process runner for existing TrustTunnel core.

Exit criteria: Rust CLI can import and start a TrustTunnel profile.

### Stage 3: Core Registry And Downloads

- Add trusted source registry.
- Add installed core index.
- Implement download, checksum, extract, rollback.
- Keep sources visible in UI and docs.

Exit criteria: app can show TrustTunnel installed and future cores available but not active until installed.

### Stage 4: Flutter Shell

- Rebuild the reference UI in Flutter.
- Connect to Rust IPC.
- Implement expanded and compact modes.
- Implement per-core tabs and profile list.

Exit criteria: Flutter UI can list cores/profiles and start/stop TrustTunnel through Rust.

### Stage 5: Per-Core Settings

- Add schema-driven settings forms.
- TrustTunnel settings appear only for TrustTunnel.
- Future core settings appear only when the core is installed.
- Keep global settings limited to theme, language, trust sources, diagnostics defaults, and app behavior.

Exit criteria: no duplicated SOCKS/HTTP/TUN settings between app settings and core profile settings.

### Stage 6: Security And Release

- Move secrets to OS-protected storage.
- Add signed installer.
- Add GitHub Actions builds.
- Add release checklist for third-party licenses.

Exit criteria: reproducible alpha release with visible third-party notices.

## First Implementation Slice

The first useful slice should be intentionally small:

1. Create branch `codex/rust-flutter-migration`.
2. Add `crates/poh_core` and `crates/poh_cli`.
3. Define adapter trait and profile model.
4. Port only TrustTunnel import detection and TOML output tests.
5. Add Flutter app shell after the Rust model stops moving every hour.

This keeps the foundation modular without blocking on visual work too early.

## Progress: 2026-06-16

- Created Rust workspace and locked it to the stable toolchain.
- Ported TrustTunnel import detection, deeplink parsing, TOML parsing, and TOML generation to Rust.
- Added trusted source registry with visible source metadata for TrustTunnel and planned future cores.
- Added runtime materialization with secret placeholders and redacted previews.
- Added local core store with checksum validation, staging install, rollback, installed manifest verification, and path traversal protection.
- Added session layer that launches only verified cores and redacts captured process output.
- Added CLI smoke commands: `sources`, `runtime-smoke`, `store-smoke`, and `session-smoke`.
- Installed Flutter SDK locally under `C:\Users\mirot\devtools\flutter`.
- Added Flutter desktop shell under `apps/desktop_flutter`, based on the local `newgui` and `newaddgui` references.
- Generated the Windows runner and verified `flutter analyze`, `flutter test`, and `flutter build windows`.
- Replaced Flutter demo server data with real profiles loaded from `%LOCALAPPDATA%\ProxyOpenHub\desktop-state.json`, with a fallback to the legacy TrustTunnel state path.
- Added Proxy Open Hub logo assets to the Flutter shell and Windows runner icon.
- Added `poh_cli desktop-session-plan <state-path> <profile-id>` so Flutter can ask Rust to convert a saved TrustTunnel profile into a redacted runtime session plan.
- Connected the Flutter Connect flow to the Rust session-plan command. Actual core process launch is still the next migration step.

## Open Decisions

- IPC: JSON-RPC over named pipe vs FFI bridge.
- Repository strategy: keep WPF and new app in one repository during migration, or split after Rust skeleton is stable.
- Secret storage crate and Windows credential backend.
- Which future core is second after TrustTunnel: sing-box is the best candidate because it has broad protocol coverage.
- Release naming: keep `Proxy Open Hub` for the product and use core names only inside tabs.
