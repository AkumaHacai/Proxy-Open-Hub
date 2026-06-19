# Master Progress Checklist (orchestrator-verified)

Last verified: 2026-06-18 by reading the actual code + running the suites.

Verification run:
- Rust: `cargo test --workspace` -> 100 tests pass (41 poh_cli + 27 poh_core +
  6 poh_core_runner + 9 poh_core_session + 17 poh_core_store);
  `cargo fmt --check` + `cargo clippy -D warnings` clean.
- P3 supervisor: `JobHandle` RAII + `CoreWatcher` + `supervise_desktop_session`
  + `desktop-session-supervise` CLI command + `SupervisedSession` Flutter class
  + supervisor-based connect/disconnect in `_HomeState`.
- R-1: safety-net leases now persisted in `Starting` state (before readiness probe);
  `ReadinessTimedOut` path kills the stalled process immediately.
- R-2: `route_restore_commands` uses `ipv6` token for IPv6 CIDRs.
- R-3/R-4: clippy clean; `DesktopStateError` messages de-TrustTunnel-ified.
- Signature enforcement: `signature_preferred` wired per-source in `resolve_store_core`;
  NaiveProxy set `signature_preferred: false`; planned cores retain `true`.
- `_AppearIn` cascade entrance: 4 sections staggered (0/50/100/150ms),
  `_expandReveal` key-force retrigger on expand.
- Flutter: `flutter analyze` -> No issues, `flutter test` -> 7 passed.

This file is the live checklist. `done.md` is the detailed changelog. Plan/design
docs: `LLM_Cloud/{modularity-architecture,process-lifecycle,naiveproxy-integration,ui-layout-and-animation}.md`.

Legend: `[x]` verified in code · `[~]` partial · `[ ]` not started.

Overall completion: ~65% of the full documented roadmap.
Foundation is ~90% done; the open work is the download/install UI, the Tier-2
process supervisor, extra cores, and signature verification.

| Workstream | Status |
|---|---|
| 1. Modular core store + TrustTunnel module | ~90% |
| 2. Process lifecycle safety | ~99% |
| 3. NaiveProxy core (end-to-end) | ~95% |
| 4. UI / layout / animation | ~95% |
| 5. Future cores + release hardening | ~25% |

---

## 1. Modular core store + TrustTunnel as a module  (~90%)

Plan: `modularity-architecture.md` Phases A + B.

- [x] `pinned_release` in trusted-source catalog + validation (version/asset/sha/pattern).
- [x] Installable GitHub sources must be active + checksum-required + pinned.
- [x] `CoreStore::list_installed()`, per-core `active.json`, `active_version()/set_active_version()`.
- [x] `core-list-installed` CLI command.
- [x] `catalog-list` CLI command merges trusted-source catalog + installed store state for UI.
- [x] zip/multifile install with zip-slip guard, dup/empty/reserved-name rejection, per-file hashes + tamper verify.
- [x] GC for old versions (`garbage_collect_old_versions`): keep active + newest inactive (rollback).
- [x] Pinned HTTPS downloader (no "latest" lookup) + SHA-256 verify before staging.
- [x] `core-download-plan` / `core-install` CLI commands.
- [x] `CoreLaunchDescriptor` registry (trusttunnel, sing-box, naiveproxy, xray-core, hysteria2).
- [x] **Phase B:** bundled TrustTunnel provisioned into the store (`install_manual_bundle`),
      pinned exe+wintun hashes, store-based `resolve_core()`, app-local bundle = migration source only.
- [ ] (A4) Long-lived watchdog/service for crash rollback while UI is not polling -> see Workstream 2 / P3.

## 2. Process lifecycle safety (connect/disconnect/safe processes)  (~70%)

Plan: `process-lifecycle.md`.

