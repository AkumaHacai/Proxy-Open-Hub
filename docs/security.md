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
- Trusted core sources are locked by default. A core can become installable only when it is active, uses an approved source type, and requires checksum verification.
- Installed core manifests are checked against trusted sources, allowed asset patterns, executable paths, and SHA-256 digests. This is the first barrier against fake or swapped core binaries.
- Bundled TrustTunnel is launched only from the fixed app-local `native/bundled/win-x64` path and is checked against pinned SHA-256 digests before every start.
- Import from Flutter is passed to Rust through stdin, so `tt://` payloads are no longer written to a temporary import file.

MVP limitation:

- `InMemorySecretStore` is safe for not writing secrets to disk, but it is not durable. Replace it with Windows Credential Manager or DPAPI before a real release.
- Current `desktop-state.json` still stores the MVP `Secrets` map in plaintext. Move it to DPAPI or Windows Credential Manager before a public release.
- Signature/AuthentiCode validation is not implemented yet. The current Rust policy prepares `SignatureStatus`, but real release downloads must add signature verification or pinned publisher checks before enabling automatic installation.
