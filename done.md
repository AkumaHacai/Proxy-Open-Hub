# Done - Security And Modularity Work

Date: 2026-06-18

## LLM_Cloud Security Pass

- Read the local audit material from `LLM_Cloud`.
- Hardened bundled TrustTunnel launch:
  - removed broad CWD/parent-folder search for `trusttunnel_client.exe`;
  - normal launch uses the fixed app-local bundle path;
  - `trusttunnel_client.exe` and `wintun.dll` are checked by pinned SHA-256;
  - `POH_TRUSTTUNNEL_CORE_PATH` is allowed only in dev/debug mode and still must match the pinned hash.
- Updated `scripts/build-desktop.ps1` to copy bundled native runtime files beside the Flutter Windows app.
- Moved new imported secrets to DPAPI-backed `ProtectedSecrets`.
- Added migration from legacy plaintext `Secrets` to `ProtectedSecrets`.
- Added restrictive Windows ACLs for state/runtime/config/session/log files.
- Changed Flutter import flow so imported text goes through Rust stdin instead of a temporary file.
- Added size limits for import input and desktop state.
- Added import preview before save and explicit confirmation for high-risk TLS/LAN listener warnings.
- Added persistent UI risk indicators for profiles with disabled TLS verification or custom certificate material.
- Improved log redaction for `tt://`, password, client random, and known secret values.
- Improved Windows process status/stop checks by parsing `tasklist` CSV and checking expected image name.
- Hardened relative path validation against Windows reserved device names.
- Updated `.gitignore` for IDE/cache/temp/build/audit artifacts.

## Modularity Phase A

- Added `pinned_release` support to trusted sources.
- Installable GitHub sources must now be active, checksum-required, and pinned.
- Installed manifests are checked against owner/repo, source type, allowed asset pattern, pinned version, pinned asset, and pinned archive SHA.
- Added `CoreStore::list_installed()`.
- Added per-core `active.json`, `active_version()`, and `set_active_version()`.
- Added CLI command `poh_cli core-list-installed`.
- Added zip/multifile core install with:
  - zip-slip guard;
  - duplicate archive path rejection;
  - empty archive rejection;
  - Windows reserved device path rejection;
  - installed file hash collection;
  - tamper verification for installed files.
- Added core-store GC for old versions:
  - normal installs now keep the active version plus the newest inactive version
    as rollback;
  - only valid installed-version directories with `core-manifest.json` are
    deleted, leaving staging/unknown debris for diagnosis;
  - regression test covers `1.0.0 -> 1.1.0 -> 1.2.0` retention and explicit
    inactive-retention `0`.
- Added pinned GitHub downloader:
  - no "latest release" lookup;
  - no planned/unpinned source download;
  - release asset URL is derived from trusted catalog data;
  - artifact size is limited;
  - SHA-256 is verified before install request creation.
- Added CLI commands:
  - `poh_cli core-download-plan <core-id>`;
  - `poh_cli core-install <core-id> [executable-relative-path]`.
- Added session lifecycle foundation:
  - `SessionLifecycleState`;
  - session file lock with stale-lock recovery;
  - startup readiness probes;
  - shared startup/stop timing model.
- Wired desktop TrustTunnel sessions into the new lifecycle foundation:
  - start/stop are single-instance protected;
  - SOCKS mode uses TCP readiness instead of a blind sleep;
  - TUN mode keeps a short delay-only startup grace;
  - dead processes are marked `faulted` instead of silently losing session context;
  - `session.json` writes are atomic.
- Added launch descriptor foundation:
  - descriptor registry for TrustTunnel, sing-box, NaiveProxy, Xray-core, and Hysteria2;
  - default executable paths for `core-install <core-id>`;
  - descriptor-based working directory/runtime path behavior;
  - TrustTunnel desktop launch now gets command args and log file policy from the descriptor.
- Rewrote `InfoProject.md` and `todone_arh.md` into clean ASCII documentation.

## Modularity Phase B - TrustTunnel As Managed Module (architecture pass)

- Added `CoreStore::install_manual_bundle()` so a locally bundled, multi-file
  core is installed into the managed store layout `cores/<core_id>/<version>/`,
  copying each file only after its bytes match a pinned per-file SHA-256.
- Extracted shared install plumbing (`plan_paths`, `reset_staging`,
  `promote_staged`) so manual-bundle and download installs share the same
  staging/rollback/active-version promotion path.
- Replaced the app-local `find_trusttunnel_client()` lookup with store-based
  `resolve_core()` in the desktop session flow:
  - TrustTunnel self-provisions from the app-local bundle into the store on
    first run, then launches from the store (migration path preserved);
  - re-provisions automatically if the stored copy is missing or tampered;
  - other cores resolve from the store's active installed version (forward path
    for downloadable modules like NaiveProxy);
  - `POH_TRUSTTUNNEL_CORE_PATH` remains a dev-only, hash-checked direct path.
- Pinned `trusttunnel_client.exe` / `wintun.dll` SHA-256 checks are preserved and
  now expressed once as the store manifest's per-file hash list.
- Launch working directory/log path continue to come from the launch descriptor,
  so `wintun.dll` loads from the same store version directory.
- Tests added for manual-bundle install (happy path + wrong-hash rejection +
  post-install tamper detection).

## Process Lifecycle P1 - Identity + safe stop (architecture pass)

See `LLM_Cloud/process-lifecycle.md` (section 13, P1) for the full design.