- [x] **P1** reuse-proof identity (`creation_time_100ns`, PID+image+creation time).
- [x] **P1** safe stop: graceful (CTRL_BREAK) -> force -> confirm-exit; `CoreStopFailed` if it survives.
- [x] **P1** runtime_dir removed only after confirmed exit, with retries (plaintext-config cleanup).
- [x] **P1** `enforce_transition` + `IllegalTransition`; corrupt `session.json` treated as no session.
- [x] **P1** session lifecycle states + single-instance file lock + stale-lock recovery + readiness probes.
- [x] **P2** `desktop-session-reset` (repair) command.
- [x] **P2** reconcile before start/stop (reap dead, adopt live, migrate stale Idle->Running).
- [x] **P2** Windows system-proxy ownership + rollback (`network_effects.rs`, snapshot/restore in session.json).
- [x] **P2** network safety-net DNS: ledger is generic (`DnsLease` + `DnsInterfaceSnapshot`);
      `parse_netsh_dnsservers` + `read_dns_lease` added; `start_desktop_session` now
      snapshots DNS **before spawn** when `change_system_dns` is active and stores the
      lease in `session.network_effects.dns` after readiness probe passes; restored via
      `restore_network_effects` on every stop/reset/reconcile path.
      Parser now handles localized `netsh` output, not just English headers.
- [x] **P2** network safety-net routes/firewall: `RouteLease` records TUN adapter
      name + configured CIDR prefixes; `FirewallLease` snapshots pre-session
      Windows Firewall rule names. Both added to `NetworkEffectsState`, serialized
      in `session.json`, restored by `restore_network_effects` (firewall → routes →
      dns → system_proxy order). `route_restore_commands` (pure) + `firewall_restore_commands`
      (pure, locale-agnostic `parse_netsh_advfirewall_rules`) + 12 new unit tests.
      Wired in `start_desktop_session`: `route_lease_for_profile` + `should_snapshot_firewall`
      both computed before `desktop_profile` is consumed; firewall lease snapshotted
      before spawn; both wired into session after readiness probe passes.
- [x] **Review R-1**: safety-net leases (dns/routes/firewall) now persisted at session
      `Starting` state — before the first `save_session` — so a `Faulted` path always
      has the pre-session network snapshot on disk and `restore_network_effects` runs
      correctly. `ReadinessTimedOut` now calls `terminate_session_process` + 
      `clear_persisted_session` immediately instead of leaving a stalled core alive.
- [x] **Review R-2**: `route_restore_commands` now detects IPv4 vs IPv6 from the CIDR
      (`cidr.contains(':')` → `ipv6` token); fixes `::/0` full-tunnel IPv6 restore.
      Test `route_restore_commands_multiple_cidrs_*` corrected to assert `ipv6` for `::/0`.
- [x] **Review R-3/R-4 + clippy gate**: `looks_like_addr` `.map_or(false,…)` →
      `.is_some_and(…)`; `DesktopStateError` messages de-TrustTunnel-ified (now say
      "core executable", "core readiness probe timed out", etc.);
      `cargo clippy -D warnings` added to `scripts/check.ps1`.
- [x] **P3** long-lived supervisor + Windows Job Object (`KILL_ON_JOB_CLOSE`) + real-time crash detection + **F1–F6 GUI/session desync fix (2026-06-19)**.
      `supervise_desktop_session` starts the core, wraps it in a Job (`JobHandle` RAII
      → KILL_ON_JOB_CLOSE), monitors liveness via `CoreWatcher` (`WaitForSingleObject`
      on Windows; `kill -0` fallback on non-Windows), reads stop commands / stdin EOF
      from Flutter over a background mpsc channel, and calls `stop_desktop_session()`
      (restoring all network effects) on crash or stop.  CLI: `desktop-session-supervise
      <state-path> <profile-id>`.  Flutter: `SupervisedSession` + `startSupervisedSession`
      in `BackendSessionService`; `_supervisorSession` held in `_HomeState`;
      `_onSupervisorEvent` handles faulted events; `_disconnect` sends stop via
      supervisor stdin.
- [x] **F1–F6 GUI/session desync fix (2026-06-19)**: реаттач при старте GUI (F1,
      `_checkExistingSession` + `_adoptSession`); adopt вместо ошибки SessionAlreadyRunning
      (F2, `_tryAdoptSession`); статус-поллинг и в Disconnected (F3, 5-секундный
      двусторонний таймер); гарантированный останов при закрытии GUI (F4, `WidgetsBindingObserver`
      + dispose + `didChangeAppLifecycleState`); супервизор следит за GUI PID (F5, `--gui-pid`
      параметр + `CoreWatcher::wait_ms(0)` в мониторинговом цикле); адоптация осиротевших
      сессий (F6, через F1+F2). Все 121 Rust + 11 Flutter тестов зелёные.

