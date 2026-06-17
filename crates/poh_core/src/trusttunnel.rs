use std::collections::BTreeMap;

use crate::adapter::{
    AdapterError, CoreAdapter, ImportInput, ImportMatch, ParsedProfile, RuntimeConfig, RuntimeFile,
    SettingsField, SettingsFieldKind, SettingsSchema, SettingsSection, ValidationWarning,
};
use crate::model::{CoreCapabilities, CoreId, CoreManifest, Profile, ProfileId, ProtocolColor};
use crate::security::RuntimeSecurityPolicy;
use crate::trusttunnel_config::TrustTunnelCoreProfile;
use crate::trusttunnel_deeplink::TrustTunnelDeeplinkParser;
use crate::trusttunnel_toml::{TrustTunnelTomlBuilder, TrustTunnelTomlParser};

pub struct TrustTunnelAdapter {
    manifest: CoreManifest,
}

impl Default for TrustTunnelAdapter {
    fn default() -> Self {
        Self {
            manifest: CoreManifest {
                id: CoreId::from("trusttunnel"),
                display_name: "TrustTunnel".to_string(),
                summary: "Official TrustTunnel / vpn_easy core adapter".to_string(),
                homepage: None,
                license: None,
                protocol_colors: vec![
                    ProtocolColor {
                        protocol: "http2".to_string(),
                        color: "#23885A".to_string(),
                    },
                    ProtocolColor {
                        protocol: "http3".to_string(),
                        color: "#2F6DF2".to_string(),
                    },
                ],
                capabilities: CoreCapabilities {
                    can_import: true,
                    can_download: false,
                    supports_tun: true,
                    supports_socks: true,
                    supports_http_proxy: true,
                    supports_system_proxy: true,
                    supports_config_test: true,
                },
            },
        }
    }
}

impl TrustTunnelAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CoreAdapter for TrustTunnelAdapter {
    fn manifest(&self) -> &CoreManifest {
        &self.manifest
    }

    fn detect_import(&self, input: &ImportInput) -> ImportMatch {
        let content = input.content.trim();
        if content.starts_with("tt://") {
            return ImportMatch::new(100, "TrustTunnel deeplink");
        }

        let lower = content.to_ascii_lowercase();
        if lower.contains("[endpoint]") && lower.contains("hostname") {
            return ImportMatch::new(90, "TrustTunnel TOML endpoint config");
        }

        if lower.contains("upstream_protocol") || lower.contains("[listener.tun]") {
            return ImportMatch::new(60, "Possible TrustTunnel TOML config");
        }

        ImportMatch::none("No TrustTunnel markers found")
    }

    fn parse_profile(&self, input: &ImportInput) -> Result<ParsedProfile, AdapterError> {
        let detection = self.detect_import(input);
        if !detection.matched() {
            return Err(AdapterError::UnsupportedInput);
        }

        let content = input.content.trim();
        let (display_name, source_format, config, secrets) = if content.starts_with("tt://") {
            let (config, secrets, display_name) = TrustTunnelDeeplinkParser
                .parse(content)
                .map_err(|error| AdapterError::Import(error.to_string()))?;
            (display_name, "deeplink".to_string(), config, secrets)
        } else {
            let (config, secrets) = TrustTunnelTomlParser.parse(content);
            (
                config.endpoint.hostname.clone(),
                "toml".to_string(),
                config,
                secrets,
            )
        };

        let name = if display_name.trim().is_empty() {
            "TrustTunnel profile".to_string()
        } else {
            display_name
        };
        let core_profile = TrustTunnelCoreProfile {
            source_format,
            config,
        };
        let core_config = serde_json::to_value(core_profile)
            .map_err(|error| AdapterError::Import(error.to_string()))?;
        let mut profile = Profile::new(
            ProfileId::new(format!("trusttunnel:{}", stable_slug(&name))),
            name.clone(),
            CoreId::from("trusttunnel"),
            core_config,
        );

        profile
            .ui_metadata
            .insert("import_reason".to_string(), detection.reason);

        let mut parsed = ParsedProfile::new(profile);
        if !secrets.password.is_empty() {
            parsed
                .secrets
                .insert("endpoint.password".to_string(), secrets.password);
        }

        if !secrets.client_random.is_empty() {
            parsed
                .secrets
                .insert("endpoint.client_random".to_string(), secrets.client_random);
        }

        if !secrets.socks_password.is_empty() {
            parsed.secrets.insert(
                "listener.socks.password".to_string(),
                secrets.socks_password,
            );
        }
        parsed.warnings = self.validate(&parsed.profile)?;

        Ok(parsed)
    }

