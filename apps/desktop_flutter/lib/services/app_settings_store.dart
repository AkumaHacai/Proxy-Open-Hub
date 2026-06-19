import 'dart:convert';
import 'dart:io';

import '../theme/poh_theme.dart';

class AppSettings {
  const AppSettings({
    required this.themeMode,
    required this.accent,
    required this.accentFollowsCore,
    required this.animationsEnabled,
    required this.animationDurationMs,
    required this.density,
    required this.startupMode,
    required this.defaultCore,
    required this.defaultRoute,
    required this.routesByCore,
    required this.routeRulesByCore,
    required this.activeModeByCore,
    required this.routePresetsByCore,
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
      accentFollowsCore: true,
      animationsEnabled: true,
      animationDurationMs: 520,
      density: 'comfortable',
      startupMode: 'expanded',
      defaultCore: 'TrustTunnel',
      defaultRoute: 'Default',
      routesByCore: {'trusttunnel': 'Default'},
      routeRulesByCore: {'trusttunnel': RouteRules.empty},
      activeModeByCore: {'trusttunnel': 'tun'},
      routePresetsByCore: {
        'trusttunnel': RoutePreset.defaults,
        'naiveproxy': RoutePreset.defaults,
      },
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
      accentFollowsCore: _accentFollowsCoreFromJson(json, defaults),
      animationsEnabled:
          json['animationsEnabled'] as bool? ?? defaults.animationsEnabled,
      animationDurationMs: ((json['animationDurationMs'] as num?)?.toInt() ??
              defaults.animationDurationMs)
          .clamp(150, 1500),
      density: json['density']?.toString() ?? defaults.density,
      startupMode: json['startupMode']?.toString() ?? defaults.startupMode,
      defaultCore: json['defaultCore']?.toString() ?? defaults.defaultCore,
      defaultRoute: json['defaultRoute']?.toString() ?? defaults.defaultRoute,
      routesByCore: _stringMap(
        json['routesByCore'],
        {
          'trusttunnel':
              json['defaultRoute']?.toString() ?? defaults.defaultRoute
        },
      ),
      routeRulesByCore: _routeRulesMap(
        json['routeRulesByCore'],
        defaults.routeRulesByCore,
      ),
      activeModeByCore: _stringMap(
        json['activeModeByCore'],
        defaults.activeModeByCore,
      ),
      routePresetsByCore: _routePresetMap(
        json['routePresetsByCore'],
        defaults.routePresetsByCore,
      ),
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
  final bool accentFollowsCore;
  final bool animationsEnabled;
  final int animationDurationMs;
  final String density;
  final String startupMode;
  final String defaultCore;
  final String defaultRoute;
  final Map<String, String> routesByCore;
  final Map<String, RouteRules> routeRulesByCore;
  final Map<String, String> activeModeByCore;
  final Map<String, List<RoutePreset>> routePresetsByCore;
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
    bool? accentFollowsCore,
    bool? animationsEnabled,
    int? animationDurationMs,
    String? density,
    String? startupMode,
    String? defaultCore,
    String? defaultRoute,
    Map<String, String>? routesByCore,
    Map<String, RouteRules>? routeRulesByCore,
    Map<String, String>? activeModeByCore,
    Map<String, List<RoutePreset>>? routePresetsByCore,
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
      accentFollowsCore: accentFollowsCore ?? this.accentFollowsCore,
      animationsEnabled: animationsEnabled ?? this.animationsEnabled,
      animationDurationMs:
          (animationDurationMs ?? this.animationDurationMs).clamp(150, 1500),
      density: density ?? this.density,
      startupMode: startupMode ?? this.startupMode,
      defaultCore: defaultCore ?? this.defaultCore,
      defaultRoute: defaultRoute ?? this.defaultRoute,
      routesByCore: Map.unmodifiable(routesByCore ?? this.routesByCore),
      routeRulesByCore:
          Map.unmodifiable(routeRulesByCore ?? this.routeRulesByCore),
      activeModeByCore:
          Map.unmodifiable(activeModeByCore ?? this.activeModeByCore),
      routePresetsByCore: _immutablePresetMap(
        routePresetsByCore ?? this.routePresetsByCore,
      ),
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
      'accentFollowsCore': accentFollowsCore,
      'animationsEnabled': animationsEnabled,
      'animationDurationMs': animationDurationMs,
      'density': density,
      'startupMode': startupMode,
      'defaultCore': defaultCore,
      'defaultRoute': defaultRoute,
      'routesByCore': routesByCore,
      'routeRulesByCore': {
        for (final entry in routeRulesByCore.entries)
          entry.key: entry.value.toJson(),
      },
      'activeModeByCore': activeModeByCore,
      'routePresetsByCore': {
        for (final entry in routePresetsByCore.entries)
          entry.key: [
            for (final preset in entry.value) preset.toJson(),
          ],
      },
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

class RouteRules {
  const RouteRules({
    this.appExeNames = '',
    this.includedDomains = '',
    this.excludedDomains = '',
    this.includedCidrs = '',
    this.excludedCidrs = '',
  });

  static const empty = RouteRules();

  factory RouteRules.fromJson(Object? value) {
    if (value is! Map) {
      return empty;
    }

    return RouteRules(
      appExeNames: value['appExeNames']?.toString() ?? '',
      includedDomains: value['includedDomains']?.toString() ?? '',
      excludedDomains: value['excludedDomains']?.toString() ?? '',
      includedCidrs: value['includedCidrs']?.toString() ?? '',
      excludedCidrs: value['excludedCidrs']?.toString() ?? '',
    );
  }

  factory RouteRules.fromPresetJson(Object? value) {
    if (value is! Map) {
      return empty;
    }

    return RouteRules(
      appExeNames:
          _stringListField(value['appExeNames'] ?? value['app_exe_names']),
      includedDomains: _stringListField(
          value['includedDomains'] ?? value['included_domains']),
      excludedDomains: _stringListField(
          value['excludedDomains'] ?? value['excluded_domains']),
      includedCidrs:
          _stringListField(value['includedCidrs'] ?? value['included_cidrs']),
      excludedCidrs:
          _stringListField(value['excludedCidrs'] ?? value['excluded_cidrs']),
    );
  }

  final String appExeNames;
  final String includedDomains;
  final String excludedDomains;
  final String includedCidrs;
  final String excludedCidrs;

  bool get hasAnyRule {
    return appExeNames.trim().isNotEmpty ||
        includedDomains.trim().isNotEmpty ||
        excludedDomains.trim().isNotEmpty ||
        includedCidrs.trim().isNotEmpty ||
        excludedCidrs.trim().isNotEmpty;
  }

  RouteRules copyWith({
    String? appExeNames,
    String? includedDomains,
    String? excludedDomains,
    String? includedCidrs,
    String? excludedCidrs,
  }) {
    return RouteRules(
      appExeNames: appExeNames ?? this.appExeNames,
      includedDomains: includedDomains ?? this.includedDomains,
      excludedDomains: excludedDomains ?? this.excludedDomains,
      includedCidrs: includedCidrs ?? this.includedCidrs,
      excludedCidrs: excludedCidrs ?? this.excludedCidrs,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'appExeNames': appExeNames,
      'includedDomains': includedDomains,
      'excludedDomains': excludedDomains,
      'includedCidrs': includedCidrs,
      'excludedCidrs': excludedCidrs,
    };
  }
}

class RoutePreset {
  const RoutePreset({
    required this.id,
    required this.name,
    required this.description,
    required this.rules,
  });

  static const defaults = [
    RoutePreset(
      id: 'proxy_all',
      name: 'Proxy Everything',
      description: 'All device traffic is routed through the proxy server.',
      rules: RouteRules(
        includedCidrs: '0.0.0.0/0\n::/0',
      ),
    ),
    RoutePreset(
      id: 'bypass_ru',
      name: 'Bypass RU',
      description:
          'Local services and popular RU domains go direct, everything else via proxy.',
      rules: RouteRules(
        excludedDomains:
            'yandex.ru\nvk.com\nmail.ru\ngosuslugi.ru\nsberbank.ru',
        includedCidrs: '0.0.0.0/0\n::/0',
      ),
    ),
    RoutePreset(
      id: 'list_only',
      name: 'List Only via Proxy',
      description:
          'Only the traffic for domains and IPs you manually add will go through the proxy.',
      rules: RouteRules.empty,
    ),
  ];

  factory RoutePreset.fromJson(Object? value) {
    if (value is! Map) {
      return defaults.first;
    }

    return RoutePreset(
      id: value['id']?.toString() ?? '',
      name: value['name']?.toString() ??
          value['name_en']?.toString() ??
          'Route preset',
      description: value['description']?.toString() ?? '',
      rules: RouteRules.fromPresetJson(value['rules']),
    );
  }

  final String id;
  final String name;
  final String description;
  final RouteRules rules;

  RoutePreset copyWith({
    String? id,
    String? name,
    String? description,
    RouteRules? rules,
  }) {
    return RoutePreset(
      id: id ?? this.id,
      name: name ?? this.name,
      description: description ?? this.description,
      rules: rules ?? this.rules,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'name': name,
      'description': description,
      'rules': rules.toJson(),
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

Map<String, RouteRules> _routeRulesMap(
  Object? value,
  Map<String, RouteRules> fallback,
) {
  if (value is! Map) {
    return Map.unmodifiable(fallback);
  }

  final result = <String, RouteRules>{};
  for (final entry in value.entries) {
    final key = entry.key?.toString().trim() ?? '';
    if (key.isNotEmpty) {
      result[key] = RouteRules.fromJson(entry.value);
    }
  }

  return Map.unmodifiable(result.isEmpty ? fallback : result);
}

Map<String, List<RoutePreset>> _routePresetMap(
  Object? value,
  Map<String, List<RoutePreset>> fallback,
) {
  if (value is! Map) {
    return _immutablePresetMap(fallback);
  }

  final result = <String, List<RoutePreset>>{};
  for (final entry in value.entries) {
    final key = entry.key?.toString().trim() ?? '';
    final presets = entry.value;
    if (key.isNotEmpty && presets is List) {
      final parsed = presets
          .map(RoutePreset.fromJson)
          .where((preset) => preset.id.trim().isNotEmpty)
          .toList(growable: false);
      if (parsed.isNotEmpty) {
        result[key] = parsed;
      }
    }
  }

  return _immutablePresetMap(result.isEmpty ? fallback : result);
}

Map<String, List<RoutePreset>> _immutablePresetMap(
  Map<String, List<RoutePreset>> value,
) {
  return Map.unmodifiable({
    for (final entry in value.entries)
      entry.key: List<RoutePreset>.unmodifiable(entry.value),
  });
}

String _stringListField(Object? value) {
  if (value is List) {
    return value.map((entry) => entry.toString()).join('\n');
  }

  return value?.toString() ?? '';
}

Map<String, String> _stringMap(Object? value, Map<String, String> fallback) {
  if (value is! Map) {
    return Map.unmodifiable(fallback);
  }

  final result = <String, String>{};
  for (final entry in value.entries) {
    final key = entry.key?.toString().trim() ?? '';
    final mapValue = entry.value?.toString().trim() ?? '';
    if (key.isNotEmpty && mapValue.isNotEmpty) {
      result[key] = mapValue;
    }
  }

  return Map.unmodifiable(result.isEmpty ? fallback : result);
}

bool _accentFollowsCoreFromJson(
  Map<String, dynamic> json,
  AppSettings defaults,
) {
  final stored = json['accentFollowsCore'];
  if (stored is bool) {
    return stored;
  }

  return json.containsKey('accent') ? false : defaults.accentFollowsCore;
}

T _enumByName<T extends Enum>(List<T> values, String? name, T fallback) {
  for (final value in values) {
    if (value.name == name) {
      return value;
    }
  }

  return fallback;
}
