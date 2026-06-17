use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

mod desktop_state;

use poh_core::{
    sha256_hex, CoreAdapter, CoreId, CoreRegistry, ImportInput, InstalledCoreManifest,
    SignatureStatus, SourceStatus, SourceType, TrustTunnelAdapter, TrustedCoreSource,
    TrustedSourcePolicy,
};
use poh_core_runner::{MapSecretResolver, RuntimeMaterializer};
use poh_core_session::{CoreLaunchSpec, CoreProcess};
use poh_core_store::{CoreArtifact, CoreInstallRequest, CoreStore};

const MAX_DESKTOP_IMPORT_BYTES: u64 = 2 * 1024 * 1024;

fn main() -> ExitCode {
    let mut registry = CoreRegistry::new();
    registry.register(TrustTunnelAdapter::new());

    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        None | Some("list") => {
            print_manifests(&registry);
            ExitCode::SUCCESS
        }
        Some("detect") => {
            let input = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
            if input.trim().is_empty() {
                eprintln!("Usage: poh_cli detect <profile text>");
                return ExitCode::from(2);
            }

            let matches = registry.detect_import(&ImportInput::text(input));
            println!(
                "{}",
                serde_json::to_string_pretty(&matches).expect("detections should serialize")
            );
            ExitCode::SUCCESS
        }
        Some("sources") => {
            let json = include_str!("../../../core-registry/trusted-sources.json");
            match TrustedSourcePolicy::default().parse_sources(json) {
                Ok(sources) => {
                    println!("Trusted core sources: {}", sources.len());
                    for source in sources {
                        let install_state = if source.is_installable() {
                            "installable"
                        } else {
                            "locked"
                        };
                        println!(
                            "- {} ({}) :: {:?} :: {}",
                            source.display_name, source.core_id, source.status, install_state
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Trusted source policy failed: {error}");
                    ExitCode::from(1)
                }
            }
        }
        Some("runtime-smoke") => runtime_smoke(),
        Some("store-smoke") => store_smoke(),
        Some("session-smoke") => session_smoke(),
        Some("desktop-import-profile") => desktop_import_profile(&args[1..]),
        Some("desktop-session-plan") => desktop_session_plan(&args[1..]),
        Some("desktop-session-start") => desktop_session_start(&args[1..]),
        Some("desktop-session-stop") => desktop_session_stop(),
        Some("desktop-session-status") => desktop_session_status(),
        Some("desktop-session-log") => desktop_session_log(),
        Some(command) => {
            eprintln!("Unknown command: {command}");
            eprintln!(
                "Usage: poh_cli [list|sources|runtime-smoke|store-smoke|session-smoke|desktop-import-profile <input-text-file|->|desktop-session-plan <state-path> <profile-id>|desktop-session-start <state-path> <profile-id>|desktop-session-stop|desktop-session-status|desktop-session-log|detect <profile text>]"
            );
            ExitCode::from(2)
        }
    }
}

fn print_manifests(registry: &CoreRegistry) {
    println!("Proxy Open Hub core registry");
    for manifest in registry.manifests() {
        println!(
            "- {} ({}) :: {}",
            manifest.display_name, manifest.id, manifest.summary
        );
    }
}

fn runtime_smoke() -> ExitCode {
    let adapter = TrustTunnelAdapter::new();
    let parsed = match adapter.parse_profile(&ImportInput::text(
        r#"
        [endpoint]
        hostname = "tt.example.test"
        addresses = ["tt.example.test:443"]
        username = "ttuser"
        password = "demo-secret"
        client_random = "demo-random"
        upstream_protocol = "http2"

        [listener.tun]
        included_routes = ["0.0.0.0/0"]
        excluded_routes = ["10.0.0.0/8"]
        "#,
    )) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("Import failed: {error}");
            return ExitCode::from(1);
        }
    };

    let runtime_config = match adapter.build_runtime_config(&parsed.profile) {
        Ok(runtime_config) => runtime_config,
        Err(error) => {
            eprintln!("Runtime config failed: {error}");
            return ExitCode::from(1);
        }
    };
    let resolver = MapSecretResolver::new(parsed.secrets);
    let materialized = match RuntimeMaterializer::default().materialize(&runtime_config, &resolver)
    {
        Ok(materialized) => materialized,
        Err(error) => {
            eprintln!("Materialization failed: {error}");
            return ExitCode::from(1);
        }
    };

    println!("{}", materialized.redacted_preview());
    ExitCode::SUCCESS
}

