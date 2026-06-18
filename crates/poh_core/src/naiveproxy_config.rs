use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NaiveProxyEndpoint {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password_secret_ref: String,
    pub path: String,
    pub query: String,
}

impl Default for NaiveProxyEndpoint {
    fn default() -> Self {
        Self::local_socks_default()
    }
}

impl NaiveProxyEndpoint {
    pub fn local_socks_default() -> Self {
        Self {
            scheme: "socks".to_string(),
            host: "127.0.0.1".to_string(),
            port: Some(1080),
            username: String::new(),
            password_secret_ref: String::new(),
            path: String::new(),
            query: String::new(),
        }
    }

    pub fn proxy_default() -> Self {
        Self {
            scheme: "https".to_string(),
            host: String::new(),
            port: None,
            username: String::new(),
            password_secret_ref: String::new(),
            path: String::new(),
            query: String::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NaiveProxyConfig {
    pub listen: NaiveProxyEndpoint,
    pub proxy: NaiveProxyEndpoint,
    pub extra_headers: Vec<String>,
    pub host_resolver_rules: String,
    pub resolver_range: String,
    pub tunnel_timeout: String,
    pub idle_timeout: String,
    pub insecure_concurrency: Option<u32>,
    pub no_post_quantum: bool,
}

impl Default for NaiveProxyConfig {
    fn default() -> Self {
        Self {
            listen: NaiveProxyEndpoint::local_socks_default(),
            proxy: NaiveProxyEndpoint::proxy_default(),
            extra_headers: Vec::new(),
            host_resolver_rules: String::new(),
            resolver_range: String::new(),
            tunnel_timeout: String::new(),
            idle_timeout: String::new(),
            insecure_concurrency: None,
            no_post_quantum: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NaiveProxySecretCandidate {
    pub proxy_password: String,
    pub listen_password: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NaiveProxyCoreProfile {
    pub source_format: String,
    pub config: NaiveProxyConfig,
}
