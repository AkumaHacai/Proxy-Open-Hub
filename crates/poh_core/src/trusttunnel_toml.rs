use std::collections::BTreeMap;

use crate::trusttunnel_config::{
    EndpointConfig, ListenerConfig, ListenerMode, LogLevel, RoutingMode, RoutingProfile,
    SecretCandidate, SocksConfig, TrustTunnelConfig, TunConfig, UpstreamProtocol,
};

pub struct TrustTunnelTomlParser;

impl TrustTunnelTomlParser {
    pub fn parse(&self, toml: &str) -> (TrustTunnelConfig, SecretCandidate) {
        let values = parse_flat(toml);
        let endpoint = EndpointConfig {
            hostname: get(&values, "endpoint.hostname", ""),
            custom_sni: get(&values, "endpoint.custom_sni", ""),
            addresses: get_array(&values, "endpoint.addresses", &[]),
            has_ipv6: get_bool(&values, "endpoint.has_ipv6", true),
            username: get(&values, "endpoint.username", ""),
            password_secret_ref: String::new(),
            client_random_secret_ref: String::new(),
            skip_verification: get_bool(&values, "endpoint.skip_verification", false),
            certificate_pem: get(&values, "endpoint.certificate", ""),
            upstream_protocol: parse_upstream(&get(&values, "endpoint.upstream_protocol", "http2")),
            fallback_protocol: match get(&values, "endpoint.upstream_fallback_protocol", "")
                .as_str()
            {
                "http2" => Some(UpstreamProtocol::Http2),
                "http3" => Some(UpstreamProtocol::Http3),
                _ => None,
            },
            anti_dpi: get_bool(&values, "endpoint.anti_dpi", false),
            post_quantum_group_enabled: get_bool(&values, "post_quantum_group_enabled", true),
            dns_upstreams: get_array_fallback(
                &values,
                "endpoint.dns_upstreams",
                "dns_upstreams",
                &[],
            ),
        };

        let listener_mode = if values.contains_key("listener.socks.address") {
            ListenerMode::Socks
        } else {
            ListenerMode::Tun
        };
        let listener = ListenerConfig {
            mode: listener_mode,
            tun: TunConfig {
                bound_if: get(&values, "listener.tun.bound_if", ""),
                included_routes: get_array(
                    &values,
                    "listener.tun.included_routes",
                    &["0.0.0.0/0", "2000::/3"],
                ),
                excluded_routes: get_array(
                    &values,
                    "listener.tun.excluded_routes",
                    &[
                        "0.0.0.0/8",
                        "10.0.0.0/8",
                        "169.254.0.0/16",
                        "172.16.0.0/12",
                        "192.168.0.0/16",
                        "224.0.0.0/3",
                    ],
                ),
                mtu_size: get_i32(&values, "listener.tun.mtu_size", 1280),
                tcp_recv_buf_size: get_i32(&values, "listener.tun.tcp_recv_buf_size", 0),
                tcp_send_buf_size: get_i32(&values, "listener.tun.tcp_send_buf_size", 0),
                change_system_dns: get_bool(&values, "listener.tun.change_system_dns", true),
                device_name: get(&values, "listener.tun.device_name", ""),
                use_existing: get_bool(&values, "listener.tun.use_existing", false),
            },
            socks: SocksConfig {
                address: get(&values, "listener.socks.address", "127.0.0.1:1080"),
                username: get(&values, "listener.socks.username", ""),
                password_secret_ref: String::new(),
                allow_lan_access: get_bool(&values, "listener.socks.allow_lan_access", false),
                http_proxy_address: get(&values, "listener.socks.http_proxy_address", ""),
                http_proxy_allow_lan_access: get_bool(
                    &values,
                    "listener.socks.http_proxy_allow_lan_access",
                    false,
                ),
            },
        };

        let routing = RoutingProfile {
            mode: match get(&values, "vpn_mode", "").as_str() {
                "selective" => RoutingMode::Selective,
                _ => RoutingMode::General,
            },
            kill_switch_enabled: get_bool(&values, "killswitch_enabled", true),
            kill_switch_allow_ports: get_i32_array(&values, "killswitch_allow_ports"),
            exclusions: get_array(&values, "exclusions", &[]),
            ..RoutingProfile::default()
        };

        (
            TrustTunnelConfig {
                log_level: parse_log_level(&get(&values, "loglevel", "info")),
                routing,
                endpoint,
                listener,
            },
            SecretCandidate {
                password: get(&values, "endpoint.password", ""),
                client_random: get(&values, "endpoint.client_random", ""),
                socks_password: get(&values, "listener.socks.password", ""),
            },
        )
    }
}

