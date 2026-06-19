#[cfg(windows)]
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Ledger of OS-level network changes a session is responsible for undoing.
///
/// Each entry stores the pre-session state so stop / reset / crash-reconcile can
/// put the machine back even after a force-kill (where the core itself never gets
/// to clean up). Effects are restored in a fixed order: firewall → routes → dns →
/// system_proxy (least-to-most disruptive in reverse).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetworkEffectsState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_proxy: Option<SystemProxyLease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns: Option<DnsLease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routes: Option<RouteLease>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firewall: Option<FirewallLease>,
}

impl NetworkEffectsState {
    pub fn is_empty(&self) -> bool {
        self.system_proxy.is_none()
            && self.dns.is_none()
            && self.routes.is_none()
            && self.firewall.is_none()
    }
}

/// Snapshot of per-interface DNS configuration captured before the core changes
/// it, so we can force it back on teardown/crash.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DnsLease {
    pub interfaces: Vec<DnsInterfaceSnapshot>,
    pub applied_at_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsFamily {
    Ipv4,
    Ipv6,
}

impl DnsFamily {
    /// `netsh interface <family>` sub-command token.
    fn netsh_token(self) -> &'static str {
        match self {
            DnsFamily::Ipv4 => "ipv4",
            DnsFamily::Ipv6 => "ipv6",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum DnsConfig {
    /// Resolver addresses come from DHCP.
    Dhcp,
    /// Statically configured resolver addresses, in priority order.
    Static { servers: Vec<String> },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DnsInterfaceSnapshot {
    pub interface: String,
    pub family: DnsFamily,
    pub config: DnsConfig,
}

/// Records which TUN interface routes were added for so they can be removed on
/// crash. We store the configured CIDR prefixes (from `included_routes`) rather
/// than a full route table diff — on teardown we issue targeted `netsh` deletes
/// for exactly those prefixes through the named interface.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RouteLease {
    /// Name of the TUN adapter added by the core (e.g. `"TrustTunnel"`).
    pub tun_interface: String,
    /// CIDR prefixes the core was configured to route through the TUN adapter.
    /// Each entry becomes a `netsh interface ipv4 delete route` on restore.
    pub configured_routes: Vec<String>,
    pub applied_at_unix_ms: u64,
}

/// Snapshot of Windows Firewall rule names taken before the core starts. On
/// restore we re-enumerate current rules, diff against this set, and delete any
/// rules that were not present before the session (i.e. kill-switch rules that
/// the core added and did not remove when it crashed).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FirewallLease {
    pub pre_session_rule_names: Vec<String>,
    pub applied_at_unix_ms: u64,
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
    #[error("dns restore command failed for {interface}: {message}")]
    DnsCommand { interface: String, message: String },
    #[error("route restore command failed for interface {interface}: {message}")]
    RouteCommand { interface: String, message: String },
    #[error("firewall restore command failed for rule {rule}: {message}")]
    FirewallCommand { rule: String, message: String },
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

/// Undo every recorded effect, attempting all of them even if one fails (a
/// safety-net must not abort half-way and leave the rest of the network broken).
/// Order: firewall → routes → dns → system_proxy (most-impactful first so a
/// kill-switch crash never permanently locks the user out).
/// Returns the first error encountered, if any.
pub fn restore_network_effects(state: &NetworkEffectsState) -> Result<(), NetworkEffectError> {
    let mut first_error: Option<NetworkEffectError> = None;

    if let Some(firewall) = &state.firewall {
        if let Err(error) = restore_firewall_lease(firewall) {
            first_error.get_or_insert(error);
        }
    }

    if let Some(routes) = &state.routes {
        if let Err(error) = restore_route_lease(routes) {
            first_error.get_or_insert(error);
        }
    }

    if let Some(dns) = &state.dns {
        if let Err(error) = restore_dns_lease(dns) {
            first_error.get_or_insert(error);
        }
    }

    if let Some(lease) = &state.system_proxy {
        if let Err(error) = restore_system_proxy_snapshot(&lease.previous) {
            first_error.get_or_insert(error);
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Build the `netsh` argument lists that restore one DNS lease. Pure (no I/O) so
/// the rollback logic is unit-testable without touching the real network.
pub fn dns_restore_commands(lease: &DnsLease) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    for snapshot in &lease.interfaces {
        let family = snapshot.family.netsh_token();
        let name_arg = format!("name={}", snapshot.interface);
        match &snapshot.config {
            DnsConfig::Static { servers } if !servers.is_empty() => {
                commands.push(vec![
                    "interface".to_string(),
                    family.to_string(),
                    "set".to_string(),
                    "dnsservers".to_string(),
                    name_arg.clone(),
                    "static".to_string(),
                    servers[0].clone(),
                    "primary".to_string(),
                    "validate=no".to_string(),
                ]);
                for (offset, server) in servers.iter().enumerate().skip(1) {
                    commands.push(vec![
                        "interface".to_string(),
                        family.to_string(),
                        "add".to_string(),
                        "dnsservers".to_string(),
                        name_arg.clone(),
                        server.clone(),
                        format!("index={}", offset + 1),
                        "validate=no".to_string(),
                    ]);
                }
            }
            // DHCP, or a Static lease with no recorded servers (treat as DHCP).
            _ => commands.push(vec![
                "interface".to_string(),
                family.to_string(),
                "set".to_string(),
                "dnsservers".to_string(),
                name_arg,
                "source=dhcp".to_string(),
            ]),
        }
    }

    commands
}

#[cfg(windows)]
fn restore_dns_lease(lease: &DnsLease) -> Result<(), NetworkEffectError> {
    let mut first_error: Option<NetworkEffectError> = None;
    for (index, args) in dns_restore_commands(lease).into_iter().enumerate() {
        let interface = lease
            .interfaces
            .get(index)
            .map(|snapshot| snapshot.interface.clone())
            .unwrap_or_default();
        let output = Command::new("netsh").args(&args).output()?;
        if !output.status.success() && first_error.is_none() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            first_error = Some(NetworkEffectError::DnsCommand { interface, message });
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(not(windows))]
fn restore_dns_lease(_lease: &DnsLease) -> Result<(), NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

// ── Routes ─────────────────────────────────────────────────────────────────

/// Build the `netsh` argument lists that remove TUN routes recorded in a lease.
/// Pure (no I/O): each CIDR in `configured_routes` becomes one
/// `netsh interface ipv4 delete route prefix=CIDR interface=NAME` command.
/// If a route is already gone (e.g. wintun cleaned up on crash), the runner
/// ignores the "element not found" error — see `restore_route_lease`.
pub fn route_restore_commands(lease: &RouteLease) -> Vec<Vec<String>> {
    lease
        .configured_routes
        .iter()
        .filter(|cidr| !cidr.trim().is_empty())
        .map(|cidr| {
            let family = if cidr.contains(':') { "ipv6" } else { "ipv4" };
            vec![
                "interface".to_string(),
                family.to_string(),
                "delete".to_string(),
                "route".to_string(),
                format!("prefix={}", cidr.trim()),
                format!("interface={}", lease.tun_interface),
            ]
        })
        .collect()
}

#[cfg(windows)]
fn restore_route_lease(lease: &RouteLease) -> Result<(), NetworkEffectError> {
    let mut first_error: Option<NetworkEffectError> = None;
    for args in route_restore_commands(lease) {
        let output = Command::new("netsh").args(&args).output()?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
            // Route already gone (wintun auto-cleaned on adapter teardown) — not an error.
            if stdout.contains("element not found")
                || stdout.contains("no matching")
                || stdout.contains("not found")
            {
                continue;
            }
            if first_error.is_none() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let message = if stderr.is_empty() {
                    String::from_utf8_lossy(&output.stdout).trim().to_string()
                } else {
                    stderr
                };
                first_error = Some(NetworkEffectError::RouteCommand {
                    interface: lease.tun_interface.clone(),
                    message,
                });
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(not(windows))]
fn restore_route_lease(_lease: &RouteLease) -> Result<(), NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

// ── Firewall ────────────────────────────────────────────────────────────────

/// Build `netsh advfirewall firewall delete rule` arg lists for rules that were
/// NOT in the pre-session snapshot — i.e. rules the core added (kill-switch).
/// Pure (no I/O): takes the current rule names as a parameter so the diff logic
/// can be unit-tested without running `netsh`.
pub fn firewall_restore_commands(
    lease: &FirewallLease,
    current_rule_names: &[String],
) -> Vec<Vec<String>> {
    let pre: std::collections::HashSet<&str> = lease
        .pre_session_rule_names
        .iter()
        .map(String::as_str)
        .collect();
    current_rule_names
        .iter()
        .filter(|name| !pre.contains(name.as_str()))
        .map(|name| {
            vec![
                "advfirewall".to_string(),
                "firewall".to_string(),
                "delete".to_string(),
                "rule".to_string(),
                format!("name={}", name),
            ]
        })
        .collect()
}

/// Parse the output of `netsh advfirewall firewall show rule name=all` into a
/// list of rule names. Uses the separator-line heuristic (a `---` line always
/// follows the rule-name line) so it works regardless of Windows UI locale.
pub fn parse_netsh_advfirewall_rules(output: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().collect();
    let mut names = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("---") && i > 0 {
            let prev = lines[i - 1].trim();
            if let Some((_key, value)) = prev.split_once(':') {
                let name = value.trim();
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// Snapshot the names of all current Windows Firewall rules so new rules added
/// by the core (e.g. kill-switch) can be identified and removed on teardown.
pub fn read_firewall_lease(applied_at_unix_ms: u64) -> Result<FirewallLease, NetworkEffectError> {
    read_firewall_lease_impl(applied_at_unix_ms)
}

#[cfg(windows)]
fn read_firewall_lease_impl(applied_at_unix_ms: u64) -> Result<FirewallLease, NetworkEffectError> {
    let output = Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", "name=all"])
        .output()?;
    let pre_session_rule_names =
        parse_netsh_advfirewall_rules(&String::from_utf8_lossy(&output.stdout));
    Ok(FirewallLease {
        pre_session_rule_names,
        applied_at_unix_ms,
    })
}

#[cfg(not(windows))]
fn read_firewall_lease_impl(_applied_at_unix_ms: u64) -> Result<FirewallLease, NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

#[cfg(windows)]
fn restore_firewall_lease(lease: &FirewallLease) -> Result<(), NetworkEffectError> {
    let output = Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", "name=all"])
        .output()?;
    let current_rules = parse_netsh_advfirewall_rules(&String::from_utf8_lossy(&output.stdout));
    let commands = firewall_restore_commands(lease, &current_rules);

    let mut first_error: Option<NetworkEffectError> = None;
    for args in &commands {
        let rule_name = args
            .iter()
            .find(|a| a.starts_with("name="))
            .map(|a| a["name=".len()..].to_string())
            .unwrap_or_default();
        let result = Command::new("netsh").args(args).output()?;
        if !result.status.success() {
            let stdout = String::from_utf8_lossy(&result.stdout).to_ascii_lowercase();
            if stdout.contains("no rules match") || stdout.contains("not found") {
                continue;
            }
            if first_error.is_none() {
                let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
                let message = if stderr.is_empty() {
                    String::from_utf8_lossy(&result.stdout).trim().to_string()
                } else {
                    stderr
                };
                first_error = Some(NetworkEffectError::FirewallCommand {
                    rule: rule_name,
                    message,
                });
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(not(windows))]
fn restore_firewall_lease(_lease: &FirewallLease) -> Result<(), NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

/// Snapshot the current DNS configuration for all network interfaces so it can
/// be restored later by [`restore_network_effects`] if the core dies without
/// cleaning up.
///
/// `applied_at_unix_ms` records when the snapshot was taken, matching the
/// convention of [`prepare_system_proxy`].
///
/// Returns `Err` only on I/O failures running the `netsh` commands; interfaces
/// with unparseable output are silently omitted.
pub fn read_dns_lease(applied_at_unix_ms: u64) -> Result<DnsLease, NetworkEffectError> {
    read_dns_lease_impl(applied_at_unix_ms)
}

#[cfg(windows)]
fn read_dns_lease_impl(applied_at_unix_ms: u64) -> Result<DnsLease, NetworkEffectError> {
    let ipv4 = Command::new("netsh")
        .args(["interface", "ipv4", "show", "dnsservers"])
        .output()?;
    let ipv6 = Command::new("netsh")
        .args(["interface", "ipv6", "show", "dnsservers"])
        .output()?;

    let mut interfaces = Vec::new();
    interfaces.extend(parse_netsh_dnsservers(
        &String::from_utf8_lossy(&ipv4.stdout),
        DnsFamily::Ipv4,
    ));
    interfaces.extend(parse_netsh_dnsservers(
        &String::from_utf8_lossy(&ipv6.stdout),
        DnsFamily::Ipv6,
    ));

    Ok(DnsLease {
        interfaces,
        applied_at_unix_ms,
    })
}

#[cfg(not(windows))]
fn read_dns_lease_impl(_applied_at_unix_ms: u64) -> Result<DnsLease, NetworkEffectError> {
    Err(NetworkEffectError::UnsupportedPlatform)
}

/// Parse the output of `netsh interface <ipv4|ipv6> show dnsservers` into a
/// list of per-interface DNS snapshots. Pure (no I/O) so it can be
/// unit-tested without running `netsh`.
pub fn parse_netsh_dnsservers(output: &str, family: DnsFamily) -> Vec<DnsInterfaceSnapshot> {
    struct IfaceState {
        name: String,
        is_dhcp: Option<bool>,
        servers: Vec<String>,
    }

    impl IfaceState {
        fn finish(self, family: DnsFamily) -> Option<DnsInterfaceSnapshot> {
            let is_dhcp = self.is_dhcp?;
            let config = if is_dhcp || self.servers.is_empty() {
                DnsConfig::Dhcp
            } else {
                DnsConfig::Static {
                    servers: self.servers,
                }
            };
            Some(DnsInterfaceSnapshot {
                interface: self.name,
                family,
                config,
            })
        }
    }

    let mut result = Vec::new();
    let mut current: Option<IfaceState> = None;

    for line in output.lines() {
        if let Some(name) = extract_interface_name(line) {
            if let Some(state) = current.take() {
                if let Some(snap) = state.finish(family) {
                    result.push(snap);
                }
            }
            current = Some(IfaceState {
                name,
                is_dhcp: None,
                servers: Vec::new(),
            });
            continue;
        }

        let Some(ref mut state) = current else {
            continue;
        };
        let trimmed = line.trim();

        if let Some(rest) = dns_label_value(trimmed) {
            if trimmed.to_ascii_lowercase().contains("dhcp") {
                state.is_dhcp = Some(true);
            } else {
                state.is_dhcp = Some(false);
            }
            if !rest.is_empty() && !rest.eq_ignore_ascii_case("none") && looks_like_addr(rest) {
                state.servers.push(rest.to_string());
            }
        } else if matches!(state.is_dhcp, Some(false)) {
            // Continuation IP line for static mode. Keyword lines are indented
            // with 4 spaces; continuation IP lines are aligned under the first
            // IP address (~44 spaces). Requiring ≥ 16 leading spaces safely
            // distinguishes them from "Register with which suffix:" lines.
            if line.starts_with("                ") && looks_like_addr(trimmed) {
                state.servers.push(trimmed.to_string());
            }
        }
    }

    if let Some(state) = current {
        if let Some(snap) = state.finish(family) {
            result.push(snap);
        }
    }

    result
}

fn extract_interface_name(line: &str) -> Option<String> {
    if line.starts_with(char::is_whitespace) {
        return None;
    }

    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    let name = &rest[..end];
    if name.trim().is_empty() {
        return None;
    }

    Some(name.to_string())
}

fn dns_label_value(line: &str) -> Option<&str> {
    if !line.to_ascii_lowercase().contains("dns") {
        return None;
    }

    let (_, value) = line.split_once(':')?;
    Some(value.trim())
}

fn looks_like_addr(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_hexdigit() || c == ':')
        && !s.contains(' ')
        && !s.contains('=')
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
            ..NetworkEffectsState::default()
        };

        let encoded = serde_json::to_string(&state).unwrap();
        let decoded = serde_json::from_str::<NetworkEffectsState>(&encoded).unwrap();

        assert_eq!(decoded, state);
        assert!(!decoded.is_empty());
    }

    fn s(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }

    fn dns_lease(config: DnsConfig) -> DnsLease {
        DnsLease {
            interfaces: vec![DnsInterfaceSnapshot {
                interface: "Ethernet".to_string(),
                family: DnsFamily::Ipv4,
                config,
            }],
            applied_at_unix_ms: 7,
        }
    }

    #[test]
    fn dns_lease_round_trips_and_marks_state_non_empty() {
        let state = NetworkEffectsState {
            dns: Some(dns_lease(DnsConfig::Static {
                servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
            })),
            ..NetworkEffectsState::default()
        };

        let encoded = serde_json::to_string(&state).unwrap();
        let decoded = serde_json::from_str::<NetworkEffectsState>(&encoded).unwrap();

        assert_eq!(decoded, state);
        assert!(!decoded.is_empty());
    }

    #[test]
    fn dns_restore_commands_for_dhcp_resets_to_dhcp() {
        let commands = dns_restore_commands(&dns_lease(DnsConfig::Dhcp));
        assert_eq!(
            commands,
            vec![vec![
                "interface".to_string(),
                "ipv4".to_string(),
                "set".to_string(),
                "dnsservers".to_string(),
                "name=Ethernet".to_string(),
                "source=dhcp".to_string(),
            ]]
        );
    }

    #[test]
    fn dns_restore_commands_for_static_sets_primary_then_adds() {
        let commands = dns_restore_commands(&dns_lease(DnsConfig::Static {
            servers: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
        }));

        assert_eq!(commands.len(), 2);
        assert_eq!(
            commands[0],
            s(&[
                "interface",
                "ipv4",
                "set",
                "dnsservers",
                "name=Ethernet",
                "static",
                "1.1.1.1",
                "primary",
                "validate=no",
            ])
        );
        assert_eq!(
            commands[1],
            s(&[
                "interface",
                "ipv4",
                "add",
                "dnsservers",
                "name=Ethernet",
                "8.8.8.8",
                "index=2",
                "validate=no",
            ])
        );
    }

    #[test]
    fn dns_restore_commands_static_without_servers_falls_back_to_dhcp() {
        let commands = dns_restore_commands(&dns_lease(DnsConfig::Static { servers: vec![] }));
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].last().unwrap(), "source=dhcp");
    }

    #[test]
    fn dns_restore_commands_uses_ipv6_token() {
        let lease = DnsLease {
            interfaces: vec![DnsInterfaceSnapshot {
                interface: "Wi-Fi".to_string(),
                family: DnsFamily::Ipv6,
                config: DnsConfig::Dhcp,
            }],
            applied_at_unix_ms: 1,
        };
        let commands = dns_restore_commands(&lease);
        assert_eq!(commands[0][1].as_str(), "ipv6");
        assert_eq!(commands[0][4].as_str(), "name=Wi-Fi");
    }

    #[test]
    fn parse_netsh_dnsservers_dhcp_interface() {
        let output = concat!(
            "Configuration for interface \"Wi-Fi\"\r\n",
            "    DNS servers configured through DHCP:  192.168.1.1\r\n",
            "    Register with which suffix:           Primary only\r\n",
        );
        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].interface, "Wi-Fi");
        assert_eq!(snaps[0].config, DnsConfig::Dhcp);
    }

    #[test]
    fn parse_netsh_dnsservers_static_with_continuation() {
        let output = concat!(
            "Configuration for interface \"Ethernet\"\r\n",
            "    Statically Configured DNS Servers:    8.8.8.8\r\n",
            "                                          8.8.4.4\r\n",
            "    Register with which suffix:           Primary only\r\n",
        );
        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].interface, "Ethernet");
        assert_eq!(
            snaps[0].config,
            DnsConfig::Static {
                servers: vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()]
            }
        );
    }

    #[test]
    fn parse_netsh_dnsservers_static_none_becomes_dhcp() {
        let output = concat!(
            "Configuration for interface \"Loopback Pseudo-Interface 1\"\r\n",
            "    Statically Configured DNS Servers:    None\r\n",
            "    Register with which suffix:           None\r\n",
        );
        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].config, DnsConfig::Dhcp);
    }

