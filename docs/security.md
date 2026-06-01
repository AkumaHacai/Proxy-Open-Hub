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

MVP limitation:

- `InMemorySecretStore` is safe for not writing secrets to disk, but it is not durable. Replace it with Windows Credential Manager or DPAPI before a real release.
