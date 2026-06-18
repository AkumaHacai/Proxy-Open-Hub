use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose, Engine as _};
use poh_core::{
    sha256_hex, CoreId, CoreRegistry, EndpointConfig, ImportInput, InstalledCoreFile,
    InstalledCoreManifest, ListenerConfig, ListenerMode, LogLevel, NaiveProxyAdapter,
    NaiveProxyCoreProfile, Profile, ProfileId, Redactor, RoutingMode, RoutingProfile,
    SignatureStatus, SocksConfig, SourceType, TrustTunnelAdapter, TrustTunnelConfig,
    TrustTunnelCoreProfile, TrustedCoreSource, TrustedSourcePolicy, TunConfig, UpstreamProtocol,
    ValidationWarning,
};
use poh_core_runner::{MapSecretResolver, MaterializedRuntime, RuntimeMaterializer};
use poh_core_session::{
    wait_for_process_startup, CoreLaunchDescriptor, SessionFileLock, SessionLifecycleState,
    SessionStartupProbe, SessionTimings,
};
use poh_core_store::CoreStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::network_effects::{
    apply_system_proxy_lease, prepare_system_proxy, restore_network_effects, NetworkEffectError,
    NetworkEffectsState, SystemProxyConfig,
};

#[cfg(windows)]
use windows_sys::Win32::Foundation::LocalFree;
#[cfg(windows)]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopSessionPlan {
    pub profile_id: String,
    pub profile_name: String,
    pub core_id: String,
    pub command_args: Vec<String>,
    pub redacted_preview: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopSessionStart {
    pub profile_id: String,
    pub profile_name: String,
    pub core_id: String,
    pub pid: u32,
    pub executable_path: String,
    pub config_path: String,
    pub log_path: String,
    pub runtime_dir: String,
    pub state_path: String,
    pub command_args: Vec<String>,
    pub redacted_preview: String,
    pub started_at_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopSessionStatus {
    pub running: bool,
    pub session: Option<PersistedDesktopSession>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopSessionLog {
    pub running: bool,
    pub log_path: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopImportResult {
    pub profile_id: String,
    pub profile_name: String,
    pub core_id: String,
    pub state_path: String,
    pub secrets_imported: usize,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopImportPreview {
    pub profile_name: String,
    pub core_id: String,
    pub secrets_detected: usize,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct PersistedDesktopSession {
    pub profile_id: String,
    pub profile_name: String,
    pub core_id: String,
    pub pid: u32,
    pub executable_path: String,
    pub config_path: String,
    pub log_path: String,
    pub runtime_dir: String,
    #[serde(default)]
    pub state_path: String,
    pub started_at_unix_ms: u64,
    #[serde(default = "default_persisted_session_state")]
    pub lifecycle_state: SessionLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default)]
    pub updated_at_unix_ms: u64,
    /// OS process creation time (Windows FILETIME, 100ns ticks). Combined with
    /// `pid` it forms a reuse-proof identity: a recycled PID has a different
    /// creation time, so it can never be mistaken for our core. `0` means the
    /// session predates identity tracking (legacy fallback to image-name match).
    #[serde(default)]
    pub creation_time_100ns: u64,
    #[serde(default, skip_serializing_if = "NetworkEffectsState::is_empty")]
    pub network_effects: NetworkEffectsState,
}

const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PROFILE_ID_SUFFIX: usize = 10_000;
const DPAPI_SECRET_PREFIX: &str = "dpapi:v1:";
const BUNDLED_TRUSTTUNNEL_SHA256: &str =
    "d260f372d5d8f051180e829e47f0b2ae65c1aeda63ab0e56fefa7b68349f2ab0";
const BUNDLED_WINTUN_SHA256: &str =
    "e5da8447dc2c320edc0fc52fa01885c103de8c118481f683643cacc3220dafce";

fn desktop_core_registry() -> CoreRegistry {
    let mut registry = CoreRegistry::new();
    registry.register(TrustTunnelAdapter::new());
    registry.register(NaiveProxyAdapter::new());
    registry
}

pub fn build_desktop_session_plan(
    state_path: &Path,
    profile_id: &str,
) -> Result<DesktopSessionPlan, DesktopStateError> {
    let state = load_desktop_state(state_path)?;
    let secrets = state.resolved_secrets()?;
    let profile = state
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| DesktopStateError::ProfileNotFound(profile_id.to_string()))?;
    let (profile, materialized, redacted_preview) = build_materialized_session(profile, &secrets)?;

    Ok(DesktopSessionPlan {
        profile_id: profile.id.to_string(),
        profile_name: profile.name,
        core_id: profile.core_id.to_string(),
        command_args: materialized.command_args,
        redacted_preview,
    })
}

pub fn start_desktop_session(
    state_path: &Path,
    profile_id: &str,
) -> Result<DesktopSessionStart, DesktopStateError> {
    let _lock = SessionFileLock::acquire(session_lock_file()?)?;
    if let Some(session) = reconcile_desktop_session_unlocked()? {
        return Err(DesktopStateError::SessionAlreadyRunning(session.pid));
    }

    let state = load_desktop_state(state_path)?;
    let secrets = state.resolved_secrets()?;
    let desktop_profile = state
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| DesktopStateError::ProfileNotFound(profile_id.to_string()))?;
    let system_proxy_config = system_proxy_config_for_profile(&desktop_profile)?;
    let startup_probe = startup_probe_for_profile(&desktop_profile);
    let (profile, materialized, redacted_preview) =
        build_materialized_session(desktop_profile, &secrets)?;
    let launch_descriptor = CoreLaunchDescriptor::for_core(&profile.core_id).ok_or_else(|| {
        DesktopStateError::Session(poh_core_session::SessionError::MissingLaunchDescriptor(
            profile.core_id.to_string(),
        ))
    })?;
    let resolved_core = resolve_core(&profile.core_id)?;
    let executable_path = resolved_core.executable_path.clone();
    let started_at_unix_ms = now_unix_ms();
    let runtime_dir = runtime_root()?.join(format!(
        "{}-{}",
        sanitize_path_segment(profile.id.as_str()),
        started_at_unix_ms
    ));
    let mut runtime_guard = RuntimeDirGuard::new(runtime_dir.clone());
    let written = materialized.write_to(&runtime_dir)?;
    restrict_runtime_permissions(&runtime_dir)?;
    for path in &written {
        restrict_runtime_permissions(path)?;
    }
    let config_path = written
        .first()
        .cloned()
        .ok_or_else(|| DesktopStateError::RuntimeConfigMissing("runtime config".to_string()))?;
    let log_path = launch_descriptor
        .log_path(&runtime_dir)
        .unwrap_or_else(|| runtime_dir.join("core.log"));
    let stdout = File::create(&log_path)?;
    restrict_runtime_permissions(&log_path)?;
    let stderr = stdout.try_clone()?;
    let launch_spec = launch_descriptor.build_spec(
        &executable_path,
        &resolved_core.install_dir,
        &runtime_dir,
        &materialized,
    )?;
    let command_args = launch_spec.args.clone();
    let mut command = Command::new(&launch_spec.executable_path);
    command
        .current_dir(&launch_spec.working_dir)
        .args(&launch_spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    configure_core_command(&mut command);

    for (key, value) in &launch_spec.environment {
        command.env(key, value);
    }

    let mut child = command.spawn()?;

    let pid = child.id();
    // Snapshot the OS creation time immediately so later stop/status calls can
    // prove this PID is still our core and not a recycled one.
    let creation_time_100ns = process_creation_time(pid).unwrap_or(0);
    let mut session = PersistedDesktopSession {
        profile_id: profile.id.to_string(),
        profile_name: profile.name,
        core_id: profile.core_id.to_string(),
        pid,
        executable_path: executable_path.display().to_string(),
        config_path: config_path.display().to_string(),
        log_path: log_path.display().to_string(),
        runtime_dir: runtime_dir.display().to_string(),
        state_path: state_path.display().to_string(),
        started_at_unix_ms,
        lifecycle_state: SessionLifecycleState::Starting,
        last_error: None,
        updated_at_unix_ms: started_at_unix_ms,
        creation_time_100ns,
        network_effects: NetworkEffectsState::default(),
    };
    if let Err(error) = save_session(&session) {
        let _ = child.kill();
        return Err(error);
    }

    let startup_timings = startup_timings_for_probe(&startup_probe);
    if let Err(error) = wait_for_process_startup(&mut child, &startup_probe, startup_timings) {
        let log_tail = read_log_tail(&log_path);
        session.lifecycle_state = SessionLifecycleState::Faulted;
        session.last_error = Some(format!("{error}: {log_tail}"));
        session.updated_at_unix_ms = now_unix_ms();
        let _ = save_session(&session);
        runtime_guard.keep();
        return match error {
            poh_core_session::SessionError::CoreExitedDuringStartup(code) => {
                Err(DesktopStateError::CoreExitedDuringStartup(code, log_tail))
            }
            poh_core_session::SessionError::ReadinessTimedOut(address) => {
                Err(DesktopStateError::CoreReadinessTimedOut(address, log_tail))
            }
            other => Err(DesktopStateError::Session(other)),
        };
    }

    if let Some(system_proxy_config) = system_proxy_config {
        match prepare_system_proxy(&system_proxy_config, now_unix_ms()) {
            Ok(lease) => {
                session.network_effects.system_proxy = Some(lease);
                session.updated_at_unix_ms = now_unix_ms();
                if let Err(error) = save_session(&session) {
                    let _ = terminate_session_process(&session);
                    let _ = clear_persisted_session(Some(session.clone()));
                    return Err(error);
                }
                if let Err(error) = session
                    .network_effects
                    .system_proxy
                    .as_ref()
                    .map(apply_system_proxy_lease)
                    .transpose()
                {
                    let _ = terminate_session_process(&session);
                    let _ = clear_persisted_session(Some(session.clone()));
                    return Err(error.into());
                }
            }
            Err(error) => {
                let _ = terminate_session_process(&session);
                let _ = clear_persisted_session(Some(session.clone()));
                return Err(error.into());
            }
        }
    }

    if let Err(error) = enforce_transition(&mut session, SessionLifecycleState::Running) {
        let _ = terminate_session_process(&session);
        let _ = clear_persisted_session(Some(session.clone()));
        return Err(error);
    }
    runtime_guard.keep();

    Ok(DesktopSessionStart {
        profile_id: session.profile_id.clone(),
        profile_name: session.profile_name.clone(),
        core_id: session.core_id.clone(),
        pid: session.pid,
        executable_path: session.executable_path.clone(),
        config_path: session.config_path.clone(),
        log_path: session.log_path.clone(),
        runtime_dir: session.runtime_dir.clone(),
        state_path: session.state_path.clone(),
        command_args,
        redacted_preview,
        started_at_unix_ms,
    })
}

pub fn stop_desktop_session() -> Result<DesktopSessionStatus, DesktopStateError> {
    let _lock = SessionFileLock::acquire(session_lock_file()?)?;
    let Some(session) = reconcile_desktop_session_unlocked()? else {
        return Ok(DesktopSessionStatus {
            running: false,
            session: None,
        });
    };

    let mut stopping_session = session.clone();
    if enforce_transition(&mut stopping_session, SessionLifecycleState::Stopping).is_err() {
        // Disconnect must always be able to clean up; record Stopping directly if
        // the recorded state was unexpected rather than refusing to stop.
        stopping_session.lifecycle_state = SessionLifecycleState::Stopping;
        stopping_session.updated_at_unix_ms = now_unix_ms();
        let _ = save_session(&stopping_session);
    }

    if is_session_process_running(&session)? {
        terminate_session_process(&session)?;
    }

    // Only declare the session gone once the process is confirmed dead. A
    // surviving core would otherwise be orphaned with no record to stop it.
    if is_session_process_running(&session)? {
        let mut faulted = session.clone();
        faulted.lifecycle_state = SessionLifecycleState::Faulted;
        faulted.last_error = Some("core process did not exit after stop".to_string());
        faulted.updated_at_unix_ms = now_unix_ms();
        let _ = save_session(&faulted);
        return Err(DesktopStateError::CoreStopFailed(session.pid));
    }

    clear_persisted_session(Some(session.clone()))?;

    Ok(DesktopSessionStatus {
        running: false,
        session: Some(session),
    })
}

pub fn reset_desktop_session() -> Result<DesktopSessionStatus, DesktopStateError> {
    let _lock = SessionFileLock::acquire(session_lock_file()?)?;
    if let Some(session) = load_session()? {
        if is_session_process_running(&session)? {
            terminate_session_process(&session)?;
            if is_session_process_running(&session)? {
                let mut faulted = session.clone();
                faulted.lifecycle_state = SessionLifecycleState::Faulted;
                faulted.last_error = Some("core process did not exit during reset".to_string());
                faulted.updated_at_unix_ms = now_unix_ms();
                let _ = save_session(&faulted);
                return Err(DesktopStateError::CoreStopFailed(session.pid));
            }
        }
        clear_persisted_session(Some(session))?;
    }
    cleanup_orphaned_runtime_dirs()?;

    Ok(DesktopSessionStatus {
        running: false,
        session: None,
    })
}

pub fn desktop_session_status() -> Result<DesktopSessionStatus, DesktopStateError> {
    desktop_session_status_unlocked()
}

fn reconcile_desktop_session_unlocked() -> Result<Option<PersistedDesktopSession>, DesktopStateError>
{
    let Some(session) = load_session()? else {
        cleanup_orphaned_runtime_dirs()?;
        return Ok(None);
    };

    let running = is_session_process_running(&session)?;
    if running {
        if session.lifecycle_state == SessionLifecycleState::Stopping {
            terminate_session_process(&session)?;
            if is_session_process_running(&session)? {
                let mut faulted = session.clone();
                faulted.lifecycle_state = SessionLifecycleState::Faulted;
                faulted.last_error = Some("core process did not exit during reconcile".to_string());
                faulted.updated_at_unix_ms = now_unix_ms();
                let _ = save_session(&faulted);
                return Err(DesktopStateError::CoreStopFailed(session.pid));
            }
            clear_persisted_session(Some(session))?;
            cleanup_orphaned_runtime_dirs()?;
            return Ok(None);
        }

        if session.lifecycle_state == SessionLifecycleState::Idle {
            let mut migrated = session.clone();
            migrated.lifecycle_state = SessionLifecycleState::Running;
            migrated.updated_at_unix_ms = now_unix_ms();
            let _ = save_session(&migrated);
            return Ok(Some(migrated));
        }

        return Ok(Some(session));
    }

    clear_persisted_session(Some(session))?;
    cleanup_orphaned_runtime_dirs()?;
    Ok(None)
}

fn desktop_session_status_unlocked() -> Result<DesktopSessionStatus, DesktopStateError> {
    let Some(session) = load_session()? else {
        return Ok(DesktopSessionStatus {
            running: false,
            session: None,
        });
    };
    let running = is_session_process_running(&session)?;
    if running && session.lifecycle_state == SessionLifecycleState::Idle {
        let mut migrated = session.clone();
        migrated.lifecycle_state = SessionLifecycleState::Running;
        migrated.updated_at_unix_ms = now_unix_ms();
        let _ = save_session(&migrated);
        return Ok(DesktopSessionStatus {
            running,
            session: Some(migrated),
        });
    }

    if !running {
        let mut faulted = session.clone();
        if matches!(
            faulted.lifecycle_state,
            SessionLifecycleState::Starting
                | SessionLifecycleState::Running
                | SessionLifecycleState::Stopping
        ) {
            faulted.lifecycle_state = SessionLifecycleState::Faulted;
            faulted.last_error = Some("session process is not running".to_string());
            faulted.updated_at_unix_ms = now_unix_ms();
            clear_persisted_session(Some(session))?;
            return Ok(DesktopSessionStatus {
                running,
                session: Some(faulted),
            });
        }
    }

    Ok(DesktopSessionStatus {
        running,
        session: Some(session),
    })
}

fn default_persisted_session_state() -> SessionLifecycleState {
    SessionLifecycleState::Running
}

pub fn desktop_session_log() -> Result<DesktopSessionLog, DesktopStateError> {
    let status = desktop_session_status()?;
    let Some(session) = status.session else {
        return Ok(DesktopSessionLog {
            running: false,
            log_path: None,
            content: "No active TrustTunnel session.".to_string(),
        });
    };

    let log_path = PathBuf::from(&session.log_path);
    let content = read_log_tail(&log_path);
    let redacted_content = redact_session_log(&content, &session);
    Ok(DesktopSessionLog {
        running: status.running,
        log_path: Some(session.log_path),
        content: redacted_content,
    })
}

pub fn import_desktop_profile(input: &str) -> Result<DesktopImportResult, DesktopStateError> {
    if input.len() > MAX_IMPORT_BYTES {
        return Err(DesktopStateError::ImportTooLarge(input.len()));
    }

    let registry = desktop_core_registry();
    let import_input = ImportInput::text(input.to_string());
    let detections = registry.detect_import(&import_input);
    let detection = detections
        .first()
        .ok_or(poh_core::AdapterError::UnsupportedInput)?;
    let adapter = registry
        .adapter(&detection.core_id)
        .ok_or_else(|| DesktopStateError::CoreAdapterNotFound(detection.core_id.to_string()))?;
    let parsed = adapter.parse_profile(&import_input)?;
    let warnings = parsed.warnings.clone();
    let state_path = default_desktop_state_file();
    let mut state = load_or_default_desktop_state(&state_path)?;
    let profile_id = unique_profile_id(parsed.profile.id.as_str(), &state);
    let profile_name = parsed.profile.name.clone();
    let core_id = parsed.profile.core_id.to_string();
    let secrets_imported = parsed.secrets.len();
    let (desktop_profile, secrets) = desktop_profile_from_import(profile_id.clone(), parsed)?;

    state.profiles.push(desktop_profile);
    state.insert_protected_secrets(secrets)?;
    save_desktop_state(&state_path, &state)?;

    Ok(DesktopImportResult {
        profile_id,
        profile_name,
        core_id,
        state_path: state_path.display().to_string(),
        secrets_imported,
        warnings,
    })
}

pub fn preview_desktop_profile(input: &str) -> Result<DesktopImportPreview, DesktopStateError> {
    if input.len() > MAX_IMPORT_BYTES {
        return Err(DesktopStateError::ImportTooLarge(input.len()));
    }

    let registry = desktop_core_registry();
    let import_input = ImportInput::text(input.to_string());
    let detections = registry.detect_import(&import_input);
    let detection = detections
        .first()
        .ok_or(poh_core::AdapterError::UnsupportedInput)?;
    let adapter = registry
        .adapter(&detection.core_id)
        .ok_or_else(|| DesktopStateError::CoreAdapterNotFound(detection.core_id.to_string()))?;
    let parsed = adapter.parse_profile(&import_input)?;

    Ok(DesktopImportPreview {
        profile_name: parsed.profile.name,
        core_id: parsed.profile.core_id.to_string(),
        secrets_detected: parsed.secrets.len(),
        warnings: parsed.warnings,
    })
}

fn load_desktop_state(state_path: &Path) -> Result<DesktopState, DesktopStateError> {
    let metadata = fs::metadata(state_path)?;
    if metadata.len() > MAX_STATE_BYTES {
        return Err(DesktopStateError::StateTooLarge(
            state_path.display().to_string(),
        ));
    }

    let content = fs::read_to_string(state_path)?;
    let mut state = serde_json::from_str::<DesktopState>(&content)?;
    if state.migrate_plaintext_secrets()? {
        save_desktop_state(state_path, &state)?;
    }

    Ok(state)
}

fn load_or_default_desktop_state(state_path: &Path) -> Result<DesktopState, DesktopStateError> {
    if state_path.exists() {
        return load_desktop_state(state_path);
    }

    let legacy_state_path = legacy_desktop_state_file();
    if legacy_state_path.exists() {
        return load_desktop_state(&legacy_state_path);
    }

    Ok(DesktopState::default())
}

fn save_desktop_state(state_path: &Path, state: &DesktopState) -> Result<(), DesktopStateError> {
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(state_path, serde_json::to_string_pretty(state)?)?;
    restrict_runtime_permissions(state_path)?;
    Ok(())
}

fn desktop_profile_from_import(
    profile_id: String,
    parsed: poh_core::ParsedProfile,
) -> Result<(DesktopProfile, BTreeMap<String, String>), DesktopStateError> {
    if parsed.profile.core_id == CoreId::from("trusttunnel") {
        let profile_name = parsed.profile.name.clone();
        let core_profile =
            serde_json::from_value::<TrustTunnelCoreProfile>(parsed.profile.core_config)?;
        return Ok(desktop_profile_from_trusttunnel_import(
            profile_id,
            profile_name,
            core_profile,
            parsed.secrets,
        ));
    }

    Ok(desktop_profile_from_generic_import(profile_id, parsed))
}

fn desktop_profile_from_trusttunnel_import(
    profile_id: String,
    profile_name: String,
    core_profile: TrustTunnelCoreProfile,
    imported_secrets: BTreeMap<String, String>,
) -> (DesktopProfile, BTreeMap<String, String>) {
    let TrustTunnelConfig {
        log_level: _,
        endpoint,
        routing,
        listener,
    } = core_profile.config;
    let ListenerConfig { mode, tun, socks } = listener;
    let mut secrets = BTreeMap::new();
    let password_secret_ref = import_secret_ref(
        &profile_id,
        "endpoint.password",
        &imported_secrets,
        &mut secrets,
    );
    let client_random_secret_ref = import_secret_ref(
        &profile_id,
        "endpoint.client_random",
        &imported_secrets,
        &mut secrets,
    );
    let socks_password_secret_ref = import_secret_ref(
        &profile_id,
        "listener.socks.password",
        &imported_secrets,
        &mut secrets,
    );

    (
        DesktopProfile {
            id: profile_id,
            display_name: profile_name,
            core_id: "trusttunnel".to_string(),
            core_config: None,
            secret_refs: BTreeMap::new(),
            endpoint: DesktopEndpoint {
                hostname: endpoint.hostname,
                custom_sni: endpoint.custom_sni,
                addresses: endpoint.addresses,
                has_ipv6: endpoint.has_ipv6,
                username: endpoint.username,
                password_secret_ref,
                client_random_secret_ref,
                skip_verification: endpoint.skip_verification,
                certificate_pem: endpoint.certificate_pem,
                upstream_protocol: upstream_protocol_to_desktop(endpoint.upstream_protocol),
                fallback_protocol: endpoint.fallback_protocol.map(upstream_protocol_to_desktop),
                anti_dpi: endpoint.anti_dpi,
                post_quantum_group_enabled: endpoint.post_quantum_group_enabled,
                dns_upstreams: endpoint.dns_upstreams,
            },
            routing: DesktopRouting {
                id: routing.id,
                name: routing.name,
                mode: routing_mode_to_desktop(routing.mode),
                exclusions: routing.exclusions,
                kill_switch_enabled: routing.kill_switch_enabled,
                kill_switch_allow_ports: routing.kill_switch_allow_ports,
                description: routing.description,
            },
            listener: DesktopListener {
                mode: listener_mode_to_desktop(mode),
                tun: DesktopTun {
                    bound_if: tun.bound_if,
                    included_routes: tun.included_routes,
                    excluded_routes: tun.excluded_routes,
                    mtu_size: tun.mtu_size,
                    tcp_recv_buf_size: tun.tcp_recv_buf_size,
                    tcp_send_buf_size: tun.tcp_send_buf_size,
                    change_system_dns: tun.change_system_dns,
                    device_name: tun.device_name,
                    use_existing: tun.use_existing,
                },
                socks: DesktopSocks {
                    address: socks.address,
                    username: socks.username,
                    password_secret_ref: socks_password_secret_ref,
                    allow_lan_access: socks.allow_lan_access,
                    http_proxy_address: socks.http_proxy_address,
                    http_proxy_allow_lan_access: socks.http_proxy_allow_lan_access,
                },
                system_proxy: DesktopSystemProxy::default(),
            },
        },
        secrets,
    )
}

fn desktop_profile_from_generic_import(
    profile_id: String,
    parsed: poh_core::ParsedProfile,
) -> (DesktopProfile, BTreeMap<String, String>) {
    let mut protected_secrets = BTreeMap::new();
    let mut secret_refs = BTreeMap::new();
    for (key, value) in parsed.secrets {
        if value.trim().is_empty() {
            continue;
        }

        let secret_ref = format!("secret://{profile_id}/{key}");
        secret_refs.insert(key, secret_ref.clone());
        protected_secrets.insert(secret_ref, value);
    }

    (
        DesktopProfile {
            id: profile_id,
            display_name: parsed.profile.name,
            core_id: parsed.profile.core_id.to_string(),
            core_config: Some(parsed.profile.core_config),
            secret_refs,
            ..DesktopProfile::default()
        },
        protected_secrets,
    )
}

fn import_secret_ref(
    profile_id: &str,
    key: &str,
    imported_secrets: &BTreeMap<String, String>,
    secrets: &mut BTreeMap<String, String>,
) -> String {
    let Some(value) = imported_secrets.get(key) else {
        return String::new();
    };
    if value.trim().is_empty() {
        return String::new();
    }

    let secret_ref = format!("secret://{profile_id}/{key}");
    secrets.insert(secret_ref.clone(), value.clone());
    secret_ref
}

fn unique_profile_id(base: &str, state: &DesktopState) -> String {
    let base = sanitize_path_segment(base);
    if !state
        .profiles
        .iter()
        .any(|profile| profile.id == base || sanitize_path_segment(&profile.id) == base)
    {
        return base;
    }

    for suffix in 2..=MAX_PROFILE_ID_SUFFIX {
        let candidate = format!("{base}-{suffix}");
        if !state.profiles.iter().any(|profile| {
            profile.id == candidate || sanitize_path_segment(&profile.id) == candidate
        }) {
            return candidate;
        }
    }

    format!("{base}-{}", now_unix_ms())
}

fn build_materialized_session(
    desktop_profile: DesktopProfile,
    secrets: &BTreeMap<String, String>,
) -> Result<(Profile, MaterializedRuntime, String), DesktopStateError> {
    let registry = desktop_core_registry();
    let profile = desktop_profile.clone().into_profile()?;
    let adapter = registry
        .adapter(&profile.core_id)
        .ok_or_else(|| DesktopStateError::CoreAdapterNotFound(profile.core_id.to_string()))?;
    let secret_values = secret_resolver_values(&desktop_profile, secrets)?;
    let runtime_config = adapter.build_runtime_config(&profile)?;
    let resolver = MapSecretResolver::new(secret_values);
    let materialized = RuntimeMaterializer::default().materialize(&runtime_config, &resolver)?;
    let redacted_preview = materialized.redacted_preview();

    Ok((profile, materialized, redacted_preview))
}

fn secret_resolver_values(
    profile: &DesktopProfile,
    secrets: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, DesktopStateError> {
    if profile.core_config.is_some() {
        return profile
            .secret_refs
            .iter()
            .map(|(key, secret_ref)| Ok((key.clone(), read_optional_secret(secrets, secret_ref)?)))
            .collect();
    }

    let mut values = BTreeMap::new();
    values.insert(
        "endpoint.password".to_string(),
        read_required_secret(secrets, &profile.endpoint.password_secret_ref)?,
    );
    values.insert(
        "endpoint.client_random".to_string(),
        read_optional_secret(secrets, &profile.endpoint.client_random_secret_ref)?,
    );
    values.insert(
        "listener.socks.password".to_string(),
        read_optional_secret(secrets, &profile.listener.socks.password_secret_ref)?,
    );

    Ok(values)
}

fn read_required_secret(
    secrets: &BTreeMap<String, String>,
    secret_ref: &str,
) -> Result<String, DesktopStateError> {
    let value = read_optional_secret(secrets, secret_ref)?;
    if value.is_empty() {
        return Err(DesktopStateError::SecretNotFound(
            "endpoint.password".to_string(),
        ));
    }

    Ok(value)
}

fn read_optional_secret(
    secrets: &BTreeMap<String, String>,
    secret_ref: &str,
) -> Result<String, DesktopStateError> {
    if secret_ref.trim().is_empty() {
        return Ok(String::new());
    }

    secrets
        .get(secret_ref)
        .cloned()
        .ok_or_else(|| DesktopStateError::SecretNotFound(secret_ref.to_string()))
}

#[derive(Debug, Error)]
pub enum DesktopStateError {
    #[error("profile not found in desktop state: {0}")]
    ProfileNotFound(String),
    #[error("secret was not found in desktop state: {0}")]
    SecretNotFound(String),
    #[error("secret protection failed: {0}")]
    SecretProtection(String),
    #[error("desktop session is already running with pid {0}")]
    SessionAlreadyRunning(u32),
    #[error("core adapter is not registered: {0}")]
    CoreAdapterNotFound(String),
    #[error("TrustTunnel client executable was not found")]
    CoreExecutableNotFound,
    #[error("invalid TrustTunnel client executable path: {0}")]
    InvalidCorePath(PathBuf),
    #[error("runtime config was not materialized: {0}")]
    RuntimeConfigMissing(String),
    #[error("TrustTunnel core exited during startup with code {0:?}: {1}")]
    CoreExitedDuringStartup(Option<i32>, String),
    #[error("TrustTunnel readiness probe timed out for {0}: {1}")]
    CoreReadinessTimedOut(String, String),
    #[error("import input is too large: {0} bytes")]
    ImportTooLarge(usize),
    #[error("desktop state is too large: {0}")]
    StateTooLarge(String),
    #[error("POH_TRUSTTUNNEL_CORE_PATH is allowed only for dev runs")]
    CoreOverrideDisabled,
    #[error("core is not installed in the managed store: {0}")]
    CoreNotInstalled(String),
    #[error("core process {0} did not exit after stop")]
    CoreStopFailed(u32),
    #[error("illegal session transition from {0:?} to {1:?}")]
    IllegalTransition(SessionLifecycleState, SessionLifecycleState),
    #[error(transparent)]
    NetworkEffect(#[from] NetworkEffectError),
    #[error("bundled core integrity mismatch for {path}: expected {expected}, got {actual}")]
    CoreIntegrityMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("invalid system proxy configuration: {0}")]
    InvalidSystemProxyConfig(String),
    #[error("bundled core sidecar is missing: {0}")]
    CoreSidecarMissing(String),
    #[error(transparent)]
    Adapter(#[from] poh_core::AdapterError),
    #[error(transparent)]
    Runner(#[from] poh_core_runner::RunnerError),
    #[error(transparent)]
    Session(#[from] poh_core_session::SessionError),
    #[error(transparent)]
    CoreStore(#[from] poh_core_store::CoreStoreError),
    #[error(transparent)]
    Security(#[from] poh_core::SecurityError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Resolved location of a verified core executable inside the managed store.
struct ResolvedCore {
    executable_path: PathBuf,
    install_dir: PathBuf,
}

const BUNDLED_TRUSTTUNNEL_VERSION: &str = "1.0.49";

/// Resolve the executable for a profile's core from the managed core store.
///
/// Every core now launches from `cores/<core_id>/<version>/`. TrustTunnel keeps
/// a migration path: if it has not been provisioned into the store yet, the
/// app-local bundle is verified by pinned hashes and copied into the store on
/// first use. Other cores (downloadable modules) must already be installed.
fn resolve_core(core_id: &CoreId) -> Result<ResolvedCore, DesktopStateError> {
    if core_id.as_str() == "trusttunnel" {
        return resolve_trusttunnel_core();
    }

    resolve_store_core(core_id)
}

fn resolve_store_core(core_id: &CoreId) -> Result<ResolvedCore, DesktopStateError> {
    let store = CoreStore::new(core_store_root());
    let sources = embedded_trusted_sources()?;
    let version = store
        .active_version(core_id)?
        .ok_or_else(|| DesktopStateError::CoreNotInstalled(core_id.to_string()))?;
    let verified = store.verify_core(core_id, &version, &sources)?;
    Ok(ResolvedCore {
        executable_path: verified.executable_path,
        install_dir: verified.install_dir,
    })
}

fn resolve_trusttunnel_core() -> Result<ResolvedCore, DesktopStateError> {
    // Dev-only override stays a direct, hash-checked path and never touches the store.
    if let Ok(path) = env::var("POH_TRUSTTUNNEL_CORE_PATH") {
        if !dev_core_override_enabled() {
            return Err(DesktopStateError::CoreOverrideDisabled);
        }

        let candidate = PathBuf::from(path);
        if candidate.exists() {
            let executable_path = verify_trusttunnel_bundle(candidate)?;
            let install_dir = executable_path
                .parent()
                .ok_or_else(|| DesktopStateError::InvalidCorePath(executable_path.clone()))?
                .to_path_buf();
            return Ok(ResolvedCore {
                executable_path,
                install_dir,
            });
        }
    }

    let store = CoreStore::new(core_store_root());
    let core_id = CoreId::from("trusttunnel");
    let sources = embedded_trusted_sources()?;

    // Fast path: already provisioned into the store and still intact.
    if let Some(version) = store.active_version(&core_id)? {
        if let Ok(verified) = store.verify_core(&core_id, &version, &sources) {
            return Ok(ResolvedCore {
                executable_path: verified.executable_path,
                install_dir: verified.install_dir,
            });
        }
    }

    // Migration path: provision the app-local bundle into the managed store.
    let bundle_dir = app_local_bundle_dir()?;
    let manifest = bundled_trusttunnel_manifest();
    store.install_manual_bundle(&manifest, &bundle_dir, &sources)?;
    let verified = store.verify_core(&core_id, &manifest.version, &sources)?;
    Ok(ResolvedCore {
        executable_path: verified.executable_path,
        install_dir: verified.install_dir,
    })
}

fn app_local_bundle_dir() -> Result<PathBuf, DesktopStateError> {
    let current_exe = env::current_exe()?;
    let app_dir = current_exe
        .parent()
        .ok_or(DesktopStateError::CoreExecutableNotFound)?;
    let bundle_dir = app_dir.join("native").join("bundled").join("win-x64");
    if !bundle_dir.join("trusttunnel_client.exe").exists() {
        return Err(DesktopStateError::CoreExecutableNotFound);
    }

    Ok(bundle_dir)
}

fn bundled_trusttunnel_manifest() -> InstalledCoreManifest {
    let mut files = vec![InstalledCoreFile {
        path: "trusttunnel_client.exe".to_string(),
        sha256: BUNDLED_TRUSTTUNNEL_SHA256.to_string(),
    }];
    if cfg!(windows) {
        files.push(InstalledCoreFile {
            path: "wintun.dll".to_string(),
            sha256: BUNDLED_WINTUN_SHA256.to_string(),
        });
    }

    InstalledCoreManifest {
        core_id: CoreId::from("trusttunnel"),
        display_name: "TrustTunnel".to_string(),
        version: BUNDLED_TRUSTTUNNEL_VERSION.to_string(),
        source_type: SourceType::ManualBundle,
        owner: None,
        repo: None,
        asset_name: "manual".to_string(),
        executable_path: "trusttunnel_client.exe".to_string(),
        sha256: "manual".to_string(),
        signature_status: SignatureStatus::Unknown,
        installed_at_unix_ms: now_unix_ms(),
        files,
    }
}

fn embedded_trusted_sources() -> Result<Vec<TrustedCoreSource>, DesktopStateError> {
    let json = include_str!("../../../core-registry/trusted-sources.json");
    Ok(TrustedSourcePolicy::default().parse_sources(json)?)
}

fn core_store_root() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("ProxyOpenHub")
        .join("cores")
}

fn verify_trusttunnel_bundle(candidate: PathBuf) -> Result<PathBuf, DesktopStateError> {
    verify_file_hash(&candidate, BUNDLED_TRUSTTUNNEL_SHA256)?;

    if cfg!(windows) {
        let sidecar = candidate
            .parent()
            .ok_or_else(|| DesktopStateError::InvalidCorePath(candidate.clone()))?
            .join("wintun.dll");
        if !sidecar.exists() {
            return Err(DesktopStateError::CoreSidecarMissing(
                sidecar.display().to_string(),
            ));
        }
        verify_file_hash(&sidecar, BUNDLED_WINTUN_SHA256)?;
    }

    Ok(candidate)
}

fn verify_file_hash(path: &Path, expected: &str) -> Result<(), DesktopStateError> {
    let bytes = fs::read(path)?;
    let actual = sha256_hex(&bytes);
    if actual.eq_ignore_ascii_case(expected) {
        return Ok(());
    }

    Err(DesktopStateError::CoreIntegrityMismatch {
        path: path.display().to_string(),
        expected: expected.to_string(),
        actual,
    })
}

fn dev_core_override_enabled() -> bool {
    cfg!(debug_assertions) || env::var("POH_DEV").is_ok_and(|value| value == "1")
}

fn runtime_root() -> Result<PathBuf, DesktopStateError> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let root = base.join("ProxyOpenHub").join("runtime");
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn default_desktop_state_file() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("ProxyOpenHub")
        .join("desktop-state.json")
}

fn legacy_desktop_state_file() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("TrustTunnel")
        .join("desktop-state.json")
}

fn session_file() -> Result<PathBuf, DesktopStateError> {
    Ok(runtime_root()?.join("session.json"))
}

fn session_lock_file() -> Result<PathBuf, DesktopStateError> {
    Ok(runtime_root()?.join("session.lock"))
}

fn load_session() -> Result<Option<PersistedDesktopSession>, DesktopStateError> {
    let path = session_file()?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    Ok(parse_persisted_session(&content))
}

fn parse_persisted_session(content: &str) -> Option<PersistedDesktopSession> {
    match serde_json::from_str(content) {
        Ok(session) => Some(session),
        Err(error) => {
            // A corrupt session record must not wedge connect/disconnect forever.
            // Treat it as "no session"; the next save_session overwrites it.
            eprintln!("Ignoring corrupt session.json: {error}");
            None
        }
    }
}

fn save_session(session: &PersistedDesktopSession) -> Result<(), DesktopStateError> {
    let path = session_file()?;
    let json = serde_json::to_string_pretty(session)?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, json)?;
    restrict_runtime_permissions(&temp_path)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(&temp_path, &path)?;
    restrict_runtime_permissions(&path)?;
    Ok(())
}

fn clear_persisted_session(
    session: Option<PersistedDesktopSession>,
) -> Result<(), DesktopStateError> {
    if let Some(session) = &session {
        restore_network_effects(&session.network_effects)?;
    }

    let session_path = session_file()?;
    if session_path.exists() {
        let _ = fs::remove_file(session_path);
    }

    if let Some(session) = session {
        let runtime_dir = PathBuf::from(&session.runtime_dir);
        if runtime_dir.exists() {
            // Retry: this directory holds the materialized config with the
            // plaintext secret, so removal must not be silently skipped if the
            // just-stopped core briefly still holds a handle.
            remove_dir_all_with_retries(&runtime_dir);
        }
    }

    Ok(())
}

fn startup_probe_for_profile(profile: &DesktopProfile) -> SessionStartupProbe {
    if let Some(probe) = startup_probe_for_generic_profile(profile) {
        return probe;
    }

    if profile.listener.mode == 1 {
        let address = profile.listener.socks.address.trim();
        if !address.is_empty() {
            return SessionStartupProbe::tcp(address);
        }
    }

    SessionStartupProbe::DelayOnly
}

fn startup_probe_for_generic_profile(profile: &DesktopProfile) -> Option<SessionStartupProbe> {
    if profile.core_id != "naiveproxy" {
        return None;
    }

    let core_config = profile.core_config.clone()?;
    let core_profile = serde_json::from_value::<NaiveProxyCoreProfile>(core_config).ok()?;
    let port = core_profile.config.listen.port?;
    let host = core_profile.config.listen.host.trim();
    if host.is_empty() {
        return None;
    }

    Some(SessionStartupProbe::tcp(format_socket_address(host, port)))
}

fn format_socket_address(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn startup_timings_for_probe(probe: &SessionStartupProbe) -> SessionTimings {
    match probe {
        SessionStartupProbe::DelayOnly => SessionTimings {
            startup_grace: Duration::from_millis(800),
            ..SessionTimings::default()
        },
        SessionStartupProbe::TcpSocket { .. } => SessionTimings::default(),
    }
}

const DEFAULT_PROXY_OVERRIDE: &str = "<local>";

fn system_proxy_config_for_profile(
    profile: &DesktopProfile,
) -> Result<Option<SystemProxyConfig>, DesktopStateError> {
    if !profile.listener.system_proxy.enabled {
        return Ok(None);
    }

    let proxy_server = match profile.core_id.as_str() {
        "" | "trusttunnel" => trusttunnel_system_proxy_server(profile)?,
        "naiveproxy" => naiveproxy_system_proxy_server(profile)?,
        core_id => {
            return Err(DesktopStateError::InvalidSystemProxyConfig(format!(
                "core {core_id} does not expose a known local proxy listener"
            )));
        }
    };
    let proxy_override = if profile
        .listener
        .system_proxy
        .proxy_override
        .trim()
        .is_empty()
    {
        DEFAULT_PROXY_OVERRIDE.to_string()
    } else {
        profile
            .listener
            .system_proxy
            .proxy_override
            .trim()
            .to_string()
    };

    Ok(Some(SystemProxyConfig::new(proxy_server, proxy_override)))
}

fn trusttunnel_system_proxy_server(profile: &DesktopProfile) -> Result<String, DesktopStateError> {
    match profile.listener.system_proxy.mode {
        1 => http_proxy_server(&profile.listener.socks.http_proxy_address),
        2 => socks_proxy_server(&profile.listener.socks.address),
        _ if !profile.listener.socks.http_proxy_address.trim().is_empty() => {
            http_proxy_server(&profile.listener.socks.http_proxy_address)
        }
        _ if profile.listener.mode == 1 => socks_proxy_server(&profile.listener.socks.address),
        _ => Err(DesktopStateError::InvalidSystemProxyConfig(
            "TrustTunnel system proxy needs SOCKS mode or an HTTP proxy address".to_string(),
        )),
    }
}

fn naiveproxy_system_proxy_server(profile: &DesktopProfile) -> Result<String, DesktopStateError> {
    let core_config = profile.core_config.clone().ok_or_else(|| {
        DesktopStateError::InvalidSystemProxyConfig(
            "NaiveProxy profile does not contain core config".to_string(),
        )
    })?;
    let core_profile = serde_json::from_value::<NaiveProxyCoreProfile>(core_config)?;
    let listen = core_profile.config.listen;
    let port = listen.port.ok_or_else(|| {
        DesktopStateError::InvalidSystemProxyConfig(
            "NaiveProxy listener must include a port for system proxy".to_string(),
        )
    })?;
    let address = format_socket_address(listen.host.trim(), port);
    match profile.listener.system_proxy.mode {
        1 if listen.scheme.eq_ignore_ascii_case("http") => http_proxy_server(&address),
        2 if listen.scheme.eq_ignore_ascii_case("socks")
            || listen.scheme.eq_ignore_ascii_case("socks5") =>
        {
            socks_proxy_server(&address)
        }
        1 => Err(DesktopStateError::InvalidSystemProxyConfig(
            "NaiveProxy listener is not an HTTP proxy".to_string(),
        )),
        2 => Err(DesktopStateError::InvalidSystemProxyConfig(
            "NaiveProxy listener is not a SOCKS proxy".to_string(),
        )),
        _ if listen.scheme.eq_ignore_ascii_case("http") => http_proxy_server(&address),
        _ if listen.scheme.eq_ignore_ascii_case("socks")
            || listen.scheme.eq_ignore_ascii_case("socks5") =>
        {
            socks_proxy_server(&address)
        }
        _ => Err(DesktopStateError::InvalidSystemProxyConfig(format!(
            "unsupported NaiveProxy listener scheme: {}",
            listen.scheme
        ))),
    }
}

fn http_proxy_server(address: &str) -> Result<String, DesktopStateError> {
    let address = normalize_proxy_address(address)?;
    Ok(format!("http={address};https={address}"))
}

fn socks_proxy_server(address: &str) -> Result<String, DesktopStateError> {
    let address = normalize_proxy_address(address)?;
    Ok(format!("socks={address}"))
}

fn normalize_proxy_address(address: &str) -> Result<String, DesktopStateError> {
    let address = address.trim();
    if address.is_empty()
        || address.contains(';')
        || address.chars().any(char::is_whitespace)
        || !address.contains(':')
    {
        return Err(DesktopStateError::InvalidSystemProxyConfig(format!(
            "invalid local proxy address: {address}"
        )));
    }

    Ok(address.to_string())
}

fn is_session_process_running(
    session: &PersistedDesktopSession,
) -> Result<bool, DesktopStateError> {
    #[cfg(windows)]
    {
        let Some(snapshot) = query_windows_process(session.pid)? else {
            return Ok(false);
        };
        let expected_image = Path::new(&session.executable_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("trusttunnel_client.exe");
        let image_matches =
            snapshot.pid == session.pid && snapshot.image_name.eq_ignore_ascii_case(expected_image);
        if !image_matches {
            return Ok(false);
        }

        // Strong identity: if a creation time was recorded, the live PID must
        // still carry it, otherwise this is a recycled PID belonging to another
        // process and must not be treated as (or killed as) our core.
        if session.creation_time_100ns != 0 {
            return Ok(process_creation_time(session.pid) == Some(session.creation_time_100ns));
        }

        Ok(true)
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-0", &session.pid.to_string()])
            .status()?;
        Ok(status.success())
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsProcessSnapshot {
    image_name: String,
    pid: u32,
}

#[cfg(windows)]
fn query_windows_process(pid: u32) -> Result<Option<WindowsProcessSnapshot>, DesktopStateError> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    if line.contains("No tasks") {
        return Ok(None);
    }

    let fields = parse_csv_row(line);
    let Some(image_name) = fields.first() else {
        return Ok(None);
    };
    let Some(pid_field) = fields.get(1) else {
        return Ok(None);
    };
    let Ok(actual_pid) = pid_field.trim().parse::<u32>() else {
        return Ok(None);
    };

    Ok(Some(WindowsProcessSnapshot {
        image_name: image_name.to_string(),
        pid: actual_pid,
    }))
}

#[cfg(windows)]
fn parse_csv_row(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                let _ = chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    fields.push(current.trim().to_string());
    fields
}

fn stop_process(pid: u32) -> Result<(), DesktopStateError> {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()?;
        Ok(())
    }
}

/// Best-effort graceful stop request before a forced kill. On Windows this is a
/// `CTRL_BREAK` to the core's process group (the core is spawned with
/// `CREATE_NEW_PROCESS_GROUP`); on other platforms it is `SIGTERM`. It is only a
/// hint - the caller always confirms exit and force-kills on timeout, so this
/// failing (e.g. no console attached) is harmless.
fn request_graceful_stop(pid: u32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT};
        unsafe {
            let _ = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const FORCE_STOP_TIMEOUT: Duration = Duration::from_secs(3);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Stop the session's core: ask it to exit gracefully, then force-kill if it
/// does not, always confirming the process is actually gone (by verified
/// identity) before returning success.
fn terminate_session_process(session: &PersistedDesktopSession) -> Result<(), DesktopStateError> {
    request_graceful_stop(session.pid);
    if wait_for_session_exit(session, GRACEFUL_STOP_TIMEOUT)? {
        return Ok(());
    }

    stop_process(session.pid)?;
    let _ = wait_for_session_exit(session, FORCE_STOP_TIMEOUT)?;
    Ok(())
}

fn wait_for_session_exit(
    session: &PersistedDesktopSession,
    timeout: Duration,
) -> Result<bool, DesktopStateError> {
    let started = Instant::now();
    loop {
        if !is_session_process_running(session)? {
            return Ok(true);
        }

        if started.elapsed() >= timeout {
            return Ok(false);
        }

        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

/// OS process creation time as a FILETIME (100ns ticks). Returns `None` if the
/// process is gone or cannot be queried, which the callers treat as "not ours".
#[cfg(windows)]
fn process_creation_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return None;
    }

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }

        Some(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
    }
}

#[cfg(not(windows))]
fn process_creation_time(_pid: u32) -> Option<u64> {
    None
}

/// Spawn the core in its own process group so it can be asked to exit gracefully
/// with `CTRL_BREAK` and so a console signal to our process does not reach it.
#[cfg(windows)]
fn configure_core_command(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(windows))]
fn configure_core_command(_command: &mut Command) {}

fn enforce_transition(
    session: &mut PersistedDesktopSession,
    next: SessionLifecycleState,
) -> Result<(), DesktopStateError> {
    if !session.lifecycle_state.can_transition_to(next) {
        return Err(DesktopStateError::IllegalTransition(
            session.lifecycle_state,
            next,
        ));
    }

    session.lifecycle_state = next;
    session.updated_at_unix_ms = now_unix_ms();
    save_session(session)
}

fn remove_dir_all_with_retries(dir: &Path) {
    for attempt in 0..5 {
        if !dir.exists() {
            return;
        }
        if fs::remove_dir_all(dir).is_ok() {
            return;
        }
        // The core may still hold the runtime config open for a moment after
        // exit; back off briefly and retry so the plaintext config is removed.
        thread::sleep(PROCESS_POLL_INTERVAL * (attempt + 1));
    }
}

fn read_log_tail(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return "no startup log was written".to_string();
    };
    let mut content = String::new();
    let _ = file.read_to_string(&mut content);
    let trimmed = content.trim();
    if trimmed.is_empty() {
        "startup log is empty".to_string()
    } else {
        trimmed
            .chars()
            .rev()
            .take(2000)
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    }
}

fn redact_session_log(content: &str, session: &PersistedDesktopSession) -> String {
    let state_path = if session.state_path.trim().is_empty() {
        default_desktop_state_file()
    } else {
        PathBuf::from(&session.state_path)
    };

    let Ok(state) = load_desktop_state(&state_path) else {
        return Redactor::redact(content);
    };
    let Some(profile) = state
        .profiles
        .iter()
        .find(|profile| profile.id == session.profile_id)
    else {
        return Redactor::redact(content);
    };
    let Ok(secrets) = state
        .resolved_secrets()
        .and_then(|secrets| secret_resolver_values(profile, &secrets))
    else {
        return Redactor::redact(content);
    };

    Redactor::redact_secrets(content, &secrets)
}

fn cleanup_orphaned_runtime_dirs() -> Result<(), DesktopStateError> {
    let root = runtime_root()?;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }

    Ok(())
}

fn protect_secret(value: &str) -> Result<String, DesktopStateError> {
    #[cfg(windows)]
    {
        let encrypted = dpapi_protect(value.as_bytes())?;
        Ok(format!(
            "{DPAPI_SECRET_PREFIX}{}",
            general_purpose::STANDARD.encode(encrypted)
        ))
    }

    #[cfg(not(windows))]
    {
        Ok(format!(
            "plain-dev:v1:{}",
            general_purpose::STANDARD.encode(value.as_bytes())
        ))
    }
}

fn unprotect_secret(value: &str) -> Result<String, DesktopStateError> {
    #[cfg(windows)]
    {
        let Some(encoded) = value.strip_prefix(DPAPI_SECRET_PREFIX) else {
            return Ok(value.to_string());
        };
        let encrypted = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| DesktopStateError::SecretProtection(error.to_string()))?;
        let plain = dpapi_unprotect(&encrypted)?;
        String::from_utf8(plain)
            .map_err(|error| DesktopStateError::SecretProtection(error.to_string()))
    }

    #[cfg(not(windows))]
    {
        let Some(encoded) = value.strip_prefix("plain-dev:v1:") else {
            return Ok(value.to_string());
        };
        let plain = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| DesktopStateError::SecretProtection(error.to_string()))?;
        String::from_utf8(plain)
            .map_err(|error| DesktopStateError::SecretProtection(error.to_string()))
    }
}

#[cfg(windows)]
fn dpapi_protect(bytes: &[u8]) -> Result<Vec<u8>, DesktopStateError> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(DesktopStateError::SecretProtection(
            "CryptProtectData failed".to_string(),
        ));
    }

    let encrypted =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(output.pbData as _);
    }

    Ok(encrypted)
}

