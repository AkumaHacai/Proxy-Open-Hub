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
- [x] A2. Add GC for old versions: active + rollback retention.
- [x] A3. Add HTTPS downloader for pinned GitHub assets with SHA-256 verification.
- [x] A3. Add CLI `core-download-plan <core-id>` and `core-install <core-id> [executable-relative-path]`.
- [x] A4. Add session lifecycle states, single-instance file lock, stale lock recovery, and readiness probe helpers.
- [x] A4. Wire desktop TrustTunnel start/stop/status into session lock, startup readiness, and faulted session state.
- [ ] A4. Add long-lived watchdog/service layer for automatic crash rollback while the UI process is not polling.
- [x] A5. Add `CoreLaunchDescriptor` registry with executable path, working directory, runtime path mode, append args, and log file policy.
- [x] A5. Use descriptors for `core-install <core-id>` default executable paths.
- [x] A5. Use TrustTunnel descriptor for desktop launch args/log path instead of hardcoding command args in the desktop session flow.
- [x] A5. Move bundled TrustTunnel lookup itself into managed store / descriptor-backed module resolution.

## Phase B - TrustTunnel As Managed Module (DONE - architecture pass 2026-06-18)

- [x] Move bundled TrustTunnel into store layout `cores/trusttunnel/<version>/`.
- [x] Keep pinned SHA checks for `trusttunnel_client.exe` and `wintun.dll`.
- [x] Launch TrustTunnel command args/log path through the shared descriptor/session layer.
- [x] Resolve TrustTunnel executable from managed core store instead of app-local bundle lookup.
- [x] Keep a migration path for the current app-local bundle until an official pinned download source exists.

### How Phase B was implemented (read before touching this area)

- New store API `CoreStore::install_manual_bundle(manifest, source_dir, trusted_sources)`
  in `crates/poh_core_store/src/lib.rs`. It installs a locally bundled, multi-file
  core into `cores/<core_id>/<version>/` by copying each file listed in
  `manifest.files` only after its bytes match the pinned per-file SHA-256, then
  reuses the same staging/rollback/`set_active_version` promotion path as the
  downloader. The `"manual"` sha256 sentinel is allowed (a bundle has no single
  archive hash) but per-file hashes are mandatory, so a bundled core is held to
  the same integrity bar as a downloaded one.
- Shared install plumbing was extracted to avoid divergence: `plan_paths`,
  `reset_staging`, `promote_staged`. `install()` and `install_manual_bundle()`
  now share the promote/rollback tail.
- Desktop launch now resolves through the store. `crates/poh_cli/src/desktop_state.rs`
  replaces `find_trusttunnel_client()` with `resolve_core(core_id)`:
  - `trusttunnel` -> `resolve_trusttunnel_core()`: fast path verifies the active
    store version; if missing/tampered it provisions the app-local bundle
    (`<app>/native/bundled/win-x64/`) into the store via `install_manual_bundle`,
    then `verify_core` before launch. This is the migration path.
  - any other core -> `resolve_store_core()`: must already be installed and
    verifies the active version (forward path for downloadable modules).
  - `POH_TRUSTTUNNEL_CORE_PATH` stays a dev-only, hash-checked direct path and
    never writes into the store.
- Pinned hashes live in one place now: `BUNDLED_TRUSTTUNNEL_SHA256` /
  `BUNDLED_WINTUN_SHA256` feed `bundled_trusttunnel_manifest()` (version
  `BUNDLED_TRUSTTUNNEL_VERSION = "1.0.49"`). The launch working directory comes
  from the descriptor (`ExecutableParent`), so `wintun.dll` loads from the same
  store version directory.
- Tests added: `store_installs_manual_bundle_with_pinned_file_hashes`,
  `store_rejects_manual_bundle_with_wrong_pinned_hash`. Full workspace: 49 tests
  pass; `cargo fmt --all --check` clean.

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

## Next Safe Step (handoff for the finishing coder)

