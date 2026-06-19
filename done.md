# Done - Security And Modularity Work

## GUI/Session desync + orphaned supervisor — F1–F6 (Claude Sonnet 4.6, 2026-06-19)

Plan ref: `LLM_Cloud/gui-session-sync-investigation.md` §4 Priority 1+2.

### Диагностика §2 (Группы A + B) — выводы из кода

- **A1 ✅** `initState` не звал `desktop-session-status` → GUI стартовал в Disconnected независимо от живой сессии.
- **A2 ✅** `PersistedDesktopSession` содержит `profile_id`, `core_id`, `started_at_unix_ms` — достаточно для реаттача.
- **A3 ✅** `SessionAlreadyRunning(pid)` рендерился как ошибка `_connectionMessage`.
- **A4 ✅** `_startSessionStatusPolling` содержал `if (!_connected)` guard → молчал без connected.
- **B3 ✅** `dispose` не звал `supervisor.stop()`; завершение зависело только на pipe EOF.
- **B4 ✅** Job Object держится супервизором; смерть GUI напрямую ядро не гасит.

### F1 — Реаттач при старте GUI

`_loadProfiles` завершает async-инициализацию вызовом `_checkExistingSession()`:
- зовёт `poh_cli desktop-session-status`
- если `running == true` → `_adoptSession(session)` → GUI переходит в Connected,
  запускает трафик и статус-поллинг

`_adoptSession` ищет или создаёт таб для `session.coreId`, выставляет `selectedServerId`,
фазу `connected`, `connectedAt`, `_supervisorSession = null` (disconnect идёт через
`stopSession()`). Rust-сторона уже проверила `creation_time_100ns`.

### F2 — Adopt вместо ошибки SessionAlreadyRunning

Блок catch в `_connect` теперь обнаруживает `error.message.contains('already running')` и
вызывает `_tryAdoptSession(id)` вместо показа ошибки. Если сессия найдена — adopt; если нет —
сброс в idle с сообщением.

### F3 — Статус-поллинг и в Disconnected

`_startSessionStatusPolling` переработан: убран `!_connected` guard, добавлен двусторонний
анализ (5-секундный интервал):
- connected + not running → показать "core exited", restart idle polling
- not connected + not working + running → adopt
Поллинг стартует из `_loadProfiles` (если не adopted), а также перезапускается после
disconnect, selectServer, supervisor fault/exited.

### F4 — Гарантированный останов при закрытии GUI

`_ProxyOpenHubAppState` добавлен `with WidgetsBindingObserver`:
- `initState`: `WidgetsBinding.instance.addObserver(this)`
- `dispose`: `removeObserver` + `_supervisorSession?.stop()` (fire-and-forget)
- `didChangeAppLifecycleState(detached)`: явный stop до закрытия пайпа

### F5 — Супервизор следит за PID родительского GUI

