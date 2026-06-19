use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

mod desktop_state;
mod network_effects;

use poh_core::{
    sha256_hex, CoreAdapter, CoreId, CoreRegistry, ImportInput, InstalledCoreManifest,
    NaiveProxyAdapter, PinnedRelease, SignatureStatus, SourceStatus, SourceType,
    TrustTunnelAdapter, TrustedCoreSource, TrustedSourcePolicy,
};
use poh_core_runner::{MapSecretResolver, RuntimeMaterializer};
use poh_core_session::{CoreLaunchDescriptor, CoreLaunchSpec, CoreProcess};
use poh_core_store::{CoreArtifact, CoreDownloader, CoreInstallRequest, CoreStore};
use serde::Serialize;

const MAX_DESKTOP_IMPORT_BYTES: u64 = 2 * 1024 * 1024;

fn main() -> ExitCode {
    let mut registry = CoreRegistry::new();
    registry.register(TrustTunnelAdapter::new());
    registry.register(NaiveProxyAdapter::new());

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
        Some("core-list-installed") => core_list_installed(),
        Some("catalog-list") => catalog_list(),
        Some("core-download-plan") => core_download_plan(&args[1..]),
        Some("core-install") => core_install(&args[1..]),
        Some("runtime-smoke") => runtime_smoke(),
        Some("store-smoke") => store_smoke(),
        Some("session-smoke") => session_smoke(),
        Some("desktop-preview-profile") => desktop_preview_profile(&args[1..]),
        Some("desktop-import-profile") => desktop_import_profile(&args[1..]),
        Some("desktop-session-plan") => desktop_session_plan(&args[1..]),
        Some("desktop-session-start") => desktop_session_start(&args[1..]),
        Some("desktop-session-stop") => desktop_session_stop(),
        Some("desktop-session-reset") => desktop_session_reset(),
        Some("desktop-session-status") => desktop_session_status(),
        Some("desktop-session-log") => desktop_session_log(),
        Some("desktop-session-supervise") => desktop_session_supervise(&args[1..]),
        Some("desktop-list-profiles") => desktop_list_profiles(&args[1..]),
        Some("desktop-core-schema") => desktop_core_schema(&args[1..]),
        Some("desktop-core-modes") => desktop_core_modes(&args[1..]),
        Some("desktop-validate-profile") => desktop_validate_profile(&args[1..]),
        Some("desktop-update-profile") => desktop_update_profile(&args[1..]),
        Some(command) => {
            eprintln!("Unknown command: {command}");
            eprintln!(
                "Usage: poh_cli [list|sources|catalog-list|core-list-installed|core-download-plan <core-id>|core-install <core-id> [executable-relative-path]|runtime-smoke|store-smoke|session-smoke|desktop-preview-profile <input-text-file|->|desktop-import-profile <input-text-file|->|desktop-session-plan <state-path> <profile-id>|desktop-session-start <state-path> <profile-id>|desktop-session-stop|desktop-session-reset|desktop-session-status|desktop-session-log|desktop-session-supervise <state-path> <profile-id>|detect <profile text>]"
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

fn load_trusted_sources() -> Result<Vec<TrustedCoreSource>, String> {
    let json = include_str!("../../../core-registry/trusted-sources.json");
    TrustedSourcePolicy::default()
        .parse_sources(json)
        .map_err(|error| error.to_string())
}

fn find_trusted_source<'a>(
    sources: &'a [TrustedCoreSource],
    core_id: &str,
) -> Result<&'a TrustedCoreSource, String> {
    sources
        .iter()
        .find(|source| source.core_id.as_str() == core_id)
        .ok_or_else(|| format!("Unknown trusted core source: {core_id}"))
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
        files: Vec::new(),
    };
    let request = CoreInstallRequest {
        manifest,
        artifact: CoreArtifact::single_file(artifact_bytes, "sing-box.exe"),
    };
    let root = env::temp_dir().join(format!("poh-store-smoke-{}", now_unix_ms()));
    let store = CoreStore::new(&root);
    match store.install(&request, &smoke_sources_for(&request.manifest)) {
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

fn core_list_installed() -> ExitCode {
    let store = CoreStore::new(local_core_store_root());
    match store.list_installed() {
        Ok(installed) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&installed)
                    .expect("installed core list should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Core list failed: {error}");
            ExitCode::from(1)
        }
    }
}

#[derive(Serialize)]
struct CoreCatalogResponse {
    cores: Vec<CoreCatalogEntry>,
}

#[derive(Serialize)]
struct CoreCatalogEntry {
    core_id: CoreId,
    display_name: String,
    source_type: SourceType,
    source_status: SourceStatus,
    install_enabled: bool,
    installable: bool,
    installed: bool,
    active_version: Option<String>,
    installed_versions: Vec<String>,
    active_executable_path: Option<String>,
    descriptor_available: bool,
    executable_relative_path: Option<String>,
    pinned_release: Option<PinnedRelease>,
    homepage: Option<String>,
    license: Option<String>,
    notes: Option<String>,
    signature_preferred: bool,
}

