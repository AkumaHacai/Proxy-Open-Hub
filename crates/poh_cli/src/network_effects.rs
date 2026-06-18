#[cfg(windows)]
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetworkEffectsState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_proxy: Option<SystemProxyLease>,
}

impl NetworkEffectsState {
    pub fn is_empty(&self) -> bool {
        self.system_proxy.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SystemProxyLease {
    pub desired: SystemProxyConfig,
    pub previous: SystemProxySnapshot,
    pub applied_at_unix_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SystemProxyConfig {
    pub proxy_server: String,
    pub proxy_override: String,
}

impl SystemProxyConfig {
    pub fn new(proxy_server: impl Into<String>, proxy_override: impl Into<String>) -> Self {
        Self {
            proxy_server: proxy_server.into(),
            proxy_override: proxy_override.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SystemProxySnapshot {
    pub proxy_enable: Option<u32>,
    pub proxy_server: Option<String>,
    pub proxy_override: Option<String>,
}

#[derive(Debug, Error)]
pub enum NetworkEffectError {
    #[cfg(not(windows))]
    #[error("network effects are not supported on this platform")]
    UnsupportedPlatform,
    #[error("system proxy command failed for {operation}: {message}")]
    RegistryCommand { operation: String, message: String },
    #[error("invalid system proxy registry value for {name}: {value}")]
    InvalidRegistryValue { name: String, value: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn prepare_system_proxy(
    config: &SystemProxyConfig,
    applied_at_unix_ms: u64,
) -> Result<SystemProxyLease, NetworkEffectError> {
    let snapshot = read_system_proxy_snapshot()?;
    Ok(SystemProxyLease {
        desired: config.clone(),
        previous: snapshot,
        applied_at_unix_ms,
    })
}

pub fn apply_system_proxy_lease(lease: &SystemProxyLease) -> Result<(), NetworkEffectError> {
    if let Err(error) = write_system_proxy(&lease.desired) {
        let _ = restore_system_proxy_snapshot(&lease.previous);
        return Err(error);
    }

    Ok(())
}

pub fn restore_network_effects(state: &NetworkEffectsState) -> Result<(), NetworkEffectError> {
    if let Some(lease) = &state.system_proxy {
        restore_system_proxy_snapshot(&lease.previous)?;
    }

    Ok(())
}

#[cfg(windows)]
const INTERNET_SETTINGS_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

#[cfg(windows)]
fn read_system_proxy_snapshot() -> Result<SystemProxySnapshot, NetworkEffectError> {
    Ok(SystemProxySnapshot {
        proxy_enable: read_registry_dword("ProxyEnable")?,
        proxy_server: read_registry_string("ProxyServer")?,
        proxy_override: read_registry_string("ProxyOverride")?,
    })
}

#[cfg(not(windows))]
fn read_system_proxy_snapshot() -> Result<SystemProxySnapshot, NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

#[cfg(windows)]
fn write_system_proxy(config: &SystemProxyConfig) -> Result<(), NetworkEffectError> {
    write_registry_string("ProxyServer", &config.proxy_server)?;
    if config.proxy_override.trim().is_empty() {
        delete_registry_value("ProxyOverride")?;
    } else {
        write_registry_string("ProxyOverride", &config.proxy_override)?;
    }
    write_registry_dword("ProxyEnable", 1)?;
    notify_proxy_settings_changed();
    Ok(())
}

#[cfg(not(windows))]
fn write_system_proxy(_config: &SystemProxyConfig) -> Result<(), NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

#[cfg(windows)]
fn restore_system_proxy_snapshot(snapshot: &SystemProxySnapshot) -> Result<(), NetworkEffectError> {
    if matches!(snapshot.proxy_enable, None | Some(0)) {
        restore_registry_dword("ProxyEnable", snapshot.proxy_enable)?;
        restore_registry_string("ProxyServer", &snapshot.proxy_server)?;
        restore_registry_string("ProxyOverride", &snapshot.proxy_override)?;
    } else {
        restore_registry_string("ProxyServer", &snapshot.proxy_server)?;
        restore_registry_string("ProxyOverride", &snapshot.proxy_override)?;
        restore_registry_dword("ProxyEnable", snapshot.proxy_enable)?;
    }

    notify_proxy_settings_changed();
    Ok(())
}

#[cfg(not(windows))]
fn restore_system_proxy_snapshot(
    _snapshot: &SystemProxySnapshot,
) -> Result<(), NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

#[cfg(windows)]
fn restore_registry_dword(name: &str, value: Option<u32>) -> Result<(), NetworkEffectError> {
    match value {
        Some(value) => write_registry_dword(name, value),
        None => delete_registry_value(name),
    }
}

#[cfg(windows)]
fn restore_registry_string(name: &str, value: &Option<String>) -> Result<(), NetworkEffectError> {
    match value {
        Some(value) => write_registry_string(name, value),
        None => delete_registry_value(name),
    }
}

#[cfg(windows)]
fn read_registry_dword(name: &str) -> Result<Option<u32>, NetworkEffectError> {
    let Some(value) = read_registry_value(name)? else {
        return Ok(None);
    };

    let raw = value.data.trim();
    if let Some(hex) = raw.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).map(Some).map_err(|_| {
            NetworkEffectError::InvalidRegistryValue {
                name: name.to_string(),
                value: raw.to_string(),
            }
        })
    } else {
        raw.parse::<u32>()
            .map(Some)
            .map_err(|_| NetworkEffectError::InvalidRegistryValue {
                name: name.to_string(),
                value: raw.to_string(),
            })
    }
}

#[cfg(windows)]
fn read_registry_string(name: &str) -> Result<Option<String>, NetworkEffectError> {
    Ok(read_registry_value(name)?.map(|value| value.data))
}

#[cfg(windows)]
#[derive(Debug)]
struct RegistryValue {
    data: String,
}

#[cfg(windows)]
fn read_registry_value(name: &str) -> Result<Option<RegistryValue>, NetworkEffectError> {
    let output = Command::new("reg")
        .args(["query", INTERNET_SETTINGS_KEY, "/v", name])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().map(str::trim) {
        if !line.starts_with(name) {
            continue;
        }

        let mut parts = line.split_whitespace();
        let value_name = parts.next();
        let value_type = parts.next();
        if value_name != Some(name) {
            continue;
        }
        let Some(value_type) = value_type else {
            continue;
        };
        let Some(type_index) = line.find(value_type) else {
            continue;
        };
        let data_start = type_index + value_type.len();
        return Ok(Some(RegistryValue {
            data: line[data_start..].trim().to_string(),
        }));
    }

    Ok(None)
}

#[cfg(windows)]
fn write_registry_dword(name: &str, value: u32) -> Result<(), NetworkEffectError> {
    run_registry_command(
        "write dword",
        Command::new("reg")
            .args([
                "add",
                INTERNET_SETTINGS_KEY,
                "/v",
                name,
                "/t",
                "REG_DWORD",
                "/d",
                &value.to_string(),
                "/f",
            ])
            .output()?,
    )
}

#[cfg(windows)]
fn write_registry_string(name: &str, value: &str) -> Result<(), NetworkEffectError> {
    run_registry_command(
        "write string",
        Command::new("reg")
            .args([
                "add",
                INTERNET_SETTINGS_KEY,
                "/v",
                name,
                "/t",
                "REG_SZ",
                "/d",
                value,
                "/f",
            ])
            .output()?,
    )
}

#[cfg(windows)]
fn delete_registry_value(name: &str) -> Result<(), NetworkEffectError> {
    let output = Command::new("reg")
        .args(["delete", INTERNET_SETTINGS_KEY, "/v", name, "/f"])
        .output()?;
    if output.status.success() {
        return Ok(());
    }

    if read_registry_value(name)?.is_none() {
        return Ok(());
    }

    run_registry_command("delete value", output)
}

#[cfg(windows)]
fn run_registry_command(
    operation: &str,
    output: std::process::Output,
) -> Result<(), NetworkEffectError> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if stderr.is_empty() { stdout } else { stderr };
    Err(NetworkEffectError::RegistryCommand {
        operation: operation.to_string(),
        message,
    })
}

#[cfg(windows)]
fn notify_proxy_settings_changed() {
    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };

    unsafe {
        let _ = InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        let _ = InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_effects_state_skips_empty() {
        assert!(NetworkEffectsState::default().is_empty());
        let json = serde_json::to_string(&NetworkEffectsState::default()).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn system_proxy_lease_round_trips_previous_snapshot() {
        let state = NetworkEffectsState {
            system_proxy: Some(SystemProxyLease {
                desired: SystemProxyConfig::new(
                    "http=127.0.0.1:8080;https=127.0.0.1:8080",
                    "<local>",
                ),
                previous: SystemProxySnapshot {
                    proxy_enable: Some(0),
                    proxy_server: Some("old.proxy:3128".to_string()),
                    proxy_override: None,
                },
                applied_at_unix_ms: 42,
            }),
        };

        let encoded = serde_json::to_string(&state).unwrap();
        let decoded = serde_json::from_str::<NetworkEffectsState>(&encoded).unwrap();

        assert_eq!(decoded, state);
        assert!(!decoded.is_empty());
    }
}
