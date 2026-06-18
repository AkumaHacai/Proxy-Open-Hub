use std::collections::BTreeMap;

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use serde_json::{json, Map, Value};
use url::Url;

use crate::adapter::{
    AdapterError, CoreAdapter, ImportInput, ImportMatch, ParsedProfile, RuntimeConfig, RuntimeFile,
    SettingsField, SettingsFieldKind, SettingsSchema, SettingsSection, ValidationWarning,
};
use crate::model::{CoreCapabilities, CoreId, CoreManifest, Profile, ProfileId, ProtocolColor};
use crate::naiveproxy_config::{
    NaiveProxyConfig, NaiveProxyCoreProfile, NaiveProxyEndpoint, NaiveProxySecretCandidate,
};
use crate::security::RuntimeSecurityPolicy;

const PROXY_PASSWORD_KEY: &str = "proxy.password";
const LISTEN_PASSWORD_KEY: &str = "listen.password";
const DEFAULT_LISTEN_URL: &str = "socks://127.0.0.1:1080";
const USERINFO_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b':')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

pub struct NaiveProxyAdapter {
    manifest: CoreManifest,
}

impl Default for NaiveProxyAdapter {
    fn default() -> Self {
        Self {
            manifest: CoreManifest {
                id: CoreId::from("naiveproxy"),
                display_name: "NaiveProxy".to_string(),
                summary: "Local SOCKS/HTTP proxy core based on Chromium networking".to_string(),
                homepage: Some("https://github.com/klzgrad/naiveproxy".to_string()),
                license: Some("BSD-3-Clause".to_string()),
                protocol_colors: vec![
                    ProtocolColor {
                        protocol: "https".to_string(),
                        color: "#2F6DF2".to_string(),
                    },
                    ProtocolColor {
                        protocol: "quic".to_string(),
                        color: "#7A35E8".to_string(),
                    },
                    ProtocolColor {
                        protocol: "socks".to_string(),
                        color: "#23885A".to_string(),
                    },
                ],
                capabilities: CoreCapabilities {
                    can_import: true,
                    can_download: true,
                    supports_tun: false,
                    supports_socks: true,
                    supports_http_proxy: true,
                    supports_system_proxy: true,
                    supports_config_test: true,
                },
            },
        }
    }
}

impl NaiveProxyAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CoreAdapter for NaiveProxyAdapter {
    fn manifest(&self) -> &CoreManifest {
        &self.manifest
    }

    fn detect_import(&self, input: &ImportInput) -> ImportMatch {
        let content = input.content.trim();
        if json_contains_listen_and_proxy(content) {
            return ImportMatch::new(95, "NaiveProxy config.json");
        }

        if looks_like_proxy_url(content) {
            return ImportMatch::new(50, "Possible NaiveProxy proxy URL");
        }

        ImportMatch::none("No NaiveProxy markers found")
    }

