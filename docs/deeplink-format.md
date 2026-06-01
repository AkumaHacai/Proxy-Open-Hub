# Deeplink Import

`tt://?...` links are parsed in `TrustTunnel.Core.Deeplinks`, outside the UI.

The current parser supports the known binary payload shape by decoding base64/base64url, extracting printable tokens, and mapping:

- domain-like token -> hostname
- host:port token -> address
- short credential token -> username
- remaining long token -> password secret candidate

If TrustTunnel publishes a stable binary schema, this module should be upgraded to parse fields by version/tag instead of printable-token inference.
