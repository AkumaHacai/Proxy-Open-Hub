# Config Format

The app stores a typed profile and generates TrustTunnel TOML only at the boundary to the native adapter.

Supported TOML fields:

- top-level: `loglevel`, `vpn_mode`, `killswitch_enabled`, `killswitch_allow_ports`, `post_quantum_group_enabled`, `exclusions`
- `[endpoint]`: `hostname`, `addresses`, `has_ipv6`, `username`, `password`, `client_random`, `skip_verification`, `certificate`, `upstream_protocol`, `anti_dpi`, `dns_upstreams`
- `[listener.tun]`: `bound_if`, `included_routes`, `excluded_routes`, `mtu_size`, `tcp_recv_buf_size`, `tcp_send_buf_size`, `change_system_dns`, `device_name`, `use_existing`
- `[listener.socks]`: `address`, `username`, `password`

Passwords and client random values are imported as secret candidates and represented in profiles only as `secret://trusttunnel/...` refs.