Phase A and Phase B are done. The launch path is now core-agnostic: it always
resolves a verified core from the managed store, and TrustTunnel is just a
manual-bundle module that self-provisions on first run. Adding the next core is
now mostly an adapter + a pinned catalog entry, with no changes to the session
layer.

Recommended order:

1. **Phase C - NaiveProxy as the first downloadable module** (see
   `LLM_Cloud/naiveproxy-integration.md`, Stage 2-5). The plumbing it needs
   already exists:
   - catalog: add a real `pinned_release` (version, asset_name, sha256) to the
     `naiveproxy` entry in `core-registry/trusted-sources.json` and flip it to
     `status: active`, `install_enabled: true` once a version is confirmed;
   - install: `poh_cli core-install naiveproxy` already works through the
     downloader/store once the source is pinned + active;
   - launch: a `naiveproxy` descriptor already exists
     (`CoreLaunchDescriptor::for_core`, LocalProxy/`naive.exe`); `resolve_store_core`
     will resolve it with no session-layer changes.
   - remaining new code: `NaiveProxyAdapter` in `poh_core` (parse `config.json`
     and proxy URLs, materialize `config.json` via secret placeholders +
     `proxy.password` secret key in `poh_core_runner::is_secret_key`), a
     LocalProxy readiness probe wiring, and the desktop profile shape for a core
     without TUN. Do NOT store the proxy password in the profile; URL-encode it
     in the secret value (see the security note in the integration doc).

2. **Process lifecycle hardening (connect/disconnect/safe process control)** -
   full plan in `LLM_Cloud/process-lifecycle.md`. This is the riskiest area:
   `poh_cli` is an ephemeral CLI, so after start the OS handle is dropped and all
   stop/status is PID-based.
   - **P1 DONE (2026-06-18):** reuse-proof process identity (PID + creation time,
     closes F-3), graceful->force stop with confirmed exit + `CoreStopFailed`,
     runtime_dir cleanup with retries, `enforce_transition`/`IllegalTransition`,
     corrupt-`session.json` resilience. See process-lifecycle.md section 13 + done.md.
   - **P2 NEXT:** orphan reconciliation/reaping on app start + a `desktop-session-reset`
     force-reset, and the network safety-net (system-proxy ownership + unconditional
     revert; `network_effects` tracking; TUN/DNS/kill-switch repair). This is the
     real fix for "internet stays broken after a force-kill / crash" - graceful
     CTRL_BREAK in P1 is only best-effort.
   - **P3 (= A4):** long-lived supervisor with a Windows Job Object
     (`KILL_ON_JOB_CLOSE`) that guarantees no orphaned cores + real-time crash
     detection. Supersedes/expands the A4 item below.

3. **Remaining hardening (still open in Phase A):**
   - A4: long-lived watchdog/service for automatic crash rollback + system proxy
     revert while the UI is not polling (today faulted state is only detected on
     the next status poll). Detailed design is now in
     `LLM_Cloud/process-lifecycle.md` (Tier 2).
   - Signature/AuthentiCode validation before enabling the install/update UI for
     downloadable cores (`SignatureStatus` is still only `Unknown`).

### A2 GC implementation note

- `CoreStore::garbage_collect_old_versions(core_id, inactive_retention)` now
  keeps the active version plus the newest inactive version by default. It only
  deletes directories that are valid store segments and contain
  `core-manifest.json`; staging folders and unknown debris are left alone for
  diagnosis.
- `promote_staged()` calls GC after a successful active-version promotion, so
  normal installs and manual-bundle provisioning converge to active + rollback
  retention automatically.
- Regression coverage: installing `1.0.0 -> 1.1.0 -> 1.2.0` leaves `1.2.0`
  active and `1.1.0` as rollback, then an explicit retention `0` GC removes the
  inactive copy.

Do not reintroduce the old app-local `native/bundled` executable lookup in the
launch path; `app_local_bundle_dir()` is now only a provisioning source for the
store, not a launch target.