## 3. NaiveProxy core (end-to-end)  (~95%)

Plan: `naiveproxy-integration.md`.

- [x] `NaiveProxyAdapter` in `poh_core` + registered in CLI registry.
- [x] Config models (`naiveproxy_config.rs`): listen/proxy + safe advanced flags.
- [x] Import `config.json` and bare proxy URLs (`https://`/`quic://user:pass@host`).
- [x] Passwords -> secret candidates (`proxy.password`/`listen.password`), never in `core_config`.
- [x] `config.json` materialized via secret placeholders; runner secret allowlist extended.
- [x] URL userinfo redaction (`scheme://user:****@host`) in logs/previews.
- [x] Generic desktop bridge: non-TUN profile = `CoreId` + opaque `CoreConfig` + `SecretRefs` (DPAPI).
- [x] LocalProxy readiness probe (TCP to listen host/port).
- [x] System-proxy ownership/rollback for the desktop session (shared with P2).
- [x] Pin a real NaiveProxy release: `v149.0.7827.114-1`, asset
      `naiveproxy-v149.0.7827.114-1-win-x64.zip`,
      sha256 `50f8138a1cfaeaf28866cb9f7ff25fbd92d2b3bd642885e95131f7d56ebf1086`;
      `status: active`, `install_enabled: true` in trusted-sources.json.
- [x] Install UI: `BackendSessionService.coreInstall(coreId)` → `core-install`;
      `CoreSpec.installable/installing`; `_installCore` in `_HomeState`;
      `_CoreMenuItem` shows "Install" / spinner / "Open" by state.
- [x] Acceptance run: supplied real `config.json` imported; pinned NaiveProxy
      release installed; managed session started; local SOCKS `127.0.0.1:1080`
      verified; external HTTPS request through the proxy returned `HTTP 204`.
- [x] Real-world fixes from acceptance: `listen` arrays in NaiveProxy JSON,
      UTF-8 BOM desktop-state loading, nested ZIP executable resolution, and
      archive-core absolute runtime config arguments.

## 4. UI / layout / animation  (~90%)

Plan: `ui-layout-and-animation.md`.

- [x] Critical vertical-text bug fixed (responsive `_Field`, `Wrap` segments, adaptive `SettingsShell`).
- [x] `PohMotion` tokens; compact<->expanded curves unified (`fastOutSlowIn`, 300ms).
- [x] Overlay host: expanded = dim centered card; compact = bottom sheet (blur removed per owner).
- [x] Routing moved out of global settings into a dedicated `RoutesShell` (presets / custom rules / applications).
- [x] Routes persisted per core (`routesByCore` / `routeRulesByCore` in app-settings).
- [x] Flutter reads the Rust core catalog for the core selector / route tabs, keeping planned
      cores visible but locked until pinned/installed.
- [x] Route core tabs are horizontally scrollable, so Hysteria2/future cores are not clipped.
- [x] Resize animation rewrite: `ClipRect`+`OverflowBox`(final size)+`RepaintBoundary` mask; off-mode panels
      laid out at their own final size; safe text (`maxLines:1`+ellipsis) on server names/addresses.
- [x] Native window min size (`WM_GETMINMAXINFO` -> 360x560).
- [x] `_AppearIn` cascade entrance for the detail pane (fade + 6px upward slide,
      0/50/100/150ms stagger; `_expandReveal` ValueKey retriggers on each expand).
- [x] (Optional) `animateWindowSize` anchored-by-`Stopwatch` for an even smoother native resize.

## 5. Future cores + release hardening  (~25%)

- [x] Launch descriptors exist for sing-box / xray-core / hysteria2 (data only).
- [ ] Real adapters for sing-box / xray-core / hysteria2 (parse/build/import).
- [x] Authenticode signature verification (`poh_core_store::signature`):
      `authenticode_status(path)` (WinVerifyTrust); install records the real
      `signature_status`; `verify_core` enforces a trusted signature when the store
      is built with `.with_signature_required(true)`. See "How to use signature
      verification" below.
