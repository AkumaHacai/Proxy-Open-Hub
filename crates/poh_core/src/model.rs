use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CoreId(String);

impl CoreId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CoreId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for CoreId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ProfileId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProtocolColor {
    pub protocol: String,
    pub color: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreCapabilities {
    pub can_import: bool,
    pub can_download: bool,
    pub supports_tun: bool,
    pub supports_socks: bool,
    pub supports_http_proxy: bool,
    pub supports_system_proxy: bool,
    pub supports_config_test: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreManifest {
    pub id: CoreId,
    pub display_name: String,
    pub summary: String,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub protocol_colors: Vec<ProtocolColor>,
    pub capabilities: CoreCapabilities,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub core_id: CoreId,
    pub tags: Vec<String>,
    pub ui_metadata: BTreeMap<String, String>,
    pub core_config: serde_json::Value,
}

impl Profile {
    pub fn new(
        id: impl Into<ProfileId>,
        name: impl Into<String>,
        core_id: impl Into<CoreId>,
        core_config: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            core_id: core_id.into(),
            tags: Vec::new(),
            ui_metadata: BTreeMap::new(),
            core_config,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected { since_unix_ms: u64 },
    Disconnecting,
    Error { message: String },
}
