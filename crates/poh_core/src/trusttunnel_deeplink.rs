use base64::prelude::{Engine as _, BASE64_STANDARD};
use thiserror::Error;

use crate::trusttunnel_config::{
    EndpointConfig, ListenerConfig, ListenerMode, SecretCandidate, TrustTunnelConfig,
    UpstreamProtocol,
};

const MAX_SUPPORTED_VERSION: u64 = 1;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DeeplinkParseError {
    #[error("invalid TrustTunnel link scheme")]
    InvalidScheme,
    #[error("missing TrustTunnel payload")]
    MissingPayload,
    #[error("corrupted TrustTunnel payload")]
    CorruptedPayload,
    #[error("missing server address")]
    MissingAddress,
    #[error("missing TLS hostname")]
    MissingHostname,
    #[error("missing credentials")]
    MissingCredentials,
    #[error("unsupported TrustTunnel deeplink version")]
    UnsupportedVersion,
}

pub struct TrustTunnelDeeplinkParser;

impl TrustTunnelDeeplinkParser {
    pub fn parse(
        &self,
        link: &str,
    ) -> Result<(TrustTunnelConfig, SecretCandidate, String), DeeplinkParseError> {
        if !link.to_ascii_lowercase().starts_with("tt://") {
            return Err(DeeplinkParseError::InvalidScheme);
        }

        let payload = link
            .split_once('?')
            .map(|(_, payload)| payload.trim())
            .unwrap_or_else(|| link.trim_start_matches("tt://").trim());
        if payload.is_empty() {
            return Err(DeeplinkParseError::MissingPayload);
        }

        let bytes = decode_payload(payload)?;
        let decoded = decode_fields(&bytes)?;

        if decoded.addresses.is_empty() {
            return Err(DeeplinkParseError::MissingAddress);
        }

        if decoded.hostname.trim().is_empty() {
            return Err(DeeplinkParseError::MissingHostname);
        }

        if decoded.username.trim().is_empty() || decoded.password.trim().is_empty() {
            return Err(DeeplinkParseError::MissingCredentials);
        }

        let display_name = if decoded.name.trim().is_empty() {
            decoded.hostname.clone()
        } else {
            decoded.name.clone()
        };

        Ok((
            TrustTunnelConfig {
                endpoint: EndpointConfig {
                    hostname: decoded.hostname,
                    custom_sni: decoded.custom_sni,
                    addresses: decoded.addresses,
                    has_ipv6: decoded.has_ipv6,
                    username: decoded.username,
                    skip_verification: decoded.skip_verification,
                    certificate_pem: decoded.certificate_pem,
                    upstream_protocol: decoded.upstream_protocol,
                    anti_dpi: decoded.anti_dpi,
                    dns_upstreams: decoded.dns_upstreams,
                    ..EndpointConfig::default()
                },
                listener: ListenerConfig {
                    mode: ListenerMode::Tun,
                    ..ListenerConfig::default()
                },
                ..TrustTunnelConfig::default()
            },
            SecretCandidate {
                password: decoded.password,
                client_random: decoded.client_random,
                socks_password: String::new(),
            },
            display_name,
        ))
    }
}

fn decode_payload(payload: &str) -> Result<Vec<u8>, DeeplinkParseError> {
    let mut normalized = payload.replace('-', "+").replace('_', "/");
    let missing_padding = (4 - normalized.len() % 4) % 4;
    normalized.push_str(&"=".repeat(missing_padding));
    BASE64_STANDARD
        .decode(normalized)
        .map_err(|_| DeeplinkParseError::CorruptedPayload)
}

