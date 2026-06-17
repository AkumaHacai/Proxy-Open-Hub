use std::fs;
use std::path::{Path, PathBuf};

use poh_core::{
    sha256_hex, CoreId, InstalledCoreManifest, InstalledCorePolicy, SecurityError,
    TrustedCoreSource,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreArtifact {
    pub bytes: Vec<u8>,
    pub executable_relative_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInstallRequest {
    pub manifest: InstalledCoreManifest,
    pub artifact: CoreArtifact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInstallPlan {
    pub core_id: CoreId,
    pub version: String,
    pub install_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub executable_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInstallResult {
    pub manifest: InstalledCoreManifest,
    pub install_dir: PathBuf,
    pub executable_path: PathBuf,
    pub previous_install_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCore {
    pub manifest: InstalledCoreManifest,
    pub install_dir: PathBuf,
    pub executable_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstalledCore {
    pub manifest: InstalledCoreManifest,
    pub install_dir: PathBuf,
    pub executable_path: PathBuf,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
struct ActiveCoreVersion {
    version: String,
}

#[derive(Clone, Debug)]
pub struct CoreStore {
    root_dir: PathBuf,
    policy: InstalledCorePolicy,
}

impl CoreStore {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            policy: InstalledCorePolicy::default(),
        }
    }

    pub fn plan_install(
        &self,
        request: &CoreInstallRequest,
    ) -> Result<CoreInstallPlan, CoreStoreError> {
        validate_store_segment(request.manifest.core_id.as_str())?;
        validate_store_segment(&request.manifest.version)?;
        validate_relative_path(&request.artifact.executable_relative_path)?;
        if request.artifact.executable_relative_path != request.manifest.executable_path {
            return Err(CoreStoreError::ManifestMismatch(
                "artifact executable path differs from manifest".to_string(),
            ));
        }

        let core_dir = self.root_dir.join(request.manifest.core_id.as_str());
        let install_dir = core_dir.join(&request.manifest.version);
        let staging_dir = core_dir.join(format!(".staging-{}", request.manifest.version));
        let manifest_path = install_dir.join("core-manifest.json");
        let executable_path = install_dir.join(&request.manifest.executable_path);

        Ok(CoreInstallPlan {
            core_id: request.manifest.core_id.clone(),
            version: request.manifest.version.clone(),
            install_dir,
            staging_dir,
            manifest_path,
            executable_path,
        })
    }

    pub fn install(
        &self,
        request: &CoreInstallRequest,
        trusted_sources: &[TrustedCoreSource],
    ) -> Result<CoreInstallResult, CoreStoreError> {
        self.policy
            .validate_manifest(&request.manifest, trusted_sources)?;
        self.policy
            .verify_artifact_bytes(&request.manifest, &request.artifact.bytes)?;
        if sha256_hex(&request.artifact.bytes) != request.manifest.sha256 {
            return Err(CoreStoreError::ManifestMismatch(
                "artifact SHA-256 changed during install".to_string(),
            ));
        }

        let plan = self.plan_install(request)?;
        if plan.staging_dir.exists() {
            fs::remove_dir_all(&plan.staging_dir)?;
        }

        fs::create_dir_all(&plan.staging_dir)?;
        let staged_executable = plan
            .staging_dir
            .join(&request.artifact.executable_relative_path);
        let executable_parent = staged_executable.parent().ok_or_else(|| {
            CoreStoreError::UnsafePath(request.artifact.executable_relative_path.clone())
        })?;
        fs::create_dir_all(executable_parent)?;
        fs::write(&staged_executable, &request.artifact.bytes)?;
        fs::write(
            plan.staging_dir.join("core-manifest.json"),
            serde_json::to_vec_pretty(&request.manifest)?,
        )?;

        let previous_install_dir = if plan.install_dir.exists() {
            let backup_dir = plan.install_dir.with_extension("rollback");
            if backup_dir.exists() {
                fs::remove_dir_all(&backup_dir)?;
            }
            fs::rename(&plan.install_dir, &backup_dir)?;
            Some(backup_dir)
        } else {
            None
        };

        if let Err(error) = fs::rename(&plan.staging_dir, &plan.install_dir) {
            if let Some(previous) = &previous_install_dir {
                if previous.exists() && !plan.install_dir.exists() {
                    let _ = fs::rename(previous, &plan.install_dir);
                }
            }
            return Err(CoreStoreError::Io(error));
        }

        self.set_active_version(&request.manifest.core_id, &request.manifest.version)?;

        Ok(CoreInstallResult {
            manifest: request.manifest.clone(),
            install_dir: plan.install_dir,
            executable_path: plan.executable_path,
            previous_install_dir,
        })
    }

    pub fn read_installed_manifest(
        &self,
        core_id: &CoreId,
        version: &str,
    ) -> Result<InstalledCoreManifest, CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        validate_store_segment(version)?;
        let path = self
            .root_dir
            .join(core_id.as_str())
            .join(version)
            .join("core-manifest.json");
        let json = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn active_version(&self, core_id: &CoreId) -> Result<Option<String>, CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        let path = active_version_path(&self.root_dir, core_id);
        if !path.exists() {
            return Ok(None);
        }

        let active = serde_json::from_str::<ActiveCoreVersion>(&fs::read_to_string(path)?)?;
        validate_store_segment(&active.version)?;
        Ok(Some(active.version))
    }

    pub fn set_active_version(
        &self,
        core_id: &CoreId,
        version: &str,
    ) -> Result<(), CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        validate_store_segment(version)?;
        self.read_installed_manifest(core_id, version)?;

        let core_dir = self.root_dir.join(core_id.as_str());
        fs::create_dir_all(&core_dir)?;
        let active_path = active_version_path(&self.root_dir, core_id);
        let temp_path = active_path.with_extension("json.tmp");
        let active = ActiveCoreVersion {
            version: version.to_string(),
        };
        fs::write(&temp_path, serde_json::to_vec_pretty(&active)?)?;
        if active_path.exists() {
            fs::remove_file(&active_path)?;
        }
        fs::rename(temp_path, active_path)?;
        Ok(())
    }

    pub fn list_installed(&self) -> Result<Vec<InstalledCore>, CoreStoreError> {
        if !self.root_dir.exists() {
            return Ok(Vec::new());
        }

        let mut installed = Vec::new();
        for core_entry in fs::read_dir(&self.root_dir)? {
            let core_entry = core_entry?;
            if !core_entry.file_type()?.is_dir() {
                continue;
            }

            let core_id_text = core_entry.file_name().to_string_lossy().to_string();
            if core_id_text.starts_with('.') || validate_store_segment(&core_id_text).is_err() {
                continue;
            }

            let core_id = CoreId::from(core_id_text.as_str());
            let active_version = self.active_version(&core_id)?;
            for version_entry in fs::read_dir(core_entry.path())? {
                let version_entry = version_entry?;
                if !version_entry.file_type()?.is_dir() {
                    continue;
                }

                let version = version_entry.file_name().to_string_lossy().to_string();
                if version.starts_with('.') || validate_store_segment(&version).is_err() {
                    continue;
                }

                let manifest_path = version_entry.path().join("core-manifest.json");
                if !manifest_path.exists() {
                    continue;
                }

                let manifest = self.read_installed_manifest(&core_id, &version)?;
                let install_dir = self.root_dir.join(core_id.as_str()).join(&version);
                let executable_path = install_dir.join(&manifest.executable_path);
                installed.push(InstalledCore {
                    active: active_version.as_deref() == Some(manifest.version.as_str()),
                    manifest,
                    install_dir,
                    executable_path,
                });
            }
        }

        installed.sort_by(|left, right| {
            left.manifest
                .core_id
                .cmp(&right.manifest.core_id)
                .then_with(|| left.manifest.version.cmp(&right.manifest.version))
        });
        Ok(installed)
    }

    pub fn verify_core(
        &self,
        core_id: &CoreId,
        version: &str,
        trusted_sources: &[TrustedCoreSource],
    ) -> Result<VerifiedCore, CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        validate_store_segment(version)?;
        let manifest = self.read_installed_manifest(core_id, version)?;
        self.policy.validate_manifest(&manifest, trusted_sources)?;

        let install_dir = self.root_dir.join(core_id.as_str()).join(version);
        let executable_path = install_dir.join(&manifest.executable_path);
        if !executable_path.exists() {
            return Err(CoreStoreError::CoreMissing(executable_path));
        }

        let canonical_install_dir = install_dir.canonicalize()?;
        let canonical_executable = executable_path.canonicalize()?;
        if !canonical_executable.starts_with(&canonical_install_dir) {
            return Err(CoreStoreError::UnsafePath(manifest.executable_path.clone()));
        }

        if manifest.sha256 != "manual" {
            let executable_bytes = fs::read(&canonical_executable)?;
            self.policy
                .verify_artifact_bytes(&manifest, &executable_bytes)?;
        }

        Ok(VerifiedCore {
            manifest,
            install_dir,
            executable_path: canonical_executable,
        })
    }
}

fn active_version_path(root_dir: &Path, core_id: &CoreId) -> PathBuf {
    root_dir.join(core_id.as_str()).join("active.json")
}

#[derive(Debug, Error)]
pub enum CoreStoreError {
    #[error(transparent)]
    Security(#[from] SecurityError),
    #[error("manifest mismatch: {0}")]
    ManifestMismatch(String),
    #[error("unsafe path: {0}")]
    UnsafePath(String),
    #[error("installed core executable is missing: {0}")]
    CoreMissing(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn validate_relative_path(value: &str) -> Result<(), CoreStoreError> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() || value.contains('\\') {
        return Err(CoreStoreError::UnsafePath(value.to_string()));
    }

    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            _ => return Err(CoreStoreError::UnsafePath(value.to_string())),
        }
    }

    Ok(())
}

fn validate_store_segment(value: &str) -> Result<(), CoreStoreError> {
    if value.trim().is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(CoreStoreError::UnsafePath(value.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use poh_core::{PinnedRelease, SignatureStatus, SourceStatus, SourceType, TrustedCoreSource};

    use super::*;

    #[test]
    fn store_installs_verified_artifact() {
        let root = temp_root();
        let artifact_bytes = b"trusted-core-binary".to_vec();
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(&artifact_bytes),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
        };
        let request = CoreInstallRequest {
            manifest: manifest.clone(),
            artifact: CoreArtifact {
                bytes: artifact_bytes,
                executable_relative_path: "sing-box.exe".to_string(),
            },
        };
        let store = CoreStore::new(&root);

        let result = store.install(&request, &trusted_sources()).unwrap();

        assert!(result.executable_path.exists());
        let installed = store
            .read_installed_manifest(&CoreId::from("sing-box"), "1.0.0")
            .unwrap();
        assert_eq!(installed, manifest);
        let verified = store
            .verify_core(&CoreId::from("sing-box"), "1.0.0", &trusted_sources())
            .unwrap();
        assert_eq!(verified.manifest.core_id, CoreId::from("sing-box"));
        assert_eq!(
            store.active_version(&CoreId::from("sing-box")).unwrap(),
            Some("1.0.0".to_string())
        );
        let installed = store.list_installed().unwrap();
        assert_eq!(installed.len(), 1);
        assert!(installed[0].active);
        assert_eq!(installed[0].manifest.version, "1.0.0");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_rejects_tampered_installed_executable() {
        let root = temp_root();
        let artifact_bytes = b"trusted-core-binary".to_vec();
        let request = CoreInstallRequest {
            manifest: InstalledCoreManifest {
                core_id: CoreId::from("sing-box"),
                display_name: "sing-box".to_string(),
                version: "1.0.0".to_string(),
                source_type: SourceType::GithubRelease,
                owner: Some("SagerNet".to_string()),
                repo: Some("sing-box".to_string()),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                executable_path: "sing-box.exe".to_string(),
                sha256: sha256_hex(&artifact_bytes),
                signature_status: SignatureStatus::Unknown,
                installed_at_unix_ms: 1,
            },
            artifact: CoreArtifact {
                bytes: artifact_bytes,
                executable_relative_path: "sing-box.exe".to_string(),
            },
        };
        let store = CoreStore::new(&root);
        let result = store.install(&request, &trusted_sources()).unwrap();
        fs::write(&result.executable_path, b"tampered").unwrap();

        let error = store
            .verify_core(&CoreId::from("sing-box"), "1.0.0", &trusted_sources())
            .unwrap_err();
        assert!(matches!(error, CoreStoreError::Security(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_rejects_swapped_artifact() {
        let root = temp_root();
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(b"expected"),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
        };
        let request = CoreInstallRequest {
            manifest,
            artifact: CoreArtifact {
                bytes: b"swapped".to_vec(),
                executable_relative_path: "sing-box.exe".to_string(),
            },
        };

        let error = CoreStore::new(&root)
            .install(&request, &trusted_sources())
            .unwrap_err();
        assert!(matches!(error, CoreStoreError::Security(_)));
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn store_rejects_path_traversal() {
        let root = temp_root();
        let request = CoreInstallRequest {
            manifest: InstalledCoreManifest {
                core_id: CoreId::from("sing-box"),
                display_name: "sing-box".to_string(),
                version: "1.0.0".to_string(),
                source_type: SourceType::GithubRelease,
                owner: Some("SagerNet".to_string()),
                repo: Some("sing-box".to_string()),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                executable_path: "../sing-box.exe".to_string(),
                sha256: sha256_hex(b"trusted"),
                signature_status: SignatureStatus::Unknown,
                installed_at_unix_ms: 1,
            },
            artifact: CoreArtifact {
                bytes: b"trusted".to_vec(),
                executable_relative_path: "../sing-box.exe".to_string(),
            },
        };

        let error = CoreStore::new(&root)
            .install(&request, &trusted_sources())
            .unwrap_err();
        assert!(matches!(
            error,
            CoreStoreError::Security(_) | CoreStoreError::UnsafePath(_)
        ));
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn trusted_sources() -> Vec<TrustedCoreSource> {
        vec![TrustedCoreSource {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            source_type: SourceType::GithubRelease,
            status: SourceStatus::Active,
            homepage: Some("https://example.com".to_string()),
            license: Some("GPL-3.0-or-later".to_string()),
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            install_enabled: true,
            checksum_required: true,
            signature_preferred: true,
            allowed_asset_patterns: vec!["sing-box-*-windows-amd64.zip".to_string()],
            pinned_release: Some(PinnedRelease {
                version: "1.0.0".to_string(),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                sha256: sha256_hex(b"trusted-core-binary"),
                min_app_version: None,
            }),
            notes: None,
        }]
    }

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "poh-core-store-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
