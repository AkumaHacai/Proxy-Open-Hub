# Native Core Integration

The desktop client loads the official Windows TrustTunnel adapter dynamically from the application output directory.

Expected files next to `TrustTunnel.Desktop.exe`:

- `vpn_easy.dll` - official Windows adapter exported from `platform/windows`.
- `trusttunnel_client.exe` - official release-package CLI fallback when `vpn_easy.dll` is not bundled.
- `wintun.dll` - required for TUN profiles.
- `vpn_easy_service.exe` - required only when a profile uses service mode.

The repository currently bundles the official `trusttunnel_client-v1.0.49-windows-x86_64.zip` release contents needed for the CLI fallback:

- `native/bundled/win-x64/trusttunnel_client.exe`
- `native/bundled/win-x64/wintun.dll`

The C# side calls the official C ABI:

- Direct mode: `vpn_easy_start` / `vpn_easy_stop`.
- Direct mode, if exported by the DLL: `vpn_easy_start_ex` / `vpn_easy_stop_ex`.
- Service mode: `vpn_easy_service_start` / `vpn_easy_service_stop`.
- First service install: `vpn_easy_service_install`.
- CLI fallback: `trusttunnel_client.exe --config <runtime toml> --loglevel info`.

The native state callback is mapped from `ag::VpnSessionState`:

- `0` -> disconnected
- `1` -> connecting
- `2` -> connected
- `3`, `4`, `5` -> recovery / reconnecting

Service mode uses:

- Service name: `TrustTunnelEasyService`
- Pipe: `\\.\pipe\trusttunnel-easy`
- Service log: `%ProgramData%\TrustTunnel\vpn_easy_service.log`

Building the official `vpn_easy.dll` native core currently requires the upstream Windows build chain: CMake, MSVC/Visual Studio Build Tools, Conan bootstrap dependencies, and the TrustTunnel Rust components used by the upstream project. If these tools are not in PATH, the GUI can still build and can run the official release-package CLI fallback when `trusttunnel_client.exe` is copied into the output directory.

Upstream references:

- `platform/windows/include/vpn/vpn_easy.h`
- `platform/windows/include/vpn/vpn_easy_service.h`
- `core/include/vpn/vpn.h`