pub struct TrustTunnelTomlBuilder;

impl TrustTunnelTomlBuilder {
    pub fn build(
        &self,
        config: &TrustTunnelConfig,
        endpoint_password: &str,
        client_random: &str,
        socks_password: &str,
    ) -> String {
        let mut output = String::new();
        line_string(&mut output, "loglevel", log_level(config.log_level));
        line_string(&mut output, "vpn_mode", routing_mode(config.routing.mode));
        line_bool(
            &mut output,
            "killswitch_enabled",
            config.routing.kill_switch_enabled,
        );
        line_i32_array(
            &mut output,
            "killswitch_allow_ports",
            &config.routing.kill_switch_allow_ports,
        );
        line_bool(
            &mut output,
            "post_quantum_group_enabled",
            config.endpoint.post_quantum_group_enabled,
        );
        line_string_array(&mut output, "exclusions", &config.routing.exclusions);

        output.push_str("\n[endpoint]\n");
        line_string(&mut output, "hostname", &config.endpoint.hostname);
        if !config.endpoint.custom_sni.trim().is_empty() {
            line_string(&mut output, "custom_sni", &config.endpoint.custom_sni);
        }

        line_string_array(&mut output, "addresses", &config.endpoint.addresses);
        line_bool(&mut output, "has_ipv6", config.endpoint.has_ipv6);
        line_string(&mut output, "username", &config.endpoint.username);
        line_string(&mut output, "password", endpoint_password);
        line_string(&mut output, "client_random", client_random);
        line_bool(
            &mut output,
            "skip_verification",
            config.endpoint.skip_verification,
        );
        line_string(&mut output, "certificate", &config.endpoint.certificate_pem);
        line_string_array(&mut output, "dns_upstreams", &config.endpoint.dns_upstreams);
        line_string(
            &mut output,
            "upstream_protocol",
            config.endpoint.upstream_protocol.as_toml(),
        );
        if let Some(fallback) = config.endpoint.fallback_protocol {
            line_string(
                &mut output,
                "upstream_fallback_protocol",
                fallback.as_toml(),
            );
        }

        line_bool(&mut output, "anti_dpi", config.endpoint.anti_dpi);

        output.push('\n');
        match config.listener.mode {
            ListenerMode::Tun => {
                output.push_str("[listener.tun]\n");
                line_string(&mut output, "bound_if", &config.listener.tun.bound_if);
                line_string_array(
                    &mut output,
                    "included_routes",
                    &config.listener.tun.included_routes,
                );
                line_string_array(
                    &mut output,
                    "excluded_routes",
                    &config.listener.tun.excluded_routes,
                );
                line_i32(&mut output, "mtu_size", config.listener.tun.mtu_size);
                line_i32(
                    &mut output,
                    "tcp_recv_buf_size",
                    config.listener.tun.tcp_recv_buf_size,
                );
                line_i32(
                    &mut output,
                    "tcp_send_buf_size",
                    config.listener.tun.tcp_send_buf_size,
                );
                line_bool(
                    &mut output,
                    "change_system_dns",
                    config.listener.tun.change_system_dns,
                );
                line_string(&mut output, "device_name", &config.listener.tun.device_name);
                line_bool(
                    &mut output,
                    "use_existing",
                    config.listener.tun.use_existing,
                );
            }
            ListenerMode::Socks => {
                output.push_str("[listener.socks]\n");
                line_string(&mut output, "address", &config.listener.socks.address);
                if !config.listener.socks.username.trim().is_empty() {
                    line_string(&mut output, "username", &config.listener.socks.username);
                }

                if !socks_password.trim().is_empty() {
                    line_string(&mut output, "password", socks_password);
                }

                if config.listener.socks.allow_lan_access {
                    line_bool(&mut output, "allow_lan_access", true);
                }

                if !config.listener.socks.http_proxy_address.trim().is_empty() {
                    line_string(
                        &mut output,
                        "http_proxy_address",
                        &config.listener.socks.http_proxy_address,
                    );
                    line_bool(
                        &mut output,
                        "http_proxy_allow_lan_access",
                        config.listener.socks.http_proxy_allow_lan_access,
                    );
                }
            }
        }

        output
    }
}

fn parse_flat(toml: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut section = String::new();
    for raw in toml.lines() {
        let line = strip_comment(raw.trim_start_matches('\u{feff}'))
            .trim()
            .to_string();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let full_key = if section.trim().is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", section, key)
        };
        values.insert(full_key.to_ascii_lowercase(), value.trim().to_string());
    }

    values
}

