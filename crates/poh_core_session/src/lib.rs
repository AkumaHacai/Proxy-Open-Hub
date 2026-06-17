use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use poh_core::Redactor;
use poh_core_runner::MaterializedRuntime;
use poh_core_store::VerifiedCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleState {
    #[default]
    Idle,
    Preparing,
    Starting,
    Running,
    Stopping,
    Faulted,
}

impl SessionLifecycleState {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::Preparing)
                | (Self::Idle, Self::Starting)
                | (Self::Preparing, Self::Starting)
                | (Self::Preparing, Self::Faulted)
                | (Self::Starting, Self::Running)
                | (Self::Starting, Self::Faulted)
                | (Self::Running, Self::Stopping)
                | (Self::Running, Self::Faulted)
                | (Self::Stopping, Self::Idle)
                | (Self::Stopping, Self::Faulted)
                | (Self::Faulted, Self::Preparing)
                | (Self::Faulted, Self::Idle)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionTimings {
    pub startup_grace: Duration,
    pub poll_interval: Duration,
    pub tcp_connect_timeout: Duration,
    pub stop_timeout: Duration,
}

impl Default for SessionTimings {
    fn default() -> Self {
        Self {
            startup_grace: Duration::from_secs(3),
            poll_interval: Duration::from_millis(100),
            tcp_connect_timeout: Duration::from_millis(120),
            stop_timeout: Duration::from_secs(3),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionStartupProbe {
    DelayOnly,
    TcpSocket { address: String },
}

impl SessionStartupProbe {
    pub fn tcp(address: impl Into<String>) -> Self {
        Self::TcpSocket {
            address: address.into(),
        }
    }

    fn is_ready(&self, timings: &SessionTimings) -> Result<bool, SessionError> {
        match self {
            Self::DelayOnly => Ok(false),
            Self::TcpSocket { address } => tcp_socket_is_ready(address, timings),
        }
    }
}

#[derive(Debug)]
pub struct SessionFileLock {
    path: PathBuf,
}

impl SessionFileLock {
    pub fn acquire(path: impl Into<PathBuf>) -> Result<Self, SessionError> {
        Self::acquire_with_stale_timeout(path, Duration::from_secs(120))
    }

    pub fn acquire_with_stale_timeout(
        path: impl Into<PathBuf>,
        stale_timeout: Duration,
    ) -> Result<Self, SessionError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        if path.exists() && lock_is_stale(&path, stale_timeout) {
            let _ = fs::remove_file(&path);
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                let _ = writeln!(file, "created_at_unix_ms={}", now_unix_ms());
                Ok(Self { path })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(SessionError::SessionLockBusy(path))
            }
            Err(error) => Err(SessionError::Io(error)),
        }
    }
}

impl Drop for SessionFileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreLaunchSpec {
    pub executable_path: PathBuf,
    pub working_dir: PathBuf,
    pub args: Vec<String>,
    pub environment: BTreeMap<String, String>,
}

impl CoreLaunchSpec {
    pub fn from_verified_core(core: &VerifiedCore, runtime: &MaterializedRuntime) -> Self {
        Self {
            executable_path: core.executable_path.clone(),
            working_dir: core.install_dir.clone(),
            args: runtime.command_args.clone(),
            environment: runtime.environment.clone(),
        }
    }

    pub fn redacted_command_line(&self) -> String {
        let command = std::iter::once(self.executable_path.display().to_string())
            .chain(self.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        Redactor::redact(&command)
    }
}

#[derive(Debug)]
pub struct CoreProcess {
    spec: CoreLaunchSpec,
    child: Child,
}

impl CoreProcess {
    pub fn start(spec: CoreLaunchSpec) -> Result<Self, SessionError> {
        validate_launch_spec(&spec)?;
        let mut command = Command::new(&spec.executable_path);
        command
            .current_dir(&spec.working_dir)
            .args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in &spec.environment {
            command.env(key, value);
        }

        let child = command.spawn()?;
        Ok(Self { spec, child })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn spec(&self) -> &CoreLaunchSpec {
        &self.spec
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, SessionError> {
        Ok(self.child.try_wait()?)
    }

    pub fn stop(&mut self) -> Result<(), SessionError> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
        }

        let _ = self.child.wait();
        Ok(())
    }