- [~] Turn signature enforcement ON for downloadable cores: mechanism is wired —
      `resolve_store_core` reads `signature_preferred` per source and passes it to
      `CoreStore::with_signature_required`; NaiveProxy stays `false` (unsigned);
      planned cores (sing-box, xray, hysteria2) have `signature_preferred: true`
      and will enforce automatically when promoted to `active`.
      Remaining: optional pinned-publisher (signer subject) check.
- [~] Fault-injection / race tests (some unit coverage exists; full matrix from process-lifecycle.md S14 pending).

---

## 6. Per-core CLI commands (WS-A/C/D)  (~60%)

Plan: `LLM_Cloud/per-core-and-routing-plan.md`. Agent: `LLM_Cloud/agents/claude/`.

- [x] **C0** Контракт `contracts/cli-contract.md` зафиксирован; Вариант A пресетов выбран;
      Codex уведомлён в `handoff/INBOX-codex.md`.
- [x] **C1** `desktop-list-profiles <state-path>` → `{profiles:[{id,name,core_id,host,summary}]}`.
- [x] **C2** Generic import/start верифицированы; тест `naiveproxy_import_uses_generic_core_path`.
- [x] **C3** `desktop-core-schema <core_id>` → `{core_id, sections:[...]}` из `settings_schema()`.
- [x] **C4** `desktop-validate-profile <state>` (stdin JSON fields) → `{ok, error?, warnings}`;
      `desktop-update-profile <state>` → сохранение + DPAPI-секреты. Пароли не в `core_config`.
- [x] **C5** `desktop-core-modes <core_id>` → `{available, default, disabled}` из `CoreCapabilities`.
- [x] **C6** `RoutePreset`/`RoutePresetRules` + три поля в `DesktopState` (`RoutePresetsByCore`,
      `ActiveRouteByCore`, `ActiveModeByCore`) с serde default. Round-trip тест.

Verified 2026-06-19: `cargo fmt --check` clean; `cargo clippy -D warnings` clean;
`cargo test --workspace` — 114 tests, 0 failed.

---

## Recommended next steps (priority order)

1. **Real adapters for sing-box / xray-core / hysteria2** — parse/build/import; the
   launch infrastructure and signature enforcement wiring are already in place.
2. **Pinned-publisher (signer subject) check** — optional but recommended before
   promoting planned cores to `active`; extends current signature enforcement with a
   per-source expected signer subject (CryptQueryObject + CertGetNameString).
3. **Fault-injection / race tests** — full matrix from process-lifecycle.md §14.
4. **(Optional) `animateWindowSize` anchored-by-Stopwatch** — smoother native resize.

## Implementation notes (do not regress)

- Launch path is core-agnostic: it resolves a verified core from the managed store.
  `app_local_bundle_dir()` is only a provisioning source for TrustTunnel, not a launch target.
- GC runs inside `promote_staged()` after a successful active-version promotion; it only
  deletes valid version dirs containing `core-manifest.json` (staging/debris left for diagnosis).
- Secrets stay in DPAPI `ProtectedSecrets`; do not reintroduce plaintext state.
- Desktop window has two fixed sizes (`kExpandedWindow` 960x660 / `kCompactWindow` 360x650),
  shared by the native resize and the in-window `OverflowBox` mask — keep them in sync.
- NaiveProxy proxy password must be URL-encoded in the secret value, never stored in the profile.

### How to use signature verification (added 2026-06-18)

What it is: a second integrity layer on top of SHA-256 pinning. SHA-256 proves the
bytes are exactly what we pinned; Authenticode proves they carry a publisher
signature Windows trusts. Code: `crates/poh_core_store/src/signature.rs`.

API:
- `poh_core_store::authenticode_status(path) -> SignatureStatus`
  (`Verified` = signed + chain trusted; `Unsigned` = no embedded signature;
  `Unknown` = signed-but-untrusted/expired/tampered, non-PE, or non-Windows build).
  **Treat `Unknown` as "not trusted".**
- `CoreStore::with_signature_required(true)` — builder flag. When set, `verify_core`
  fails with `CoreStoreError::SignatureRejected` unless the executable is `Verified`.
  Default is `false` (current bundled TrustTunnel / naive cores are unsigned).