    fn parse_profile(&self, input: &ImportInput) -> Result<ParsedProfile, AdapterError> {
        let detection = self.detect_import(input);
        if !detection.matched() {
            return Err(AdapterError::UnsupportedInput);
        }

        let content = input.content.trim();
        let imported = if json_contains_listen_and_proxy(content) {
            parse_config_json(content)?
        } else {
            parse_proxy_url_profile(content)?
        };

        let name = format!("NaiveProxy {}", imported.config.proxy.host);
        let core_profile = NaiveProxyCoreProfile {
            source_format: imported.source_format,
            config: imported.config,
        };
        let core_config = serde_json::to_value(core_profile)
            .map_err(|error| AdapterError::Import(error.to_string()))?;
        let mut profile = Profile::new(
            ProfileId::new(format!("naiveproxy:{}", stable_slug(&name))),
            name,
            CoreId::from("naiveproxy"),
            core_config,
        );
        profile
            .ui_metadata
            .insert("import_reason".to_string(), detection.reason);

        let mut parsed = ParsedProfile::new(profile);
        if !imported.secrets.proxy_password.is_empty() {
            parsed.secrets.insert(
                PROXY_PASSWORD_KEY.to_string(),
                imported.secrets.proxy_password,
            );
        }
        if !imported.secrets.listen_password.is_empty() {
            parsed.secrets.insert(
                LISTEN_PASSWORD_KEY.to_string(),
                imported.secrets.listen_password,
            );
        }
        parsed.warnings = self.validate(&parsed.profile)?;
        parsed.warnings.extend(imported.warnings);

        Ok(parsed)
    }

    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema {
            sections: vec![
                SettingsSection {
                    key: "proxy".to_string(),
                    title: "Proxy server".to_string(),
                    fields: vec![
                        field(
                            "proxy.scheme",
                            "Proxy protocol",
                            true,
                            SettingsFieldKind::Select {
                                options: vec![
                                    "https".to_string(),
                                    "quic".to_string(),
                                    "http".to_string(),
                                    "socks".to_string(),
                                ],
                            },
                        ),
                        field("proxy.host", "Proxy host", true, SettingsFieldKind::Text),
                        field(
                            "proxy.port",
                            "Proxy port",
                            false,
                            SettingsFieldKind::Integer {
                                min: Some(1),
                                max: Some(65535),
                            },
                        ),
                        field("proxy.username", "Username", false, SettingsFieldKind::Text),
                        field(
                            "proxy.password",
                            "Password",
                            false,
                            SettingsFieldKind::Secret,
                        ),
                    ],
                },
                SettingsSection {
                    key: "listen".to_string(),
                    title: "Local listener".to_string(),
                    fields: vec![
                        field(
                            "listen.scheme",
                            "Listener protocol",
                            true,
                            SettingsFieldKind::Select {
                                options: vec![
                                    "socks".to_string(),
                                    "http".to_string(),
                                    "redir".to_string(),
                                ],
                            },
                        ),
                        field("listen.host", "Listen host", true, SettingsFieldKind::Text),
                        field(
                            "listen.port",
                            "Listen port",
                            true,
                            SettingsFieldKind::Integer {
                                min: Some(1),
                                max: Some(65535),
                            },
                        ),
                        field(
                            "listen.username",
                            "Local username",
                            false,
                            SettingsFieldKind::Text,
                        ),
                        field(
                            "listen.password",
                            "Local password",
                            false,
                            SettingsFieldKind::Secret,
                        ),
                    ],
                },
                SettingsSection {
                    key: "advanced".to_string(),
                    title: "Advanced".to_string(),
                    fields: vec![
                        field(
                            "extra_headers",
                            "Extra headers",
                            false,
                            SettingsFieldKind::Multiline,
                        ),
                        field(
                            "host_resolver_rules",
                            "Host resolver rules",
                            false,
                            SettingsFieldKind::Text,
                        ),
                        field(
                            "insecure_concurrency",
                            "Insecure concurrency",
                            false,
                            SettingsFieldKind::Integer {
                                min: Some(1),
                                max: Some(64),
                            },
                        ),
                        field(
                            "no_post_quantum",
                            "Disable post-quantum",
                            false,
                            SettingsFieldKind::Boolean,
                        ),
                    ],
                },
            ],
        }
    }

    fn validate(&self, profile: &Profile) -> Result<Vec<ValidationWarning>, AdapterError> {
        if profile.core_id != CoreId::from("naiveproxy") {
            return Err(AdapterError::InvalidProfile(format!(
                "expected naiveproxy profile, got {}",
                profile.core_id
            )));
        }

        let core_profile =
            serde_json::from_value::<NaiveProxyCoreProfile>(profile.core_config.clone())
                .map_err(|error| AdapterError::InvalidProfile(error.to_string()))?;
        let config = core_profile.config;
        let mut warnings = Vec::new();

        ensure_scheme(
            &config.listen.scheme,
            &["socks", "http", "redir"],
            "listen.scheme",
        )?;
        ensure_scheme(
            &config.proxy.scheme,
            &["http", "https", "quic", "socks"],
            "proxy.scheme",
        )?;
        if config.proxy.host.trim().is_empty() {
            return Err(AdapterError::InvalidProfile(
                "proxy host is required".to_string(),
            ));
        }
        if config.listen.host.trim().is_empty() {
            return Err(AdapterError::InvalidProfile(
                "listen host is required".to_string(),
            ));
        }

        if !is_loopback_host(&config.listen.host) {
            warnings.push(ValidationWarning {
                field: "listen.host".to_string(),
                message: "High risk: local listener accepts LAN connections.".to_string(),
            });
        }
        if config.proxy.scheme == "http" {
            warnings.push(ValidationWarning {
                field: "proxy.scheme".to_string(),
                message: "High risk: upstream proxy does not use TLS.".to_string(),
            });
        }
        if config.insecure_concurrency.is_some() {
            warnings.push(ValidationWarning {
                field: "insecure_concurrency".to_string(),
                message: "NaiveProxy marks this option as insecure; use only when necessary."
                    .to_string(),
            });
        }
        if config.no_post_quantum {
            warnings.push(ValidationWarning {
                field: "no_post_quantum".to_string(),
                message: "Post-quantum key agreement is disabled for this profile.".to_string(),
            });
        }

        Ok(warnings)
    }

    fn build_runtime_config(&self, profile: &Profile) -> Result<RuntimeConfig, AdapterError> {
        self.validate(profile)?;
        let core_profile =
            serde_json::from_value::<NaiveProxyCoreProfile>(profile.core_config.clone())
                .map_err(|error| AdapterError::RuntimeConfig(error.to_string()))?;
        let config = core_profile.config;
        let mut output = Map::new();
        output.insert(
            "listen".to_string(),
            Value::String(build_endpoint_url(&config.listen, LISTEN_PASSWORD_KEY)),
        );
        output.insert(
            "proxy".to_string(),
            Value::String(build_endpoint_url(&config.proxy, PROXY_PASSWORD_KEY)),
        );
        insert_string_array(&mut output, "extra-headers", &config.extra_headers);
        insert_string(
            &mut output,
            "host-resolver-rules",
            &config.host_resolver_rules,
        );
        insert_string(&mut output, "resolver-range", &config.resolver_range);
        insert_string(&mut output, "tunnel-timeout", &config.tunnel_timeout);
        insert_string(&mut output, "idle-timeout", &config.idle_timeout);
        if let Some(value) = config.insecure_concurrency {
            output.insert("insecure-concurrency".to_string(), json!(value));
        }
        if config.no_post_quantum {
            output.insert("no-post-quantum".to_string(), Value::Bool(true));
        }

        let runtime_config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "config.json".to_string(),
                content: serde_json::to_string_pretty(&Value::Object(output))
                    .map_err(|error| AdapterError::RuntimeConfig(error.to_string()))?,
                sensitive: true,
            }],
            command_args: vec!["config.json".to_string()],
            environment: BTreeMap::new(),
        };
        RuntimeSecurityPolicy::default()
            .validate_config(&runtime_config)
            .map_err(|error| AdapterError::Security(error.to_string()))?;

        Ok(runtime_config)
    }
}