fn strip_comment(line: &str) -> String {
    let mut in_string = false;
    let mut previous = '\0';
    for (index, ch) in line.char_indices() {
        if ch == '"' && previous != '\\' {
            in_string = !in_string;
        }

        if !in_string && ch == '#' {
            return line[..index].to_string();
        }

        previous = ch;
    }

    line.to_string()
}

fn get(values: &BTreeMap<String, String>, key: &str, fallback: &str) -> String {
    values
        .get(&key.to_ascii_lowercase())
        .map(|value| unquote(value))
        .unwrap_or_else(|| fallback.to_string())
}

fn get_i32(values: &BTreeMap<String, String>, key: &str, fallback: i32) -> i32 {
    values
        .get(&key.to_ascii_lowercase())
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(fallback)
}

fn get_bool(values: &BTreeMap<String, String>, key: &str, fallback: bool) -> bool {
    values
        .get(&key.to_ascii_lowercase())
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(fallback)
}

fn get_array(values: &BTreeMap<String, String>, key: &str, fallback: &[&str]) -> Vec<String> {
    values
        .get(&key.to_ascii_lowercase())
        .map(|value| parse_string_array(value))
        .unwrap_or_else(|| fallback.iter().map(|value| value.to_string()).collect())
}

fn get_array_fallback(
    values: &BTreeMap<String, String>,
    preferred_key: &str,
    fallback_key: &str,
    fallback: &[&str],
) -> Vec<String> {
    if values.contains_key(&preferred_key.to_ascii_lowercase()) {
        get_array(values, preferred_key, fallback)
    } else {
        get_array(values, fallback_key, fallback)
    }
}

