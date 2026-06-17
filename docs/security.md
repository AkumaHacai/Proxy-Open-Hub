# Security Notes

Sensitive values:

- password
- client random
- full `tt://?...` payload
- certificate material
- exported diagnostics

Implemented guardrails:

- Deep-link input is cleared after successful import.
- Source deep-link is not stored in a profile.
- TOML import returns password/client random as secret candidates, then the app materializes secret refs.
- Profile preview uses redaction before rendering.
- Diagnostic log redacts `tt://`, password, client random and certificate assignments.
- Rust core adapters keep secrets outside the generic profile model.
- Runtime files from adapters are validated before launch materialization: no absolute paths, no `..`, no Windows backslash paths, no oversized generated config files, no unsafe environment keys.
- Trusted core sources are locked by default. A GitHub-release core can become installable only when it is active, uses an approved source type, requires checksum verification, and declares a `pinned_release`.
- Installed core manifests are checked against trusted sources, pinned release metadata, allowed asset patterns, executable paths, and SHA-256 digests. This is the first barrier against fake or swapped core binaries.
- Zip/multifile core artifacts are extracted only through the core store. Each archive entry passes path validation, duplicate paths are rejected, and installed file hashes are recorded for later tamper checks.
- Downloadable cores are fetched only from the pinned GitHub release asset URL derived from the trusted catalog. The downloader enforces an artifact size limit and verifies SHA-256 before the bytes can become an install request.
- Bundled TrustTunnel is launched only from the fixed app-local `native/bundled/win-x64` path and is checked against pinned SHA-256 digests before every start.
- Import from Flutter is passed to Rust through stdin, so `tt://` payloads are no longer written to a temporary import file.
- Imported profile secrets are stored in `ProtectedSecrets` using Windows DPAPI. Legacy plaintext `Secrets` are migrated on load.
- Runtime `config.toml`, `session.json`, session logs and desktop state files receive restrictive Windows ACLs after creation.
- Session start/stop commands are protected by a single-instance lock. Stale locks are recoverable, session writes are atomic, and missing processes are marked faulted instead of silently discarding context.
- Import preview runs before saving; high-risk TLS/LAN warnings require explicit user confirmation.
- Profiles with disabled TLS verification or custom certificate material get a persistent UI indicator.

MVP limitation:

- Signature/AuthentiCode validation is not implemented yet. The current Rust policy prepares `SignatureStatus`, but real release downloads must add signature verification or pinned publisher checks before enabling automatic installation.
- Process lifetime is still managed by one-shot CLI commands. The current implementation verifies PID image name before stop/status and records faulted sessions, but a future long-lived service should keep a real process handle and run rollback hooks immediately on crashes.
