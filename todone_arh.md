# Modularity Architecture Progress

Start date: 2026-06-18

Goal: move Proxy Open Hub to a modular architecture where adapters remain in our
Rust code, while core binaries are installed, verified, updated, and isolated by
the managed core store.

## Phase A - Modularity Foundation

- [x] A1. Add `pinned_release` to the trusted source catalog.
- [x] A1. Validate pinned release metadata: version, asset name, SHA-256, allowed asset pattern.
- [x] A1. Reject installable GitHub sources without `pinned_release`.
- [x] A1. Check installed manifests against pinned release before verify/start.
- [x] A2. Add `CoreStore::list_installed()`.
- [x] A2. Add per-core `active.json` plus `CoreStore::active_version()` / `set_active_version()`.
- [x] A2. Mark a version active after successful install.
- [x] A2. Add CLI command `poh_cli core-list-installed`.
- [x] A2. Add zip/multifile install with zip-slip guard.
- [x] A2. Separate pinned archive SHA from installed file hashes for tamper detection.
- [ ] A2. Add GC for old versions: active + rollback retention.
- [x] A3. Add HTTPS downloader for pinned GitHub assets with SHA-256 verification.
- [x] A3. Add CLI `core-download-plan <core-id>` and `core-install <core-id> [executable-relative-path]`.
- [x] A4. Add session lifecycle states, single-instance file lock, stale lock recovery, and readiness probe helpers.
- [x] A4. Wire desktop TrustTunnel start/stop/status into session lock, startup readiness, and faulted session state.
- [ ] A4. Add long-lived watchdog/service layer for automatic crash rollback while the UI process is not polling.
- [x] A5. Add `CoreLaunchDescriptor` registry with executable path, working directory, runtime path mode, append args, and log file policy.
- [x] A5. Use descriptors for `core-install <core-id>` default executable paths.
- [x] A5. Use TrustTunnel descriptor for desktop launch args/log path instead of hardcoding command args in the desktop session flow.
- [ ] A5. Move bundled TrustTunnel lookup itself into managed store / descriptor-backed module resolution.

## Phase B - TrustTunnel As Managed Module

- [ ] Move bundled TrustTunnel into store layout `cores/trusttunnel/<version>/`.
- [ ] Keep pinned SHA checks for `trusttunnel_client.exe` and `wintun.dll`.
- [x] Launch TrustTunnel command args/log path through the shared descriptor/session layer.
- [ ] Resolve TrustTunnel executable from managed core store instead of app-local bundle lookup.
- [ ] Keep a migration path for the current app-local bundle until an official pinned download source exists.

## Phase C - NaiveProxy As First Downloadable Module

- [ ] Pin release metadata: version, asset name, SHA-256.
- [ ] Add `NaiveProxyAdapter` to `poh_core`.
- [ ] Import `config.json` and proxy URLs without storing plaintext passwords in profiles.
- [ ] Materialize `config.json` through secret placeholders + DPAPI secret store.
- [ ] Wire install flow through catalog/store/downloader.
- [ ] Add LocalProxy readiness probe and system proxy rollback.

## Already Connected To Earlier Work

- Secrets remain in DPAPI `ProtectedSecrets`; the modular path does not reintroduce plaintext state.
- Runtime/session/state/config/log files keep restrictive ACLs.
- Trusted source policy is strict: a downloadable core must have a pinned release before install UI can be enabled.
- Fake/swapped core protection now checks source owner/repo, asset pattern, pinned version/asset/SHA, archive SHA, and installed file hashes.
- Downloader only supports installable active GitHub-release sources and verifies the downloaded bytes before install staging.

## Next Safe Step

Start Phase B: move bundled TrustTunnel from the app-local `native/bundled`
lookup into the managed core store layout while keeping the current pinned
SHA checks and migration path.
