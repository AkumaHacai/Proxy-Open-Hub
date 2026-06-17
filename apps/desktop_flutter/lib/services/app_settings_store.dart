import 'dart:convert';
import 'dart:io';

import '../theme/poh_theme.dart';

class AppSettings {
  const AppSettings({
    required this.themeMode,
    required this.accent,
    required this.animationsEnabled,
    required this.animationDurationMs,
    required this.density,
    required this.startupMode,
    required this.defaultCore,
    required this.defaultRoute,
    required this.autoConnect,
    required this.socksLan,
    required this.socksAddress,
    required this.httpEnabled,
    required this.httpAddress,
    required this.pingHost,
    required this.httpsUrl,
    required this.timeoutSeconds,
  });

  factory AppSettings.defaults() {
    return const AppSettings(
      themeMode: PohThemeMode.dark,
      accent: PohAccent.forest,
      animationsEnabled: true,
      animationDurationMs: 520,
      density: 'comfortable',
      startupMode: 'expanded',
      defaultCore: 'TrustTunnel',
      defaultRoute: 'Default',
      autoConnect: false,
      socksLan: false,
      socksAddress: '127.0.0.1:1080',
      httpEnabled: true,
      httpAddress: '127.0.0.1:8080',
      pingHost: '8.8.8.8',
      httpsUrl: 'https://www.google.com/generate_204',
      timeoutSeconds: 5,
    );
  }

  factory AppSettings.fromJson(Map<String, dynamic> json) {
    final defaults = AppSettings.defaults();
    return defaults.copyWith(
      themeMode: _enumByName(
        PohThemeMode.values,
        json['themeMode']?.toString(),
        defaults.themeMode,
      ),
      accent: _enumByName(
        PohAccent.values,
        json['accent']?.toString(),
        defaults.accent,
      ),
      animationsEnabled:
          json['animationsEnabled'] as bool? ?? defaults.animationsEnabled,
      animationDurationMs: ((json['animationDurationMs'] as num?)?.toInt() ??
              defaults.animationDurationMs)
          .clamp(150, 1500),
      density: json['density']?.toString() ?? defaults.density,
      startupMode: json['startupMode']?.toString() ?? defaults.startupMode,
      defaultCore: json['defaultCore']?.toString() ?? defaults.defaultCore,
      defaultRoute: json['defaultRoute']?.toString() ?? defaults.defaultRoute,
      autoConnect: json['autoConnect'] as bool? ?? defaults.autoConnect,
      socksLan: json['socksLan'] as bool? ?? defaults.socksLan,
      socksAddress: json['socksAddress']?.toString() ?? defaults.socksAddress,
      httpEnabled: json['httpEnabled'] as bool? ?? defaults.httpEnabled,
      httpAddress: json['httpAddress']?.toString() ?? defaults.httpAddress,
      pingHost: json['pingHost']?.toString() ?? defaults.pingHost,
      httpsUrl: json['httpsUrl']?.toString() ?? defaults.httpsUrl,
      timeoutSeconds:
          ((json['timeoutSeconds'] as num?)?.toInt() ?? defaults.timeoutSeconds)
              .clamp(1, 30),
    );
  }

  final PohThemeMode themeMode;
  final PohAccent accent;
  final bool animationsEnabled;
  final int animationDurationMs;
  final String density;
  final String startupMode;
  final String defaultCore;
  final String defaultRoute;
  final bool autoConnect;
  final bool socksLan;
  final String socksAddress;
  final bool httpEnabled;
  final String httpAddress;
  final String pingHost;
  final String httpsUrl;
  final int timeoutSeconds;

  AppSettings copyWith({
    PohThemeMode? themeMode,
    PohAccent? accent,
    bool? animationsEnabled,
    int? animationDurationMs,
    String? density,
    String? startupMode,
    String? defaultCore,
    String? defaultRoute,
    bool? autoConnect,
    bool? socksLan,
    String? socksAddress,
    bool? httpEnabled,
    String? httpAddress,
    String? pingHost,
    String? httpsUrl,
    int? timeoutSeconds,
  }) {
    return AppSettings(
      themeMode: themeMode ?? this.themeMode,
      accent: accent ?? this.accent,
      animationsEnabled: animationsEnabled ?? this.animationsEnabled,
      animationDurationMs:
          (animationDurationMs ?? this.animationDurationMs).clamp(150, 1500),
      density: density ?? this.density,
      startupMode: startupMode ?? this.startupMode,
      defaultCore: defaultCore ?? this.defaultCore,
      defaultRoute: defaultRoute ?? this.defaultRoute,
      autoConnect: autoConnect ?? this.autoConnect,
      socksLan: socksLan ?? this.socksLan,
      socksAddress: socksAddress ?? this.socksAddress,
      httpEnabled: httpEnabled ?? this.httpEnabled,
      httpAddress: httpAddress ?? this.httpAddress,
      pingHost: pingHost ?? this.pingHost,
      httpsUrl: httpsUrl ?? this.httpsUrl,
      timeoutSeconds: (timeoutSeconds ?? this.timeoutSeconds).clamp(1, 30),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'themeMode': themeMode.name,
      'accent': accent.name,
      'animationsEnabled': animationsEnabled,
      'animationDurationMs': animationDurationMs,
      'density': density,
      'startupMode': startupMode,
      'defaultCore': defaultCore,
      'defaultRoute': defaultRoute,
      'autoConnect': autoConnect,
      'socksLan': socksLan,
      'socksAddress': socksAddress,
      'httpEnabled': httpEnabled,
      'httpAddress': httpAddress,
      'pingHost': pingHost,
      'httpsUrl': httpsUrl,
      'timeoutSeconds': timeoutSeconds,
    };
  }
}

class AppSettingsStore {
  const AppSettingsStore();

  Future<AppSettings> load() async {
    final file = await settingsFile();
    if (!await file.exists()) {
      return AppSettings.defaults();
    }

    try {
      final decoded = jsonDecode(await file.readAsString());
      if (decoded is Map<String, dynamic>) {
        return AppSettings.fromJson(decoded);
      }
    } on FormatException {
      // Corrupt settings should not block the app from opening.
    } on FileSystemException {
      // Fall back to defaults if the profile directory is temporarily locked.
    }

    return AppSettings.defaults();
  }

  Future<void> save(AppSettings settings) async {
    final file = await settingsFile();
    await file.parent.create(recursive: true);
    const encoder = JsonEncoder.withIndent('  ');
    await file.writeAsString(encoder.convert(settings.toJson()));
  }

  Future<File> settingsFile() async {
    final localAppData = Platform.environment['LOCALAPPDATA'];
    final base = localAppData == null || localAppData.isEmpty
        ? Directory.current.path
        : localAppData;

    return File(
      '$base${Platform.pathSeparator}ProxyOpenHub'
      '${Platform.pathSeparator}app-settings.json',
    );
  }
}

T _enumByName<T extends Enum>(List<T> values, String? name, T fallback) {
  for (final value in values) {
    if (value.name == name) {
      return value;
    }
  }

  return fallback;
}