    fn settings_schema(&self) -> SettingsSchema {
        SettingsSchema {
            sections: vec![
                SettingsSection {
                    key: "endpoint".to_string(),
                    title: "Endpoint".to_string(),
                    fields: vec![
                        field("hostname", "Hostname / SNI", true, SettingsFieldKind::Text),
                        field("addresses", "Addresses", true, SettingsFieldKind::Multiline),
                        field("username", "Username", true, SettingsFieldKind::Text),
                        field("password", "Password", true, SettingsFieldKind::Secret),
                        field(
                            "upstream_protocol",
                            "Upstream protocol",
                            true,
                            SettingsFieldKind::Select {
                                options: vec!["http2".to_string(), "http3".to_string()],
                            },
                        ),
                    ],
                },
                SettingsSection {
                    key: "listener".to_string(),
                    title: "Listeners".to_string(),
                    fields: vec![
                        field("tun_enabled", "TUN mode", false, SettingsFieldKind::Boolean),
                        field(
                            "socks_address",
                            "SOCKS5 address",
                            false,
                            SettingsFieldKind::Text,
                        ),
                        field(
                            "socks_username",
                            "SOCKS5 username",
                            false,
                            SettingsFieldKind::Text,
                        ),
                        field(
                            "socks_password",
                            "SOCKS5 password",
                            false,
                            SettingsFieldKind::Secret,
                        ),
                        field(
                            "http_address",
                            "HTTP proxy address",
                            false,
                            SettingsFieldKind::Text,
                        ),
                    ],
                },
                SettingsSection {
                    key: "routing".to_string(),
                    title: "Routing / DNS".to_string(),
                    fields: vec![
                        field(
                            "vpn_mode",
                            "Routing mode",
                            false,
                            SettingsFieldKind::Select {
                                options: vec![
                                    "general".to_string(),
                                    "whitelist".to_string(),
                                    "blacklist".to_string(),
                                ],
                            },
                        ),
                        field(
                            "dns_upstreams",
                            "DNS upstreams",
                            false,
                            SettingsFieldKind::Multiline,
                        ),
                        field(
                            "included_routes",
                            "Included routes",
                            false,
                            SettingsFieldKind::Multiline,
                        ),
                        field(
                            "excluded_routes",
                            "Excluded routes",
                            false,
                            SettingsFieldKind::Multiline,
                        ),
                    ],
                },
            ],
        }
    }

    fn validate(&self, profile: &Profile) -> Result<Vec<ValidationWarning>, AdapterError> {
        if profile.core_id != CoreId::from("trusttunnel") {
            return Err(AdapterError::InvalidProfile(format!(
                "expected trusttunnel profile, got {}",
                profile.core_id
            )));
        }

        let core_profile =
            serde_json::from_value::<TrustTunnelCoreProfile>(profile.core_config.clone())
                .map_err(|error| AdapterError::InvalidProfile(error.to_string()))?;
        let config = core_profile.config;
        let mut warnings = Vec::new();

        if config.endpoint.skip_verification {
            warnings.push(ValidationWarning {
                field: "endpoint.skip_verification".to_string(),
                message: "High risk: TLS certificate verification is disabled.".to_string(),
            });
        }

        if !config.endpoint.certificate_pem.trim().is_empty() {
            warnings.push(ValidationWarning {
                field: "endpoint.certificate".to_string(),
                message: "High risk: profile ships a custom TLS certificate.".to_string(),
            });
        }

        if config.listener.socks.allow_lan_access {
            warnings.push(ValidationWarning {
                field: "listener.socks.allow_lan_access".to_string(),
                message: "High risk: SOCKS listener accepts LAN connections.".to_string(),
            });
        }

        if !config.listener.socks.address.trim().is_empty()
            && !is_loopback_listener(&config.listener.socks.address)
        {
            warnings.push(ValidationWarning {
                field: "listener.socks.address".to_string(),
                message: "High risk: SOCKS listener is not bound to loopback.".to_string(),
            });
        }

        if !config.listener.socks.http_proxy_address.trim().is_empty()
            && !is_loopback_listener(&config.listener.socks.http_proxy_address)
        {
            warnings.push(ValidationWarning {
                field: "listener.socks.http_proxy_address".to_string(),
                message: "High risk: HTTP proxy listener is not bound to loopback.".to_string(),
            });
        }

        if config.listener.socks.http_proxy_allow_lan_access {
            warnings.push(ValidationWarning {
                field: "listener.socks.http_proxy_allow_lan_access".to_string(),
                message: "High risk: HTTP proxy accepts LAN connections.".to_string(),
            });
        }

        Ok(warnings)
    }

