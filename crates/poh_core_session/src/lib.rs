use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

use poh_core::Redactor;
use poh_core_runner::MaterializedRuntime;
use poh_core_store::VerifiedCore;
use thiserror::Error;

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
    #[error(transparent)]
    Io(#[from] std::io::Error),
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