- Added reuse-proof process identity: `PersistedDesktopSession.creation_time_100ns`,
  captured right after spawn via `OpenProcess`/`GetProcessTimes`. `is_session_process_running`
  now matches PID + image name + creation time (legacy sessions fall back to image name),
  so a recycled PID can no longer be mistaken for - or killed as - our core (closes F-3).
- Safe stop: best-effort graceful (`CREATE_NEW_PROCESS_GROUP` at spawn + `CTRL_BREAK`),
  then force `taskkill /F`, then confirm the process is actually gone by identity. If it
  survives, the session is marked `Faulted` and `CoreStopFailed` is returned instead of
  falsely reporting "stopped".
- `runtime_dir` (which holds the materialized plaintext `config.toml`) is removed only
  after confirmed exit, with retries (closes part of F-2).
- `enforce_transition` + `IllegalTransition` error; used in start (->Running) and stop
  (->Stopping). State machine extended so stop is reachable from Preparing/Starting/Faulted.
- `load_session` is resilient to a corrupt `session.json` (treated as no session).
- Tests: +5 in poh_cli (identity / creation-time / corrupt-session parse), +1 in
  poh_core_session (stop reachable from any active state).
- Remaining (handed off): graceful CTRL_BREAK is best-effort (often no console -> force
  kill), so the network safety-net + orphan reconciliation (P2) and the Tier-2 supervisor
  with a Job Object (P3) are still required. See process-lifecycle.md.

## Process Lifecycle P2 - Reset/reconcile slice

- Added CLI command `desktop-session-reset` for a user/support "repair state"
  action: it acquires the session lock, stops the verified session process if it
  is alive, confirms exit by process identity, removes `session.json`, and
  cleans orphaned runtime directories.
- Added reconciliation before `desktop-session-start` and `desktop-session-stop`:
  - dead sessions are cleared before a new start;
  - a crashed-in-the-middle `Stopping` session is reaped before reporting stop
    complete;
  - old `Idle` live sessions are migrated to `Running`;
  - live `Running`/`Starting`/`Faulted` sessions are preserved/adopted instead
    of launching a second inbound.
- Remaining P2: system proxy ownership/revert and broader network safety-net
  for TUN/DNS/kill-switch repair after force-kill or crash.

## Modularity Phase C - NaiveProxy adapter slice

- Added `NaiveProxyAdapter` to `poh_core` and registered it in the CLI registry.
- Added NaiveProxy config models for `listen`, `proxy`, and safe advanced flags.
- Import supports:
  - `config.json` with `listen` + `proxy`;
  - bare proxy URLs such as `https://user:pass@host` / `quic://...`.
- Proxy and listener passwords are extracted into secret candidates
  (`proxy.password`, `listen.password`) and are not stored in `Profile.core_config`.
- `config.json` runtime output uses secret placeholders, and
  `poh_core_runner` now allows/substitutes NaiveProxy secret keys.
- Added userinfo URL redaction so logs/previews hide
  `scheme://user:password@host` values.
- Tests added for detection, import, plaintext avoidance, placeholder runtime
  generation, runner substitution, registry detection, and URL userinfo
  redaction.
- Remaining Phase C: choose/pin a real NaiveProxy release, enable the catalog
  install path, add generic desktop profile storage/import for non-TUN cores,
  wire LocalProxy readiness, and add system-proxy ownership/rollback.

## UI Pass 1 - Settings layout fix, routing skeleton, motion tokens

Full plan/notes in `LLM_Cloud/ui-layout-and-animation.md` (section 8).

- Fixed the critical compact-mode bug where Settings text wrapped one letter per
  line: `SettingsShell` no longer hard-codes 760x600 (sizes to available room,
  collapses the nav to an icon rail below 620px); `_Field` is responsive
  (label stacks above the control when narrow); `_Segmented`/`_AccentSwatches`
  use `Wrap` so they never overflow.
- Added a `Routing` settings tab skeleton (mode / rules / DNS / kill switch /
  core-specific), values local-only for now (banner says so).
- Added `PohMotion` motion tokens and unified the compact<->expanded curves:
  layout `AnimatedPositioned` and the window resize now share
  `Curves.fastOutSlowIn`, reducing the "jumping" during the morph.
- Verified: `dart format`, `flutter analyze` (No issues), `flutter test` (3/3,
  incl. new compact-width no-overflow + Routing-tab regression tests).
- Remaining (UI pass 2): compact bottom-sheet + blur overlay host, native window
  min size, `_AppearIn` entrance, wiring Routing to desktop-state.

## Verified

- `cargo fmt --all` / `cargo fmt --all --check` (clean)
- `cargo test --workspace` (63 tests pass)
- `cargo build --workspace`
- `cargo clippy --workspace` (clean)
- Earlier security/Phase A pass also verified:
- `cargo fmt --all`
- `cargo test --workspace`
- Previous pass also verified:
  - `flutter analyze`
  - `scripts/check.ps1`
  - `scripts/build-desktop.ps1 -RustProfile debug`
  - import/preview smoke through stdin and temporary `LOCALAPPDATA`

## Remaining

- Add long-lived watchdog/service behavior with automatic crash rollback.
- Add system proxy rollback hooks when proxy automation is enabled.
- Finish NaiveProxy as the first downloadable optional module after a pinned release is selected
  (adapter is ready; pinned catalog entry + generic desktop profile/import + LocalProxy readiness remain).
- Add signature/AuthentiCode or publisher validation before enabling automatic install/update UI.

Done since last list: bundled TrustTunnel is now resolved from and provisioned
into the managed core store (Phase B). See the Phase B section above and
`todone_arh.md`.