struct NaiveImport {
    source_format: String,
    config: NaiveProxyConfig,
    secrets: NaiveProxySecretCandidate,
    warnings: Vec<ValidationWarning>,
}

fn parse_config_json(content: &str) -> Result<NaiveImport, AdapterError> {
    let value = serde_json::from_str::<Value>(content)
        .map_err(|error| AdapterError::Import(error.to_string()))?;
    let object = value.as_object().ok_or_else(|| {
        AdapterError::Import("NaiveProxy config must be a JSON object".to_string())
    })?;
    let listen = string_field(object, "listen")?;
    let proxy = string_field(object, "proxy")?;
    let (listen, listen_password) = parse_endpoint_url(
        listen,
        &["socks", "http", "redir"],
        LISTEN_PASSWORD_KEY,
        "listen",
    )?;
    let (proxy, proxy_password) = parse_endpoint_url(
        proxy,
        &["http", "https", "quic", "socks"],
        PROXY_PASSWORD_KEY,
        "proxy",
    )?;
    let warnings = unsupported_debug_warnings(object);

    Ok(NaiveImport {
        source_format: "config.json".to_string(),
        config: NaiveProxyConfig {
            listen,
            proxy,
            extra_headers: string_array_field(object, "extra-headers"),
            host_resolver_rules: optional_string_field(object, "host-resolver-rules"),
            resolver_range: optional_string_field(object, "resolver-range"),
            tunnel_timeout: optional_string_field(object, "tunnel-timeout"),
            idle_timeout: optional_string_field(object, "idle-timeout"),
            insecure_concurrency: optional_u32_field(object, "insecure-concurrency"),
            no_post_quantum: optional_bool_field(object, "no-post-quantum"),
        },
        secrets: NaiveProxySecretCandidate {
            proxy_password,
            listen_password,
        },
        warnings,
    })
}

