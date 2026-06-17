use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::adapter::{RuntimeConfig, RuntimeFile};
use crate::model::CoreId;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    ManualBundle,
    GithubRelease,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Active,
    Planned,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrustedCoreSource {
    pub core_id: CoreId,
    pub display_name: String,
    pub source_type: SourceType,
    pub status: SourceStatus,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
    #[serde(default)]
    pub install_enabled: bool,
    #[serde(default)]
    pub checksum_required: bool,
    #[serde(default)]
    pub signature_preferred: bool,
    #[serde(default)]
    pub allowed_asset_patterns: Vec<String>,
    pub notes: Option<String>,
}

impl TrustedCoreSource {
    pub fn is_installable(&self) -> bool {
        self.status == SourceStatus::Active && self.install_enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureStatus {
    Unknown,
    Unsigned,
    Verified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstalledCoreManifest {
    pub core_id: CoreId,
    pub display_name: String,
    pub version: String,
    pub source_type: SourceType,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub asset_name: String,
    pub executable_path: String,
    pub sha256: String,
    pub signature_status: SignatureStatus,
    pub installed_at_unix_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub struct InstalledCorePolicy {
    source_policy: TrustedSourcePolicy,
}

impl InstalledCorePolicy {
    pub fn validate_manifest(
        &self,
        manifest: &InstalledCoreManifest,
        sources: &[TrustedCoreSource],
    ) -> Result<(), SecurityError> {
        self.source_policy.validate_sources(sources)?;
        let source = sources
            .iter()
            .find(|source| source.core_id == manifest.core_id)
            .ok_or_else(|| SecurityError::UntrustedInstalledCore(manifest.core_id.to_string()))?;

        if source.status != SourceStatus::Active {
            return Err(SecurityError::UntrustedInstalledCore(format!(
                "{} source is not active",
                manifest.core_id
            )));
        }

        if source.source_type != manifest.source_type {
            return Err(SecurityError::UntrustedInstalledCore(format!(
                "{} source type mismatch",
                manifest.core_id
            )));
        }

        match source.source_type {
            SourceType::ManualBundle => {}
            SourceType::GithubRelease => {
                if source.owner != manifest.owner || source.repo != manifest.repo {
                    return Err(SecurityError::UntrustedInstalledCore(format!(
                        "{} GitHub source mismatch",
                        manifest.core_id
                    )));
                }

                if !source
                    .allowed_asset_patterns
                    .iter()
                    .any(|pattern| wildcard_match(pattern, &manifest.asset_name))
                {
                    return Err(SecurityError::UntrustedInstalledCore(format!(
                        "{} asset is not allowed: {}",
                        manifest.core_id, manifest.asset_name
                    )));
                }

                if source.checksum_required {
                    validate_sha256_hex(&manifest.sha256)?;
                }
            }
        }

        validate_relative_path(&manifest.executable_path)?;
        if manifest.version.trim().is_empty() {
            return Err(SecurityError::UntrustedInstalledCore(format!(
                "{} version is empty",
                manifest.core_id
            )));
        }

        if manifest.sha256 != "manual" {
            validate_sha256_hex(&manifest.sha256)?;
        }

        Ok(())
    }

    pub fn verify_artifact_bytes(
        &self,
        manifest: &InstalledCoreManifest,
        artifact_bytes: &[u8],
    ) -> Result<(), SecurityError> {
        if manifest.sha256 == "manual" {
            return Err(SecurityError::ChecksumMismatch(
                "manual manifests cannot verify artifact bytes".to_string(),
            ));
        }

        let actual = sha256_hex(artifact_bytes);
        if !actual.eq_ignore_ascii_case(&manifest.sha256) {
            return Err(SecurityError::ChecksumMismatch(format!(
                "{} expected {}, got {}",
                manifest.asset_name, manifest.sha256, actual
            )));
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct TrustedSourcePolicy {
    require_https: bool,
    require_checksum_for_downloads: bool,
}

impl Default for TrustedSourcePolicy {
    fn default() -> Self {
        Self {
            require_https: true,
            require_checksum_for_downloads: true,
        }
    }
}

impl TrustedSourcePolicy {
    pub fn parse_sources(&self, json: &str) -> Result<Vec<TrustedCoreSource>, SecurityError> {
        let sources = serde_json::from_str::<Vec<TrustedCoreSource>>(json)
            .map_err(|error| SecurityError::InvalidSourceManifest(error.to_string()))?;
        self.validate_sources(&sources)?;
        Ok(sources)
    }

    pub fn validate_sources(&self, sources: &[TrustedCoreSource]) -> Result<(), SecurityError> {
        let mut seen = BTreeMap::<CoreId, ()>::new();
        for source in sources {
            if source.core_id.as_str().trim().is_empty() {
                return Err(SecurityError::InvalidTrustedSource(
                    "core_id must not be empty".to_string(),
                ));
            }

            if seen.insert(source.core_id.clone(), ()).is_some() {
                return Err(SecurityError::InvalidTrustedSource(format!(
                    "duplicate trusted source for {}",
                    source.core_id
                )));
            }

            if self.require_https {
                if let Some(homepage) = &source.homepage {
                    validate_https_url(homepage)?;
                }
            }

            match source.source_type {
                SourceType::ManualBundle => {
                    if source.install_enabled {
                        return Err(SecurityError::InvalidTrustedSource(format!(
                            "{} is a manual bundle and cannot be auto-installed",
                            source.core_id
                        )));
                    }
                }
                SourceType::GithubRelease => {
                    validate_github_name("owner", source.owner.as_deref())?;
                    validate_github_name("repo", source.repo.as_deref())?;

                    if source.is_installable()
                        && self.require_checksum_for_downloads
                        && !source.checksum_required
                    {
                        return Err(SecurityError::InvalidTrustedSource(format!(
                            "{} is installable but does not require checksums",
                            source.core_id
                        )));
                    }
                }
            }

            if source.install_enabled && source.status != SourceStatus::Active {
                return Err(SecurityError::InvalidTrustedSource(format!(
                    "{} is installable but not active",
                    source.core_id
                )));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSecurityPolicy {
    max_runtime_file_bytes: usize,
}

impl Default for RuntimeSecurityPolicy {
    fn default() -> Self {
        Self {
            max_runtime_file_bytes: 2 * 1024 * 1024,
        }
    }
}

impl RuntimeSecurityPolicy {
    pub fn validate_config(&self, config: &RuntimeConfig) -> Result<(), SecurityError> {
        for file in &config.files {
            self.validate_runtime_file(file)?;
        }

        for arg in &config.command_args {
            if arg.contains('\0') || arg.contains('\n') || arg.contains('\r') {
                return Err(SecurityError::UnsafeRuntimeArgument(arg.clone()));
            }
        }

        for (key, value) in &config.environment {
            if !is_safe_env_key(key) {
                return Err(SecurityError::UnsafeEnvironmentKey(key.clone()));
            }

            if value.contains('\0') {
                return Err(SecurityError::UnsafeEnvironmentValue(key.clone()));
            }
        }

        Ok(())
    }

    fn validate_runtime_file(&self, file: &RuntimeFile) -> Result<(), SecurityError> {
        if file.content.len() > self.max_runtime_file_bytes {
            return Err(SecurityError::RuntimeFileTooLarge(
                file.relative_path.clone(),
            ));
        }

        validate_relative_path(&file.relative_path)?;

        Ok(())
    }
}

pub struct Redactor;

impl Redactor {
    pub fn redact(input: &str) -> String {
        let mut output = redact_tt_links(input);
        for key in [
            "password",
            "passwd",
            "client_random",
            "token",
            "secret",
            "certificate",
            "private_key",
        ] {
            output = redact_assignment(&output, key);
        }

        output
    }

    pub fn redact_secrets(input: &str, secrets: &BTreeMap<String, String>) -> String {
        let mut output = Self::redact(input);
        for value in secrets.values() {
            if value.len() >= 4 {
                output = output.replace(value, "<redacted>");
            }
        }

        output
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SecurityError {
    #[error("trusted source manifest is invalid: {0}")]
    InvalidSourceManifest(String),
    #[error("trusted source is invalid: {0}")]
    InvalidTrustedSource(String),
    #[error("URL must use HTTPS: {0}")]
    InsecureUrl(String),
    #[error("unsafe runtime file path: {0}")]
    UnsafeRuntimePath(String),
    #[error("runtime file is too large: {0}")]
    RuntimeFileTooLarge(String),
    #[error("unsafe runtime argument: {0}")]
    UnsafeRuntimeArgument(String),
    #[error("unsafe environment key: {0}")]
    UnsafeEnvironmentKey(String),
    #[error("unsafe environment value for key: {0}")]
    UnsafeEnvironmentValue(String),
    #[error("installed core is not trusted: {0}")]
    UntrustedInstalledCore(String),
    #[error("checksum mismatch: {0}")]
    ChecksumMismatch(String),
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn validate_https_url(value: &str) -> Result<(), SecurityError> {
    if !value.starts_with("https://") {
        return Err(SecurityError::InsecureUrl(value.to_string()));
    }

    Ok(())
}

fn validate_github_name(field: &str, value: Option<&str>) -> Result<(), SecurityError> {
    let Some(value) = value else {
        return Err(SecurityError::InvalidTrustedSource(format!(
            "github source missing {field}"
        )));
    };

    if value.is_empty()
        || value.starts_with('.')
        || value.ends_with('.')
        || value.contains("..")
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(SecurityError::InvalidTrustedSource(format!(
            "github source has invalid {field}: {value}"
        )));
    }

    Ok(())
}

fn is_safe_env_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn validate_relative_path(value: &str) -> Result<(), SecurityError> {
    let path = Path::new(value);
    if path.is_absolute() || value.contains('\\') || value.trim().is_empty() {
        return Err(SecurityError::UnsafeRuntimePath(value.to_string()));
    }

    for component in path.components() {
        match component {
            Component::Normal(name) if is_windows_reserved_device_name(&name.to_string_lossy()) => {
                return Err(SecurityError::UnsafeRuntimePath(value.to_string()));
            }
            Component::Normal(_) => {}
            _ => return Err(SecurityError::UnsafeRuntimePath(value.to_string())),
        }
    }

    Ok(())
}

fn validate_sha256_hex(value: &str) -> Result<(), SecurityError> {
    if value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(SecurityError::UntrustedInstalledCore(
            "invalid SHA-256 digest".to_string(),
        ))
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern.eq_ignore_ascii_case(value);
    }

    let mut remaining = value;
    let mut first = true;
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if first && !pattern.starts_with('*') {
            if !remaining
                .to_ascii_lowercase()
                .starts_with(&part.to_ascii_lowercase())
            {
                return false;
            }
            remaining = &remaining[part.len()..];
            first = false;
            continue;
        }

        let Some(index) = remaining
            .to_ascii_lowercase()
            .find(&part.to_ascii_lowercase())
        else {
            return false;
        };
        remaining = &remaining[index + part.len()..];
        first = false;
    }

    if !pattern.ends_with('*') {
        if let Some(last) = parts.last() {
            return value
                .to_ascii_lowercase()
                .ends_with(&last.to_ascii_lowercase());
        }
    }

    true
}

fn redact_tt_links(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("tt://") {
            output.push_str("tt://<redacted>");
            let end = rest
                .char_indices()
                .find_map(|(offset, ch)| ch.is_whitespace().then_some(offset))
                .unwrap_or(rest.len());
            index += end;
            continue;
        }

        let ch = rest.chars().next().expect("index should point at a char");
        output.push(ch);
        index += ch.len_utf8();
    }

    output
}

fn redact_assignment(input: &str, key: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for line in input.lines() {
        if let Some(redacted) = redact_delimited_value(line, key, '=') {
            output.push_str(&redacted);
        } else if let Some(redacted) = redact_delimited_value(line, key, ':') {
            output.push_str(&redacted);
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }

    if !input.ends_with('\n') {
        output.pop();
    }

    output
}

fn redact_delimited_value(line: &str, key: &str, delimiter: char) -> Option<String> {
    let (left, _) = line.split_once(delimiter)?;
    if !left_side_mentions_key(left, key) {
        return None;
    }

    Some(format!("{left}{delimiter} <redacted>"))
}

fn left_side_mentions_key(left: &str, key: &str) -> bool {
    let normalized = left
        .trim()
        .trim_matches(|ch| matches!(ch, '"' | '\'' | '[' | ']' | '{' | '}'))
        .to_ascii_lowercase();
    let key = key.to_ascii_lowercase();

    normalized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|part| part == key)
}

fn is_windows_reserved_device_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end_matches(|ch| ch == ' ' || ch == '.')
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_source_policy_accepts_registry_manifest() {
        let json = include_str!("../../../core-registry/trusted-sources.json");
        let sources = TrustedSourcePolicy::default().parse_sources(json).unwrap();

        assert!(sources
            .iter()
            .any(|source| source.core_id.as_str() == "trusttunnel"));
        assert!(sources.iter().all(|source| !source.is_installable()));
    }

    #[test]
    fn trusted_source_policy_rejects_installable_without_checksum() {
        let json = r#"
        [
          {
            "core_id": "example",
            "display_name": "Example",
            "source_type": "github_release",
            "status": "active",
            "homepage": "https://example.com",
            "license": "MIT",
            "owner": "owner",
            "repo": "repo",
            "install_enabled": true,
            "checksum_required": false
          }
        ]
        "#;

        let error = TrustedSourcePolicy::default()
            .parse_sources(json)
            .unwrap_err();
        assert!(matches!(error, SecurityError::InvalidTrustedSource(_)));
    }

    #[test]
    fn runtime_policy_blocks_path_traversal() {
        let config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "../escape.toml".to_string(),
                content: "test".to_string(),
                sensitive: false,
            }],
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };

        let error = RuntimeSecurityPolicy::default()
            .validate_config(&config)
            .unwrap_err();
        assert_eq!(
            error,
            SecurityError::UnsafeRuntimePath("../escape.toml".to_string())
        );
    }

    #[test]
    fn redactor_hides_links_assignments_and_known_secret_values() {
        let mut secrets = BTreeMap::new();
        secrets.insert("endpoint.password".to_string(), "super-secret".to_string());
        let input = "tt://?abc endpoint.password = \"super-secret\"\n{\"client_random\":\"abcd\"}";
        let redacted = Redactor::redact_secrets(input, &secrets);

        assert!(!redacted.contains("super-secret"));
        assert!(!redacted.contains("tt://?abc"));
        assert!(redacted.contains("tt://<redacted>"));
        assert!(redacted.contains("\"client_random\": <redacted>"));
    }

    #[test]
    fn runtime_policy_blocks_windows_reserved_device_names() {
        let config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "CON.toml".to_string(),
                content: "test".to_string(),
                sensitive: false,
            }],
            command_args: Vec::new(),
            environment: BTreeMap::new(),
        };

        let error = RuntimeSecurityPolicy::default()
            .validate_config(&config)
            .unwrap_err();
        assert_eq!(
            error,
            SecurityError::UnsafeRuntimePath("CON.toml".to_string())
        );
    }

    #[test]
    fn installed_core_policy_rejects_planned_core_manifest() {
        let json = include_str!("../../../core-registry/trusted-sources.json");
        let sources = TrustedSourcePolicy::default().parse_sources(json).unwrap();
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: "0".repeat(64),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 0,
        };

        let error = InstalledCorePolicy::default()
            .validate_manifest(&manifest, &sources)
            .unwrap_err();
        assert!(matches!(error, SecurityError::UntrustedInstalledCore(_)));
    }

    #[test]
    fn installed_core_policy_accepts_active_github_manifest_with_checksum() {
        let sources = vec![TrustedCoreSource {
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
            notes: None,
        }];
        let artifact = b"example archive bytes";
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(artifact),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 0,
        };

        let policy = InstalledCorePolicy::default();
        policy.validate_manifest(&manifest, &sources).unwrap();
        policy.verify_artifact_bytes(&manifest, artifact).unwrap();
    }

    #[test]
    fn installed_core_policy_rejects_checksum_mismatch() {
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("example"),
            display_name: "Example".to_string(),
            version: "1.0.0".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("owner".to_string()),
            repo: Some("repo".to_string()),
            asset_name: "example.zip".to_string(),
            executable_path: "example.exe".to_string(),
            sha256: "0".repeat(64),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 0,
        };

        let error = InstalledCorePolicy::default()
            .verify_artifact_bytes(&manifest, b"not zero")
            .unwrap_err();
        assert!(matches!(error, SecurityError::ChecksumMismatch(_)));
    }
}
