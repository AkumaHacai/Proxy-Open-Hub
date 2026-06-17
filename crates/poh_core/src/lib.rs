pub mod adapter;
pub mod model;
pub mod registry;
pub mod security;
pub mod trusttunnel;
pub mod trusttunnel_config;
pub mod trusttunnel_deeplink;
pub mod trusttunnel_toml;

pub use adapter::{
    AdapterError, CoreAdapter, ImportInput, ImportMatch, ParsedProfile, RuntimeConfig, RuntimeFile,
    SettingsField, SettingsFieldKind, SettingsSchema, SettingsSection, ValidationWarning,
};
pub use model::{
    ConnectionState, CoreCapabilities, CoreId, CoreManifest, Profile, ProfileId, ProtocolColor,
};
pub use registry::{CoreDetection, CoreRegistry};
pub use security::{
    sha256_hex, InstalledCoreManifest, InstalledCorePolicy, PinnedRelease, Redactor,
    RuntimeSecurityPolicy, SecurityError, SignatureStatus, SourceStatus, SourceType,
    TrustedCoreSource, TrustedSourcePolicy,
};
pub use trusttunnel::TrustTunnelAdapter;
pub use trusttunnel_config::{
    EndpointConfig, ListenerConfig, ListenerMode, LogLevel, RoutingMode, RoutingProfile,
    SecretCandidate, SocksConfig, TrustTunnelConfig, TrustTunnelCoreProfile, TunConfig,
    UpstreamProtocol,
};
pub use trusttunnel_deeplink::{DeeplinkParseError, TrustTunnelDeeplinkParser};
pub use trusttunnel_toml::{TrustTunnelTomlBuilder, TrustTunnelTomlParser};