fn parse_proxy_url_profile(content: &str) -> Result<NaiveImport, AdapterError> {
    let (proxy, proxy_password) = parse_endpoint_url(
        content,
        &["http", "https", "quic", "socks"],
        PROXY_PASSWORD_KEY,
        "proxy",
    )?;
    let (listen, listen_password) = parse_endpoint_url(
        DEFAULT_LISTEN_URL,
        &["socks", "http", "redir"],
        LISTEN_PASSWORD_KEY,
        "listen",
    )?;

    Ok(NaiveImport {
        source_format: "proxy_url".to_string(),
        config: NaiveProxyConfig {
            listen,
            proxy,
            ..NaiveProxyConfig::default()
        },
        secrets: NaiveProxySecretCandidate {
            proxy_password,
            listen_password,
        },
        warnings: Vec::new(),
    })
}

fn parse_endpoint_url(
    value: &str,
    allowed_schemes: &[&str],
    secret_key: &str,
    field_name: &str,
) -> Result<(NaiveProxyEndpoint, String), AdapterError> {
    let url = Url::parse(value.trim())
        .map_err(|error| AdapterError::Import(format!("{field_name} URL is invalid: {error}")))?;
    let scheme = url.scheme().to_ascii_lowercase();
    ensure_scheme(&scheme, allowed_schemes, field_name)?;
    if url.fragment().is_some() {
        return Err(AdapterError::Import(format!(
            "{field_name} URL fragments are not supported"
        )));
    }

    let host = url
        .host_str()
        .ok_or_else(|| AdapterError::Import(format!("{field_name} URL host is required")))?
        .to_string();
    let username = decode_userinfo(url.username());
    let password = url
        .password()
        .map(|password| encode_userinfo(&decode_userinfo(password)))
        .unwrap_or_default();
    let path = match url.path() {
        "" | "/" => String::new(),
        path => path.to_string(),
    };
    let query = url.query().unwrap_or_default().to_string();
    let endpoint = NaiveProxyEndpoint {
        scheme,
        host,
        port: url.port(),
        username,
        password_secret_ref: if password.is_empty() {
            String::new()
        } else {
            secret_key.to_string()
        },
        path,
        query,
    };

    Ok((endpoint, password))
}

fn json_contains_listen_and_proxy(content: &str) -> bool {
    serde_json::from_str::<Value>(content)
        .ok()
        .and_then(|value| {
            let object = value.as_object()?;
            Some(object.get("listen")?.is_string() && object.get("proxy")?.is_string())
        })
        .unwrap_or(false)
}

fn looks_like_proxy_url(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    ["https://", "http://", "quic://", "socks://"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        && content.contains('@')
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, AdapterError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| AdapterError::Import(format!("{key} must be a string")))
}