    fn build_runtime_config(&self, profile: &Profile) -> Result<RuntimeConfig, AdapterError> {
        self.validate(profile)?;
        let core_profile =
            serde_json::from_value::<TrustTunnelCoreProfile>(profile.core_config.clone())
                .map_err(|error| AdapterError::RuntimeConfig(error.to_string()))?;
        let toml = TrustTunnelTomlBuilder.build(
            &core_profile.config,
            "<endpoint.password>",
            "<endpoint.client_random>",
            "<listener.socks.password>",
        );

        let runtime_config = RuntimeConfig {
            files: vec![RuntimeFile {
                relative_path: "config.toml".to_string(),
                content: toml,
                sensitive: true,
            }],
            command_args: vec!["--config".to_string(), "config.toml".to_string()],
            environment: BTreeMap::new(),
        };
        RuntimeSecurityPolicy::default()
            .validate_config(&runtime_config)
            .map_err(|error| AdapterError::Security(error.to_string()))?;

        Ok(runtime_config)
    }
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

fn is_loopback_listener(address: &str) -> bool {
    let host = listener_host(address);
    host.eq_ignore_ascii_case("localhost")
        || host == "::1"
        || host == "[::1]"
        || host.starts_with("127.")
}

fn listener_host(address: &str) -> &str {
    let trimmed = address.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((host, _)) = rest.split_once(']') {
            return host;
        }
    }

    trimmed
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CoreRegistry;

    #[test]
    fn detects_trusttunnel_deeplink() {
        let adapter = TrustTunnelAdapter::new();
        let detected = adapter.detect_import(&ImportInput::text("tt://?abc"));

        assert_eq!(detected.confidence, 100);
    }

    #[test]
    fn parses_trusttunnel_toml_profile() {
        let adapter = TrustTunnelAdapter::new();
        let profile = adapter
            .parse_profile(&ImportInput::text(
                r#"
                loglevel = "info"

                [endpoint]
                hostname = "tt.hel2.mumuru.ru"
                upstream_protocol = "http2"
                "#,
            ))
            .expect("TOML should be recognized");

        assert_eq!(profile.profile.core_id, CoreId::from("trusttunnel"));
        assert_eq!(profile.profile.name, "tt.hel2.mumuru.ru");
        assert_eq!(profile.profile.core_config["source_format"], "toml");
    }

    #[test]
    fn validates_insecure_tls_and_lan_listener() {
        let adapter = TrustTunnelAdapter::new();
        let parsed = adapter
            .parse_profile(&ImportInput::text(
                r#"
                [endpoint]
                hostname = "tt.example.test"
                skip_verification = true

                [listener.socks]
                address = "0.0.0.0:1080"
                allow_lan_access = true
                http_proxy_address = "0.0.0.0:8080"
                http_proxy_allow_lan_access = true
                "#,
            ))
            .expect("profile should parse");

        let fields = parsed
            .warnings
            .iter()
            .map(|warning| warning.field.as_str())
            .collect::<Vec<_>>();

        assert!(fields.contains(&"endpoint.skip_verification"));
        assert!(fields.contains(&"listener.socks.address"));
        assert!(fields.contains(&"listener.socks.allow_lan_access"));
        assert!(fields.contains(&"listener.socks.http_proxy_address"));
        assert!(fields.contains(&"listener.socks.http_proxy_allow_lan_access"));
    }

    #[test]
    fn registry_returns_matching_core_first() {
        let mut registry = CoreRegistry::new();
        registry.register(TrustTunnelAdapter::new());

        let matches = registry.detect_import(&ImportInput::text("tt://?abc"));

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].core_id, CoreId::from("trusttunnel"));
    }
}