fn catalog_list() -> ExitCode {
    let sources = match load_trusted_sources() {
        Ok(sources) => sources,
        Err(error) => {
            eprintln!("Trusted source policy failed: {error}");
            return ExitCode::from(1);
        }
    };

    let store = CoreStore::new(local_core_store_root());
    let installed = match store.list_installed() {
        Ok(installed) => installed,
        Err(error) => {
            eprintln!("Core catalog failed to read installed cores: {error}");
            return ExitCode::from(1);
        }
    };
    let mut installed_by_core = BTreeMap::<CoreId, Vec<_>>::new();
    for core in installed {
        installed_by_core
            .entry(core.manifest.core_id.clone())
            .or_default()
            .push(core);
    }

    let cores = sources
        .into_iter()
        .map(|source| {
            let installed = installed_by_core
                .remove(&source.core_id)
                .unwrap_or_default();
            let active = installed.iter().find(|core| core.active);
            let descriptor = CoreLaunchDescriptor::for_core(&source.core_id);
            let installable = source.is_installable();

            CoreCatalogEntry {
                core_id: source.core_id,
                display_name: source.display_name,
                source_type: source.source_type,
                source_status: source.status.clone(),
                install_enabled: source.install_enabled,
                installable,
                installed: !installed.is_empty(),
                active_version: active.map(|core| core.manifest.version.clone()),
                installed_versions: installed
                    .iter()
                    .map(|core| core.manifest.version.clone())
                    .collect(),
                active_executable_path: active
                    .map(|core| core.executable_path.display().to_string()),
                descriptor_available: descriptor.is_some(),
                executable_relative_path: descriptor
                    .map(|descriptor| descriptor.executable_relative_path),
                pinned_release: source.pinned_release,
                homepage: source.homepage,
                license: source.license,
                notes: source.notes,
                signature_preferred: source.signature_preferred,
            }
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string_pretty(&CoreCatalogResponse { cores })
            .expect("core catalog should serialize")
    );
    ExitCode::SUCCESS
}

fn core_download_plan(args: &[String]) -> ExitCode {
    let [core_id] = args else {
        eprintln!("Usage: poh_cli core-download-plan <core-id>");
        return ExitCode::from(2);
    };

    let sources = match load_trusted_sources() {
        Ok(sources) => sources,
        Err(error) => {
            eprintln!("Trusted source policy failed: {error}");
            return ExitCode::from(1);
        }
    };
    let source = match find_trusted_source(&sources, core_id) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };

    match CoreDownloader::plan(source) {
        Ok(plan) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&plan).expect("download plan should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Core download plan failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn core_install(args: &[String]) -> ExitCode {
    let Some(core_id) = args.first() else {
        eprintln!("Usage: poh_cli core-install <core-id> [executable-relative-path]");
        return ExitCode::from(2);
    };
    if args.len() > 2 {
        eprintln!("Usage: poh_cli core-install <core-id> [executable-relative-path]");
        return ExitCode::from(2);
    };

    let sources = match load_trusted_sources() {
        Ok(sources) => sources,
        Err(error) => {
            eprintln!("Trusted source policy failed: {error}");
            return ExitCode::from(1);
        }
    };
    let source = match find_trusted_source(&sources, core_id) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let executable_relative_path = args.get(1).cloned().or_else(|| {
        CoreLaunchDescriptor::for_core(&source.core_id)
            .map(|descriptor| descriptor.executable_relative_path)
    });
    let Some(executable_relative_path) = executable_relative_path else {
        eprintln!(
            "Core install failed: launch descriptor is missing for {} and no executable path was provided",
            source.core_id
        );
        return ExitCode::from(1);
    };

    let downloaded = match CoreDownloader::default().download(source) {
        Ok(downloaded) => downloaded,
        Err(error) => {
            eprintln!("Core download failed: {error}");
            return ExitCode::from(1);
        }
    };
    let request =
        match downloaded.into_install_request(source, executable_relative_path, now_unix_ms()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("Core install request failed: {error}");
                return ExitCode::from(1);
            }
        };

    let store = CoreStore::new(local_core_store_root());
    match store.install(&request, &sources) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "core_id": result.manifest.core_id,
                    "version": result.manifest.version,
                    "asset_name": result.manifest.asset_name,
                    "install_dir": result.install_dir,
                    "executable_path": result.executable_path,
                    "files": result.manifest.files,
                }))
                .expect("install result should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Core install failed: {error}");
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
        files: Vec::new(),
    };
    let request = CoreInstallRequest {
        manifest,
        artifact: CoreArtifact::single_file(artifact_bytes, "poh_cli.exe"),
    };
    let root = env::temp_dir().join(format!("poh-session-smoke-{}", now_unix_ms()));
    let store = CoreStore::new(&root);
    let result = (|| {
        let trusted_sources = smoke_sources_for(&request.manifest);
        store.install(&request, &trusted_sources)?;
        let verified = store.verify_core(
            &CoreId::from("sing-box"),
            "0.0.1-session-smoke",
            &trusted_sources,
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

fn desktop_preview_profile(args: &[String]) -> ExitCode {
    let [input_path] = args else {
        eprintln!("Usage: poh_cli desktop-preview-profile <input-text-file|->");
        return ExitCode::from(2);
    };

    let input = match read_import_input(input_path) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("Cannot read import input: {error}");
            return ExitCode::from(1);
        }
    };

    match desktop_state::preview_desktop_profile(&input) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("preview result should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop preview failed: {error}");
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

fn desktop_session_reset() -> ExitCode {
    match desktop_state::reset_desktop_session() {
        Ok(status) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&status).expect("status should serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Desktop session reset failed: {error}");
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

fn desktop_session_supervise(args: &[String]) -> ExitCode {
    let (state_path, profile_id, gui_pid) = match args {
        [s, p] => (s.as_str(), p.as_str(), None),
        [s, p, flag, pid_str] if flag == "--gui-pid" => {
            (s.as_str(), p.as_str(), pid_str.parse::<u32>().ok())
        }
        _ => {
            eprintln!(
                "Usage: poh_cli desktop-session-supervise <state-path> <profile-id> [--gui-pid <pid>]"
            );
            return ExitCode::from(2);
        }
    };
    match desktop_state::supervise_desktop_session(
        std::path::Path::new(state_path),
        profile_id,
        gui_pid,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Supervisor error: {error}");
            ExitCode::from(1)
        }
    }
}

// ── C1: desktop-list-profiles ─────────────────────────────────────────────

fn desktop_list_profiles(args: &[String]) -> ExitCode {
    let state_path = match args.first() {
        Some(p) => std::path::Path::new(p.as_str()),
        None => {
            eprintln!("Usage: poh_cli desktop-list-profiles <state-path>");
            return ExitCode::from(2);
        }
    };
    match desktop_state::list_desktop_profiles(state_path) {
        Ok(list) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&list).expect("serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::from(1)
        }
    }
}

// ── C3: desktop-core-schema ───────────────────────────────────────────────

fn desktop_core_schema(args: &[String]) -> ExitCode {
    let core_id = match args.first() {
        Some(id) => id.as_str(),
        None => {
            eprintln!("Usage: poh_cli desktop-core-schema <core-id>");
            return ExitCode::from(2);
        }
    };
    match desktop_state::core_schema(core_id) {
        Ok(schema) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schema).expect("serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::from(1)
        }
    }
}

