use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use poh_core::{
    sha256_hex, CoreAdapter, CoreId, EndpointConfig, ImportInput, ListenerConfig, ListenerMode,
    LogLevel, Profile, ProfileId, Redactor, RoutingMode, RoutingProfile, SocksConfig,
    TrustTunnelAdapter, TrustTunnelConfig, TrustTunnelCoreProfile, TunConfig, UpstreamProtocol,
    ValidationWarning,
};
use poh_core_runner::{MapSecretResolver, MaterializedRuntime, RuntimeMaterializer};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}

const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PROFILE_ID_SUFFIX: usize = 10_000;
const BUNDLED_TRUSTTUNNEL_SHA256: &str =
    "d260f372d5d8f051180e829e47f0b2ae65c1aeda63ab0e56fefa7b68349f2ab0";
const BUNDLED_WINTUN_SHA256: &str =
    "e5da8447dc2c320edc0fc52fa01885c103de8c118481f683643cacc3220dafce";

pub fn build_desktop_session_plan(
    state_path: &Path,
    profile_id: &str,
) -> Result<DesktopSessionPlan, DesktopStateError> {
    let state = load_desktop_state(state_path)?;
    let profile = state
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| DesktopStateError::ProfileNotFound(profile_id.to_string()))?;
    let (profile, materialized, redacted_preview) =
        build_materialized_session(profile, &state.secrets)?;

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
    let status = desktop_session_status()?;
    if status.running {
        if let Some(session) = status.session {
            return Err(DesktopStateError::SessionAlreadyRunning(session.pid));
        }
    }
    cleanup_orphaned_runtime_dirs()?;

    let state = load_desktop_state(state_path)?;
    let desktop_profile = state
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| DesktopStateError::ProfileNotFound(profile_id.to_string()))?;
    let (profile, materialized, redacted_preview) =
        build_materialized_session(desktop_profile, &state.secrets)?;
    let executable_path = find_trusttunnel_client()?;
    let started_at_unix_ms = now_unix_ms();
    let runtime_dir = runtime_root()?.join(format!(
        "{}-{}",
        sanitize_path_segment(profile.id.as_str()),
        started_at_unix_ms
    ));
    let mut runtime_guard = RuntimeDirGuard::new(runtime_dir.clone());
    let written = materialized.write_to(&runtime_dir)?;
    let config_path = written
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("config.toml"))
        })
        .cloned()
        .ok_or_else(|| DesktopStateError::RuntimeConfigMissing("config.toml".to_string()))?;
    let log_path = runtime_dir.join("trusttunnel.log");
    let stdout = File::create(&log_path)?;
    let stderr = stdout.try_clone()?;
    let command_args = vec![
        "--config".to_string(),
        config_path.display().to_string(),
        "--loglevel".to_string(),
        "info".to_string(),
    ];
    let working_dir = executable_path
        .parent()
        .ok_or_else(|| DesktopStateError::InvalidCorePath(executable_path.clone()))?;
    let mut child = Command::new(&executable_path)
        .current_dir(working_dir)
        .args(&command_args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()?;

    thread::sleep(Duration::from_millis(700));
    if let Some(status) = child.try_wait()? {
        let log_tail = read_log_tail(&log_path);
        return Err(DesktopStateError::CoreExitedDuringStartup(
            status.code(),
            log_tail,
        ));
    }

    let pid = child.id();
    let session = PersistedDesktopSession {
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
    };
    if let Err(error) = save_session(&session) {
        let _ = child.kill();
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
    let Some(session) = load_session()? else {
        return Ok(DesktopSessionStatus {
            running: false,
            session: None,
        });
    };

    if is_session_process_running(&session)? {
        stop_process(session.pid)?;
    }

    let _ = fs::remove_file(session_file()?);
    let runtime_dir = PathBuf::from(&session.runtime_dir);
    if runtime_dir.exists() {
        let _ = fs::remove_dir_all(runtime_dir);
    }

    Ok(DesktopSessionStatus {
        running: false,
        session: Some(session),
    })
}

pub fn desktop_session_status() -> Result<DesktopSessionStatus, DesktopStateError> {
    let Some(session) = load_session()? else {
        return Ok(DesktopSessionStatus {
            running: false,
            session: None,
        });
    };
    let running = is_session_process_running(&session)?;
    if !running {
        let _ = fs::remove_file(session_file()?);
    }

    Ok(DesktopSessionStatus {
        running,
        session: Some(session),
    })
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

    let adapter = TrustTunnelAdapter::new();
    let parsed = adapter.parse_profile(&ImportInput::text(input.to_string()))?;
    let warnings = parsed.warnings.clone();
    let core_profile =
        serde_json::from_value::<TrustTunnelCoreProfile>(parsed.profile.core_config)?;
    let state_path = default_desktop_state_file();
    let mut state = load_or_default_desktop_state(&state_path)?;
    let profile_id = unique_profile_id(parsed.profile.id.as_str(), &state);
    let profile_name = parsed.profile.name;
    let secrets_imported = parsed.secrets.len();
    let (desktop_profile, secrets) = desktop_profile_from_import(
        profile_id.clone(),
        profile_name.clone(),
        core_profile,
        parsed.secrets,
    );

    state.profiles.push(desktop_profile);
    state.secrets.extend(secrets);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&state_path, serde_json::to_string_pretty(&state)?)?;

    Ok(DesktopImportResult {
        profile_id,
        profile_name,
        core_id: "trusttunnel".to_string(),
        state_path: state_path.display().to_string(),
        secrets_imported,
        warnings,
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
    Ok(serde_json::from_str(&content)?)
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

fn desktop_profile_from_import(
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
            },
        },
        secrets,
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
    let adapter = TrustTunnelAdapter::new();
    let secret_values = secret_resolver_values(&desktop_profile, secrets)?;
    let profile = desktop_profile.into_profile()?;
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
    #[error("desktop session is already running with pid {0}")]
    SessionAlreadyRunning(u32),
    #[error("TrustTunnel client executable was not found")]
    CoreExecutableNotFound,
    #[error("invalid TrustTunnel client executable path: {0}")]
    InvalidCorePath(PathBuf),
    #[error("runtime config was not materialized: {0}")]
    RuntimeConfigMissing(String),
    #[error("TrustTunnel core exited during startup with code {0:?}: {1}")]
    CoreExitedDuringStartup(Option<i32>, String),
    #[error("import input is too large: {0} bytes")]
    ImportTooLarge(usize),
    #[error("desktop state is too large: {0}")]
    StateTooLarge(String),
    #[error("POH_TRUSTTUNNEL_CORE_PATH is allowed only for dev runs")]
    CoreOverrideDisabled,
    #[error("bundled core integrity mismatch for {path}: expected {expected}, got {actual}")]
    CoreIntegrityMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("bundled core sidecar is missing: {0}")]
    CoreSidecarMissing(String),
    #[error(transparent)]
    Adapter(#[from] poh_core::AdapterError),
    #[error(transparent)]
    Runner(#[from] poh_core_runner::RunnerError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn find_trusttunnel_client() -> Result<PathBuf, DesktopStateError> {
    if let Ok(path) = env::var("POH_TRUSTTUNNEL_CORE_PATH") {
        if !dev_core_override_enabled() {
            return Err(DesktopStateError::CoreOverrideDisabled);
        }

        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return verify_trusttunnel_bundle(candidate);
        }
    }

    let current_exe = env::current_exe()?;
    let Some(app_dir) = current_exe.parent() else {
        return Err(DesktopStateError::CoreExecutableNotFound);
    };
    let candidate = app_dir
        .join("native")
        .join("bundled")
        .join("win-x64")
        .join("trusttunnel_client.exe");
    if candidate.exists() {
        return verify_trusttunnel_bundle(candidate);
    }

    Err(DesktopStateError::CoreExecutableNotFound)
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

fn load_session() -> Result<Option<PersistedDesktopSession>, DesktopStateError> {
    let path = session_file()?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

fn save_session(session: &PersistedDesktopSession) -> Result<(), DesktopStateError> {
    let path = session_file()?;
    let json = serde_json::to_string_pretty(session)?;
    fs::write(path, json)?;
    Ok(())
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

        Ok(snapshot.pid == session.pid && snapshot.image_name.eq_ignore_ascii_case(expected_image))
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
            .args(["-TERM", &pid.to_string()])
            .status()?;
        Ok(())
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
    let Ok(secrets) = secret_resolver_values(profile, &state.secrets) else {
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
    #[serde(rename = "Secrets", default)]
    secrets: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DesktopProfile {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "DisplayName", default)]
    display_name: String,
    #[serde(rename = "Endpoint", default)]
    endpoint: DesktopEndpoint,
    #[serde(rename = "Routing", default)]
    routing: DesktopRouting,
    #[serde(rename = "Listener", default)]
    listener: DesktopListener,
}

impl DesktopProfile {
    fn into_profile(self) -> Result<Profile, DesktopStateError> {
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

        let (profile, secrets) = desktop_profile_from_import(
            "profile-1".to_string(),
            "tt.example.test".to_string(),
            core_profile,
            imported_secrets,
        );

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
}