fn store_smoke() -> ExitCode {
    let artifact_bytes = b"trusted demo core".to_vec();
    let manifest = InstalledCoreManifest {
        core_id: CoreId::from("sing-box"),
        display_name: "sing-box".to_string(),
        version: "0.0.1-smoke".to_string(),
        source_type: SourceType::GithubRelease,
        owner: Some("SagerNet".to_string()),
        repo: Some("sing-box".to_string()),
        asset_name: "sing-box-0.0.1-smoke-windows-amd64.zip".to_string(),
        executable_path: "sing-box.exe".to_string(),
        sha256: sha256_hex(&artifact_bytes),
        signature_status: SignatureStatus::Unknown,
        installed_at_unix_ms: now_unix_ms(),
    };
    let request = CoreInstallRequest {
        manifest,
        artifact: CoreArtifact {
            bytes: artifact_bytes,
            executable_relative_path: "sing-box.exe".to_string(),
        },
    };
    let root = env::temp_dir().join(format!("poh-store-smoke-{}", now_unix_ms()));
    let store = CoreStore::new(&root);
    match store.install(&request, &smoke_sources()) {
        Ok(result) => {
            println!("Installed {}", result.executable_path.display());
            if let Err(error) = std::fs::remove_dir_all(&root) {
                eprintln!("Cleanup failed: {error}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Core store smoke failed: {error}");
            if root.exists() {
                let _ = std::fs::remove_dir_all(&root);
            }
            ExitCode::from(1)
        }
    }
}

fn session_smoke() -> ExitCode {
    let artifact_bytes = match std::fs::read(std::env::current_exe().unwrap_or_default()) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("Cannot read current executable: {error}");
            return ExitCode::from(1);
        }
    };
    let manifest = InstalledCoreManifest {
        core_id: CoreId::from("sing-box"),
        display_name: "sing-box".to_string(),
        version: "0.0.1-session-smoke".to_string(),
        source_type: SourceType::GithubRelease,
        owner: Some("SagerNet".to_string()),
        repo: Some("sing-box".to_string()),
        asset_name: "sing-box-0.0.1-session-smoke-windows-amd64.zip".to_string(),
        executable_path: "poh_cli.exe".to_string(),
        sha256: sha256_hex(&artifact_bytes),
        signature_status: SignatureStatus::Unknown,
        installed_at_unix_ms: now_unix_ms(),
    };
    let request = CoreInstallRequest {
        manifest,
        artifact: CoreArtifact {
            bytes: artifact_bytes,
            executable_relative_path: "poh_cli.exe".to_string(),
        },
    };
    let root = env::temp_dir().join(format!("poh-session-smoke-{}", now_unix_ms()));
    let store = CoreStore::new(&root);
    let result = (|| {
        store.install(&request, &smoke_sources())?;
        let verified = store.verify_core(
            &CoreId::from("sing-box"),
            "0.0.1-session-smoke",
            &smoke_sources(),
        )?;
        let runtime = poh_core_runner::MaterializedRuntime {
            files: Vec::new(),
            command_args: vec!["list".to_string()],
            environment: std::collections::BTreeMap::new(),
        };
        let spec = CoreLaunchSpec::from_verified_core(&verified, &runtime);
        let process = CoreProcess::start(spec)?;
        Ok::<_, Box<dyn std::error::Error>>(process.wait_with_redacted_output()?)
    })();

    match result {
        Ok(output) if output.status.success() => {
            println!("Session smoke ok");
            if let Err(error) = std::fs::remove_dir_all(&root) {
                eprintln!("Cleanup failed: {error}");
            }
            ExitCode::SUCCESS
        }
        Ok(output) => {
            eprintln!("Session smoke exited with {}", output.status);
            eprintln!("{}", output.stderr);
            if root.exists() {
                let _ = std::fs::remove_dir_all(&root);
            }
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("Session smoke failed: {error}");
            if root.exists() {
                let _ = std::fs::remove_dir_all(&root);
            }
            ExitCode::from(1)
        }
    }
}

fn desktop_import_profile(args: &[String]) -> ExitCode {
    let [input_path] = args else {
        eprintln!("Usage: poh_cli desktop-import-profile <input-text-file|->");
        return ExitCode::from(2);
    };

    let input = match read_import_input(input_path) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("Cannot read import input: {error}");
            return ExitCode::from(1);
        }
    };

    match desktop_state::import_desktop_profile(&input) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("import result should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop import failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn read_import_input(input_path: &str) -> io::Result<String> {
    if input_path == "-" {
        let stdin = io::stdin();
        return read_utf8_limited(stdin.lock());
    }

    let metadata = std::fs::metadata(input_path)?;
    if metadata.len() > MAX_DESKTOP_IMPORT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "import input is too large: {} bytes (limit: {MAX_DESKTOP_IMPORT_BYTES})",
                metadata.len()
            ),
        ));
    }

    read_utf8_limited(File::open(input_path)?)
}

fn read_utf8_limited(reader: impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_DESKTOP_IMPORT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DESKTOP_IMPORT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("import input is too large (limit: {MAX_DESKTOP_IMPORT_BYTES})"),
        ));
    }

    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn desktop_session_plan(args: &[String]) -> ExitCode {
    let [state_path, profile_id] = args else {
        eprintln!("Usage: poh_cli desktop-session-plan <state-path> <profile-id>");
        return ExitCode::from(2);
    };

    match desktop_state::build_desktop_session_plan(std::path::Path::new(state_path), profile_id) {
        Ok(plan) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&plan).expect("session plan should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop session plan failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn desktop_session_start(args: &[String]) -> ExitCode {
    let [state_path, profile_id] = args else {
        eprintln!("Usage: poh_cli desktop-session-start <state-path> <profile-id>");
        return ExitCode::from(2);
    };

    match desktop_state::start_desktop_session(std::path::Path::new(state_path), profile_id) {
        Ok(session) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&session).expect("session should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop session start failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn desktop_session_stop() -> ExitCode {
    match desktop_state::stop_desktop_session() {
        Ok(status) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&status).expect("status should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop session stop failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn desktop_session_status() -> ExitCode {
    match desktop_state::desktop_session_status() {
        Ok(status) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&status).expect("status should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop session status failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn desktop_session_log() -> ExitCode {
    match desktop_state::desktop_session_log() {
        Ok(log) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&log).expect("log should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop session log failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn smoke_sources() -> Vec<TrustedCoreSource> {
    vec![TrustedCoreSource {
        core_id: CoreId::from("sing-box"),
        display_name: "sing-box".to_string(),
        source_type: SourceType::GithubRelease,
        status: SourceStatus::Active,
        homepage: Some("https://example.com".to_string()),
        license: Some("GPL-3.0-or-later".to_string()),
        owner: Some("SagerNet".to_string()),
        repo: Some("sing-box".to_string()),
        install_enabled: true,
        checksum_required: true,
        signature_preferred: true,
        allowed_asset_patterns: vec!["sing-box-*-windows-amd64.zip".to_string()],
        notes: None,
    }]
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