fn optional_string_field(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn string_array_field(object: &Map<String, Value>, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_u32_field(object: &Map<String, Value>, key: &str) -> Option<u32> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn optional_bool_field(object: &Map<String, Value>, key: &str) -> bool {
    object.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn unsupported_debug_warnings(object: &Map<String, Value>) -> Vec<ValidationWarning> {
    ["ssl-key-log-file", "log-net-log"]
        .iter()
        .filter(|key| object.contains_key(**key))
        .map(|key| ValidationWarning {
            field: (*key).to_string(),
            message: "Sensitive debug output is intentionally not imported.".to_string(),
        })
        .collect()
}

fn insert_string(output: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.trim().is_empty() {
        output.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn insert_string_array(output: &mut Map<String, Value>, key: &str, value: &[String]) {
    if !value.is_empty() {
        output.insert(key.to_string(), json!(value));
    }
}

fn build_endpoint_url(endpoint: &NaiveProxyEndpoint, secret_key: &str) -> String {
    let mut output = String::new();
    output.push_str(&endpoint.scheme);
    output.push_str("://");
    if !endpoint.username.is_empty() || !endpoint.password_secret_ref.is_empty() {
        output.push_str(&encode_userinfo(&endpoint.username));
        if !endpoint.password_secret_ref.is_empty() {
            output.push(':');
            output.push('<');
            output.push_str(secret_key);
            output.push('>');
        }
        output.push('@');
    }
    output.push_str(&format_host(&endpoint.host));
    if let Some(port) = endpoint.port {
        output.push(':');
        output.push_str(&port.to_string());
    }
    output.push_str(&endpoint.path);
    if !endpoint.query.is_empty() {
        output.push('?');
        output.push_str(&endpoint.query);
    }

    output
}

fn ensure_scheme(scheme: &str, allowed_schemes: &[&str], field: &str) -> Result<(), AdapterError> {
    if allowed_schemes
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    {
        Ok(())
    } else {
        Err(AdapterError::InvalidProfile(format!(
            "{field} uses unsupported scheme: {scheme}"
        )))
    }
}

fn encode_userinfo(value: &str) -> String {
    utf8_percent_encode(value, USERINFO_ENCODE_SET).to_string()
}

fn decode_userinfo(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

fn format_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim().trim_matches(['[', ']']);
    host.eq_ignore_ascii_case("localhost") || host == "::1" || host.starts_with("127.")
}

fn field(
    key: impl Into<String>,
    title: impl Into<String>,
    required: bool,
    kind: SettingsFieldKind,
) -> SettingsField {
    SettingsField {
        key: key.into(),
        title: title.into(),
        description: None,
        required,
        kind,
    }
}

fn stable_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if slug.is_empty() {
        "profile".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CoreRegistry;

    #[test]
    fn detects_naiveproxy_json_config() {
        let adapter = NaiveProxyAdapter::new();
        let detected = adapter.detect_import(&ImportInput::text(
            r#"{"listen":"socks://127.0.0.1:1080","proxy":"https://u:p@example.com"}"#,
        ));

        assert_eq!(detected.confidence, 95);
    }

    #[test]
    fn parses_json_without_storing_plaintext_proxy_password() {
        let adapter = NaiveProxyAdapter::new();
        let parsed = adapter
            .parse_profile(&ImportInput::text(
                r#"{
                  "listen": "socks://127.0.0.1:1080",
                  "proxy": "https://user:p%40ss%3Aword@example.com:443",
                  "insecure-concurrency": 2,
                  "no-post-quantum": true
                }"#,
            ))
            .expect("config should parse");
        let core_config = serde_json::to_string(&parsed.profile.core_config).unwrap();

        assert_eq!(parsed.profile.core_id, CoreId::from("naiveproxy"));
        assert_eq!(
            parsed.secrets.get(PROXY_PASSWORD_KEY),
            Some(&"p%40ss%3Aword".to_string())
        );
        assert!(!core_config.contains("p@ss:word"));
        assert!(!core_config.contains("p%40ss%3Aword"));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.field == "insecure_concurrency"));
    }

    #[test]
    fn parses_bare_proxy_url_with_default_listener() {
        let adapter = NaiveProxyAdapter::new();
        let parsed = adapter
            .parse_profile(&ImportInput::text(
                "quic://naive%20user:p%40ss%3Aword@example.com:443",
            ))
            .expect("proxy URL should parse");
        let core_profile =
            serde_json::from_value::<NaiveProxyCoreProfile>(parsed.profile.core_config).unwrap();

        assert_eq!(core_profile.config.listen.scheme, "socks");
        assert_eq!(core_profile.config.listen.host, "127.0.0.1");
        assert_eq!(core_profile.config.proxy.username, "naive user");
        assert_eq!(
            parsed.secrets.get(PROXY_PASSWORD_KEY),
            Some(&"p%40ss%3Aword".to_string())
        );
    }

    #[test]
    fn builds_config_json_with_secret_placeholder() {
        let adapter = NaiveProxyAdapter::new();
        let parsed = adapter
            .parse_profile(&ImportInput::text(
                "https://naive%20user:p%40ss%3Aword@example.com:8443",
            ))
            .expect("proxy URL should parse");
        let runtime_config = adapter.build_runtime_config(&parsed.profile).unwrap();

        assert_eq!(runtime_config.command_args, vec!["config.json".to_string()]);
        assert!(runtime_config.files[0].content.contains("<proxy.password>"));
        assert!(runtime_config.files[0]
            .content
            .contains("https://naive%20user:<proxy.password>@example.com:8443"));
    }

    #[test]
    fn registry_returns_naiveproxy_for_config_json() {
        let mut registry = CoreRegistry::new();
        registry.register(NaiveProxyAdapter::new());

        let matches = registry.detect_import(&ImportInput::text(
            r#"{"listen":"socks://127.0.0.1:1080","proxy":"https://u:p@example.com"}"#,
        ));

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].core_id, CoreId::from("naiveproxy"));
    }
}