fn decode_fields(bytes: &[u8]) -> Result<DecodedDeeplink, DeeplinkParseError> {
    let mut decoded = DecodedDeeplink::default();
    let mut offset = 0;

    while offset < bytes.len() {
        let tag = read_varint(bytes, &mut offset)?;
        let length = read_varint(bytes, &mut offset)? as usize;
        if offset + length > bytes.len() {
            return Err(DeeplinkParseError::CorruptedPayload);
        }

        let value = &bytes[offset..offset + length];
        offset += length;

        match tag {
            0x00 => {
                decoded.version = read_varint_at(value, 0)?;
                if decoded.version > MAX_SUPPORTED_VERSION {
                    return Err(DeeplinkParseError::UnsupportedVersion);
                }
            }
            0x01 => decoded.hostname = utf8(value)?,
            0x02 => decoded.addresses.push(utf8(value)?),
            0x03 => decoded.custom_sni = utf8(value)?,
            0x04 => decoded.has_ipv6 = read_bool(value)?,
            0x05 => decoded.username = utf8(value)?,
            0x06 => decoded.password = utf8(value)?,
            0x07 => decoded.skip_verification = read_bool(value)?,
            0x08 => decoded.certificate_pem = der_certificates_to_pem(value),
            0x09 => {
                decoded.upstream_protocol = if read_varint_at(value, 0)? == 0x02 {
                    UpstreamProtocol::Http3
                } else {
                    UpstreamProtocol::Http2
                };
            }
            0x0A => decoded.anti_dpi = read_bool(value)?,
            0x0B => decoded.client_random = utf8(value)?,
            0x0C => decoded.name = utf8(value)?,
            0x0D => decoded.dns_upstreams = read_string_array(value)?,
            _ => {}
        }
    }

    Ok(decoded)
}

fn read_varint_at(bytes: &[u8], start: usize) -> Result<u64, DeeplinkParseError> {
    let mut offset = start;
    read_varint(bytes, &mut offset)
}

fn read_varint(bytes: &[u8], offset: &mut usize) -> Result<u64, DeeplinkParseError> {
    if *offset >= bytes.len() {
        return Err(DeeplinkParseError::CorruptedPayload);
    }

    let first = bytes[*offset];
    *offset += 1;
    let prefix = first >> 6;
    let size = 1usize << prefix;
    if *offset + size - 1 > bytes.len() {
        return Err(DeeplinkParseError::CorruptedPayload);
    }

    let mut value = u64::from(first & 0x3F);
    for _ in 1..size {
        value = (value << 8) | u64::from(bytes[*offset]);
        *offset += 1;
    }

    Ok(value)
}

fn utf8(value: &[u8]) -> Result<String, DeeplinkParseError> {
    String::from_utf8(value.to_vec()).map_err(|_| DeeplinkParseError::CorruptedPayload)
}

fn read_bool(value: &[u8]) -> Result<bool, DeeplinkParseError> {
    if value.len() != 1 {
        return Err(DeeplinkParseError::CorruptedPayload);
    }

    Ok(value[0] != 0)
}

fn read_string_array(value: &[u8]) -> Result<Vec<String>, DeeplinkParseError> {
    let mut items = Vec::new();
    let mut offset = 0;
    while offset < value.len() {
        let length = read_varint(value, &mut offset)? as usize;
        if offset + length > value.len() {
            return Err(DeeplinkParseError::CorruptedPayload);
        }

        items.push(utf8(&value[offset..offset + length])?);
        offset += length;
    }

    Ok(items)
}

