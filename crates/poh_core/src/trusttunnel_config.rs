use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListenerMode {
    #[default]
    Tun,
    Socks,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingMode {
    #[default]
    General,
    Selective,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamProtocol {
    #[default]
    Http2,
    Http3,
}

impl UpstreamProtocol {
    pub fn as_toml(self) -> &'static str {
        match self {
            Self::Http2 => "http2",
            Self::Http3 => "http3",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[default]
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub hostname: String,
    pub custom_sni: String,
    pub addresses: Vec<String>,
    pub has_ipv6: bool,
    pub username: String,
    pub password_secret_ref: String,
    pub client_random_secret_ref: String,
    pub skip_verification: bool,
    pub certificate_pem: String,
    pub upstream_protocol: UpstreamProtocol,
    pub fallback_protocol: Option<UpstreamProtocol>,
    pub anti_dpi: bool,
    pub post_quantum_group_enabled: bool,
    pub dns_upstreams: Vec<String>,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            custom_sni: String::new(),
            addresses: Vec::new(),
            has_ipv6: true,
            username: String::new(),
            password_secret_ref: String::new(),
            client_random_secret_ref: String::new(),
            skip_verification: false,
            certificate_pem: String::new(),
            upstream_protocol: UpstreamProtocol::Http2,
            fallback_protocol: None,
            anti_dpi: false,
            post_quantum_group_enabled: true,
            dns_upstreams: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TunConfig {
    pub bound_if: String,
    pub included_routes: Vec<String>,
    pub excluded_routes: Vec<String>,
    pub mtu_size: i32,
    pub tcp_recv_buf_size: i32,
    pub tcp_send_buf_size: i32,
    pub change_system_dns: bool,
    pub device_name: String,
    pub use_existing: bool,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            bound_if: String::new(),
            included_routes: vec!["0.0.0.0/0".to_string(), "2000::/3".to_string()],
            excluded_routes: vec![
                "0.0.0.0/8".to_string(),
                "10.0.0.0/8".to_string(),
                "169.254.0.0/16".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
                "224.0.0.0/3".to_string(),
            ],
            mtu_size: 1280,
            tcp_recv_buf_size: 0,
            tcp_send_buf_size: 0,
            change_system_dns: true,
            device_name: String::new(),
            use_existing: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocksConfig {
    pub address: String,
    pub username: String,
    pub password_secret_ref: String,
    pub allow_lan_access: bool,
    pub http_proxy_address: String,
    pub http_proxy_allow_lan_access: bool,
}

impl Default for SocksConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:1080".to_string(),
            username: String::new(),
            password_secret_ref: String::new(),
            allow_lan_access: false,
            http_proxy_address: String::new(),
            http_proxy_allow_lan_access: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub mode: ListenerMode,
    pub tun: TunConfig,
    pub socks: SocksConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RoutingProfile {
    pub id: String,
    pub name: String,
    pub mode: RoutingMode,
    pub exclusions: Vec<String>,
    pub kill_switch_enabled: bool,
    pub kill_switch_allow_ports: Vec<i32>,
    pub description: String,
}

impl Default for RoutingProfile {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "Default".to_string(),
            mode: RoutingMode::General,
            exclusions: Vec::new(),
            kill_switch_enabled: true,
            kill_switch_allow_ports: Vec::new(),
            description: "Route all traffic through VPN".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrustTunnelConfig {
    pub log_level: LogLevel,
    pub routing: RoutingProfile,
    pub endpoint: EndpointConfig,
    pub listener: ListenerConfig,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecretCandidate {
    pub password: String,
    pub client_random: String,
    pub socks_password: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrustTunnelCoreProfile {
    pub source_format: String,
    pub config: TrustTunnelConfig,
}