fn get_i32_array(values: &BTreeMap<String, String>, key: &str) -> Vec<i32> {
    values
        .get(&key.to_ascii_lowercase())
        .map(|value| {
            trim_array(value)
                .split(',')
                .filter_map(|item| item.trim().parse::<i32>().ok())
                .filter(|port| *port > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_string_array(value: &str) -> Vec<String> {
    let trimmed = trim_array(value);
    if trimmed.is_empty() {
        return Vec::new();
    }

    trimmed.split(',').map(unquote).collect()
}

fn trim_array(value: &str) -> &str {
    let value = value.trim();
    if value.starts_with('[') && value.ends_with(']') {
        &value[1..value.len() - 1]
    } else {
        ""
    }
}

fn unquote(value: &str) -> String {
    let mut value = value.trim().to_string();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value = value[1..value.len() - 1].to_string();
    }

    value
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn parse_upstream(value: &str) -> UpstreamProtocol {
    if value == "http3" {
        UpstreamProtocol::Http3
    } else {
        UpstreamProtocol::Http2
    }
}

fn parse_log_level(value: &str) -> LogLevel {
    match value {
        "debug" => LogLevel::Debug,
        "trace" => LogLevel::Trace,
        _ => LogLevel::Info,
    }
}

fn log_level(value: LogLevel) -> &'static str {
    match value {
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
        LogLevel::Trace => "trace",
    }
}

fn routing_mode(value: RoutingMode) -> &'static str {
    match value {
        RoutingMode::General => "general",
        RoutingMode::Selective => "selective",
    }
}

fn line_string(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = \"");
    output.push_str(&escape(value));
    output.push_str("\"\n");
}

fn line_bool(output: &mut String, key: &str, value: bool) {
    output.push_str(key);
    output.push_str(if value { " = true\n" } else { " = false\n" });
}

fn line_i32(output: &mut String, key: &str, value: i32) {
    output.push_str(key);
    output.push_str(" = ");
    output.push_str(&value.to_string());
    output.push('\n');
}

fn line_i32_array(output: &mut String, key: &str, values: &[i32]) {
    output.push_str(key);
    output.push_str(" = [");
    output.push_str(
        &values
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    );
    output.push_str("]\n");
}

fn line_string_array(output: &mut String, key: &str, values: &[String]) {
    output.push_str(key);
    output.push_str(" = [");
    output.push_str(
        &values
            .iter()
            .map(|value| format!("\"{}\"", escape(value)))
            .collect::<Vec<_>>()
            .join(", "),
    );
    output.push_str("]\n");
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_imports_endpoint_password_as_secret_candidate() {
        let toml = r#"
            loglevel = "info"
            vpn_mode = "general"
            killswitch_enabled = true
            killswitch_allow_ports = [22, 8080]
            post_quantum_group_enabled = true
            exclusions = ["*.example.com"]

            [endpoint]
            hostname = "vpn.example.com"
            custom_sni = "front.example.com"
            addresses = ["vpn.example.com:443"]
            has_ipv6 = true
            username = "user"
            password = "secret"
            client_random = "abcd"
            skip_verification = false
            certificate = ""
            upstream_protocol = "http2"
            upstream_fallback_protocol = "http3"
            anti_dpi = false
            dns_upstreams = ["tls://1.1.1.1"]

            [listener.tun]
            included_routes = ["0.0.0.0/0"]
            excluded_routes = ["10.0.0.0/8"]
            mtu_size = 1280
            change_system_dns = true
        "#;

        let (config, secrets) = TrustTunnelTomlParser.parse(toml);

        assert_eq!(config.endpoint.hostname, "vpn.example.com");
        assert_eq!(config.endpoint.custom_sni, "front.example.com");
        assert_eq!(secrets.password, "secret");
        assert_eq!(secrets.client_random, "abcd");
        assert_eq!(config.routing.kill_switch_allow_ports.len(), 2);
        assert_eq!(
            config.endpoint.fallback_protocol,
            Some(UpstreamProtocol::Http3)
        );
    }

    #[test]
    fn builder_escapes_secrets_and_arrays() {
        let mut config = TrustTunnelConfig::default();
        config.endpoint.hostname = "vpn.example.com".to_string();
        config.endpoint.custom_sni = "front.example.com".to_string();
        config.endpoint.addresses = vec!["vpn.example.com:443".to_string()];
        config.endpoint.username = "user".to_string();
        config.endpoint.fallback_protocol = Some(UpstreamProtocol::Http2);
        config.endpoint.dns_upstreams = vec!["tls://1.1.1.1".to_string()];

        let toml = TrustTunnelTomlBuilder.build(&config, "s\"ecret\t\u{1b}", "", "");

        assert!(toml.contains("password = \"s\\\"ecret\\t\\u001b\""));
        assert!(toml.contains("addresses = [\"vpn.example.com:443\"]"));
        assert!(toml.contains("custom_sni = \"front.example.com\""));
        assert!(toml.contains("upstream_fallback_protocol = \"http2\""));
    }

    #[test]
    fn parser_supports_routing_socks_and_legacy_dns() {
        let toml = r#"
            loglevel = "info"
            vpn_mode = "selective"
            killswitch_enabled = false
            killswitch_allow_ports = [53, 123]
            post_quantum_group_enabled = false
            exclusions = ["*.local", "192.168.0.0/16"]
            dns_upstreams = ["tls://9.9.9.9"]

            [endpoint]
            hostname = "vpn.example.com"
            addresses = ["vpn.example.com:443"]
            has_ipv6 = false
            username = "user"
            password = "secret"
            skip_verification = true
            upstream_protocol = "http3"
            upstream_fallback_protocol = "http2"
            anti_dpi = true

            [listener.socks]
            address = "127.0.0.1:1080"
            allow_lan_access = true
            http_proxy_address = "0.0.0.0:8080"
            http_proxy_allow_lan_access = true
        "#;

        let (config, secrets) = TrustTunnelTomlParser.parse(toml);

        assert_eq!(config.routing.mode, RoutingMode::Selective);
        assert!(!config.routing.kill_switch_enabled);
        assert_eq!(config.routing.kill_switch_allow_ports.len(), 2);
        assert!(!config.endpoint.has_ipv6);
        assert!(config.endpoint.skip_verification);
        assert!(config.endpoint.anti_dpi);
        assert_eq!(config.endpoint.upstream_protocol, UpstreamProtocol::Http3);
        assert_eq!(
            config.endpoint.fallback_protocol,
            Some(UpstreamProtocol::Http2)
        );
        assert_eq!(config.endpoint.dns_upstreams, vec!["tls://9.9.9.9"]);
        assert_eq!(config.listener.mode, ListenerMode::Socks);
        assert!(config.listener.socks.allow_lan_access);
        assert_eq!(config.listener.socks.http_proxy_address, "0.0.0.0:8080");
        assert!(config.listener.socks.http_proxy_allow_lan_access);
        assert_eq!(secrets.password, "secret");
    }

    #[test]
    fn parser_accepts_utf8_bom_before_first_section() {
        let toml = "\u{feff}[endpoint]\n\
            hostname = \"vpn.example.com\"\n\
            addresses = [\"vpn.example.com:443\"]\n\
            username = \"user\"\n\
            password = \"secret\"\n";

        let (config, secrets) = TrustTunnelTomlParser.parse(toml);

        assert_eq!(config.endpoint.hostname, "vpn.example.com");
        assert_eq!(config.endpoint.addresses, vec!["vpn.example.com:443"]);
        assert_eq!(secrets.password, "secret");
    }
}
