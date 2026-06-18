use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use poh_core::{
    sha256_hex, CoreId, InstalledCoreFile, InstalledCoreManifest, InstalledCorePolicy,
    SecurityError, SignatureStatus, SourceType, TrustedCoreSource, TrustedSourcePolicy,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

pub const DEFAULT_MAX_CORE_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_INACTIVE_VERSION_RETENTION: usize = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreArtifact {
    pub bytes: Vec<u8>,
    pub executable_relative_path: String,
    pub format: CoreArtifactFormat,
}

impl CoreArtifact {
    pub fn single_file(bytes: Vec<u8>, executable_relative_path: impl Into<String>) -> Self {
        Self {
            bytes,
            executable_relative_path: executable_relative_path.into(),
            format: CoreArtifactFormat::SingleFile,
        }
    }

    pub fn zip_archive(bytes: Vec<u8>, executable_relative_path: impl Into<String>) -> Self {
        Self {
            bytes,
            executable_relative_path: executable_relative_path.into(),
            format: CoreArtifactFormat::ZipArchive,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreArtifactFormat {
    SingleFile,
    ZipArchive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoreDownloadPlan {
    pub core_id: CoreId,
    pub version: String,
    pub asset_name: String,
    pub sha256: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadedCoreArtifact {
    pub plan: CoreDownloadPlan,
    pub bytes: Vec<u8>,
}

impl DownloadedCoreArtifact {
    pub fn into_install_request(
        self,
        source: &TrustedCoreSource,
        executable_relative_path: impl Into<String>,
        installed_at_unix_ms: u64,
    ) -> Result<CoreInstallRequest, CoreStoreError> {
        if source.core_id != self.plan.core_id {
            return Err(CoreStoreError::ManifestMismatch(format!(
                "{} download does not belong to {}",
                self.plan.core_id, source.core_id
            )));
        }

        let executable_relative_path = executable_relative_path.into();
        validate_relative_path(&executable_relative_path)?;
        let manifest = InstalledCoreManifest {
            core_id: source.core_id.clone(),
            display_name: source.display_name.clone(),
            version: self.plan.version.clone(),
            source_type: source.source_type.clone(),
            owner: source.owner.clone(),
            repo: source.repo.clone(),
            asset_name: self.plan.asset_name.clone(),
            executable_path: executable_relative_path.clone(),
            sha256: self.plan.sha256.clone(),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms,
            files: Vec::new(),
        };
        let artifact = if self.plan.asset_name.to_ascii_lowercase().ends_with(".zip") {
            CoreArtifact::zip_archive(self.bytes, executable_relative_path)
        } else {
            CoreArtifact::single_file(self.bytes, executable_relative_path)
        };

        Ok(CoreInstallRequest { manifest, artifact })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreDownloader {
    max_bytes: u64,
    user_agent: String,
}

impl Default for CoreDownloader {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_CORE_DOWNLOAD_BYTES,
            user_agent: "ProxyOpenHub/0.1 core-downloader".to_string(),
        }
    }
}

impl CoreDownloader {
    pub fn with_limits(max_bytes: u64, user_agent: impl Into<String>) -> Self {
        Self {
            max_bytes,
            user_agent: user_agent.into(),
        }
    }

    pub fn plan(source: &TrustedCoreSource) -> Result<CoreDownloadPlan, CoreStoreError> {
        TrustedSourcePolicy::default().validate_sources(std::slice::from_ref(source))?;
        if source.source_type != SourceType::GithubRelease {
            return Err(CoreStoreError::DownloadUnavailable(format!(
                "{} is not a GitHub release source",
                source.core_id
            )));
        }

        if !source.is_installable() {
            return Err(CoreStoreError::DownloadUnavailable(format!(
                "{} is not installable yet",
                source.core_id
            )));
        }

        let pinned = source.pinned_release.as_ref().ok_or_else(|| {
            CoreStoreError::DownloadUnavailable(format!("{} has no pinned release", source.core_id))
        })?;
        let owner = source.owner.as_deref().ok_or_else(|| {
            CoreStoreError::DownloadUnavailable(format!("{} has no owner", source.core_id))
        })?;
        let repo = source.repo.as_deref().ok_or_else(|| {
            CoreStoreError::DownloadUnavailable(format!("{} has no repo", source.core_id))
        })?;

        Ok(CoreDownloadPlan {
            core_id: source.core_id.clone(),
            version: pinned.version.clone(),
            asset_name: pinned.asset_name.clone(),
            sha256: pinned.sha256.clone(),
            url: github_release_asset_url(owner, repo, &pinned.version, &pinned.asset_name),
        })
    }

    pub fn artifact_from_bytes(
        source: &TrustedCoreSource,
        bytes: Vec<u8>,
    ) -> Result<DownloadedCoreArtifact, CoreStoreError> {
        let plan = Self::plan(source)?;
        let actual = sha256_hex(&bytes);
        if !actual.eq_ignore_ascii_case(&plan.sha256) {
            return Err(CoreStoreError::Security(SecurityError::ChecksumMismatch(
                format!(
                    "{} expected {}, got {}",
                    plan.asset_name, plan.sha256, actual
                ),
            )));
        }

        Ok(DownloadedCoreArtifact { plan, bytes })
    }

    pub fn download(
        &self,
        source: &TrustedCoreSource,
    ) -> Result<DownloadedCoreArtifact, CoreStoreError> {
        let plan = Self::plan(source)?;
        let response = ureq::get(&plan.url)
            .set("User-Agent", &self.user_agent)
            .call()
            .map_err(|error| CoreStoreError::Download(error.to_string()))?;

        if response
            .header("Content-Length")
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|content_length| content_length > self.max_bytes)
        {
            return Err(CoreStoreError::DownloadTooLarge {
                bytes: response
                    .header("Content-Length")
                    .and_then(|value| value.parse::<u64>().ok())
                    .unwrap_or_default(),
                limit: self.max_bytes,
            });
        }

        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(self.max_bytes + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > self.max_bytes {
            return Err(CoreStoreError::DownloadTooLarge {
                bytes: bytes.len() as u64,
                limit: self.max_bytes,
            });
        }

        Self::artifact_from_bytes(source, bytes)
    }
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoreGcResult {
    pub core_id: CoreId,
    pub active_version: Option<String>,
    pub retained_versions: Vec<String>,
    pub removed_versions: Vec<String>,
    pub removed_dirs: Vec<PathBuf>,
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
        validate_relative_path(&request.artifact.executable_relative_path)?;
        if request.artifact.executable_relative_path != request.manifest.executable_path {
            return Err(CoreStoreError::ManifestMismatch(
                "artifact executable path differs from manifest".to_string(),
            ));
        }

        self.plan_paths(
            &request.manifest.core_id,
            &request.manifest.version,
            &request.manifest.executable_path,
        )
    }

    fn plan_paths(
        &self,
        core_id: &CoreId,
        version: &str,
        executable_path: &str,
    ) -> Result<CoreInstallPlan, CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        validate_store_segment(version)?;
        validate_relative_path(executable_path)?;

        let core_dir = self.root_dir.join(core_id.as_str());
        let install_dir = core_dir.join(version);
        let staging_dir = core_dir.join(format!(".staging-{version}"));
        let manifest_path = install_dir.join("core-manifest.json");
        let executable_full = install_dir.join(executable_path);

        Ok(CoreInstallPlan {
            core_id: core_id.clone(),
            version: version.to_string(),
            install_dir,
            staging_dir,
            manifest_path,
            executable_path: executable_full,
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
        self.reset_staging(&plan)?;
        match request.artifact.format {
            CoreArtifactFormat::SingleFile => {
                stage_single_file(&plan.staging_dir, &request.artifact)?
            }
            CoreArtifactFormat::ZipArchive => {
                extract_zip_archive(&plan.staging_dir, &request.artifact)?
            }
        }
        let staged_executable = plan.staging_dir.join(&request.manifest.executable_path);
        if !staged_executable.exists() {
            return Err(CoreStoreError::CoreMissing(staged_executable));
        }

        let mut installed_manifest = request.manifest.clone();
        installed_manifest.files = collect_installed_files(&plan.staging_dir)?;
        self.promote_staged(&plan, installed_manifest)
    }

    /// Install a locally bundled, multi-file core (e.g. the shipped TrustTunnel
    /// runtime) into the managed store layout. The manifest must use the
    /// `"manual"` sha256 sentinel and pin a per-file SHA-256 list; each file is
    /// copied from `source_dir` only after its bytes match the pinned hash, so
    /// a bundled core is held to the same integrity bar as a downloaded one.
    pub fn install_manual_bundle(
        &self,
        manifest: &InstalledCoreManifest,
        source_dir: &Path,
        trusted_sources: &[TrustedCoreSource],
    ) -> Result<CoreInstallResult, CoreStoreError> {
        if manifest.sha256 != "manual" {
            return Err(CoreStoreError::ManifestMismatch(
                "manual bundle manifest must use \"manual\" sha256".to_string(),
            ));
        }
        if manifest.files.is_empty() {
            return Err(CoreStoreError::ManifestMismatch(
                "manual bundle manifest must pin file hashes".to_string(),
            ));
        }
        self.policy.validate_manifest(manifest, trusted_sources)?;

        let plan = self.plan_paths(
            &manifest.core_id,
            &manifest.version,
            &manifest.executable_path,
        )?;
        self.reset_staging(&plan)?;

        for file in &manifest.files {
            validate_relative_path(&file.path)?;
            let source_path = source_dir.join(&file.path);
            if !source_path.exists() {
                return Err(CoreStoreError::CoreMissing(source_path));
            }

            let bytes = fs::read(&source_path)?;
            let actual = sha256_hex(&bytes);
            if !actual.eq_ignore_ascii_case(&file.sha256) {
                return Err(CoreStoreError::Security(SecurityError::ChecksumMismatch(
                    format!("{} expected {}, got {}", file.path, file.sha256, actual),
                )));
            }

            let target = plan.staging_dir.join(&file.path);
            let parent = target
                .parent()
                .ok_or_else(|| CoreStoreError::UnsafePath(file.path.clone()))?;
            fs::create_dir_all(parent)?;
            fs::write(&target, &bytes)?;
            ensure_path_inside(&plan.staging_dir, &target)?;
        }

        let staged_executable = plan.staging_dir.join(&manifest.executable_path);
        if !staged_executable.exists() {
            return Err(CoreStoreError::CoreMissing(staged_executable));
        }

        let mut installed_manifest = manifest.clone();
        installed_manifest.files = collect_installed_files(&plan.staging_dir)?;
        self.promote_staged(&plan, installed_manifest)
    }

    fn reset_staging(&self, plan: &CoreInstallPlan) -> Result<(), CoreStoreError> {
        if plan.staging_dir.exists() {
            fs::remove_dir_all(&plan.staging_dir)?;
        }
        fs::create_dir_all(&plan.staging_dir)?;
        Ok(())
    }

    fn promote_staged(
        &self,
        plan: &CoreInstallPlan,
        installed_manifest: InstalledCoreManifest,
    ) -> Result<CoreInstallResult, CoreStoreError> {
        fs::write(
            plan.staging_dir.join("core-manifest.json"),
            serde_json::to_vec_pretty(&installed_manifest)?,
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

        self.set_active_version(&plan.core_id, &plan.version)?;
        self.garbage_collect_old_versions(&plan.core_id, DEFAULT_INACTIVE_VERSION_RETENTION)?;

        Ok(CoreInstallResult {
            manifest: installed_manifest,
            install_dir: plan.install_dir.clone(),
            executable_path: plan.executable_path.clone(),
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

    /// Remove old installed versions for one core while always keeping the
    /// active version plus the newest `inactive_retention` inactive versions.
    ///
    /// Only directories that look like installed versions (valid segment +
    /// `core-manifest.json`) are considered. Staging directories and unknown
    /// files are ignored so a failed install can be diagnosed instead of being
    /// silently erased by GC.
    pub fn garbage_collect_old_versions(
        &self,
        core_id: &CoreId,
        inactive_retention: usize,
    ) -> Result<CoreGcResult, CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        let active_version = self.active_version(core_id)?;
        let core_dir = self.root_dir.join(core_id.as_str());
        if !core_dir.exists() {
            return Ok(CoreGcResult {
                core_id: core_id.clone(),
                active_version,
                retained_versions: Vec::new(),
                removed_versions: Vec::new(),
                removed_dirs: Vec::new(),
            });
        }

        let mut installed = self.installed_version_dirs(core_id)?;
        installed.sort_by(|left, right| {
            right
                .manifest
                .installed_at_unix_ms
                .cmp(&left.manifest.installed_at_unix_ms)
                .then_with(|| right.directory_name.cmp(&left.directory_name))
        });

        let mut retained = BTreeSet::new();
        if let Some(active) = &active_version {
            retained.insert(active.clone());
        }

        let newest_inactive = installed
            .iter()
            .filter(|version| !retained.contains(&version.directory_name))
            .take(inactive_retention)
            .map(|version| version.directory_name.clone())
            .collect::<Vec<_>>();
        for version in newest_inactive {
            retained.insert(version);
        }

        let mut removed_versions = Vec::new();
        let mut removed_dirs = Vec::new();
        for version in installed {
            if retained.contains(&version.directory_name) {
                continue;
            }

            fs::remove_dir_all(&version.install_dir)?;
            removed_versions.push(version.directory_name);
            removed_dirs.push(version.install_dir);
        }

        Ok(CoreGcResult {
            core_id: core_id.clone(),
            active_version,
            retained_versions: retained.into_iter().collect(),
            removed_versions,
            removed_dirs,
        })
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

        if !manifest.files.is_empty() {
            verify_installed_files(&canonical_install_dir, &manifest.files)?;
        } else if manifest.sha256 != "manual" {
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

    fn installed_version_dirs(
        &self,
        core_id: &CoreId,
    ) -> Result<Vec<InstalledVersionDir>, CoreStoreError> {
        validate_store_segment(core_id.as_str())?;
        let core_dir = self.root_dir.join(core_id.as_str());
        if !core_dir.exists() {
            return Ok(Vec::new());
        }

        let mut installed = Vec::new();
        for entry in fs::read_dir(core_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let directory_name = entry.file_name().to_string_lossy().to_string();
            if directory_name.starts_with('.') || validate_store_segment(&directory_name).is_err() {
                continue;
            }

            let manifest_path = entry.path().join("core-manifest.json");
            if !manifest_path.exists() {
                continue;
            }

            let manifest =
                serde_json::from_str::<InstalledCoreManifest>(&fs::read_to_string(manifest_path)?)?;
            if manifest.core_id != *core_id {
                continue;
            }

            installed.push(InstalledVersionDir {
                directory_name,
                manifest,
                install_dir: entry.path(),
            });
        }

        Ok(installed)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InstalledVersionDir {
    directory_name: String,
    manifest: InstalledCoreManifest,
    install_dir: PathBuf,
}

fn active_version_path(root_dir: &Path, core_id: &CoreId) -> PathBuf {
    root_dir.join(core_id.as_str()).join("active.json")
}

fn stage_single_file(staging_dir: &Path, artifact: &CoreArtifact) -> Result<(), CoreStoreError> {
    let staged_executable = staging_dir.join(&artifact.executable_relative_path);
    let executable_parent = staged_executable
        .parent()
        .ok_or_else(|| CoreStoreError::UnsafePath(artifact.executable_relative_path.clone()))?;
    fs::create_dir_all(executable_parent)?;
    fs::write(&staged_executable, &artifact.bytes)?;
    ensure_path_inside(staging_dir, &staged_executable)?;
    Ok(())
}

fn extract_zip_archive(staging_dir: &Path, artifact: &CoreArtifact) -> Result<(), CoreStoreError> {
    let mut archive = ZipArchive::new(Cursor::new(&artifact.bytes))?;
    let mut names = BTreeSet::new();
    let mut extracted_files = 0usize;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }

        let relative_path = entry.name().to_string();
        validate_relative_path(&relative_path)?;
        if !names.insert(relative_path.clone()) {
            return Err(CoreStoreError::DuplicateArchivePath(relative_path));
        }

        let target = staging_dir.join(&relative_path);
        let parent = target
            .parent()
            .ok_or_else(|| CoreStoreError::UnsafePath(relative_path.clone()))?;
        fs::create_dir_all(parent)?;
        let mut output = File::create(&target)?;
        io::copy(&mut entry, &mut output)?;
        ensure_path_inside(staging_dir, &target)?;
        extracted_files += 1;
    }

    if extracted_files == 0 {
        return Err(CoreStoreError::EmptyArchive);
    }

    Ok(())
}

fn collect_installed_files(staging_dir: &Path) -> Result<Vec<InstalledCoreFile>, CoreStoreError> {
    let mut files = Vec::new();
    collect_installed_files_inner(staging_dir, staging_dir, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_installed_files_inner(
    root: &Path,
    current: &Path,
    files: &mut Vec<InstalledCoreFile>,
) -> Result<(), CoreStoreError> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_installed_files_inner(root, &path, files)?;
            continue;
        }

        let relative = relative_path_string(root, &path)?;
        validate_relative_path(&relative)?;
        files.push(InstalledCoreFile {
            sha256: sha256_hex(&fs::read(&path)?),
            path: relative,
        });
    }

    Ok(())
}

fn verify_installed_files(
    install_dir: &Path,
    files: &[InstalledCoreFile],
) -> Result<(), CoreStoreError> {
    for file in files {
        validate_relative_path(&file.path)?;
        let path = install_dir.join(&file.path);
        if !path.exists() {
            return Err(CoreStoreError::CoreMissing(path));
        }

        ensure_path_inside(install_dir, &path)?;
        let actual = sha256_hex(&fs::read(&path)?);
        if !actual.eq_ignore_ascii_case(&file.sha256) {
            return Err(CoreStoreError::Security(SecurityError::ChecksumMismatch(
                format!("{} expected {}, got {}", file.path, file.sha256, actual),
            )));
        }
    }

    Ok(())
}

fn ensure_path_inside(root: &Path, path: &Path) -> Result<(), CoreStoreError> {
    let canonical_root = root.canonicalize()?;
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(CoreStoreError::UnsafePath(path.display().to_string()));
    }

    Ok(())
}

fn relative_path_string(root: &Path, path: &Path) -> Result<String, CoreStoreError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CoreStoreError::UnsafePath(path.display().to_string()))?;
    let value = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    Ok(value)
}

fn github_release_asset_url(owner: &str, repo: &str, version: &str, asset_name: &str) -> String {
    format!(
        "https://github.com/{owner}/{repo}/releases/download/{}/{}",
        percent_encode_path_segment(version),
        percent_encode_path_segment(asset_name)
    )
}

fn percent_encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

#[derive(Debug, Error)]
pub enum CoreStoreError {
    #[error(transparent)]
    Security(#[from] SecurityError),
    #[error("core source cannot be downloaded: {0}")]
    DownloadUnavailable(String),
    #[error("core download failed: {0}")]
    Download(String),
    #[error("core download is too large: {bytes} bytes (limit: {limit})")]
    DownloadTooLarge { bytes: u64, limit: u64 },
    #[error("manifest mismatch: {0}")]
    ManifestMismatch(String),
    #[error("unsafe path: {0}")]
    UnsafePath(String),
    #[error("archive contains duplicate path: {0}")]
    DuplicateArchivePath(String),
    #[error("archive contains no files")]
    EmptyArchive,
    #[error("installed core executable is missing: {0}")]
    CoreMissing(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

fn validate_relative_path(value: &str) -> Result<(), CoreStoreError> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() || value.contains('\\') || value.contains(':')
    {
        return Err(CoreStoreError::UnsafePath(value.to_string()));
    }

    for component in path.components() {
        match component {
            std::path::Component::Normal(name)
                if is_windows_reserved_device_name(&name.to_string_lossy()) =>
            {
                return Err(CoreStoreError::UnsafePath(value.to_string()));
            }
            std::path::Component::Normal(_) => {}
            _ => return Err(CoreStoreError::UnsafePath(value.to_string())),
        }
    }

    Ok(())
}

fn is_windows_reserved_device_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();

    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=9).contains(&number))
        || stem
            .strip_prefix("LPT")
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=9).contains(&number))
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
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    use poh_core::{PinnedRelease, SignatureStatus, SourceStatus, SourceType, TrustedCoreSource};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

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
            files: Vec::new(),
        };
        let request = CoreInstallRequest {
            manifest: manifest.clone(),
            artifact: CoreArtifact::single_file(artifact_bytes, "sing-box.exe"),
        };
        let store = CoreStore::new(&root);

        let trusted_sources = trusted_sources_for(&request.manifest);
        let result = store.install(&request, &trusted_sources).unwrap();

        assert!(result.executable_path.exists());
        let installed = store
            .read_installed_manifest(&CoreId::from("sing-box"), "1.0.0")
            .unwrap();
        assert_eq!(installed.core_id, manifest.core_id);
        assert_eq!(installed.version, manifest.version);
        assert_eq!(installed.sha256, manifest.sha256);
        assert_eq!(installed.files.len(), 1);
        assert_eq!(installed.files[0].path, "sing-box.exe");
        let verified = store
            .verify_core(&CoreId::from("sing-box"), "1.0.0", &trusted_sources)
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
    fn store_gc_keeps_active_and_newest_inactive_version() {
        let root = temp_root();
        let core_id = CoreId::from("sing-box");
        let store = CoreStore::new(&root);

        install_test_version(&store, "1.0.0", 1);
        install_test_version(&store, "1.1.0", 2);
        install_test_version(&store, "1.2.0", 3);

        let core_dir = root.join("sing-box");
        assert!(!core_dir.join("1.0.0").exists());
        assert!(core_dir.join("1.1.0").exists());
        assert!(core_dir.join("1.2.0").exists());
        assert_eq!(
            store.active_version(&core_id).unwrap(),
            Some("1.2.0".to_string())
        );

        let installed = store.list_installed().unwrap();
        let versions = installed
            .iter()
            .map(|core| (core.manifest.version.as_str(), core.active))
            .collect::<Vec<_>>();
        assert_eq!(versions, vec![("1.1.0", false), ("1.2.0", true)]);

        let gc = store.garbage_collect_old_versions(&core_id, 0).unwrap();
        assert_eq!(gc.removed_versions, vec!["1.1.0".to_string()]);
        assert!(!core_dir.join("1.1.0").exists());
        assert!(core_dir.join("1.2.0").exists());

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
                files: Vec::new(),
            },
            artifact: CoreArtifact::single_file(artifact_bytes, "sing-box.exe"),
        };
        let store = CoreStore::new(&root);
        let trusted_sources = trusted_sources_for(&request.manifest);
        let result = store.install(&request, &trusted_sources).unwrap();
        fs::write(&result.executable_path, b"tampered").unwrap();

        let error = store
            .verify_core(&CoreId::from("sing-box"), "1.0.0", &trusted_sources)
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
            files: Vec::new(),
        };
        let request = CoreInstallRequest {
            manifest,
            artifact: CoreArtifact::single_file(b"swapped".to_vec(), "sing-box.exe"),
        };

        let error = CoreStore::new(&root)
            .install(&request, &trusted_sources_for(&request.manifest))
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
                files: Vec::new(),
            },
            artifact: CoreArtifact::single_file(b"trusted".to_vec(), "../sing-box.exe"),
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

    #[test]
    fn store_installs_zip_artifact_with_file_hashes() {
        let root = temp_root();
        let archive = zip_bytes(&[
            ("bin/sing-box.exe", b"trusted-core-binary".as_slice()),
            ("LICENSE.txt", b"license text".as_slice()),
        ]);
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "bin/sing-box.exe".to_string(),
            sha256: sha256_hex(&archive),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
            files: Vec::new(),
        };
        let request = CoreInstallRequest {
            manifest,
            artifact: CoreArtifact::zip_archive(archive, "bin/sing-box.exe"),
        };
        let store = CoreStore::new(&root);

        let trusted_sources = trusted_sources_for(&request.manifest);
        let result = store.install(&request, &trusted_sources).unwrap();

        assert!(result.executable_path.exists());
        assert_eq!(result.manifest.files.len(), 2);
        assert!(result
            .manifest
            .files
            .iter()
            .any(|file| file.path == "bin/sing-box.exe"));
        store
            .verify_core(&CoreId::from("sing-box"), "1.0.0", &trusted_sources)
            .unwrap();
        fs::write(&result.executable_path, b"tampered").unwrap();
        let error = store
            .verify_core(&CoreId::from("sing-box"), "1.0.0", &trusted_sources)
            .unwrap_err();
        assert!(matches!(error, CoreStoreError::Security(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_rejects_zip_slip_entries() {
        let root = temp_root();
        let archive = zip_bytes(&[("../escape.exe", b"trusted".as_slice())]);
        let request = CoreInstallRequest {
            manifest: InstalledCoreManifest {
                core_id: CoreId::from("sing-box"),
                display_name: "sing-box".to_string(),
                version: "1.0.0".to_string(),
                source_type: SourceType::GithubRelease,
                owner: Some("SagerNet".to_string()),
                repo: Some("sing-box".to_string()),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                executable_path: "escape.exe".to_string(),
                sha256: sha256_hex(&archive),
                signature_status: SignatureStatus::Unknown,
                installed_at_unix_ms: 1,
                files: Vec::new(),
            },
            artifact: CoreArtifact::zip_archive(archive, "escape.exe"),
        };

        let error = CoreStore::new(&root)
            .install(&request, &trusted_sources_for(&request.manifest))
            .unwrap_err();
        assert!(matches!(error, CoreStoreError::UnsafePath(_)));
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn store_rejects_reserved_zip_entries() {
        let root = temp_root();
        let archive = zip_bytes(&[("NUL.exe", b"trusted".as_slice())]);
        let request = CoreInstallRequest {
            manifest: InstalledCoreManifest {
                core_id: CoreId::from("sing-box"),
                display_name: "sing-box".to_string(),
                version: "1.0.0".to_string(),
                source_type: SourceType::GithubRelease,
                owner: Some("SagerNet".to_string()),
                repo: Some("sing-box".to_string()),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                executable_path: "NUL.exe".to_string(),
                sha256: sha256_hex(&archive),
                signature_status: SignatureStatus::Unknown,
                installed_at_unix_ms: 1,
                files: Vec::new(),
            },
            artifact: CoreArtifact::zip_archive(archive, "NUL.exe"),
        };

        let error = CoreStore::new(&root)
            .install(&request, &trusted_sources_for(&request.manifest))
            .unwrap_err();
        assert!(matches!(
            error,
            CoreStoreError::Security(_) | CoreStoreError::UnsafePath(_)
        ));
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn downloader_builds_pinned_github_plan() {
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "v1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box 1.0.0 windows amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(b"archive"),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
            files: Vec::new(),
        };
        let mut source = trusted_sources_for(&manifest).remove(0);
        source.allowed_asset_patterns = vec!["sing-box * windows amd64.zip".to_string()];

        let plan = CoreDownloader::plan(&source).unwrap();

        assert_eq!(plan.core_id, CoreId::from("sing-box"));
        assert_eq!(plan.version, "v1.0.0");
        assert_eq!(
            plan.url,
            "https://github.com/SagerNet/sing-box/releases/download/v1.0.0/sing-box%201.0.0%20windows%20amd64.zip"
        );
    }

    #[test]
    fn downloader_rejects_non_installable_source() {
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(b"archive"),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
            files: Vec::new(),
        };
        let mut source = trusted_sources_for(&manifest).remove(0);
        source.status = SourceStatus::Planned;
        source.install_enabled = false;

        let error = CoreDownloader::plan(&source).unwrap_err();

        assert!(matches!(error, CoreStoreError::DownloadUnavailable(_)));
    }

    #[test]
    fn downloader_verifies_pinned_bytes_before_install_request() {
        let archive = zip_bytes(&[("bin/sing-box.exe", b"trusted-core-binary".as_slice())]);
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "bin/sing-box.exe".to_string(),
            sha256: sha256_hex(&archive),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
            files: Vec::new(),
        };
        let source = trusted_sources_for(&manifest).remove(0);

        let downloaded = CoreDownloader::artifact_from_bytes(&source, archive).unwrap();
        let request = downloaded
            .into_install_request(&source, "bin/sing-box.exe", 10)
            .unwrap();

        assert_eq!(request.manifest.version, "1.0.0");
        assert_eq!(request.manifest.sha256, manifest.sha256);
        assert_eq!(request.artifact.format, CoreArtifactFormat::ZipArchive);
    }

    #[test]
    fn downloader_rejects_checksum_mismatch() {
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
            files: Vec::new(),
        };
        let source = trusted_sources_for(&manifest).remove(0);

        let error = CoreDownloader::artifact_from_bytes(&source, b"actual".to_vec()).unwrap_err();

        assert!(matches!(error, CoreStoreError::Security(_)));
    }

    #[test]
    fn store_installs_manual_bundle_with_pinned_file_hashes() {
        let root = temp_root();
        let source_dir = temp_root();
        fs::create_dir_all(&source_dir).unwrap();
        let exe_bytes = b"trusttunnel-binary".as_slice();
        let dll_bytes = b"wintun-binary".as_slice();
        fs::write(source_dir.join("trusttunnel_client.exe"), exe_bytes).unwrap();
        fs::write(source_dir.join("wintun.dll"), dll_bytes).unwrap();
        let manifest = manual_bundle_manifest(exe_bytes, dll_bytes);
        let sources = manual_bundle_sources();
        let store = CoreStore::new(&root);

        let result = store
            .install_manual_bundle(&manifest, &source_dir, &sources)
            .unwrap();

        assert!(result.executable_path.exists());
        assert_eq!(result.manifest.files.len(), 2);
        assert_eq!(
            store.active_version(&CoreId::from("trusttunnel")).unwrap(),
            Some("1.0.49".to_string())
        );
        store
            .verify_core(&CoreId::from("trusttunnel"), "1.0.49", &sources)
            .unwrap();

        fs::write(&result.executable_path, b"tampered").unwrap();
        let error = store
            .verify_core(&CoreId::from("trusttunnel"), "1.0.49", &sources)
            .unwrap_err();
        assert!(matches!(error, CoreStoreError::Security(_)));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(source_dir).unwrap();
    }

    #[test]
    fn store_rejects_manual_bundle_with_wrong_pinned_hash() {
        let root = temp_root();
        let source_dir = temp_root();
        fs::create_dir_all(&source_dir).unwrap();
        let exe_bytes = b"trusttunnel-binary".as_slice();
        let dll_bytes = b"wintun-binary".as_slice();
        // Disk copy of the executable does not match the pinned hash.
        fs::write(source_dir.join("trusttunnel_client.exe"), b"swapped").unwrap();
        fs::write(source_dir.join("wintun.dll"), dll_bytes).unwrap();
        let manifest = manual_bundle_manifest(exe_bytes, dll_bytes);

        let error = CoreStore::new(&root)
            .install_manual_bundle(&manifest, &source_dir, &manual_bundle_sources())
            .unwrap_err();

        assert!(matches!(error, CoreStoreError::Security(_)));
        if root.exists() {
            fs::remove_dir_all(root).unwrap();
        }
        fs::remove_dir_all(source_dir).unwrap();
    }

    fn manual_bundle_manifest(exe_bytes: &[u8], dll_bytes: &[u8]) -> InstalledCoreManifest {
        InstalledCoreManifest {
            core_id: CoreId::from("trusttunnel"),
            display_name: "TrustTunnel".to_string(),
            version: "1.0.49".to_string(),
            source_type: SourceType::ManualBundle,
            owner: None,
            repo: None,
            asset_name: "manual".to_string(),
            executable_path: "trusttunnel_client.exe".to_string(),
            sha256: "manual".to_string(),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
            files: vec![
                InstalledCoreFile {
                    path: "trusttunnel_client.exe".to_string(),
                    sha256: sha256_hex(exe_bytes),
                },
                InstalledCoreFile {
                    path: "wintun.dll".to_string(),
                    sha256: sha256_hex(dll_bytes),
                },
            ],
        }
    }

    fn manual_bundle_sources() -> Vec<TrustedCoreSource> {
        vec![TrustedCoreSource {
            core_id: CoreId::from("trusttunnel"),
            display_name: "TrustTunnel".to_string(),
            source_type: SourceType::ManualBundle,
            status: SourceStatus::Active,
            homepage: None,
            license: Some("upstream license".to_string()),
            owner: None,
            repo: None,
            install_enabled: false,
            checksum_required: false,
            signature_preferred: false,
            allowed_asset_patterns: Vec::new(),
            pinned_release: None,
            notes: None,
        }]
    }

    fn trusted_sources() -> Vec<TrustedCoreSource> {
        trusted_sources_for(&InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(b"trusted-core-binary"),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 1,
            files: Vec::new(),
        })
    }

    fn trusted_sources_for(manifest: &InstalledCoreManifest) -> Vec<TrustedCoreSource> {
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
                version: manifest.version.clone(),
                asset_name: manifest.asset_name.clone(),
                sha256: manifest.sha256.clone(),
                min_app_version: None,
            }),
            notes: None,
        }]
    }

    fn install_test_version(store: &CoreStore, version: &str, installed_at_unix_ms: u64) {
        let artifact_bytes = format!("trusted-core-binary-{version}").into_bytes();
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: version.to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: format!("sing-box-{version}-windows-amd64.zip"),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(&artifact_bytes),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms,
            files: Vec::new(),
        };
        let request = CoreInstallRequest {
            manifest,
            artifact: CoreArtifact::single_file(artifact_bytes, "sing-box.exe"),
        };
        store
            .install(&request, &trusted_sources_for(&request.manifest))
            .unwrap();
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

    fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut output = std::io::Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut output);
            let options = SimpleFileOptions::default();
            for (path, content) in files {
                zip.start_file(path, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }

        output.into_inner()
    }
}