`supervise_desktop_session(state_path, profile_id, gui_pid: Option<u32>)` — новый параметр.
CLI: `poh_cli desktop-session-supervise <state> <profile-id> [--gui-pid <pid>]`.
Flutter передаёт `pid.toString()` (Dart's `dart:io` top-level `pid`) при спавне супервизора.
В мониторинговом цикле: `gui_watcher.wait_ms(0)` — если GUI-процесс мёртв → `stop_desktop_session`.
Используется тот же `CoreWatcher` (`PROCESS_SYNCHRONIZE` + `WaitForSingleObject(0)` on Windows).

### F6 — Адоптация осиротевшей сессии

Реализована через F1 + F2. `_adoptSession` ставит `_supervisorSession = null`, disconnect
использует `stopSession()`. Rust-сторона проверяет `creation_time_100ns` в `desktop_session_status`.

### Результат

- `cargo fmt --all` clean; `cargo clippy -D warnings` clean; `cargo test --workspace` — 121 tests, 0 failed.
- `dart format` clean; `dart analyze` — No issues; `flutter test` — 11 passed.



Date: 2026-06-18

> This file is the detailed changelog. The live checklist (what is done / what
> remains, with %) is `todone_arh.md`.

## Per-core CLI commands C0–C6 (Claude Sonnet 4.6, 2026-06-19)

Plan ref: `LLM_Cloud/per-core-and-routing-plan.md`, agent tasks `LLM_Cloud/agents/claude/TASKS.md`.
Contract: `LLM_Cloud/agents/contracts/cli-contract.md`.

### C0 — Контракт (`cli-contract.md`)

Зафиксированы точные JSON-формы пяти новых команд + выбран **Вариант A** для пресетов
(хранятся в desktop-state, UI пишет напрямую без новых команд CRUD). Статусы проставлены 🟡.
Уведомление отправлено в `handoff/INBOX-codex.md`.

### C1 — `desktop-list-profiles <state-path>`

Новая публичная функция `list_desktop_profiles(state_path)` + CLI-обработчик в `main.rs`.
Возвращает `{ "profiles": [{id, name, core_id, host, summary, tags}] }`.
- TrustTunnel: `host` = `Endpoint.Hostname`; `summary` = `"HTTP/2 · TUN"` / `"HTTP/3 · SOCKS"`.
- NaiveProxy: `host` / `summary` извлекаются из `core_config.config.proxy.*`.
- Профили без `core_id` (legacy) получают `"trusttunnel"` по умолчанию.

### C2 — Generic import/start (проверено)

`import_desktop_profile` и `start_desktop_session` уже core-agnostic (детект через
`CoreRegistry`, запись `CoreId`). NaiveProxy проходит тот же путь без специального кода.
Добавлен тест `naiveproxy_import_uses_generic_core_path`.

### C3 — `desktop-core-schema <core_id>`

`core_schema(core_id)` → JSON из `adapter.settings_schema()` + обёрнутый в `{core_id, sections}`.
Тесты: TrustTunnel имеет секции `endpoint`/`listener`; NaiveProxy — `proxy`/`listen`;
неизвестный core_id → ошибка.

### C4 — `desktop-validate-profile` / `desktop-update-profile`

stdin: `{ core_id, profile_id?, display_name?, fields: {key: value} }`.
- `validate`: строит `core_config` из полей, вызывает `adapter.validate()`, возвращает
  `{ok, error?, warnings}`. Секреты не попадают в `core_config` (хранятся отдельно через
  DPAPI-ref). `InvalidProfile` → `{ok: false, error: ...}` (не системная ошибка).
- `update`: сохраняет `DesktopProfile` + новые секреты через `ProtectedSecrets` (DPAPI).
  Если `profile_id` найден — обновляет, иначе создаёт. `unique_profile_id` гарантирует
  уникальность.
- Хелперы: `naive_fields_to_core_config_saving` / `tt_fields_to_desktop_profile` /
  `extract_secret` (secret-ref → не трогаем; plaintext → DPAPI).

### C5 — `desktop-core-modes <core_id>`

`core_modes(core_id)` → `{core_id, available, default, disabled}`.
Логика доступности по `CoreCapabilities`:
- `"tun"` ← `supports_tun`
- `"system_proxy"` ← `supports_system_proxy && (supports_socks || supports_http_proxy)`
- `"local_proxy_gate"` ← `supports_socks || supports_http_proxy`
- `default` = `"tun"` если доступен, иначе `"local_proxy_gate"`.
Тесты: TrustTunnel — все три режима, default=tun; NaiveProxy — нет tun, tun в disabled.

### C6 — Пресеты маршрутизации (Вариант A)

Добавлены типы `RoutePreset { id, name, rules: RoutePresetRules }` и `RoutePresetRules`
(все поля строки: `app_exe_names`, `included_domains`, `excluded_domains`,
`included_cidrs`, `excluded_cidrs`).
Три новых поля в `DesktopState` с `serde(default)` для обратной совместимости:
- `RoutePresetsByCore: BTreeMap<String, Vec<RoutePreset>>`
- `ActiveRouteByCore: BTreeMap<String, String>`
- `ActiveModeByCore: BTreeMap<String, String>`
Тест round-trip + десериализация старых JSON без этих полей.

### Verified (2026-06-19, C0–C6)

- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — no warnings.
- `cargo test --workspace` — **114 tests pass** (55 poh_cli +14 new, остальные без изменений);
  0 failed.
- TrustTunnel-путь не регрессировал.

---

## Review findings R-1…R-4 + clippy gate (Claude Sonnet 4.6, 2026-06-18)

Source: `LLM_Cloud/review-findings-2026-06-18.md`.

### R-1 (🟠) — Safety-net leases persisted before readiness probe

**File:** `crates/poh_cli/src/desktop_state.rs` — `start_desktop_session`.

- Moved `dns_lease_snapshot`, `route_lease_opt`, and `firewall_lease_snapshot` out of
  the post-probe assignment block and directly into the `PersistedDesktopSession`
  initializer (`network_effects: NetworkEffectsState { dns, routes, firewall, .. }`).
  The first `save_session` now persists all three leases, so a `Faulted` session on
  disk always carries the pre-session network snapshot.
- `ReadinessTimedOut` path now calls `terminate_session_process(&session)` +
  `clear_persisted_session` immediately (instead of `runtime_guard.keep()` + return),
  so the lingering half-started process is killed and `restore_network_effects` runs
  promptly via the existing `clear_persisted_session` teardown path.
- Removed the dead post-probe assignment block (was lines 317–328).

### R-2 (🟠) — `route_restore_commands` IPv6 family fix

**File:** `crates/poh_cli/src/network_effects.rs` — `route_restore_commands`.

- The `netsh` family token was hardcoded to `"ipv4"` for every CIDR. Changed to:
  `let family = if cidr.contains(':') { "ipv6" } else { "ipv4" }`.
  Now `::/0` emits `netsh interface ipv6 delete route …` (correct) and `0.0.0.0/0`
  still emits `ipv4` (unchanged).
- Fixed test `route_restore_commands_multiple_cidrs_produce_multiple_commands`:
  replaced the two stale `assert!(commands[N].contains("ipv4"))` lines with
  `assert!(commands[0].contains("ipv4"))` and `assert!(commands[1].contains("ipv6"))`.

### R-3 (🟡) — Clippy `unnecessary_map_or`

**File:** `crates/poh_cli/src/network_effects.rs:620` — `looks_like_addr`.

Changed `.map_or(false, |c| c.is_ascii_hexdigit() || c == ':')` →
`.is_some_and(|c| c.is_ascii_hexdigit() || c == ':')`.

### R-4 (🟡) — `DesktopStateError` de-TrustTunnel-ified

**File:** `crates/poh_cli/src/desktop_state.rs` — `DesktopStateError` enum.

Removed hardcoded "TrustTunnel" from generic error message strings:
- `"TrustTunnel client executable was not found"` → `"core executable was not found"`
- `"invalid TrustTunnel client executable path: {0}"` → `"invalid core executable path: {0}"`
- `"TrustTunnel core exited during startup with code {0:?}: {1}"` → `"core exited during startup with code {0:?}: {1}"`
- `"TrustTunnel readiness probe timed out for {0}: {1}"` → `"core readiness probe timed out for {0}: {1}"`

### Clippy gate added to `scripts/check.ps1`

Added `cargo clippy --workspace --all-targets -- -D warnings` between `fmt --check`
and `cargo test` so clippy warnings become CI failures going forward.

### Verified (2026-06-18, R-1…R-4)

- `cargo fmt --all --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — no warnings.
- `cargo test --workspace` — 100 tests pass (41 poh_cli + 27 poh_core +
  6 poh_core_runner + 9 poh_core_session + 17 poh_core_store); 0 failed.

---

## NaiveProxy acceptance + launch fixes (Codex, 2026-06-18)

- Imported the supplied real NaiveProxy `config.json` without printing secrets.
- Fixed NaiveProxy JSON import to accept real configs where `listen` is an array
  and use the first string listener.
- Fixed desktop state loading for UTF-8 BOM-prefixed JSON files. This unblocked
  migration from legacy `Secrets` into DPAPI-backed `ProtectedSecrets`.
- Fixed archive-core launch descriptors so downloaded cores keep their working
  directory in the verified install directory, but receive runtime config file
  arguments as absolute paths. This fixes NaiveProxy failing with
  `Error reading config.json: File doesn't exist`.
- Fixed core-store ZIP installation for releases that wrap files in a top-level
  folder. The store now resolves the unique nested executable by file name and
  records that path in the installed manifest.
- Verified pinned NaiveProxy install from GitHub:
  `core-install naiveproxy` -> installed
  `v149.0.7827.114-1` into the managed core store.
- End-to-end acceptance:
  imported profile `NaiveProxy np.hel2.mumuru.ru`, started the session, confirmed
  local SOCKS listener `127.0.0.1:1080`, and performed:
  `curl --socks5-hostname 127.0.0.1:1080 https://www.google.com/generate_204`
  -> `HTTP 204`, total ~0.60s.
- Stopped the session afterwards and confirmed `desktop-session-status` is clean
  (`running:false`, `session:null`) with no lingering `naive`/`trusttunnel`
  processes.

Tests run:
- `cargo test -p poh_core naiveproxy` -> 6 passed.
- `cargo test -p poh_core_store` -> 17 passed.
- `cargo test -p poh_core_session` -> 9 passed.

## Modularity Phase C - NaiveProxy catalog pin + install UI (Claude Sonnet 4.6, 2026-06-18)

Closes Workstream 3 "Pin a real NaiveProxy release" and adds the install UI.

- Pinned NaiveProxy `v149.0.7827.114-1` in `core-registry/trusted-sources.json`:
  - `status: active`, `install_enabled: true`
  - `pinned_release.version: "v149.0.7827.114-1"`,
    `asset_name: "naiveproxy-v149.0.7827.114-1-win-x64.zip"`,
    `sha256: "50f8138a1cfaeaf28866cb9f7ff25fbd92d2b3bd642885e95131f7d56ebf1086"`
- Updated `trusted_source_policy_accepts_registry_manifest` test (was asserting
  `all(!installable)` — now verifies TrustTunnel is non-installable and NaiveProxy
  is installable with a pinned release).
- Added `BackendSessionService.coreInstall(coreId)` → calls `poh_cli core-install
  <core-id>` which downloads the pinned release, verifies SHA-256, installs into the
  managed core store, and returns an install result JSON.
- Added `installable: bool` and `installing: bool` fields to `CoreSpec` (default
  false; backed by `copyWith`).
- `_specFromCatalogEntry` now sets `installable: entry.installable && !entry.installed`
  and `installing: _installingCoreIds.contains(spec.id)`.
- Added `_installingCoreIds` set and `_installCore(coreId)` in `_HomeState`:
  - Sets `installing: true` on the spec immediately.
  - Calls `coreInstall`, then reloads catalog on success.
  - Best-effort: on error, resets installing flag and lets the next catalog load
    show the real state.
- `_CoreMenu` and `_CoreMenuItem` updated:
  - `onInstallCore: ValueChanged<String>` callback passed from `_HomeState` →
    `_DesktopShell` → `_CoreMenu` → `_CoreMenuItem`.
  - `_CoreMenuItem` now shows:
    - "Open" (accent) for active cores
    - "Install" (accent, 80% opacity) for installable uninstalled cores — clicking
      triggers `_installCore`
    - `CircularProgressIndicator` (14×14) while installing
    - "Soon" (muted) for planned/not-installable cores

## Verified (2026-06-18, NaiveProxy pin)

- `cargo test --workspace` — 56 unique tests pass (all including updated catalog test).
- `flutter analyze` — No issues.
- `flutter test` — 6/6 passed.

## Process Lifecycle P2 - DNS snapshot wiring (Claude Sonnet 4.6, 2026-06-18)

Closes the remaining P2 item: DNS lease is now captured before the core starts
and saved into the session on the happy path.

- Added `parse_netsh_dnsservers(output, family) -> Vec<DnsInterfaceSnapshot>`
  (pure, cross-platform) to `crates/poh_cli/src/network_effects.rs`: parses
  the output of `netsh interface <ipv4|ipv6> show dnsservers` into per-interface
  snapshots (DHCP / Static with continuation IPs; interfaces without a DNS config
  line are omitted; `Static { servers: [] }` and `None` both emit `Dhcp`).
- Added `read_dns_lease(applied_at_unix_ms) -> Result<DnsLease, ...>`:
  - `#[cfg(windows)]`: runs both `netsh interface ipv4 show dnsservers` and `ipv6`
    commands, merges snapshots into a `DnsLease`.
  - `#[cfg(not(windows))]`: returns `UnsupportedPlatform` error.
- Wired into `start_desktop_session` (`desktop_state.rs`):
  - `needs_dns_snapshot` is computed before `desktop_profile` is consumed.
  - `read_dns_lease` is called **before** `command.spawn()` so the snapshot is
    taken before the core can redirect system DNS to a tunnel resolver.
  - Best-effort: on error a warning is printed and the session still starts;
    `session.network_effects.dns` stays `None` (no rollback for that session).
  - After readiness probe passes, the captured lease is stored in
    `session.network_effects.dns` so it is persisted by `enforce_transition`
    (Running state) or by the system-proxy `save_session` call.
  - Teardown already restores DNS via `restore_network_effects` in
    `clear_persisted_session` (stop / reset / reconcile) — no changes needed there.
- Added `should_snapshot_dns(profile) -> bool` helper: `mode == 0` (TUN) AND
  `change_system_dns` — only TrustTunnel TUN with DNS change needs snapshotting.
- Tests: +7 pure unit tests on `parse_netsh_dnsservers`
  (DHCP, Static+continuation, None→Dhcp, multiple interfaces, no-config skip,
  empty output, IPv6 static). All 7 pass; existing 5 DNS restore tests still pass.

## Verified (2026-06-18, DNS wiring)

- `cargo build -p poh_cli` — clean.
- `cargo test --workspace` — 26 poh_cli / 6 poh_core / 8 poh_core_session / 16
  poh_core_store tests pass (56 unique; `#[cfg(windows)]`-only tests compile but
  run on Windows only).
- `parse_netsh_dnsservers_*` tests: 7/7 pass including IPv6 static + continuation.

## Remaining after this session

- Wire start-time DNS to also identify which adapter carried the default route
  (optional refinement — current snapshot covers all adapters which is correct
  but slightly broader than needed).
- TUN/route/kill-switch firewall repair with the same record→restore pattern
  (documented in `todone_arh.md`).
- Tier-2 supervisor + Job Object (P3).
- NaiveProxy catalog pin (real SHA-256 from official release) + install UI.
- Signature enforcement for downloadable cores.

## Process Lifecycle P2 - Routes & Firewall network safety-net (Claude Sonnet 4.6, 2026-06-18)

Closes the P2 tail item: TUN routes and kill-switch firewall rules are now
tracked in the session ledger and restored on any teardown path (stop / reset /
crash / reconcile).

### New types in `crates/poh_cli/src/network_effects.rs`

**`RouteLease`** — records the TUN adapter name and the CIDR prefixes the core
was configured to route through it.  On restore, issues:
```
netsh interface ipv4 delete route prefix=CIDR interface=NAME
```
for each prefix.  "Element not found" errors are silently ignored (wintun
auto-tears-down on adapter removal).

**`FirewallLease`** — snapshots all Windows Firewall rule names before the
session.  On restore, re-enumerates current rules, diffs against the snapshot,
and issues `netsh advfirewall firewall delete rule name=NAME` for each rule
that was added by the core (kill-switch rules).  "No rules match" errors are
silently ignored.

**`NetworkEffectsState`** now has four fields: `system_proxy`, `dns`, `routes`,
`firewall`.  `is_empty()` covers all four.  Restoration order changed to
**firewall → routes → dns → system_proxy** (most-impactful first so a
kill-switch crash never permanently locks the user out).

### New pure functions (unit-tested without I/O)

- `route_restore_commands(&RouteLease) -> Vec<Vec<String>>` — builds netsh argv
  list from stored CIDRs.
- `firewall_restore_commands(&FirewallLease, current_rules: &[String])` — diffs
  pre-session snapshot against caller-provided current rules; builds netsh argv.
- `parse_netsh_advfirewall_rules(output: &str) -> Vec<String>` — extracts rule
  names from `netsh advfirewall firewall show rule name=all` output using the
  separator-line heuristic (works in any Windows UI locale).

### New I/O functions

- `read_firewall_lease(applied_at_unix_ms) -> Result<FirewallLease, ...>` —
  runs `netsh advfirewall firewall show rule name=all` and stores the current
  rule names.  `#[cfg(not(windows))]` returns `UnsupportedPlatform`.
- `restore_route_lease` / `restore_firewall_lease` (private, `#[cfg(windows)]`).

### Wiring in `crates/poh_cli/src/desktop_state.rs`

- `route_lease_for_profile(profile, ts)` — builds a `RouteLease` from the
  profile's `tun.device_name` (String) and `tun.included_routes`.  Returns
  `None` if not TUN mode, device name is empty, or no included routes.
- `should_snapshot_firewall(profile)` — `mode == 0` AND `kill_switch_enabled`.
- Both derived **before** `build_materialized_session` consumes `desktop_profile`.
- `route_lease_opt` is updated with `started_at_unix_ms` after the timestamp is
  known (avoids a second call to `now_unix_ms`).
- `read_firewall_lease` called before `command.spawn()` (best-effort; warns on
  error, continues).
- After readiness probe passes: `session.network_effects.routes` and
  `session.network_effects.firewall` are populated and persisted atomically with
  the Running state.

### Tests added (all pass)

- `route_lease_round_trips_serialization`
- `route_restore_commands_generates_netsh_ipv4_delete`
- `route_restore_commands_skips_empty_routes`
- `route_restore_commands_multiple_cidrs_produce_multiple_commands`
- `firewall_lease_round_trips_serialization`
- `firewall_restore_commands_deletes_rules_added_after_snapshot`
- `firewall_restore_commands_empty_when_no_new_rules`
- `parse_netsh_advfirewall_rules_extracts_rule_names`
- `parse_netsh_advfirewall_rules_locale_agnostic` (Russian locale output)
- `parse_netsh_advfirewall_rules_empty_output`
- `parse_netsh_advfirewall_rules_rule_name_with_colon`
- `network_effects_state_includes_routes_and_firewall_in_empty_check`

## Verified (2026-06-18, Routes & Firewall safety-net)

- `cargo fmt --all --check` — clean.
- `cargo test --workspace` — **100 tests pass** (41 poh_cli + 27 poh_core +
  6 poh_core_session + 9 poh_core_session + 17 poh_core_store; 0 failed).

## Process Lifecycle P3 - Tier-2 supervisor + Windows Job Object (Claude Sonnet 4.6, 2026-06-18)

Closes P3: a long-lived supervisor process now holds the core inside a Windows
Job Object and monitors it for crashes.  Flutter speaks to it via stdin/stdout
JSON lines instead of ephemeral CLI invocations for connect/disconnect.

### New Rust: `crates/poh_cli/Cargo.toml`

Added `Win32_System_JobObjects` to the `[target.'cfg(windows)'.dependencies]`
windows-sys feature list (enables `CreateJobObjectW`, `AssignProcessToJobObject`,
`SetInformationJobObject`, `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`,
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `JobObjectExtendedLimitInformation`).

### New Rust: `crates/poh_cli/src/desktop_state.rs`

**`JobHandle`** (`#[cfg(windows)]`) — RAII wrapper around a Windows Job Object
handle.  Dropping the `JobHandle` closes the handle, which triggers
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and kills every process inside the Job —
including the core — even if the supervisor itself crashes before calling
`stop_desktop_session`.

- `JobHandle::create_kill_on_close()` — creates a Job, sets the limit flag via
  `SetInformationJobObject(JobObjectExtendedLimitInformation, ...)`.
- `JobHandle::assign_pid(pid)` — opens the process with `PROCESS_ALL_ACCESS` and
  calls `AssignProcessToJobObject`.  Supports nested Jobs (Windows 8+).

**`CoreWatcher`** — cross-platform process liveness monitor.

- Windows: opens a `PROCESS_SYNCHRONIZE` handle once at construction (tied to the
  kernel process object, immune to PID recycling); `wait_ms(n)` calls
  `WaitForSingleObject(handle, n)` — blocks up to `n` ms, returns `true` if the
  process exited.
- Non-Windows (CI/dev): sleeps the timeout, then probes via `kill -0 <pid>`.

**`supervise_desktop_session(state_path, profile_id)`** — public entry point.

1. Calls `start_desktop_session` (existing logic, readiness probe included).
2. Creates a `JobHandle` (best-effort) and assigns the core PID to it.
3. Opens a `CoreWatcher` handle.
4. Emits `{"type":"started","pid":N,"profile_id":"...","core_id":"...","log_path":"...","started_at_unix_ms":N}` — Flutter waits for this line.
5. Spawns a background thread that reads stdin lines and sends them over an
   `mpsc::channel`.
6. Monitoring loop (500 ms cadence via `WaitForSingleObject`):
   - Drains the stdin channel each iteration.
   - `{"command":"stop"}` or stdin EOF → calls `stop_desktop_session()` (network
     restored by reconcile), emits `{"type":"stopped"}`, returns.
   - Core exited → calls `stop_desktop_session()` (network restored), emits
     `{"type":"faulted","reason":"core_crash"}`, returns.

### New CLI: `crates/poh_cli/src/main.rs`

Added `Some("desktop-session-supervise") => desktop_session_supervise(&args[1..])`.

`fn desktop_session_supervise(args)` — validates `[state-path, profile-id]`
arguments, delegates to `supervise_desktop_session`.

### New Flutter: `apps/desktop_flutter/lib/services/backend_session_service.dart`

**`SupervisedSession`** class — holds the supervisor `Process` and the initial
`BackendSession` data from the `started` event.  `stop()` sends
`{"command":"stop"}`, closes stdin, then awaits exit with a 10-second timeout
(force-kills if it takes longer).

**`startSupervisedSession(server)`** — starts `poh_cli desktop-session-supervise`
via `Process.start`, waits for the first `{"type":"started"}` line on stdout
(using `_waitForSupervisorStart`), builds a `SupervisedSession`, and returns it.
If the supervisor exits before the started event, throws `BackendSessionException`
with the stderr tail as the message.

### Updated Flutter: `apps/desktop_flutter/lib/main.dart`

- Added `SupervisedSession? _supervisorSession` to `_ProxyOpenHubAppState`.
- `_connect()` now calls `startSupervisedSession` instead of
  `startTrustTunnelSession`.  Listens to supervisor stdout via
  `_onSupervisorEvent` + `_onSupervisorExited`.
- `_disconnect()` calls `supervisor.stop()` when a supervisor is alive,
  falls back to `stopTrustTunnelSession()` otherwise.
- `_onSupervisorEvent(line, tabId)` — parses supervisor events; on
  `{"type":"faulted"}` clears timers/traffic and resets UI to idle with
  "Core exited unexpectedly" message.
- `_onSupervisorExited(tabId)` — safety net in case the supervisor exits without
  emitting a faulted event while the tab is still connected.

### Verified (2026-06-18, P3 supervisor)

- `cargo fmt --all --check` — clean.
- `cargo test --workspace` — **100 tests pass** (41 poh_cli + 27 poh_core +
  6 poh_core_runner + 9 poh_core_session + 17 poh_core_store; 0 failed).
- `flutter analyze` — No issues.
- `flutter test` — 7/7 passed.

---

## Signature enforcement + _AppearIn cascade entrance (Claude Sonnet 4.6, 2026-06-18)

### Signature enforcement for downloadable cores

- Fixed `core-registry/trusted-sources.json`: set `"signature_preferred": false` for
  NaiveProxy (klzgrad does not ship Authenticode-signed binaries; SHA-256 pinning
  is the sole integrity gate for that source).
- Wired `signature_preferred` from `TrustedCoreSource` into `resolve_store_core`
  (`desktop_state.rs`): the source for the requested `core_id` is looked up in
  `embedded_trusted_sources()`; if it has `signature_preferred: true`, the store is
  built as `CoreStore::new(root).with_signature_required(true)`, otherwise no
  signature check is performed.  TrustTunnel (bundled, `resolve_trusttunnel_core`)
  remains unaffected.
- Effect: when sing-box, xray-core, or hysteria2 sources are promoted to `active`
  and their trusted-source entries have `signature_preferred: true`, their binaries
  will be rejected at launch unless they carry a valid embedded Authenticode signature.
  NaiveProxy and TrustTunnel remain unsigned-OK.

### `_AppearIn` cascade entrance for `_ExpandedDetailPane`

Plan ref: `ui-layout-and-animation.md §4.5`.

Added `_AppearIn` (`StatefulWidget`) in `apps/desktop_flutter/lib/main.dart`:
- Takes `motionDuration` and optional `delay` (default `Duration.zero`).
- When `motionDuration == Duration.zero` (animations disabled), renders child
  immediately with no animation.
- With delay > 0: uses a cancelable `Timer`; while pending, renders the child at
  `opacity: 0` (preserves layout space).  On timer fire, mounts a
  `TweenAnimationBuilder<double>` from 0→1 with `PohMotion.decel` curve: combined
  `Opacity(t)` + `Transform.translate(Offset(0, 6*(1-t)))` = fade + 6px upward slide.

Applied to four sections of `_ExpandedDetailPane` with cascade delays:
- `_ServerStrip`: 0 ms delay (fires immediately on mount)
- `_ConnectionStatus`: 50 ms delay
- Metric cards `Wrap`: 100 ms delay
- `_DetailBar`: 150 ms delay

Retrigger on expand: `_ExpandedDetailPane` receives `key: ValueKey(expandReveal)`
where `_expandReveal` is an `int` field in `_ProxyOpenHubAppState`.  It increments
each time `_toggleCompact` transitions from compact→expanded.  The key change
forces a full remount of `_ExpandedDetailPane` and all its `_AppearIn` children,
replaying the cascade animation on every expand.

Threading: `expandReveal` is propagated as a required param through
`_DesktopWindow` → `_MorphingBody` → `_ExpandedDetailPane`.

### Verified (2026-06-18, signature enforcement + _AppearIn)

- `cargo fmt --all --check` — clean.
- `cargo test --workspace` — 100 tests pass; 0 failed.
- `flutter analyze` — No issues.
- `flutter test` — 7/7 passed.

---

## Orchestrator verification (2026-06-18)

Checked the changelog claims against the actual code and ran the suites.

- Rust: `cargo test --workspace` -> 75 passed; `cargo build` clean (was 68 at audit
  time; +7 from the signature + DNS safety-net work added this session).
- Flutter: `flutter analyze` -> No issues; `flutter test` -> 6 passed.
- Spot-verified present in code: `naiveproxy.rs`/`naiveproxy_config.rs`,
  `garbage_collect_old_versions`, `network_effects.rs`, `desktop-session-reset`,
  `routes_shell.dart` + `RouteRules`, native `WM_GETMINMAXINFO` min size 360x560,
  `OverflowBox`/`RepaintBoundary` resize mask.
- Added this session: Authenticode signature verification (store) + generic DNS
  network safety-net (ledger) - see the two sections just below.
- Still NOT done: NaiveProxy catalog still `planned`/`install_enabled:false` (no pinned
  release); no watchdog/Job-Object supervisor; signature enforcement not turned on at
  runtime + no pinned-publisher check; DNS lease not yet snapshotted at start;
  no `_AppearIn` entrance.

Approx completion of the full documented roadmap: ~67%
(foundation ~90%; download/install UI, Tier-2 supervisor, extra cores are the main
gaps). Per-workstream breakdown in `todone_arh.md`.

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
- Added Windows system proxy ownership/revert for desktop sessions:
  - profiles can opt in with `Listener.SystemProxy.Enabled`;
  - mode `0` auto-picks HTTP when an HTTP local proxy is configured, otherwise
    SOCKS; mode `1` forces HTTP; mode `2` forces SOCKS;
  - `crates/poh_cli/src/network_effects.rs` snapshots `ProxyEnable`,
    `ProxyServer`, and `ProxyOverride` before applying our proxy;
  - `PersistedDesktopSession.network_effects` stores the rollback lease in
    `session.json`;
  - stop/reset/reconcile restore the previous Windows proxy settings before
    removing session state/runtime files;
  - startup applies system proxy only after the core readiness probe succeeds,
    and rolls back if applying/saving fails.
- Remaining P2: broader network safety-net for TUN/DNS/kill-switch repair after
  force-kill or crash.

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
- Added the desktop-state bridge for non-TrustTunnel cores:
  - profiles can persist as `CoreId` + opaque adapter-owned `CoreConfig` +
    `SecretRefs`;
  - imported NaiveProxy secrets go through the same DPAPI `ProtectedSecrets`
    map;
  - materialization selects the adapter by `core_id` from `CoreRegistry`;
  - NaiveProxy startup readiness probes the local listener host/port.
- Remaining Phase C: choose/pin a real NaiveProxy release and enable the catalog
  install path/UI flow.

## Modularity Phase C - Catalog bridge to Flutter

- Added `poh_cli catalog-list`: a JSON command that merges the trusted source
  catalog with the managed core store state.
- The command exposes source status, installability, installed/active versions,
  descriptor availability, executable relative path, license/homepage/notes, and
  pin metadata without enabling unpinned downloads.
- Flutter `BackendSessionService` can now read that catalog.
- Main UI now maps the Rust catalog into the visible core list:
  - TrustTunnel stays openable through the managed-bundle migration path;
  - optional cores remain visible but locked/planned until installed or pinned;
  - menu/tagline data comes from the backend when available.
- `RoutesShell` now receives the same core list instead of reading the static
  global list, and its core tabs are horizontally scrollable so Hysteria2/future
  cores are not clipped.
- Remaining Phase C: select and pin a real NaiveProxy release, then wire
  download/install UI to `core-download-plan` + `core-install`.

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

## Process Lifecycle P2 - DNS network safety-net (orchestrator pass 2026-06-18)

Generalised the `network_effects` ledger so a session can record + force-restore
DNS, not just the system proxy. Full wiring notes for the next coder are in
`todone_arh.md` ("How to use the DNS network safety-net").

- `NetworkEffectsState` is now generic: `system_proxy` + `dns` (both in `session.json`).
- New `DnsLease` / `DnsInterfaceSnapshot` / `DnsConfig` (Dhcp | Static) / `DnsFamily`.
- Pure `dns_restore_commands()` builds the `netsh` argv to put DNS back (DHCP or
  static-with-priority); `restore_dns_lease` (cfg windows) runs it.
- `restore_network_effects` now restores DNS + proxy and attempts ALL effects even
  if one fails (a safety-net must not abort half-way). It already runs in every
  teardown via `clear_persisted_session` (stop / `desktop-session-reset` / reconcile).
- New `NetworkEffectError::DnsCommand`.
- Tests: +5 pure unit tests on the rollback logic (no real network touched).
- Remaining: wire the start-time DNS snapshot to populate the lease, then add
  route / kill-switch repair with the same pattern (documented in todone_arh.md).

## Release hardening - Authenticode signature verification (orchestrator pass 2026-06-18)

Workstream 5 item. Second integrity layer on top of SHA-256 pinning. Full usage
notes for the next coder are in `todone_arh.md` ("How to use signature verification").

- New `crates/poh_core_store/src/signature.rs`: `authenticode_status(path)` using
  `WinVerifyTrust` (windows-sys, features `Win32_Security_WinTrust` +
  `Win32_Security_Cryptography`). Maps result -> `Verified` / `Unsigned` / `Unknown`;
  non-Windows builds return `Unknown`.
- `CoreStore::install()` / `install_manual_bundle()` now record the real
  `signature_status` in the stored manifest (via `promote_staged`).
- `CoreStore::with_signature_required(true)` makes `verify_core` reject anything not
  `Verified` with the new `CoreStoreError::SignatureRejected`. Default is OFF (current
  bundled cores are unsigned), so nothing at runtime is forced yet - it is wired and
  ready to enable for downloadable cores once the install UI ships.
- Tests: +2 (`authenticode_status_never_verifies_unsigned_bytes`,
  `verify_core_enforces_signature_only_when_required`, both `#[cfg(windows)]`).
- Verified: `cargo test --workspace` (70 pass), `cargo fmt --check` clean,
  `cargo clippy -p poh_core_store` clean.

## Process Lifecycle P2 - DNS snapshot wiring + localized parser

- Confirmed the DNS safety-net is wired into `start_desktop_session`: TUN profiles
  with `change_system_dns` snapshot DNS before the core can mutate it and store
  the lease in `session.network_effects.dns` after readiness succeeds.
- `restore_network_effects` already restores the recorded DNS lease on stop,
  reset, and reconcile paths.
- Fixed a practical Windows issue: `parse_netsh_dnsservers` no longer depends on
  English `netsh` headers. It now detects quoted interface headers and DNS
  label/value rows in a language-tolerant way, so localized Windows output is
  covered too.
- Added localized DHCP/static parser regressions.

## NaiveProxy pinned catalog + install UI confirmation

- Confirmed `trusted-sources.json` pins NaiveProxy
  `v149.0.7827.114-1` / `naiveproxy-v149.0.7827.114-1-win-x64.zip` /
  `50f8138a1cfaeaf28866cb9f7ff25fbd92d2b3bd642885e95131f7d56ebf1086`.
- `catalog-list` reports NaiveProxy as `active`, `install_enabled: true`,
  `installable: true`.
- `core-download-plan naiveproxy` returns the pinned GitHub asset URL and SHA.
- Flutter has the install bridge (`coreInstall`, `_installCore`,
  `CoreSpec.installable/installing`, menu "Install" / spinner / "Open").
- Not done yet: manual acceptance with a real NaiveProxy config
  (import config.json -> install -> connect -> SOCKS check).

## Verified

- `cargo fmt --all` / `cargo fmt --all --check` (clean)
- `cargo test --workspace` (84 tests pass)
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
- Latest catalog bridge pass:
  - `cargo fmt --check` (after `cargo fmt`)
  - `cargo test` (68 Rust tests pass)
  - `cargo run -p poh_cli -- catalog-list`
  - `flutter analyze`
  - `flutter test` (6 tests pass)
  - `flutter build windows` -> `apps/desktop_flutter/build/windows/x64/runner/Release/proxy_open_hub.exe`
- Latest verification pass:
  - `cargo fmt --check`
  - `cargo test` (84 Rust tests pass)
  - `cargo test -p poh_cli network_effects` (16 DNS/network tests pass)
  - `cargo run -p poh_cli -- catalog-list`
  - `cargo run -p poh_cli -- core-download-plan naiveproxy`
  - `flutter analyze`
  - `flutter test` (6 tests pass)
  - `flutter build windows`

## Remaining

- Add long-lived watchdog/service behavior with automatic crash rollback.
- Add route/firewall/kill-switch repair hooks for TrustTunnel crash/force-kill cases.
- Run NaiveProxy manual acceptance with a real config and SOCKS check.
- Turn signature/AuthentiCode enforcement on for downloadable cores and add a
  pinned-publisher check.

For the authoritative, verified status of every item (done / partial / remaining,
with per-workstream %), see the live checklist in `todone_arh.md`.