// ── C5: desktop-core-modes ────────────────────────────────────────────────

fn desktop_core_modes(args: &[String]) -> ExitCode {
    let core_id = match args.first() {
        Some(id) => id.as_str(),
        None => {
            eprintln!("Usage: poh_cli desktop-core-modes <core-id>");
            return ExitCode::from(2);
        }
    };
    match desktop_state::core_modes(core_id) {
        Ok(modes) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&modes).expect("serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::from(1)
        }
    }
}

// ── C4: desktop-validate-profile / desktop-update-profile ────────────────

fn read_stdin_json<T: serde::de::DeserializeOwned>(cmd: &str) -> Result<T, ExitCode> {
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        eprintln!("{cmd}: failed to read stdin");
        return Err(ExitCode::from(2));
    }
    serde_json::from_str::<T>(&buf).map_err(|error| {
        println!(
            "{}",
            serde_json::json!({ "error": format!("bad JSON: {error}") })
        );
        ExitCode::from(1)
    })
}

fn desktop_validate_profile(args: &[String]) -> ExitCode {
    let state_path = match args.first() {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            eprintln!("Usage: poh_cli desktop-validate-profile <state-path>  (JSON on stdin)");
            return ExitCode::from(2);
        }
    };
    let input: desktop_state::ProfileFieldsInput = match read_stdin_json("desktop-validate-profile")
    {
        Ok(v) => v,
        Err(code) => return code,
    };
    match desktop_state::validate_desktop_profile(&state_path, input) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("serialize")
            );
            if result.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::from(1)
        }
    }
}

fn desktop_update_profile(args: &[String]) -> ExitCode {
    let state_path = match args.first() {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            eprintln!("Usage: poh_cli desktop-update-profile <state-path>  (JSON on stdin)");
            return ExitCode::from(2);
        }
    };
    let input: desktop_state::ProfileFieldsInput = match read_stdin_json("desktop-update-profile") {
        Ok(v) => v,
        Err(code) => return code,
    };
    match desktop_state::update_desktop_profile(&state_path, input) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("serialize")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "error": error.to_string() }));
            ExitCode::from(1)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn smoke_sources_for(manifest: &InstalledCoreManifest) -> Vec<TrustedCoreSource> {
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
        pinned_release: Some(PinnedRelease {
            version: manifest.version.clone(),
            asset_name: manifest.asset_name.clone(),
            sha256: manifest.sha256.clone(),
            min_app_version: None,
        }),
        notes: None,
    }]
}

fn local_core_store_root() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("ProxyOpenHub")
        .join("cores")
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