    #[test]
    fn parse_netsh_dnsservers_multiple_interfaces() {
        let output = concat!(
            "Configuration for interface \"Wi-Fi\"\r\n",
            "    DNS servers configured through DHCP:  192.168.1.1\r\n",
            "    Register with which suffix:           Primary only\r\n",
            "\r\n",
            "Configuration for interface \"Ethernet\"\r\n",
            "    Statically Configured DNS Servers:    1.1.1.1\r\n",
            "    Register with which suffix:           Primary only\r\n",
        );
        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].interface, "Wi-Fi");
        assert_eq!(snaps[0].config, DnsConfig::Dhcp);
        assert_eq!(snaps[1].interface, "Ethernet");
        assert_eq!(
            snaps[1].config,
            DnsConfig::Static {
                servers: vec!["1.1.1.1".to_string()]
            }
        );
    }

    #[test]
    fn parse_netsh_dnsservers_localized_dhcp_interface() {
        let output = concat!(
            "Конфигурация для интерфейса \"Wi-Fi\"\r\n",
            "    DNS-серверы, настроенные через DHCP:  192.168.1.1\r\n",
            "    Регистрация с суффиксом:              только основной\r\n",
        );

        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].interface, "Wi-Fi");
        assert_eq!(snaps[0].config, DnsConfig::Dhcp);
    }

    #[test]
    fn parse_netsh_dnsservers_localized_static_interface() {
        let output = concat!(
            "Конфигурация для интерфейса \"Ethernet\"\r\n",
            "    DNS-серверы, настроенные статически:  9.9.9.9\r\n",
            "                                          149.112.112.112\r\n",
            "    Регистрация с суффиксом:              только основной\r\n",
        );

        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].interface, "Ethernet");
        assert_eq!(
            snaps[0].config,
            DnsConfig::Static {
                servers: vec!["9.9.9.9".to_string(), "149.112.112.112".to_string()]
            }
        );
    }

    #[test]
    fn parse_netsh_dnsservers_skips_interface_with_no_dns_line() {
        let output = concat!(
            "Configuration for interface \"Unknown\"\r\n",
            "    Register with which suffix:           None\r\n",
        );
        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv4);
        assert!(snaps.is_empty());
    }

    #[test]
    fn parse_netsh_dnsservers_empty_output() {
        assert!(parse_netsh_dnsservers("", DnsFamily::Ipv4).is_empty());
    }

    #[test]
    fn parse_netsh_dnsservers_ipv6_static() {
        let output = concat!(
            "Configuration for interface \"Wi-Fi\"\r\n",
            "    Statically Configured DNS Servers:    2001:4860:4860::8888\r\n",
            "                                          2001:4860:4860::8844\r\n",
            "    Register with which suffix:           Primary only\r\n",
        );
        let snaps = parse_netsh_dnsservers(output, DnsFamily::Ipv6);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].family, DnsFamily::Ipv6);
        assert_eq!(
            snaps[0].config,
            DnsConfig::Static {
                servers: vec![
                    "2001:4860:4860::8888".to_string(),
                    "2001:4860:4860::8844".to_string()
                ]
            }
        );
    }

    // ── Routes ─────────────────────────────────────────────────────────────

    fn route_lease(routes: &[&str]) -> RouteLease {
        RouteLease {
            tun_interface: "TrustTunnel".to_string(),
            configured_routes: routes.iter().map(|r| r.to_string()).collect(),
            applied_at_unix_ms: 42,
        }
    }

    #[test]
    fn route_lease_round_trips_serialization() {
        let lease = route_lease(&["0.0.0.0/0", "::/0"]);
        let state = NetworkEffectsState {
            routes: Some(lease.clone()),
            ..NetworkEffectsState::default()
        };
        let encoded = serde_json::to_string(&state).unwrap();
        let decoded = serde_json::from_str::<NetworkEffectsState>(&encoded).unwrap();
        assert_eq!(
            decoded.routes.as_ref().unwrap().tun_interface,
            "TrustTunnel"
        );
        assert_eq!(decoded.routes.as_ref().unwrap().configured_routes.len(), 2);
        assert!(!decoded.is_empty());
    }

    #[test]
    fn route_restore_commands_generates_netsh_ipv4_delete() {
        let commands = route_restore_commands(&route_lease(&["0.0.0.0/0"]));
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0],
            s(&[
                "interface",
                "ipv4",
                "delete",
                "route",
                "prefix=0.0.0.0/0",
                "interface=TrustTunnel",
            ])
        );
    }

    #[test]
    fn route_restore_commands_skips_empty_routes() {
        let lease = RouteLease {
            tun_interface: "TrustTunnel".to_string(),
            configured_routes: vec!["0.0.0.0/0".to_string(), "".to_string(), "  ".to_string()],
            applied_at_unix_ms: 1,
        };
        let commands = route_restore_commands(&lease);
        assert_eq!(commands.len(), 1, "blank routes must be skipped");
    }

    #[test]
    fn route_restore_commands_multiple_cidrs_produce_multiple_commands() {
        let commands = route_restore_commands(&route_lease(&["0.0.0.0/0", "::/0"]));
        assert_eq!(commands.len(), 2);
        assert!(commands[0].contains(&"prefix=0.0.0.0/0".to_string()));
        assert!(commands[1].contains(&"prefix=::/0".to_string()));
        // IPv4 CIDR uses ipv4 token; IPv6 CIDR uses ipv6 token
        assert!(commands[0].contains(&"ipv4".to_string()));
        assert!(commands[1].contains(&"ipv6".to_string()));
        // Both commands target the same interface
        assert!(commands[0].contains(&"interface=TrustTunnel".to_string()));
        assert!(commands[1].contains(&"interface=TrustTunnel".to_string()));
    }

    // ── Firewall ────────────────────────────────────────────────────────────

    #[test]
    fn firewall_lease_round_trips_serialization() {
        let lease = FirewallLease {
            pre_session_rule_names: vec!["Block All".to_string(), "Allow Loopback".to_string()],
            applied_at_unix_ms: 7,
        };
        let state = NetworkEffectsState {
            firewall: Some(lease),
            ..NetworkEffectsState::default()
        };
        let encoded = serde_json::to_string(&state).unwrap();
        let decoded = serde_json::from_str::<NetworkEffectsState>(&encoded).unwrap();
        assert_eq!(
            decoded
                .firewall
                .as_ref()
                .unwrap()
                .pre_session_rule_names
                .len(),
            2
        );
        assert!(!decoded.is_empty());
    }

    #[test]
    fn firewall_restore_commands_deletes_rules_added_after_snapshot() {
        let lease = FirewallLease {
            pre_session_rule_names: vec!["Allow Loopback".to_string(), "Block All".to_string()],
            applied_at_unix_ms: 1,
        };
        let current = vec![
            "Allow Loopback".to_string(),
            "Block All".to_string(),
            "TrustTunnel_killswitch_block".to_string(),
            "TrustTunnel_killswitch_allow_vpn".to_string(),
        ];
        let commands = firewall_restore_commands(&lease, &current);
        assert_eq!(commands.len(), 2);
        let rule_names: Vec<String> = commands
            .iter()
            .filter_map(|cmd| cmd.iter().find(|a| a.starts_with("name=")))
            .cloned()
            .collect();
        assert!(rule_names.contains(&"name=TrustTunnel_killswitch_block".to_string()));
        assert!(rule_names.contains(&"name=TrustTunnel_killswitch_allow_vpn".to_string()));
    }

    #[test]
    fn firewall_restore_commands_empty_when_no_new_rules() {
        let lease = FirewallLease {
            pre_session_rule_names: vec!["Allow Loopback".to_string()],
            applied_at_unix_ms: 1,
        };
        let current = vec!["Allow Loopback".to_string()];
        let commands = firewall_restore_commands(&lease, &current);
        assert!(commands.is_empty());
    }

    #[test]
    fn parse_netsh_advfirewall_rules_extracts_rule_names() {
        let output = concat!(
            "\r\n",
            "Rule Name:                            Allow Loopback\r\n",
            "----------------------------------------------------------------------\r\n",
            "Enabled:                              Yes\r\n",
            "Direction:                            In\r\n",
            "\r\n",
            "Rule Name:                            Block Inbound\r\n",
            "----------------------------------------------------------------------\r\n",
            "Enabled:                              Yes\r\n",
            "Direction:                            In\r\n",
        );
        let names = parse_netsh_advfirewall_rules(output);
        assert_eq!(names, vec!["Allow Loopback", "Block Inbound"]);
    }

    #[test]
    fn parse_netsh_advfirewall_rules_locale_agnostic() {
        // Russian locale — key is localized but separator heuristic still works
        let output = concat!(
            "Имя правила:                          TrustTunnel Kill Switch\r\n",
            "----------------------------------------------------------------------\r\n",
            "Состояние:                            Да\r\n",
            "\r\n",
            "Имя правила:                          Разрешить петлевой\r\n",
            "----------------------------------------------------------------------\r\n",
            "Состояние:                            Да\r\n",
        );
        let names = parse_netsh_advfirewall_rules(output);
        assert_eq!(names, vec!["TrustTunnel Kill Switch", "Разрешить петлевой"]);
    }

    #[test]
    fn parse_netsh_advfirewall_rules_empty_output() {
        assert!(parse_netsh_advfirewall_rules("").is_empty());
        assert!(parse_netsh_advfirewall_rules("Ok.\r\n").is_empty());
    }

    #[test]
    fn parse_netsh_advfirewall_rules_rule_name_with_colon() {
        // Rule names can contain colons — only the first colon splits key: value
        let output = concat!(
            "Rule Name:                            POH: Kill Switch: Block\r\n",
            "----------------------------------------------------------------------\r\n",
            "Enabled:                              Yes\r\n",
        );
        let names = parse_netsh_advfirewall_rules(output);
        assert_eq!(names, vec!["POH: Kill Switch: Block"]);
    }

    #[test]
    fn network_effects_state_includes_routes_and_firewall_in_empty_check() {
        let mut state = NetworkEffectsState::default();
        assert!(state.is_empty());

        state.routes = Some(route_lease(&["0.0.0.0/0"]));
        assert!(!state.is_empty());

        state.routes = None;
        state.firewall = Some(FirewallLease {
            pre_session_rule_names: vec![],
            applied_at_unix_ms: 1,
        });
        assert!(!state.is_empty());
    }
}