    pub fn wait_with_redacted_output(self) -> Result<CoreProcessOutput, SessionError> {
        let output = self.child.wait_with_output()?;
        Ok(CoreProcessOutput {
            status: output.status,
            stdout: Redactor::redact(&String::from_utf8_lossy(&output.stdout)),
            stderr: Redactor::redact(&String::from_utf8_lossy(&output.stderr)),
        })
    }
}

pub fn wait_for_process_startup(
    child: &mut Child,
    probe: &SessionStartupProbe,
    timings: SessionTimings,
) -> Result<(), SessionError> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(SessionError::CoreExitedDuringStartup(status.code()));
        }

        if probe.is_ready(&timings)? {
            return Ok(());
        }

        if started.elapsed() >= timings.startup_grace {
            return match probe {
                SessionStartupProbe::DelayOnly => Ok(()),
                SessionStartupProbe::TcpSocket { address } => {
                    Err(SessionError::ReadinessTimedOut(address.clone()))
                }
            };
        }

        thread::sleep(timings.poll_interval);
    }
}

pub fn wait_for_process_exit(
    child: &mut Child,
    timings: SessionTimings,
) -> Result<bool, SessionError> {
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(true);
        }

        if started.elapsed() >= timings.stop_timeout {
            return Ok(false);
        }

        thread::sleep(timings.poll_interval);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreProcessOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("executable is missing: {0}")]
    MissingExecutable(PathBuf),
    #[error("working directory is missing: {0}")]
    MissingWorkingDirectory(PathBuf),
    #[error("unsafe launch argument: {0}")]
    UnsafeArgument(String),
    #[error("unsafe environment key: {0}")]
    UnsafeEnvironmentKey(String),
    #[error("unsafe environment value for key: {0}")]
    UnsafeEnvironmentValue(String),
    #[error("session lock is already held: {0}")]
    SessionLockBusy(PathBuf),
    #[error("core exited during startup with code {0:?}")]
    CoreExitedDuringStartup(Option<i32>),
    #[error("session readiness probe timed out: {0}")]
    ReadinessTimedOut(String),
    #[error("invalid readiness probe address: {0}")]
    InvalidReadinessAddress(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn tcp_socket_is_ready(address: &str, timings: &SessionTimings) -> Result<bool, SessionError> {
    let addresses = address
        .to_socket_addrs()
        .map_err(|_| SessionError::InvalidReadinessAddress(address.to_string()))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(SessionError::InvalidReadinessAddress(address.to_string()));
    }

    Ok(addresses
        .iter()
        .any(|address| TcpStream::connect_timeout(address, timings.tcp_connect_timeout).is_ok()))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn lock_is_stale(path: &PathBuf, stale_timeout: Duration) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= stale_timeout)
}

fn validate_launch_spec(spec: &CoreLaunchSpec) -> Result<(), SessionError> {
    if !spec.executable_path.exists() {
        return Err(SessionError::MissingExecutable(
            spec.executable_path.clone(),
        ));
    }

    if !spec.working_dir.exists() {
        return Err(SessionError::MissingWorkingDirectory(
            spec.working_dir.clone(),
        ));
    }

    for arg in &spec.args {
        if arg.contains('\0') || arg.contains('\n') || arg.contains('\r') {
            return Err(SessionError::UnsafeArgument(arg.clone()));
        }
    }

    for (key, value) in &spec.environment {
        if !is_safe_env_key(key) {
            return Err(SessionError::UnsafeEnvironmentKey(key.clone()));
        }

        if value.contains('\0') {
            return Err(SessionError::UnsafeEnvironmentValue(key.clone()));
        }
    }

    Ok(())
}

