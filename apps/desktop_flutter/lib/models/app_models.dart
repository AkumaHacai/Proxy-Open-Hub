import 'package:flutter/material.dart';

enum CoreStatus { active, planned }

enum ConnectionPhase {
  idle,
  preparing,
  connecting,
  authenticating,
  connected,
  disconnecting,
}

@immutable
class CoreSpec {
  const CoreSpec({
    required this.id,
    required this.name,
    required this.letter,
    required this.accent,
    required this.protocol,
    required this.listener,
    required this.tagline,
    required this.status,
  });

  final String id;
  final String name;
  final String letter;
  final Color accent;
  final String protocol;
  final String listener;
  final String tagline;
  final CoreStatus status;
}

@immutable
class ServerProfile {
  const ServerProfile({
    required this.id,
    required this.name,
    required this.host,
    required this.countryCode,
    required this.city,
    required this.pingMs,
    required this.dns,
    required this.load,
    required this.tlsVerificationDisabled,
  });

  final String id;
  final String name;
  final String host;
  final String countryCode;
  final String city;
  final int pingMs;
  final String dns;
  final double load;
  final bool tlsVerificationDisabled;

  static ServerProfile fromDesktopStateJson(Map<String, dynamic> json) {
    final endpoint = (json['Endpoint'] as Map?)?.cast<String, dynamic>() ?? {};
    final listener = (json['Listener'] as Map?)?.cast<String, dynamic>() ?? {};
    final testResult =
        (json['TestResult'] as Map?)?.cast<String, dynamic>() ?? {};
    final hostname = endpoint['Hostname']?.toString() ?? '';
    final displayName = json['DisplayName']?.toString() ?? '';
    final countryCode = json['CountryCode']?.toString() ??
        testResult['CountryCode']?.toString() ??
        '';
    final ping = (testResult['PingMs'] as num?)?.toInt() ?? 0;
    final protocol = endpoint['UpstreamProtocol'] == 1 ? 'HTTP/3' : 'HTTP/2';
    final listenerMode = listener['Mode'] == 1 ? 'SOCKS' : 'TUN';
    final certificatePem = endpoint['CertificatePem']?.toString() ?? '';
    final tlsVerificationDisabled = endpoint['SkipVerification'] == true ||
        certificatePem.trim().isNotEmpty;

    return ServerProfile(
      id: json['Id']?.toString() ?? hostname,
      name: displayName.isNotEmpty ? displayName : hostname,
      host: hostname,
      countryCode: countryCode.isNotEmpty ? countryCode : 'TT',
      city: json['CountryName']?.toString() ?? '',
      pingMs: ping,
      dns: '$protocol - $listenerMode',
      load: ping <= 0 ? 0 : (ping / 220).clamp(0.08, 0.95).toDouble(),
      tlsVerificationDisabled: tlsVerificationDisabled,
    );
  }
}

@immutable
class SessionTab {
  const SessionTab({
    required this.id,
    required this.coreId,
    required this.selectedServerId,
    required this.phase,
    required this.connectedAt,
  });

  final int id;
  final String coreId;
  final String selectedServerId;
  final ConnectionPhase phase;
  final DateTime? connectedAt;

  SessionTab copyWith({
    String? selectedServerId,
    ConnectionPhase? phase,
    DateTime? connectedAt,
    bool clearConnectedAt = false,
  }) {
    return SessionTab(
      id: id,
      coreId: coreId,
      selectedServerId: selectedServerId ?? this.selectedServerId,
      phase: phase ?? this.phase,
      connectedAt: clearConnectedAt ? null : connectedAt ?? this.connectedAt,
    );
  }
}

const coreSpecs = <CoreSpec>[
  CoreSpec(
    id: 'trusttunnel',
    name: 'TrustTunnel',
    letter: 'T',
    accent: Color(0xFF1F7A4D),
    protocol: 'HTTP/2',
    listener: 'TUN',
    tagline: 'Native core, vpn_easy',
    status: CoreStatus.active,
  ),
  CoreSpec(
    id: 'sing-box',
    name: 'sing-box',
    letter: 'S',
    accent: Color(0xFFC97428),
    protocol: 'Mixed',
    listener: 'TUN',
    tagline: 'Trusted GitHub release',
    status: CoreStatus.planned,
  ),
  CoreSpec(
    id: 'naiveproxy',
    name: 'NaiveProxy',
    letter: 'N',
    accent: Color(0xFF2563EB),
    protocol: 'H2 CONNECT',
    listener: 'SOCKS',
    tagline: 'Trusted GitHub release',
    status: CoreStatus.planned,
  ),
  CoreSpec(
    id: 'xray-core',
    name: 'Xray-core',
    letter: 'X',
    accent: Color(0xFF7C3AED),
    protocol: 'VLESS',
    listener: 'TUN',
    tagline: 'Trusted GitHub release',
    status: CoreStatus.planned,
  ),
  CoreSpec(
    id: 'hysteria2',
    name: 'Hysteria2',
    letter: 'H',
    accent: Color(0xFFC64454),
    protocol: 'QUIC',
    listener: 'TUN',
    tagline: 'Trusted GitHub release',
    status: CoreStatus.planned,
  ),
];

CoreSpec findCore(String id) {
  return coreSpecs.firstWhere((core) => core.id == id);
}

String phaseLabel(ConnectionPhase phase) {
  return switch (phase) {
    ConnectionPhase.idle => 'Disconnected',
    ConnectionPhase.preparing => 'Preparing',
    ConnectionPhase.connecting => 'Connecting',
    ConnectionPhase.authenticating => 'Authenticating',
    ConnectionPhase.connected => 'Protected',
    ConnectionPhase.disconnecting => 'Disconnecting',
  };
}