#[cfg(windows)]
fn dpapi_unprotect(bytes: &[u8]) -> Result<Vec<u8>, DesktopStateError> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(DesktopStateError::SecretProtection(
            "CryptUnprotectData failed".to_string(),
        ));
    }

    let plain =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(output.pbData as _);
    }

    Ok(plain)
}

fn restrict_runtime_permissions(path: &Path) -> Result<(), DesktopStateError> {
    #[cfg(windows)]
    {
        let account = current_windows_account()?;
        let grant = if path.is_dir() {
            format!("{account}:(OI)(CI)F")
        } else {
            format!("{account}:F")
        };
        let status = Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r", &grant])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() {
            return Err(DesktopStateError::SecretProtection(format!(
                "icacls failed for {}",
                path.display()
            )));
        }
    }

    Ok(())
}

#[cfg(windows)]
fn current_windows_account() -> Result<String, DesktopStateError> {
    let output = Command::new("whoami").output()?;
    if !output.status.success() {
        return Err(DesktopStateError::SecretProtection(
            "whoami failed".to_string(),
        ));
    }

    let account = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if account.is_empty() {
        return Err(DesktopStateError::SecretProtection(
            "whoami returned an empty account".to_string(),
        ));
    }

    Ok(account)
}

struct RuntimeDirGuard {
    path: PathBuf,
    keep: bool,
}