fn is_safe_env_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use poh_core::{CoreId, InstalledCoreManifest, SignatureStatus, SourceType};

    use super::*;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn launch_spec_uses_verified_core_path_and_runtime_args() {
        let runtime = MaterializedRuntime {
            files: Vec::new(),
            command_args: vec!["--help".to_string()],
            environment: BTreeMap::new(),
        };
        let core = verified_current_exe();

        let spec = CoreLaunchSpec::from_verified_core(&core, &runtime);

        assert_eq!(spec.executable_path, core.executable_path);
        assert_eq!(spec.args, vec!["--help"]);
    }

    #[test]
    fn process_start_waits_and_redacts_output() {
        let runtime = MaterializedRuntime {
            files: Vec::new(),
            command_args: vec!["--help".to_string()],
            environment: BTreeMap::new(),
        };
        let core = verified_current_exe();
        let spec = CoreLaunchSpec::from_verified_core(&core, &runtime);

        let process = CoreProcess::start(spec).unwrap();
        let output = process.wait_with_redacted_output().unwrap();

        assert!(output.status.success());
    }

    #[test]
    fn process_rejects_missing_executable() {
        let runtime = MaterializedRuntime {
            files: Vec::new(),
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };
        let mut core = verified_current_exe();
        core.executable_path = core.install_dir.join("missing.exe");
        let spec = CoreLaunchSpec::from_verified_core(&core, &runtime);

        let error = CoreProcess::start(spec).unwrap_err();
        assert!(matches!(error, SessionError::MissingExecutable(_)));
    }

    #[test]
    fn lifecycle_state_rejects_illegal_transitions() {
        assert!(SessionLifecycleState::Idle.can_transition_to(SessionLifecycleState::Preparing));
        assert!(SessionLifecycleState::Starting.can_transition_to(SessionLifecycleState::Running));
        assert!(SessionLifecycleState::Running.can_transition_to(SessionLifecycleState::Stopping));
        assert!(!SessionLifecycleState::Running.can_transition_to(SessionLifecycleState::Starting));
        assert!(!SessionLifecycleState::Idle.can_transition_to(SessionLifecycleState::Running));
    }

    #[test]
    fn session_file_lock_blocks_second_owner() {
        let lock_path = std::env::temp_dir().join(format!(
            "poh-session-lock-test-{}-{}.lock",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let first = SessionFileLock::acquire(&lock_path).unwrap();

        let second = SessionFileLock::acquire(&lock_path).unwrap_err();

        assert!(matches!(second, SessionError::SessionLockBusy(_)));
        drop(first);
        let third = SessionFileLock::acquire(&lock_path).unwrap();
        drop(third);
        assert!(!lock_path.exists());
    }

    fn verified_current_exe() -> VerifiedCore {
        let executable_path = std::env::current_exe().unwrap();
        let test_id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let install_dir = std::env::temp_dir().join(format!(
            "poh-session-test-{}-{}-{}",
            std::process::id(),
            test_id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&install_dir).unwrap();
        let copied_exe = install_dir.join("core.exe");
        fs::copy(&executable_path, &copied_exe).unwrap();

        VerifiedCore {
            manifest: InstalledCoreManifest {
                core_id: CoreId::from("test-core"),
                display_name: "Test Core".to_string(),
                version: "test".to_string(),
                source_type: SourceType::ManualBundle,
                owner: None,
                repo: None,
                asset_name: "manual".to_string(),
                executable_path: "core.exe".to_string(),
                sha256: "manual".to_string(),
                signature_status: SignatureStatus::Unknown,
                installed_at_unix_ms: 0,
                files: Vec::new(),
            },
            install_dir,
            executable_path: copied_exe,
        }
    }
}
