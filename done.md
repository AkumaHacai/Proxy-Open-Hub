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

## Verified

- `cargo fmt --all`
- `cargo test --workspace`
- Previous pass also verified:
  - `flutter analyze`
  - `scripts/check.ps1`
  - `scripts/build-desktop.ps1 -RustProfile debug`
  - import/preview smoke through stdin and temporary `LOCALAPPDATA`

## Remaining

- Add core store GC for old versions and rollback retention.
- Add long-lived watchdog/service behavior with automatic crash rollback.
- Add system proxy rollback hooks when proxy automation is enabled.
- Move bundled TrustTunnel lookup into managed core store / descriptor-backed module resolution.
- Move bundled TrustTunnel into managed core store layout.
- Add NaiveProxy as the first downloadable optional module after a pinned release is selected.
- Add signature/AuthentiCode or publisher validation before enabling automatic install/update UI.
