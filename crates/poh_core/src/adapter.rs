use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{CoreManifest, Profile};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportInput {
    pub content: String,
    pub source_hint: Option<String>,
}

impl ImportInput {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            source_hint: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportMatch {
    pub confidence: u8,
    pub reason: String,
}

impl ImportMatch {
    pub fn none(reason: impl Into<String>) -> Self {
        Self {
            confidence: 0,
            reason: reason.into(),
        }
    }

    pub fn new(confidence: u8, reason: impl Into<String>) -> Self {
        Self {
            confidence: confidence.min(100),
            reason: reason.into(),
        }
    }

    pub fn matched(&self) -> bool {
        self.confidence > 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParsedProfile {
    pub profile: Profile,
    pub secrets: BTreeMap<String, String>,
    pub warnings: Vec<ValidationWarning>,
}

impl ParsedProfile {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            secrets: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeFile {
    pub relative_path: String,
    pub content: String,
    pub sensitive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub files: Vec<RuntimeFile>,
    pub command_args: Vec<String>,
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SettingsSchema {
    pub sections: Vec<SettingsSection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SettingsSection {
    pub key: String,
    pub title: String,
    pub fields: Vec<SettingsField>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SettingsField {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub required: bool,
    pub kind: SettingsFieldKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingsFieldKind {
    Boolean,
    Integer { min: Option<i64>, max: Option<i64> },
    Text,
    Secret,
    Multiline,
    Select { options: Vec<String> },
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("input is not supported by this core")]
    UnsupportedInput,
    #[error("import failed: {0}")]
    Import(String),
    #[error("profile is invalid: {0}")]
    InvalidProfile(String),
    #[error("runtime config cannot be built: {0}")]
    RuntimeConfig(String),
    #[error("security policy rejected adapter output: {0}")]
    Security(String),
}

pub trait CoreAdapter: Send + Sync {
    fn manifest(&self) -> &CoreManifest;

    fn detect_import(&self, input: &ImportInput) -> ImportMatch;

    fn parse_profile(&self, input: &ImportInput) -> Result<ParsedProfile, AdapterError>;

    fn settings_schema(&self) -> SettingsSchema;

    fn validate(&self, profile: &Profile) -> Result<Vec<ValidationWarning>, AdapterError>;

    fn build_runtime_config(&self, profile: &Profile) -> Result<RuntimeConfig, AdapterError>;
}