fn der_certificates_to_pem(value: &[u8]) -> String {
    if value.is_empty() {
        return String::new();
    }

    let mut blocks = split_der_certificates(value);
    if blocks.is_empty() {
        blocks.push(value.to_vec());
    }

    blocks
        .iter()
        .map(|block| {
            format!(
                "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
                BASE64_STANDARD.encode(block)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_der_certificates(value: &[u8]) -> Vec<Vec<u8>> {
    let mut blocks = Vec::new();
    let mut offset = 0;
    while offset < value.len() {
        if value[offset] != 0x30 {
            return Vec::new();
        }

        let start = offset;
        offset += 1;
        if offset >= value.len() {
            return Vec::new();
        }

        let length_byte = value[offset];
        offset += 1;
        let content_length = if (length_byte & 0x80) == 0 {
            usize::from(length_byte)
        } else {
            let length_bytes = usize::from(length_byte & 0x7F);
            if length_bytes == 0 || length_bytes > 4 || offset + length_bytes > value.len() {
                return Vec::new();
            }

            let mut length = 0usize;
            for _ in 0..length_bytes {
                length = (length << 8) | usize::from(value[offset]);
                offset += 1;
            }
            length
        };

        let total_length = offset - start + content_length;
        if start + total_length > value.len() {
            return Vec::new();
        }

        blocks.push(value[start..start + total_length].to_vec());
        offset = start + total_length;
    }

    blocks
}

#[derive(Debug)]
struct DecodedDeeplink {
    version: u64,
    hostname: String,
    addresses: Vec<String>,
    custom_sni: String,
    has_ipv6: bool,
    username: String,
    password: String,
    skip_verification: bool,
    certificate_pem: String,
    upstream_protocol: UpstreamProtocol,
    anti_dpi: bool,
    client_random: String,
    name: String,
    dns_upstreams: Vec<String>,
}

impl Default for DecodedDeeplink {
    fn default() -> Self {
        Self {
            version: 0,
            hostname: String::new(),
            addresses: Vec::new(),
            custom_sni: String::new(),
            has_ipv6: true,
            username: String::new(),
            password: String::new(),
            skip_verification: false,
            certificate_pem: String::new(),
            upstream_protocol: UpstreamProtocol::Http2,
            anti_dpi: false,
            client_random: String::new(),
            name: String::new(),
            dns_upstreams: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_extracts_bundled_trusttunnel_vector() {
        let link = "tt://?AAEBARF0dC5oZWwyLm11bXVydS5ydQUGdHR1c2VyBiRZcmdua0o4V2pOV090MXdRVW5jYzllYWt5VU1nb3hjSVpZY0ICFXR0LmhlbDIubXVtdXJ1LnJ1OjQ0Mw";
        let (config, secrets, display_name) = TrustTunnelDeeplinkParser.parse(link).unwrap();

        assert_eq!(config.endpoint.hostname, "tt.hel2.mumuru.ru");
        assert_eq!(display_name, "tt.hel2.mumuru.ru");
        assert_eq!(config.endpoint.username, "ttuser");
        assert_eq!(config.endpoint.addresses, vec!["tt.hel2.mumuru.ru:443"]);
        assert!(secrets.password.len() > 8);
    }

    #[test]
    fn parser_honors_tlv_fields_and_display_name() {
        let link = build_deeplink(&[
            (0x00, varint_bytes(1)),
            (0x0C, string_bytes("Pretty Server")),
            (0x01, string_bytes("vpn.example.com")),
            (0x02, string_bytes("vpn.example.com:443")),
            (
                0x0D,
                string_array_bytes(&["tls://1.1.1.1", "https://dns.google/dns-query"]),
            ),
            (0x05, string_bytes("realuser")),
            (0x06, string_bytes("real-password")),
            (0x0B, string_bytes("aabbcc/ffffff")),
            (0x09, varint_bytes(2)),
            (0x0A, vec![1]),
        ]);

        let (config, secrets, display_name) = TrustTunnelDeeplinkParser.parse(&link).unwrap();

        assert_eq!(display_name, "Pretty Server");
        assert_eq!(config.endpoint.hostname, "vpn.example.com");
        assert_eq!(config.endpoint.username, "realuser");
        assert_eq!(secrets.password, "real-password");
        assert_eq!(secrets.client_random, "aabbcc/ffffff");
        assert_eq!(config.endpoint.upstream_protocol, UpstreamProtocol::Http3);
        assert!(config.endpoint.anti_dpi);
        assert_eq!(config.endpoint.dns_upstreams[0], "tls://1.1.1.1");
    }

    fn build_deeplink(fields: &[(u64, Vec<u8>)]) -> String {
        let mut bytes = Vec::new();
        for (tag, value) in fields {
            bytes.extend(varint_bytes(*tag));
            bytes.extend(varint_bytes(value.len() as u64));
            bytes.extend(value);
        }

        format!(
            "tt://?{}",
            BASE64_STANDARD
                .encode(bytes)
                .trim_end_matches('=')
                .replace('+', "-")
                .replace('/', "_")
        )
    }

    fn string_bytes(value: &str) -> Vec<u8> {
        value.as_bytes().to_vec()
    }

    fn string_array_bytes(values: &[&str]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            let item = string_bytes(value);
            bytes.extend(varint_bytes(item.len() as u64));
            bytes.extend(item);
        }
        bytes
    }

    fn varint_bytes(value: u64) -> Vec<u8> {
        if value <= 63 {
            vec![value as u8]
        } else if value <= 16383 {
            vec![(0x40 | (value >> 8)) as u8, value as u8]
        } else if value <= 1073741823 {
            vec![
                (0x80 | (value >> 24)) as u8,
                (value >> 16) as u8,
                (value >> 8) as u8,
                value as u8,
            ]
        } else {
            vec![
                (0xC0 | (value >> 56)) as u8,
                (value >> 48) as u8,
                (value >> 40) as u8,
                (value >> 32) as u8,
                (value >> 24) as u8,
                (value >> 16) as u8,
                (value >> 8) as u8,
                value as u8,
            ]
        }
    }
}