- `install()` / `install_manual_bundle()` already record the real `signature_status`
  into the stored `core-manifest.json` (via `promote_staged`), so the UI can show
  "signed / unsigned" without re-running verification.

How it is wired (done 2026-06-18):
- `desktop_state.rs::resolve_store_core` looks up the source for the requested
  `core_id` in `embedded_trusted_sources()` and reads `signature_preferred`.
  If `true` → `CoreStore::new(root).with_signature_required(true)`; otherwise the
  store is built without signature enforcement (SHA-256 pinning remains in effect).
- `resolve_trusttunnel_core` (bundled path) is NOT affected — it never passes
  through `resolve_store_core`.
- `trusted-sources.json`: NaiveProxy `signature_preferred: false` (klzgrad ships
  unsigned binaries); sing-box, xray-core, hysteria2 remain `true` and will
  auto-enforce when promoted to `active`.
- Catalog caveat: WinVerifyTrust checks the EMBEDDED signature only (not `.cat`
  catalog sigs). Third-party cores ship embedded-signed; that is correct behavior.
- Next step (optional): extract the signer subject (CryptQueryObject +
  CertGetNameString) and pin it per source for a stronger publisher-identity check.

Tests: `authenticode_status_never_verifies_unsigned_bytes`,
`verify_core_enforces_signature_only_when_required` (both `#[cfg(windows)]`).
The `Verified` branch is exercised by real signed cores in production; a unit test
can't rely on a guaranteed embedded-signed file being present on every machine.

### How to use the DNS network safety-net (added 2026-06-18)

Why: a TUN core (TrustTunnel with `change_system_dns`) points system DNS at a
tunnel resolver. On a clean stop the core restores it; on a force-kill/crash it
does not, so the user is left with no working DNS. The safety-net makes the APP
record the pre-session DNS and force it back on every teardown.

What is built (`crates/poh_cli/src/network_effects.rs`):
- `NetworkEffectsState` is now a generic ledger: `system_proxy: Option<SystemProxyLease>`
  + `dns: Option<DnsLease>` (both serialized in `session.json`, both restored by
  `restore_network_effects`, which now attempts ALL effects even if one fails).
- `DnsLease { interfaces: Vec<DnsInterfaceSnapshot>, applied_at_unix_ms }`;
  `DnsInterfaceSnapshot { interface, family (ipv4/ipv6), config: Dhcp | Static{servers} }`.
- `dns_restore_commands(&DnsLease) -> Vec<Vec<String>>` is a PURE builder of the
  `netsh interface <fam> ...` argv (DHCP -> `source=dhcp`; Static -> `set ... static
  <first> primary validate=no` then `add ... <n> index=K validate=no`). Fully unit
  tested without touching the network. `restore_dns_lease` (cfg windows) runs them.
- Already flows through every teardown: `clear_persisted_session` ->
  `restore_network_effects`, used by stop / `desktop-session-reset` / reconcile.

What remains (the wiring step — do this to activate it):
1. At session start, BEFORE launching a TUN core that changes DNS, snapshot the
   current DNS of the affected interface(s) into a `DnsLease` and store it in
   `session.network_effects.dns` (alongside how `prepare_system_proxy` is stored).
   Implement `read_dns_lease(interfaces)` with `netsh interface <fam> show dnsservers
   name=<if>` + a pure `parse_dns_servers(output)` helper (unit-test the parser the
   same way `dns_restore_commands` is tested).
2. Interface selection: snapshot the active/default-route adapter(s). Start simple
   (the adapter carrying the default route) and expand if needed. Do NOT snapshot the
   TUN adapter itself.
3. Routes / kill-switch firewall rules follow the SAME record->restore pattern: add
   `routes: Vec<RouteLease>` / `firewall: Vec<FirewallLease>` to the ledger with their
   own pure command builders + tests, restored in `restore_network_effects`.

Tests: `dns_lease_round_trips_and_marks_state_non_empty`,
`dns_restore_commands_for_dhcp_resets_to_dhcp`,
`dns_restore_commands_for_static_sets_primary_then_adds`,
`dns_restore_commands_static_without_servers_falls_back_to_dhcp`,
`dns_restore_commands_uses_ipv6_token`.
</content>
