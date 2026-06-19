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
pub struct PinnedRelease {
    pub version: String,
    pub asset_name: String,
    pub sha256: String,
    pub min_app_version: Option<String>,
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
    #[serde(default)]
    pub pinned_release: Option<PinnedRelease>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<InstalledCoreFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InstalledCoreFile {
    pub path: String,
    pub sha256: String,
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

                if let Some(pinned) = &source.pinned_release {
                    if manifest.version != pinned.version
                        || manifest.asset_name != pinned.asset_name
                        || !manifest.sha256.eq_ignore_ascii_case(&pinned.sha256)
                    {
                        return Err(SecurityError::UntrustedInstalledCore(format!(
                            "{} does not match pinned release",
                            manifest.core_id
                        )));
                    }
                }
            }
        }

        validate_relative_path(&manifest.executable_path)?;
        for file in &manifest.files {
            validate_relative_path(&file.path)?;
            validate_sha256_hex(&file.sha256)?;
        }

        if !manifest.files.is_empty()
            && !manifest
                .files
                .iter()
                .any(|file| file.path == manifest.executable_path)
        {
            return Err(SecurityError::UntrustedInstalledCore(format!(
                "{} installed file list does not include executable",
                manifest.core_id
            )));
        }

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
                    if source.pinned_release.is_some() {
                        return Err(SecurityError::InvalidTrustedSource(format!(
                            "{} manual bundle cannot use pinned_release",
                            source.core_id
                        )));
                    }

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

                    if let Some(pinned) = &source.pinned_release {
                        validate_pinned_release(source, pinned)?;
                    } else if source.is_installable() {
                        return Err(SecurityError::InvalidTrustedSource(format!(
                            "{} is installable but has no pinned_release",
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

fn validate_pinned_release(
    source: &TrustedCoreSource,
    pinned: &PinnedRelease,
) -> Result<(), SecurityError> {
    if pinned.version.trim().is_empty() {
        return Err(SecurityError::InvalidTrustedSource(format!(
            "{} pinned release version is empty",
            source.core_id
        )));
    }

    if pinned.asset_name.trim().is_empty()
        || pinned.asset_name.contains('/')
        || pinned.asset_name.contains('\\')
    {
        return Err(SecurityError::InvalidTrustedSource(format!(
            "{} pinned release asset name is unsafe",
            source.core_id
        )));
    }

    validate_sha256_hex(&pinned.sha256).map_err(|error| {
        SecurityError::InvalidTrustedSource(format!("{} pinned release {error}", source.core_id))
    })?;

    if !source
        .allowed_asset_patterns
        .iter()
        .any(|pattern| wildcard_match(pattern, &pinned.asset_name))
    {
        return Err(SecurityError::InvalidTrustedSource(format!(
            "{} pinned asset is not allowed: {}",
            source.core_id, pinned.asset_name
        )));
    }

    if pinned
        .min_app_version
        .as_deref()
        .is_some_and(|version| version.trim().is_empty())
    {
        return Err(SecurityError::InvalidTrustedSource(format!(
            "{} pinned release min_app_version is empty",
            source.core_id
        )));
    }

    Ok(())
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
        output = redact_url_userinfo(&output);
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

fn redact_url_userinfo(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for token in split_preserving_whitespace(input) {
        output.push_str(&redact_url_userinfo_token(token));
    }

    output
}

fn split_preserving_whitespace(input: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut in_whitespace = input.chars().next().is_some_and(char::is_whitespace);

    for (index, ch) in input.char_indices() {
        let ch_is_whitespace = ch.is_whitespace();
        if ch_is_whitespace != in_whitespace {
            tokens.push(&input[start..index]);
            start = index;
            in_whitespace = ch_is_whitespace;
        }
    }

    if start < input.len() {
        tokens.push(&input[start..]);
    }

    tokens
}

fn redact_url_userinfo_token(token: &str) -> String {
    let Some(scheme_end) = token.find("://") else {
        return token.to_string();
    };
    let authority_start = scheme_end + 3;
    let Some(at_relative) = token[authority_start..].find('@') else {
        return token.to_string();
    };
    let at_index = authority_start + at_relative;
    let authority_end = token[authority_start..]
        .find(['/', '?', '#'])
        .map(|offset| authority_start + offset)
        .unwrap_or(token.len());
    if at_index > authority_end {
        return token.to_string();
    }

    let mut redacted = String::with_capacity(token.len());
    redacted.push_str(&token[..authority_start]);
    redacted.push_str("<redacted>@");
    redacted.push_str(&token[at_index + 1..]);
    redacted
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

        // trusttunnel is a manual bundle, never installable via download
        let trusttunnel = sources
            .iter()
            .find(|s| s.core_id.as_str() == "trusttunnel")
            .unwrap();
        assert!(!trusttunnel.is_installable());

        // naiveproxy has a pinned release and is the first downloadable core
        let naive = sources
            .iter()
            .find(|s| s.core_id.as_str() == "naiveproxy")
            .unwrap();
        assert!(naive.is_installable());
        assert!(naive.pinned_release.is_some());
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
    fn redactor_hides_url_userinfo() {
        let input = "proxy=https://user:p%40ss%3Aword@example.com:443/path";
        let redacted = Redactor::redact(input);

        assert!(!redacted.contains("user:p%40ss%3Aword"));
        assert_eq!(redacted, "proxy=https://<redacted>@example.com:443/path");
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
            files: Vec::new(),
        };

        let error = InstalledCorePolicy::default()
            .validate_manifest(&manifest, &sources)
            .unwrap_err();
        assert!(matches!(error, SecurityError::UntrustedInstalledCore(_)));
    }

    #[test]
    fn installed_core_policy_accepts_active_github_manifest_with_checksum() {
        let artifact = b"example archive bytes";
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
            pinned_release: Some(PinnedRelease {
                version: "1.0.0".to_string(),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                sha256: sha256_hex(artifact),
                min_app_version: None,
            }),
            notes: None,
        }];
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
            files: Vec::new(),
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
            files: Vec::new(),
        };

        let error = InstalledCorePolicy::default()
            .verify_artifact_bytes(&manifest, b"not zero")
            .unwrap_err();
        assert!(matches!(error, SecurityError::ChecksumMismatch(_)));
    }

    #[test]
    fn trusted_source_policy_rejects_installable_without_pin() {
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
            "checksum_required": true,
            "allowed_asset_patterns": ["example-*.zip"]
          }
        ]
        "#;

        let error = TrustedSourcePolicy::default()
            .parse_sources(json)
            .unwrap_err();
        assert!(matches!(error, SecurityError::InvalidTrustedSource(_)));
    }

    #[test]
    fn installed_core_policy_rejects_manifest_outside_pinned_release() {
        let artifact = b"example archive bytes";
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
            pinned_release: Some(PinnedRelease {
                version: "1.0.0".to_string(),
                asset_name: "sing-box-1.0.0-windows-amd64.zip".to_string(),
                sha256: sha256_hex(artifact),
                min_app_version: None,
            }),
            notes: None,
        }];
        let manifest = InstalledCoreManifest {
            core_id: CoreId::from("sing-box"),
            display_name: "sing-box".to_string(),
            version: "1.0.1".to_string(),
            source_type: SourceType::GithubRelease,
            owner: Some("SagerNet".to_string()),
            repo: Some("sing-box".to_string()),
            asset_name: "sing-box-1.0.1-windows-amd64.zip".to_string(),
            executable_path: "sing-box.exe".to_string(),
            sha256: sha256_hex(artifact),
            signature_status: SignatureStatus::Unknown,
            installed_at_unix_ms: 0,
            files: Vec::new(),
        };

        let error = InstalledCorePolicy::default()
            .validate_manifest(&manifest, &sources)
            .unwrap_err();
        assert!(matches!(error, SecurityError::UntrustedInstalledCore(_)));
    }
}