impl RuntimeDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for RuntimeDirGuard {
    fn drop(&mut self) {
        if !self.keep && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn sanitize_path_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "profile".to_string()
    } else {
        sanitized
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopState {
    #[serde(rename = "Profiles", default)]
    profiles: Vec<DesktopProfile>,
    #[serde(
        rename = "Secrets",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    legacy_secrets: BTreeMap<String, String>,
    #[serde(rename = "ProtectedSecrets", default)]
    protected_secrets: BTreeMap<String, String>,
}

impl DesktopState {
    fn resolved_secrets(&self) -> Result<BTreeMap<String, String>, DesktopStateError> {
        let mut resolved = BTreeMap::new();
        for (key, value) in &self.protected_secrets {
            resolved.insert(key.clone(), unprotect_secret(value)?);
        }
        for (key, value) in &self.legacy_secrets {
            resolved.entry(key.clone()).or_insert_with(|| value.clone());
        }

        Ok(resolved)
    }

    fn insert_protected_secrets(
        &mut self,
        secrets: BTreeMap<String, String>,
    ) -> Result<(), DesktopStateError> {
        self.migrate_plaintext_secrets()?;
        for (key, value) in secrets {
            self.protected_secrets.insert(key, protect_secret(&value)?);
        }

        Ok(())
    }

    fn migrate_plaintext_secrets(&mut self) -> Result<bool, DesktopStateError> {
        if self.legacy_secrets.is_empty() {
            return Ok(false);
        }

        let legacy = std::mem::take(&mut self.legacy_secrets);
        for (key, value) in legacy {
            self.protected_secrets
                .entry(key)
                .or_insert(protect_secret(&value)?);
        }

        Ok(true)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopProfile {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "DisplayName", default)]
    display_name: String,
    #[serde(rename = "CoreId", default, skip_serializing_if = "String::is_empty")]
    core_id: String,
    #[serde(
        rename = "CoreConfig",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    core_config: Option<serde_json::Value>,
    #[serde(
        rename = "SecretRefs",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    secret_refs: BTreeMap<String, String>,
    #[serde(rename = "Endpoint", default)]
    endpoint: DesktopEndpoint,
    #[serde(rename = "Routing", default)]
    routing: DesktopRouting,
    #[serde(rename = "Listener", default)]
    listener: DesktopListener,
}

impl DesktopProfile {
    fn into_profile(self) -> Result<Profile, DesktopStateError> {
        if let Some(core_config) = self.core_config {
            let core_id = if self.core_id.trim().is_empty() {
                return Err(DesktopStateError::CoreAdapterNotFound(
                    "missing profile CoreId".to_string(),
                ));
            } else {
                CoreId::from(self.core_id.as_str())
            };
            let name = if self.display_name.trim().is_empty() {
                core_id.to_string()
            } else {
                self.display_name
            };

            return Ok(Profile::new(
                ProfileId::new(self.id),
                name,
                core_id,
                core_config,
            ));
        }

        let name = if self.display_name.trim().is_empty() {
            self.endpoint.hostname.clone()
        } else {
            self.display_name.clone()
        };
        let trusttunnel_profile = TrustTunnelCoreProfile {
            source_format: "desktop_state".to_string(),
            config: TrustTunnelConfig {
                log_level: LogLevel::Info,
                routing: self.routing.into_routing_profile(),
                endpoint: self.endpoint.into_endpoint_config(),
                listener: self.listener.into_listener_config(),
            },
        };
        let core_config = serde_json::to_value(trusttunnel_profile)?;

        Ok(Profile::new(
            ProfileId::new(self.id),
            name,
            CoreId::from("trusttunnel"),
            core_config,
        ))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopEndpoint {
    #[serde(rename = "Hostname", default)]
    hostname: String,
    #[serde(rename = "CustomSni", default)]
    custom_sni: String,
    #[serde(rename = "Addresses", default)]
    addresses: Vec<String>,
    #[serde(rename = "HasIpv6", default)]
    has_ipv6: bool,
    #[serde(rename = "Username", default)]
    username: String,
    #[serde(rename = "PasswordSecretRef", default)]
    password_secret_ref: String,
    #[serde(rename = "ClientRandomSecretRef", default)]
    client_random_secret_ref: String,
    #[serde(rename = "SkipVerification", default)]
    skip_verification: bool,
    #[serde(rename = "CertificatePem", default)]
    certificate_pem: String,
    #[serde(rename = "UpstreamProtocol", default)]
    upstream_protocol: i32,
    #[serde(rename = "FallbackProtocol", default)]
    fallback_protocol: Option<i32>,
    #[serde(rename = "AntiDpi", default)]
    anti_dpi: bool,
    #[serde(rename = "PostQuantumGroupEnabled", default)]
    post_quantum_group_enabled: bool,
    #[serde(rename = "DnsUpstreams", default)]
    dns_upstreams: Vec<String>,
}

impl DesktopEndpoint {
    fn into_endpoint_config(self) -> EndpointConfig {
        EndpointConfig {
            hostname: self.hostname,
            custom_sni: self.custom_sni,
            addresses: self.addresses,
            has_ipv6: self.has_ipv6,
            username: self.username,
            password_secret_ref: self.password_secret_ref,
            client_random_secret_ref: self.client_random_secret_ref,
            skip_verification: self.skip_verification,
            certificate_pem: self.certificate_pem,
            upstream_protocol: upstream_protocol_from_desktop(self.upstream_protocol),
            fallback_protocol: self.fallback_protocol.map(upstream_protocol_from_desktop),
            anti_dpi: self.anti_dpi,
            post_quantum_group_enabled: self.post_quantum_group_enabled,
            dns_upstreams: self.dns_upstreams,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopRouting {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Mode", default)]
    mode: i32,
    #[serde(rename = "Exclusions", default)]
    exclusions: Vec<String>,
    #[serde(rename = "KillSwitchEnabled", default)]
    kill_switch_enabled: bool,
    #[serde(rename = "KillSwitchAllowPorts", default)]
    kill_switch_allow_ports: Vec<i32>,
    #[serde(rename = "Description", default)]
    description: String,
}

impl DesktopRouting {
    fn into_routing_profile(self) -> RoutingProfile {
        RoutingProfile {
            id: if self.id.is_empty() {
                "default".to_string()
            } else {
                self.id
            },
            name: if self.name.is_empty() {
                "Default".to_string()
            } else {
                self.name
            },
            mode: match self.mode {
                1 => RoutingMode::Selective,
                _ => RoutingMode::General,
            },
            exclusions: self.exclusions,
            kill_switch_enabled: self.kill_switch_enabled,
            kill_switch_allow_ports: self.kill_switch_allow_ports,
            description: self.description,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopListener {
    #[serde(rename = "Mode", default)]
    mode: i32,
    #[serde(rename = "Tun", default)]
    tun: DesktopTun,
    #[serde(rename = "Socks", default)]
    socks: DesktopSocks,
    #[serde(
        rename = "SystemProxy",
        default,
        skip_serializing_if = "DesktopSystemProxy::is_default"
    )]
    system_proxy: DesktopSystemProxy,
}

impl DesktopListener {
    fn into_listener_config(self) -> ListenerConfig {
        ListenerConfig {
            mode: match self.mode {
                1 => ListenerMode::Socks,
                _ => ListenerMode::Tun,
            },
            tun: self.tun.into_tun_config(),
            socks: self.socks.into_socks_config(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct DesktopSystemProxy {
    #[serde(rename = "Enabled", default)]
    enabled: bool,
    /// 0 = auto, 1 = HTTP, 2 = SOCKS. Stored as an integer for compatibility
    /// with the current desktop-state shape used by the Flutter UI.
    #[serde(rename = "Mode", default)]
    mode: i32,
    #[serde(rename = "ProxyOverride", default)]
    proxy_override: String,
}

impl DesktopSystemProxy {
    fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopTun {
    #[serde(rename = "BoundIf", default)]
    bound_if: String,
    #[serde(rename = "IncludedRoutes", default)]
    included_routes: Vec<String>,
    #[serde(rename = "ExcludedRoutes", default)]
    excluded_routes: Vec<String>,
    #[serde(rename = "MtuSize", default)]
    mtu_size: i32,
    #[serde(rename = "TcpRecvBufSize", default)]
    tcp_recv_buf_size: i32,
    #[serde(rename = "TcpSendBufSize", default)]
    tcp_send_buf_size: i32,
    #[serde(rename = "ChangeSystemDns", default)]
    change_system_dns: bool,
    #[serde(rename = "DeviceName", default)]
    device_name: String,
    #[serde(rename = "UseExisting", default)]
    use_existing: bool,
}

impl DesktopTun {
    fn into_tun_config(self) -> TunConfig {
        TunConfig {
            bound_if: self.bound_if,
            included_routes: self.included_routes,
            excluded_routes: self.excluded_routes,
            mtu_size: self.mtu_size,
            tcp_recv_buf_size: self.tcp_recv_buf_size,
            tcp_send_buf_size: self.tcp_send_buf_size,
            change_system_dns: self.change_system_dns,
            device_name: self.device_name,
            use_existing: self.use_existing,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopSocks {
    #[serde(rename = "Address", default)]
    address: String,
    #[serde(rename = "Username", default)]
    username: String,
    #[serde(rename = "PasswordSecretRef", default)]
    password_secret_ref: String,
    #[serde(rename = "AllowLanAccess", default)]
    allow_lan_access: bool,
    #[serde(rename = "HttpProxyAddress", default)]
    http_proxy_address: String,
    #[serde(rename = "HttpProxyAllowLanAccess", default)]
    http_proxy_allow_lan_access: bool,
}

impl DesktopSocks {
    fn into_socks_config(self) -> SocksConfig {
        SocksConfig {
            address: self.address,
            username: self.username,
            password_secret_ref: self.password_secret_ref,
            allow_lan_access: self.allow_lan_access,
            http_proxy_address: self.http_proxy_address,
            http_proxy_allow_lan_access: self.http_proxy_allow_lan_access,
        }
    }
}

fn upstream_protocol_from_desktop(value: i32) -> UpstreamProtocol {
    match value {
        1 => UpstreamProtocol::Http3,
        _ => UpstreamProtocol::Http2,
    }
}

fn upstream_protocol_to_desktop(value: UpstreamProtocol) -> i32 {
    match value {
        UpstreamProtocol::Http2 => 0,
        UpstreamProtocol::Http3 => 1,
    }
}

fn routing_mode_to_desktop(value: RoutingMode) -> i32 {
    match value {
        RoutingMode::General => 0,
        RoutingMode::Selective => 1,
    }
}

fn listener_mode_to_desktop(value: ListenerMode) -> i32 {
    match value {
        ListenerMode::Tun => 0,
        ListenerMode::Socks => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use poh_core::CoreAdapter;

    fn sample_session(pid: u32, creation_time_100ns: u64) -> PersistedDesktopSession {
        PersistedDesktopSession {
            profile_id: "profile-1".to_string(),
            profile_name: "Example".to_string(),
            core_id: "trusttunnel".to_string(),
            pid,
            executable_path: "trusttunnel_client.exe".to_string(),
            config_path: "config.toml".to_string(),
            log_path: "trusttunnel.log".to_string(),
            runtime_dir: "runtime".to_string(),
            state_path: "state.json".to_string(),
            started_at_unix_ms: 1,
            lifecycle_state: SessionLifecycleState::Running,
            last_error: None,
            updated_at_unix_ms: 1,
            creation_time_100ns,
            network_effects: NetworkEffectsState::default(),
        }
    }

    #[test]
    fn corrupt_session_record_is_treated_as_no_session() {
        assert!(parse_persisted_session("this is not json").is_none());
        assert!(parse_persisted_session("{ \"pid\":").is_none());
    }

    #[test]
    fn legacy_session_without_creation_time_defaults_to_zero() {
        let json = r#"{
            "profile_id": "p",
            "profile_name": "n",
            "core_id": "trusttunnel",
            "pid": 1234,
            "executable_path": "trusttunnel_client.exe",
            "config_path": "config.toml",
            "log_path": "trusttunnel.log",
            "runtime_dir": "runtime",
            "started_at_unix_ms": 1
        }"#;

        let session = parse_persisted_session(json).expect("legacy session should parse");
        assert_eq!(session.creation_time_100ns, 0);
        assert!(session.network_effects.is_empty());
        assert_eq!(session.pid, 1234);
    }

    #[test]
    fn session_record_round_trips_creation_time() {
        let session = sample_session(4321, 0x0123_4567_89ab_cdef);
        let encoded = serde_json::to_string(&session).unwrap();
        let decoded = parse_persisted_session(&encoded).unwrap();
        assert_eq!(decoded.creation_time_100ns, 0x0123_4567_89ab_cdef);
        assert_eq!(decoded, session);
    }

    #[cfg(windows)]
    #[test]
    fn process_creation_time_is_stable_for_live_pid_and_absent_for_dead_pid() {
        let pid = std::process::id();
        let first = process_creation_time(pid).expect("our own process has a creation time");
        let second = process_creation_time(pid).expect("creation time is stable");
        assert_eq!(first, second);
        assert!(first != 0);

        // PID 0 is the System Idle pseudo-process; we reject it explicitly.
        assert_eq!(process_creation_time(0), None);
    }

    #[cfg(windows)]
    #[test]
    fn is_session_process_running_rejects_creation_time_mismatch() {
        // Our own PID is alive, but the recorded creation time is wrong, so the
        // session must be considered not-ours (the PID-reuse guard).
        let mut session = sample_session(std::process::id(), 1);
        session.executable_path = std::env::current_exe().unwrap().display().to_string();
        assert!(!is_session_process_running(&session).unwrap());
    }

    #[test]
    fn builds_session_plan_from_desktop_state_profile() {
        let temp_path =
            std::env::temp_dir().join(format!("poh-desktop-state-{}.json", std::process::id()));
        fs::write(
            &temp_path,
            r#"
            {
              "Profiles": [
                {
                  "Id": "profile-1",
                  "DisplayName": "tt.example.test",
                  "Endpoint": {
                    "Hostname": "tt.example.test",
                    "Addresses": ["tt.example.test:443"],
                    "Username": "ttuser",
                    "PasswordSecretRef": "secret://password",
                    "UpstreamProtocol": 0,
                    "AntiDpi": true,
                    "PostQuantumGroupEnabled": true
                  },
                  "Routing": {
                    "Id": "default",
                    "Name": "Default",
                    "Mode": 0,
                    "KillSwitchEnabled": true
                  },
                  "Listener": {
                    "Mode": 1,
                    "Socks": {
                      "Address": "127.0.0.1:1080",
                      "Username": "ttuser",
                      "PasswordSecretRef": "secret://socks"
                    }
                  }
                }
              ],
              "Secrets": {
                "secret://password": "real-password",
                "secret://socks": "real-socks-password"
              }
            }
            "#,
        )
        .unwrap();

        let plan = build_desktop_session_plan(&temp_path, "profile-1").unwrap();

        assert_eq!(plan.profile_name, "tt.example.test");
        assert_eq!(plan.core_id, "trusttunnel");
        assert_eq!(plan.command_args, ["--config", "config.toml"]);
        assert!(plan.redacted_preview.contains("config.toml"));
        assert!(!plan.redacted_preview.contains("real-password"));
        assert!(!plan.redacted_preview.contains("real-socks-password"));
        let migrated = fs::read_to_string(&temp_path).unwrap();
        assert!(migrated.contains("ProtectedSecrets"));
        assert!(!migrated.contains("real-password"));
        assert!(!migrated.contains("real-socks-password"));

        fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn session_plan_requires_profile_password_secret() {
        let temp_path = std::env::temp_dir().join(format!(
            "poh-desktop-state-missing-secret-{}.json",
            std::process::id()
        ));
        fs::write(
            &temp_path,
            r#"
            {
              "Profiles": [
                {
                  "Id": "profile-1",
                  "DisplayName": "tt.example.test",
                  "Endpoint": {
                    "Hostname": "tt.example.test",
                    "Addresses": ["tt.example.test:443"],
                    "Username": "ttuser",
                    "PasswordSecretRef": "secret://missing",
                    "UpstreamProtocol": 0
                  }
                }
              ],
              "Secrets": {}
            }
            "#,
        )
        .unwrap();

        let error = build_desktop_session_plan(&temp_path, "profile-1").unwrap_err();

        assert!(matches!(error, DesktopStateError::SecretNotFound(_)));
        fs::remove_file(temp_path).unwrap();
    }

    #[test]
    fn imported_profile_keeps_secrets_in_state_map() {
        let mut imported_secrets = BTreeMap::new();
        imported_secrets.insert("endpoint.password".to_string(), "secret-pass".to_string());
        imported_secrets.insert(
            "endpoint.client_random".to_string(),
            "random-token".to_string(),
        );

        let core_profile = TrustTunnelCoreProfile {
            source_format: "toml".to_string(),
            config: TrustTunnelConfig {
                endpoint: EndpointConfig {
                    hostname: "tt.example.test".to_string(),
                    addresses: vec!["tt.example.test:443".to_string()],
                    username: "ttuser".to_string(),
                    anti_dpi: true,
                    ..EndpointConfig::default()
                },
                listener: ListenerConfig {
                    mode: ListenerMode::Socks,
                    ..ListenerConfig::default()
                },
                ..TrustTunnelConfig::default()
            },
        };

        let mut parsed = poh_core::ParsedProfile::new(Profile::new(
            ProfileId::new("trusttunnel:tt-example-test"),
            "tt.example.test",
            CoreId::from("trusttunnel"),
            serde_json::to_value(core_profile).unwrap(),
        ));
        parsed.secrets = imported_secrets;

        let (profile, secrets) =
            desktop_profile_from_import("profile-1".to_string(), parsed).unwrap();

        assert_eq!(profile.id, "profile-1");
        assert_eq!(profile.listener.mode, 1);
        assert_eq!(
            profile.endpoint.password_secret_ref,
            "secret://profile-1/endpoint.password"
        );
        assert_eq!(
            profile.endpoint.client_random_secret_ref,
            "secret://profile-1/endpoint.client_random"
        );
        assert_eq!(
            secrets.get("secret://profile-1/endpoint.password"),
            Some(&"secret-pass".to_string())
        );
        assert_eq!(
            secrets.get("secret://profile-1/endpoint.client_random"),
            Some(&"random-token".to_string())
        );
    }

    #[test]
    fn naiveproxy_import_uses_generic_profile_and_secret_refs() {
        let adapter = NaiveProxyAdapter::new();
        let parsed = adapter
            .parse_profile(&ImportInput::text(
                "https://naive%20user:p%40ss%3Aword@example.com:8443",
            ))
            .unwrap();
        let (profile, secrets) =
            desktop_profile_from_import("naive-1".to_string(), parsed).unwrap();

        assert_eq!(profile.id, "naive-1");
        assert_eq!(profile.core_id, "naiveproxy");
        assert!(profile.core_config.is_some());
        assert_eq!(
            profile.secret_refs.get("proxy.password"),
            Some(&"secret://naive-1/proxy.password".to_string())
        );
        assert_eq!(
            secrets.get("secret://naive-1/proxy.password"),
            Some(&"p%40ss%3Aword".to_string())
        );

        let (runtime_profile, materialized, redacted_preview) =
            build_materialized_session(profile.clone(), &secrets).unwrap();

        assert_eq!(runtime_profile.core_id, CoreId::from("naiveproxy"));
        assert_eq!(materialized.command_args, ["config.json"]);
        assert!(materialized.files[0]
            .content
            .contains("https://naive%20user:p%40ss%3Aword@example.com:8443"));
        assert!(!redacted_preview.contains("p%40ss%3Aword"));
        assert_eq!(
            startup_probe_for_profile(&profile),
            SessionStartupProbe::tcp("127.0.0.1:1080")
        );
    }

    #[test]
    fn trusttunnel_system_proxy_prefers_http_proxy_in_auto_mode() {
        let profile = DesktopProfile {
            id: "p".to_string(),
            listener: DesktopListener {
                mode: 1,
                socks: DesktopSocks {
                    address: "127.0.0.1:1080".to_string(),
                    http_proxy_address: "127.0.0.1:8080".to_string(),
                    ..DesktopSocks::default()
                },
                system_proxy: DesktopSystemProxy {
                    enabled: true,
                    mode: 0,
                    proxy_override: String::new(),
                },
                ..DesktopListener::default()
            },
            ..DesktopProfile::default()
        };

        let config = system_proxy_config_for_profile(&profile)
            .unwrap()
            .expect("system proxy should be enabled");

        assert_eq!(
            config.proxy_server,
            "http=127.0.0.1:8080;https=127.0.0.1:8080"
        );
        assert_eq!(config.proxy_override, "<local>");
    }

    #[test]
    fn naiveproxy_system_proxy_uses_listener_scheme() {
        let adapter = NaiveProxyAdapter::new();
        let parsed = adapter
            .parse_profile(&ImportInput::text("https://user:pass@example.com:8443"))
            .unwrap();
        let (mut profile, _secrets) =
            desktop_profile_from_import("naive-1".to_string(), parsed).unwrap();
        profile.listener.system_proxy = DesktopSystemProxy {
            enabled: true,
            mode: 0,
            proxy_override: "localhost;127.*".to_string(),
        };

        let config = system_proxy_config_for_profile(&profile)
            .unwrap()
            .expect("system proxy should be enabled");

        assert_eq!(config.proxy_server, "socks=127.0.0.1:1080");
        assert_eq!(config.proxy_override, "localhost;127.*");
    }

    #[test]
    fn secret_protection_roundtrips_without_plaintext() {
        let protected = protect_secret("very-secret-value").unwrap();

        assert!(!protected.contains("very-secret-value"));
        assert_eq!(unprotect_secret(&protected).unwrap(), "very-secret-value");
    }
}
