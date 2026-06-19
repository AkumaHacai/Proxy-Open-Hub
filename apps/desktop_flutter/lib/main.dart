import 'dart:async';
import 'dart:convert';
import 'dart:io' show Platform;
import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'models/app_models.dart';
import 'screens/import_profile_shell.dart';
import 'screens/logs_shell.dart';
import 'screens/routes_shell.dart';
import 'screens/server_profile_editor_shell.dart';
import 'screens/settings_shell.dart';
import 'services/app_settings_store.dart';
import 'services/backend_session_service.dart';
import 'services/traffic_metrics_service.dart';
import 'services/window_controls.dart';
import 'theme/poh_theme.dart';
import 'widgets/sparkline.dart';
import 'widgets/theme_reveal_wrapper.dart';

void main() {
  runApp(const ProxyOpenHubApp());
}

enum _DesktopOverlay {
  none,
  settings,
  routes,
  logs,
  importProfile,
  editProfile
}

/// The two fixed window sizes (logical px). Shared by the native resize and the
/// in-window `OverflowBox` so the inner layout is always measured at its final
/// size and never reflows on intermediate animation frames.
const Size kExpandedWindow = Size(960, 660);
const Size kCompactWindow = Size(360, 650);

class ProxyOpenHubApp extends StatefulWidget {
  const ProxyOpenHubApp({super.key});

  @override
  State<ProxyOpenHubApp> createState() => _ProxyOpenHubAppState();
}

class _ProxyOpenHubAppState extends State<ProxyOpenHubApp>
    with WidgetsBindingObserver {
  var _themeMode = PohThemeMode.dark;
  var _accent = PohAccent.forest;
  var _accentFollowsCore = true;
  var _compact = false;
  // Debug helper: launch with POH_DEBUG_COMPACT=1 / POH_DEBUG_OVERLAY=settings
  // to land directly on a state for screenshots. Harmless when unset.
  final bool _debugCompact = Platform.environment['POH_DEBUG_COMPACT'] == '1';
  final String _debugCoreId = Platform.environment['POH_DEBUG_CORE'] ?? '';
  var _animationsEnabled = true;
  var _animationDurationMs = 520;
  var _themeRevealInFlight = false;
  var _density = 'comfortable';
  var _startupMode = 'expanded';
  var _defaultCore = 'TrustTunnel';
  var _defaultRoute = 'Default';
  var _routesByCore = <String, String>{'trusttunnel': 'Default'};
  var _routeRulesByCore = <String, RouteRules>{
    'trusttunnel': RouteRules.empty,
  };
  var _activeModeByCore = <String, String>{'trusttunnel': 'tun'};
  var _routePresetsByCore = <String, List<RoutePreset>>{
    'trusttunnel': RoutePreset.defaults,
    'naiveproxy': RoutePreset.defaults,
  };
  var _autoConnect = false;
  var _socksLan = false;
  var _socksAddress = '127.0.0.1:1080';
  var _httpEnabled = true;
  var _httpAddress = '127.0.0.1:8080';
  var _pingHost = '8.8.8.8';
  var _httpsUrl = 'https://www.google.com/generate_204';
  var _timeoutSeconds = 5;
  var _coreMenuOpen = false;
  var _activeTabId = 1;
  var _nextTabId = 1;
  var _progress = 0.0;
  var _download = List<double>.filled(16, 0);
  var _upload = List<double>.filled(16, 0);
  var _coreSpecs = coreSpecs;
  var _servers = <ServerProfile>[];
  var _profilesLoaded = false;
  var _overlay = _DesktopOverlay.none;
  ServerProfile? _editingServer;
  String? _connectionMessage;
  final _settingsStore = const AppSettingsStore();
  final _backendSessionService = const BackendSessionService();
  final _installingCoreIds = <String>{};
  final _trafficMetricsService = TrafficMetricsService();
  final _themeRevealKey = GlobalKey<ThemeRevealWrapperState>();
  final _tabs = <SessionTab>[];
  final _timers = <Timer>[];
  Timer? _trafficTimer;
  Timer? _clockTimer;
  Timer? _sessionStatusTimer;
  var _trafficSampleInFlight = false;
  var _sessionStatusInFlight = false;
  SupervisedSession? _supervisorSession;
  var _expandReveal = 0;

  SessionTab get _activeTab {
    return _tabs.firstWhere(
      (tab) => tab.id == _activeTabId,
      orElse: () => const SessionTab(
        id: 1,
        coreId: 'trusttunnel',
        selectedServerId: '',
        phase: ConnectionPhase.idle,
        connectedAt: null,
      ),
    );
  }

  CoreSpec get _activeCore => findCoreIn(_coreSpecs, _activeTab.coreId);

  String get _activeRoute {
    return _routesByCore[_activeCore.id] ??
        _defaultRouteForCore(_activeCore.id);
  }

  String get _activeMode {
    return _activeModeByCore[_activeCore.id] ?? 'tun';
  }

  List<RoutePreset> get _activeRoutePresets {
    return _routePresetsByCore[_activeCore.id] ?? RoutePreset.defaults;
  }

  List<ServerProfile> get _activeServers {
    return _servers
        .where((server) => server.coreId == _activeCore.id)
        .toList(growable: false);
  }

  ServerProfile get _selectedServer {
    final servers = _activeServers;
    if (servers.isEmpty) {
      return ServerProfile(
        id: '',
        coreId: _activeCore.id,
        name: 'No server selected',
        host: 'Import or add a profile',
        countryCode: '--',
        city: '',
        pingMs: 0,
        dns: '${_activeCore.protocol} - ${_activeCore.listener}',
        load: 0,
        tlsVerificationDisabled: false,
      );
    }

    return servers.firstWhere(
      (server) => server.id == _activeTab.selectedServerId,
      orElse: () => servers.first,
    );
  }

  bool get _connected => _activeTab.phase == ConnectionPhase.connected;

  Color get _themeAccentColor {
    return _accentFollowsCore ? _activeCore.accent : _accent.color;
  }

  Duration get _motionDuration {
    return _animationsEnabled
        ? Duration(milliseconds: _animationDurationMs)
        : Duration.zero;
  }

  Duration get _themeDuration {
    return _animationsEnabled ? PohMotion.theme : Duration.zero;
  }

  bool get _working {
    return _activeTab.phase == ConnectionPhase.preparing ||
        _activeTab.phase == ConnectionPhase.connecting ||
        _activeTab.phase == ConnectionPhase.authenticating;
  }

  AppSettings get _settings {
    return AppSettings(
      themeMode: _themeMode,
      accent: _accent,
      accentFollowsCore: _accentFollowsCore,
      animationsEnabled: _animationsEnabled,
      animationDurationMs: _animationDurationMs,
      density: _density,
      startupMode: _startupMode,
      defaultCore: _defaultCore,
      defaultRoute: _defaultRoute,
      routesByCore: _routesByCore,
      routeRulesByCore: _routeRulesByCore,
      activeModeByCore: _activeModeByCore,
      routePresetsByCore: _routePresetsByCore,
      autoConnect: _autoConnect,
      socksLan: _socksLan,
      socksAddress: _socksAddress,
      httpEnabled: _httpEnabled,
      httpAddress: _httpAddress,
      pingHost: _pingHost,
      httpsUrl: _httpsUrl,
      timeoutSeconds: _timeoutSeconds,
    );
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _loadSettings();
    _loadCoreCatalog();
    _loadProfiles();
    _applyDebugOverlay();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.detached) {
      final supervisor = _supervisorSession;
      if (supervisor != null) {
        _supervisorSession = null;
        unawaited(supervisor.stop());
      }
    }
  }

  void _applyDebugOverlay() {
    final overlay = Platform.environment['POH_DEBUG_OVERLAY'];
    if ((overlay == null || overlay.isEmpty) && !_debugCompact) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) {
        return;
      }
      if (_debugCompact) {
        await WindowControls.setWindowSize(width: 360, height: 650);
      }
      if (!mounted) {
        return;
      }
      setState(() {
        if (_debugCompact) {
          _compact = true;
        }
        if (overlay != null && overlay.isNotEmpty) {
          _overlay = switch (overlay) {
            'logs' => _DesktopOverlay.logs,
            'routes' => _DesktopOverlay.routes,
            'import' => _DesktopOverlay.importProfile,
            'edit' => _DesktopOverlay.editProfile,
            _ => _DesktopOverlay.settings,
          };
        }
      });
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _clearTimers();
    _stopTraffic();
    // Fire-and-forget stop so the supervisor gets the command before the pipe
    // closes on process exit. Belt-and-suspenders: pipe closure is sufficient on
    // its own, but sending the stop command first enables an orderly teardown.
    _supervisorSession?.stop();
    _supervisorSession = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final accent = _themeAccentColor;
    final activeServers = _activeServers;

    return MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: buildPohTheme(mode: PohThemeMode.light, accentColor: accent),
      darkTheme: buildPohTheme(mode: PohThemeMode.dark, accentColor: accent),
      themeMode: _themeMode.materialMode,
      themeAnimationDuration: _themeDuration,
      themeAnimationCurve: PohMotion.standard,
      home: ThemeRevealWrapper(
        key: _themeRevealKey,
        themeMode: _themeMode,
        accent: accent,
        duration: _themeDuration,
        builder: (context, _) {
          final palette = PohPalette.of(context);
          return Scaffold(
            backgroundColor: palette.background,
            body: ColoredBox(
              color: palette.background,
              child: Stack(
                children: [
                  Positioned.fill(
                    child: _DesktopWindow(
                      compact: _compact,
                      coreMenuOpen: _coreMenuOpen,
                      cores: _coreSpecs,
                      activeCore: _activeCore,
                      activeTab: _activeTab,
                      selectedServer: _selectedServer,
                      servers: activeServers,
                      profilesLoaded: _profilesLoaded,
                      tabs: _tabs,
                      download: _download,
                      upload: _upload,
                      progress: _progress,
                      connected: _connected,
                      working: _working,
                      connectionMessage: _connectionMessage,
                      themeMode: _themeMode,
                      motionDuration: _morphDuration,
                      expandReveal: _expandReveal,
                      onToggleCompact: _toggleCompact,
                      onToggleTheme: _toggleTheme,
                      onToggleCoreMenu: () {
                        setState(() => _coreMenuOpen = !_coreMenuOpen);
                      },
                      onDismissCoreMenu: () {
                        setState(() => _coreMenuOpen = false);
                      },
                      onSelectTab: _selectTab,
                      onAddCore: _addCore,
                      onInstallCore: _installCore,
                      onSelectServer: _selectServer,
                      onEditServer: _editServer,
                      onToggleConnection: _toggleConnection,
                      onOpenSettings: () {
                        setState(() => _overlay = _DesktopOverlay.settings);
                      },
                      activeRoute: _activeRoute,
                      activeMode: _activeMode,
                      routePresets: _activeRoutePresets,
                      onSelectRouteMode: _selectActiveMode,
                      onSelectRoutePreset: _selectActivePreset,
                      onOpenRoutes: () {
                        setState(() => _overlay = _DesktopOverlay.routes);
                      },
                      onOpenLogs: () {
                        setState(() => _overlay = _DesktopOverlay.logs);
                      },
                      onOpenImportProfile: () {
                        setState(
                          () => _overlay = _DesktopOverlay.importProfile,
                        );
                      },
                    ),
                  ),
                  if (_overlay != _DesktopOverlay.none)
                    _OverlayHost(
                      compact: _compact,
                      motion: _motionDuration,
                      onDismiss: () {
                        setState(() => _overlay = _DesktopOverlay.none);
                      },
                      child: _overlay == _DesktopOverlay.settings
                          ? SettingsShell(
                              themeMode: _themeMode,
                              accent: _accent,
                              accentFollowsCore: _accentFollowsCore,
                              animationsEnabled: _animationsEnabled,
                              animationDurationMs: _animationDurationMs,
                              density: _density,
                              startupMode: _startupMode,
                              defaultCore: _defaultCore,
                              autoConnect: _autoConnect,
                              socksLan: _socksLan,
                              socksAddress: _socksAddress,
                              httpEnabled: _httpEnabled,
                              httpAddress: _httpAddress,
                              pingHost: _pingHost,
                              httpsUrl: _httpsUrl,
                              timeoutSeconds: _timeoutSeconds,
                              onThemeModeChanged: (value, origin) {
                                unawaited(_setThemeModeFrom(value, origin));
                              },
                              onAccentChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  accent: value,
                                  accentFollowsCore: false,
                                ));
                              },
                              onAccentFollowsCoreChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  accentFollowsCore: value,
                                ));
                              },
                              onAnimationsEnabledChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  animationsEnabled: value,
                                ));
                              },
                              onAnimationDurationChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  animationDurationMs: value,
                                ));
                              },
                              onDensityChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  density: value,
                                ));
                              },
                              onStartupModeChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  startupMode: value,
                                ));
                              },
                              onDefaultCoreChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  defaultCore: value,
                                ));
                              },
                              onAutoConnectChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  autoConnect: value,
                                ));
                              },
                              onSocksLanChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  socksLan: value,
                                ));
                              },
                              onSocksAddressChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  socksAddress: value,
                                ));
                              },
                              onHttpEnabledChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  httpEnabled: value,
                                ));
                              },
                              onHttpAddressChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  httpAddress: value,
                                ));
                              },
                              onPingHostChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  pingHost: value,
                                ));
                              },
                              onHttpsUrlChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  httpsUrl: value,
                                ));
                              },
                              onTimeoutChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  timeoutSeconds: value,
                                ));
                              },
                              onReset: _applySettings,
                              onSave: () {
                                unawaited(_saveSettings());
                                setState(() => _overlay = _DesktopOverlay.none);
                              },
                              onClose: () {
                                setState(() => _overlay = _DesktopOverlay.none);
                              },
                            )
                          : _overlay == _DesktopOverlay.routes
                              ? RoutesShell(
                                  activeCoreId: _activeCore.id,
                                  sessionService: _backendSessionService,
                                  cores: _coreSpecs,
                                  routesByCore: _routesByCore,
                                  routeRulesByCore: _routeRulesByCore,
                                  activeModeByCore: _activeModeByCore,
                                  routePresetsByCore: _routePresetsByCore,
                                  onRoutesChanged: _applyRouteSettings,
                                  onClose: () {
                                    setState(
                                      () => _overlay = _DesktopOverlay.none,
                                    );
                                  },
                                )
                              : _overlay == _DesktopOverlay.importProfile
                                  ? ImportProfileShell(
                                      sessionService: _backendSessionService,
                                      cores: _coreSpecs,
                                      activeCoreId: _activeCore.id,
                                      onImported: (_) {
                                        unawaited(_loadProfiles());
                                        setState(
                                          () => _overlay = _DesktopOverlay.none,
                                        );
                                      },
                                      onClose: () {
                                        setState(
                                          () => _overlay = _DesktopOverlay.none,
                                        );
                                      },
                                    )
                                  : _overlay == _DesktopOverlay.editProfile &&
                                          _editingServer != null
                                      ? ServerProfileEditorShell(
                                          sessionService:
                                              _backendSessionService,
                                          core: _activeCore,
                                          profile: _editingServer!,
                                          onSaved: (_) {
                                            unawaited(_loadProfiles());
                                          },
                                          onClose: () {
                                            setState(() {
                                              _overlay = _DesktopOverlay.none;
                                              _editingServer = null;
                                            });
                                          },
                                        )
                                      : LogsShell(
                                          sessionService:
                                              _backendSessionService,
                                          onClose: () {
                                            setState(
                                              () => _overlay =
                                                  _DesktopOverlay.none,
                                            );
                                          },
                                        ),
                    ),
                ],
              ),
            ),
          );
        },
      ),
    );
  }

  void _selectTab(int id) {
    if (id == _activeTabId) {
      return;
    }

    setState(() {
      _activeTabId = id;
      _coreMenuOpen = false;
      _progress = _activeTab.phase == ConnectionPhase.connected ? 1 : 0;
    });
  }

  Future<void> _loadSettings() async {
    final settings = await _settingsStore.load();
    if (!mounted) {
      return;
    }

    _applySettings(
      settings,
      persist: false,
      applyStartupModeToCurrentWindow: true,
    );
  }

  Future<void> _loadCoreCatalog() async {
    try {
      final catalog = await _backendSessionService.coreCatalog();
      if (!mounted) {
        return;
      }

      final entriesByCore = {
        for (final entry in catalog.cores) entry.coreId: entry,
      };
      setState(() {
        _coreSpecs = [
          for (final spec in coreSpecs)
            _specFromCatalogEntry(
              spec,
              entriesByCore[spec.id],
            ),
        ];
      });
    } on Object {
      // The app can still render with the built-in static catalog when the Rust
      // backend has not been built yet. This keeps first-run UI development
      // usable without weakening the runtime install path.
    }
  }

  CoreSpec _specFromCatalogEntry(
    CoreSpec spec,
    BackendCoreCatalogEntry? entry,
  ) {
    if (entry == null) {
      return spec;
    }

    final openable = spec.id == 'trusttunnel' || entry.installed;
    final installable = entry.installable && !entry.installed;
    return spec.copyWith(
      name: entry.displayName.isNotEmpty ? entry.displayName : spec.name,
      tagline: _catalogTagline(spec, entry),
      status: openable ? CoreStatus.active : CoreStatus.planned,
      installable: installable,
      installing: _installingCoreIds.contains(spec.id),
    );
  }

  String _catalogTagline(
    CoreSpec spec,
    BackendCoreCatalogEntry entry,
  ) {
    if (entry.activeVersion != null) {
      return 'Installed ${entry.activeVersion}';
    }
    if (spec.id == 'trusttunnel') {
      return 'Native core, managed bundle';
    }
    if (entry.installable) {
      return 'Ready to install';
    }
    if (entry.sourceStatus == 'planned') {
      return 'Trusted source planned';
    }
    return 'Locked until pinned';
  }

  void _applySettings(
    AppSettings settings, {
    bool persist = true,
    bool applyStartupModeToCurrentWindow = false,
  }) {
    setState(() {
      _themeMode = settings.themeMode;
      _accent = settings.accent;
      _accentFollowsCore = settings.accentFollowsCore;
      _animationsEnabled = settings.animationsEnabled;
      _animationDurationMs = settings.animationDurationMs;
      _density = settings.density;
      _startupMode = settings.startupMode;
      _defaultCore = settings.defaultCore;
      _defaultRoute = settings.defaultRoute;
      _routesByCore = Map.of(settings.routesByCore);
      _routesByCore.putIfAbsent('trusttunnel', () => _defaultRoute);
      _routeRulesByCore = Map.of(settings.routeRulesByCore);
      _routeRulesByCore.putIfAbsent(
        'trusttunnel',
        () => RouteRules.empty,
      );
      _activeModeByCore = Map.of(settings.activeModeByCore);
      _activeModeByCore.putIfAbsent('trusttunnel', () => 'tun');
      _routePresetsByCore = {
        for (final entry in settings.routePresetsByCore.entries)
          entry.key: List<RoutePreset>.of(entry.value),
      };
      _routePresetsByCore.putIfAbsent(
        'trusttunnel',
        () => RoutePreset.defaults,
      );
      _autoConnect = settings.autoConnect;
      _socksLan = settings.socksLan;
      _socksAddress = settings.socksAddress;
      _httpEnabled = settings.httpEnabled;
      _httpAddress = settings.httpAddress;
      _pingHost = settings.pingHost;
      _httpsUrl = settings.httpsUrl;
      _timeoutSeconds = settings.timeoutSeconds;
      if (applyStartupModeToCurrentWindow) {
        _compact = _debugCompact || settings.startupMode == 'compact';
      }
    });

    if (persist) {
      unawaited(_settingsStore.save(settings));
    }
  }

  Future<void> _saveSettings() async {
    await _settingsStore.save(_settings);
  }

  void _applyRouteSettings(
    Map<String, String> routesByCore,
    Map<String, RouteRules> routeRulesByCore,
    Map<String, String> activeModeByCore,
    Map<String, List<RoutePreset>> routePresetsByCore,
  ) {
    final nextRoutes = Map<String, String>.of(routesByCore);
    final nextRules = Map<String, RouteRules>.of(routeRulesByCore);
    final nextModes = Map<String, String>.of(activeModeByCore);
    final nextPresets = {
      for (final entry in routePresetsByCore.entries)
        entry.key: List<RoutePreset>.of(entry.value),
    };
    final trustTunnelRoute =
        nextRoutes['trusttunnel'] ?? _defaultRouteForCore('trusttunnel');
    nextRules.putIfAbsent('trusttunnel', () => RouteRules.empty);
    nextModes.putIfAbsent('trusttunnel', () => 'tun');
    nextPresets.putIfAbsent('trusttunnel', () => RoutePreset.defaults);

    _applySettings(
      _settings.copyWith(
        defaultRoute: trustTunnelRoute,
        routesByCore: nextRoutes,
        routeRulesByCore: nextRules,
        activeModeByCore: nextModes,
        routePresetsByCore: nextPresets,
      ),
    );
  }

  void _selectActiveMode(String mode) {
    final nextModes = Map<String, String>.of(_activeModeByCore)
      ..[_activeCore.id] = mode;
    _applySettings(_settings.copyWith(activeModeByCore: nextModes));
  }

  void _selectActivePreset(RoutePreset preset) {
    final coreId = _activeCore.id;
    final nextRoutes = Map<String, String>.of(_routesByCore)
      ..[coreId] = preset.id;
    final nextRules = Map<String, RouteRules>.of(_routeRulesByCore)
      ..[coreId] = preset.rules;

    _applySettings(
      _settings.copyWith(
        defaultRoute:
            coreId == 'trusttunnel' ? preset.id : _settings.defaultRoute,
        routesByCore: nextRoutes,
        routeRulesByCore: nextRules,
      ),
    );
  }

  String _defaultRouteForCore(String coreId) {
    return switch (coreId) {
      'naiveproxy' => 'Proxy only',
      'sing-box' => 'Rule set',
      'xray-core' => 'Proxy only',
      'hysteria2' => 'Proxy only',
      _ => 'Default',
    };
  }

  void _addCore(String coreId) {
    final alreadyOpen = _tabs.any((tab) => tab.coreId == coreId);
    if (alreadyOpen) {
      final existing = _tabs.firstWhere((tab) => tab.coreId == coreId);
      _selectTab(existing.id);
      return;
    }

    final core = findCoreIn(_coreSpecs, coreId);
    if (core.status != CoreStatus.active) {
      setState(() => _coreMenuOpen = false);
      return;
    }

    setState(() {
      final id = ++_nextTabId;
      _tabs.add(
        SessionTab(
          id: id,
          coreId: coreId,
          selectedServerId: _firstServerIdForCore(coreId),
          phase: ConnectionPhase.idle,
          connectedAt: null,
        ),
      );
      _activeTabId = id;
      _coreMenuOpen = false;
      _progress = 0;
    });
  }

  Future<void> _installCore(String coreId) async {
    if (_installingCoreIds.contains(coreId)) return;
    setState(() {
      _installingCoreIds.add(coreId);
      _coreSpecs = [
        for (final spec in _coreSpecs)
          if (spec.id == coreId) spec.copyWith(installing: true) else spec,
      ];
    });
    try {
      await _backendSessionService.coreInstall(coreId);
      if (!mounted) return;
      await _loadCoreCatalog();
    } on Object {
      // Keep the spec visible; catalog reload on next open will show real state.
    } finally {
      if (mounted) {
        setState(() {
          _installingCoreIds.remove(coreId);
          _coreSpecs = [
            for (final spec in _coreSpecs)
              if (spec.id == coreId) spec.copyWith(installing: false) else spec,
          ];
        });
      }
    }
  }

  void _selectServer(String id) {
    if (_connected || _working) {
      setState(() {
        _connectionMessage = 'Disconnect before switching servers';
      });
      return;
    }

    final index = _tabs.indexWhere((tab) => tab.id == _activeTabId);
    if (index < 0) {
      return;
    }

    setState(() {
      _tabs[index] = _tabs[index].copyWith(
        selectedServerId: id,
        phase: ConnectionPhase.idle,
        clearConnectedAt: true,
      );
      _connectionMessage = null;
      _progress = 0;
    });
    _clearTimers();
    _stopTraffic();
    _startSessionStatusPolling(); // F3: restart idle discovery
  }

  void _editServer(ServerProfile server) {
    if (server.id.isEmpty) {
      return;
    }

    setState(() {
      _editingServer = server;
      _overlay = _DesktopOverlay.editProfile;
    });
  }

  void _toggleConnection() {
    if (_activeServers.isEmpty) {
      return;
    }

    if (_connected) {
      _disconnect();
      return;
    }

    if (!_working) {
      _connect();
    }
  }

  Duration get _morphDuration =>
      _themeRevealInFlight ? Duration.zero : _motionDuration;

  void _toggleCompact() {
    final compact = !_compact;
    setState(() {
      _compact = compact;
      _coreMenuOpen = false;
      if (!compact) _expandReveal++;
    });

    final target = compact ? kCompactWindow : kExpandedWindow;
    unawaited(
      WindowControls.animateWindowSize(
        width: target.width,
        height: target.height,
        duration: _morphDuration,
      ),
    );
  }

  void _toggleTheme(Offset origin) {
    final next = _themeMode == PohThemeMode.dark
        ? PohThemeMode.light
        : PohThemeMode.dark;
    unawaited(_setThemeModeFrom(next, origin));
  }

  Future<void> _setThemeModeFrom(PohThemeMode mode, Offset origin) async {
    if (mode == _themeMode) {
      return;
    }

    final reveal = _themeRevealKey.currentState;
    final accent = _themeAccentColor;
    if (reveal == null || _themeDuration == Duration.zero) {
      setState(() => _themeMode = mode);
      unawaited(_saveSettings());
      return;
    }

    setState(() => _themeRevealInFlight = true);
    await reveal.revealTo(
      themeMode: mode,
      accent: accent,
      globalOrigin: origin,
    );
    if (!mounted) {
      return;
    }

    setState(() {
      _themeMode = mode;
      _themeRevealInFlight = false;
    });
    unawaited(_saveSettings());
    await WidgetsBinding.instance.endOfFrame;
    reveal.clearReveal();
  }

  Future<void> _connect() async {
    _clearTimers();
    _stopTraffic();
    final id = _activeTabId;
    final server = _selectedServer;
    _patchTab(id, phase: ConnectionPhase.preparing);
    setState(() {
      _connectionMessage = 'Preparing ${server.name}';
      _progress = 0.18;
    });

    try {
      final supervised = await _backendSessionService.startSupervisedSession(
        _activeCore.id,
        server,
      );
      if (!mounted || id != _activeTabId) {
        await supervised.stop();
        return;
      }

      _supervisorSession = supervised;

      // Listen for supervisor events (crash detection, clean stop confirmation).
      supervised.stdoutLines.listen(
        (line) => _onSupervisorEvent(line, id),
        onDone: () => _onSupervisorExited(id),
      );

      _patchTab(id, phase: ConnectionPhase.authenticating);
      setState(() {
        _connectionMessage =
            '${supervised.session.coreId} core started: PID ${supervised.session.pid}';
        _progress = 0.82;
      });
      await Future<void>.delayed(const Duration(milliseconds: 220));
      if (!mounted || id != _activeTabId) {
        return;
      }

      _patchTab(
        id,
        phase: ConnectionPhase.connected,
        connectedAt: DateTime.fromMillisecondsSinceEpoch(
          supervised.session.startedAtUnixMs,
        ),
      );
      setState(() {
        _connectionMessage = null;
        _progress = 1;
      });
      _startTraffic();
      // Status polling is kept as a safety net in case the supervisor itself
      // exits without emitting a faulted event.
      _startSessionStatusPolling();
    } catch (error) {
      if (!mounted) {
        return;
      }

      // F2: if another session is already running, adopt it instead of showing
      // an error. The user can then Disconnect to stop it, or keep using it.
      if (error is BackendSessionException &&
          error.message.contains('already running')) {
        unawaited(_tryAdoptSession(id));
        return;
      }

      _patchTab(id, phase: ConnectionPhase.idle, clearConnectedAt: true);
      setState(() {
        _connectionMessage = error.toString();
        _progress = 0;
        _download = List<double>.filled(16, 0);
        _upload = List<double>.filled(16, 0);
      });
    }
  }

  /// F2: adopt a running session when [_connect] gets SessionAlreadyRunning.
  Future<void> _tryAdoptSession(int tabId) async {
    try {
      final status = await _backendSessionService.sessionStatus();
      if (!mounted) return;
      if (status.running && status.session != null) {
        _adoptSession(status.session!);
      } else {
        _patchTab(tabId, phase: ConnectionPhase.idle, clearConnectedAt: true);
        setState(() {
          _connectionMessage = 'Connection conflict: no active session found';
          _progress = 0;
          _download = List<double>.filled(16, 0);
          _upload = List<double>.filled(16, 0);
        });
        _startSessionStatusPolling();
      }
    } catch (e) {
      if (!mounted) return;
      _patchTab(tabId, phase: ConnectionPhase.idle, clearConnectedAt: true);
      setState(() {
        _connectionMessage = e.toString();
        _progress = 0;
        _download = List<double>.filled(16, 0);
        _upload = List<double>.filled(16, 0);
      });
      _startSessionStatusPolling();
    }
  }

  Future<void> _disconnect() async {
    final id = _activeTabId;
    _clearTimers();
    _stopTraffic();
    setState(() => _connectionMessage = 'Disconnecting...');
    _patchTab(id, phase: ConnectionPhase.disconnecting);
    setState(() => _progress = 0.18);

    final supervisor = _supervisorSession;
    _supervisorSession = null;

    try {
      if (supervisor != null) {
        await supervisor.stop();
      } else {
        await _backendSessionService.stopSession();
      }
      if (!mounted || id != _activeTabId) {
        return;
      }

      _patchTab(id, phase: ConnectionPhase.idle, clearConnectedAt: true);
      setState(() {
        _connectionMessage = null;
        _progress = 0;
        _download = List<double>.filled(16, 0);
        _upload = List<double>.filled(16, 0);
      });
      _stopTraffic();
      _startSessionStatusPolling(); // F3: restart idle discovery after disconnect
    } catch (error) {
      if (!mounted || id != _activeTabId) {
        return;
      }

      _patchTab(id, phase: ConnectionPhase.connected);
      setState(() {
        _connectionMessage = error.toString();
        _progress = 1;
      });
      _startTraffic();
      _startSessionStatusPolling();
    }
  }

  /// Handles JSON-line events emitted by the long-lived supervisor process.
  void _onSupervisorEvent(String line, int tabId) {
    if (!mounted || _activeTabId != tabId) return;
    final Map<String, dynamic> event;
    try {
      final decoded = jsonDecode(line);
      if (decoded is! Map<String, dynamic>) return;
      event = decoded;
    } catch (_) {
      return;
    }
    final type = event['type'] as String?;
    if (type == 'faulted') {
      // Core exited unexpectedly; network already restored by the supervisor.
      _clearTimers();
      _stopTraffic();
      _supervisorSession = null;
      _patchTab(tabId, phase: ConnectionPhase.idle, clearConnectedAt: true);
      setState(() {
        _connectionMessage = 'Core exited unexpectedly';
        _progress = 0;
        _download = List<double>.filled(16, 0);
        _upload = List<double>.filled(16, 0);
      });
      _startSessionStatusPolling(); // F3: restart idle discovery
    }
  }

  /// Called when the supervisor process stdout stream closes (supervisor exited).
  void _onSupervisorExited(int tabId) {
    if (!mounted || _activeTabId != tabId) return;
    // If still showing as connected, treat the supervisor exit as a fault.
    if (_activeTab.phase == ConnectionPhase.connected) {
      _clearTimers();
      _stopTraffic();
      _supervisorSession = null;
      _patchTab(tabId, phase: ConnectionPhase.idle, clearConnectedAt: true);
      setState(() {
        _connectionMessage = 'Core supervisor exited unexpectedly';
        _progress = 0;
        _download = List<double>.filled(16, 0);
        _upload = List<double>.filled(16, 0);
      });
      _startSessionStatusPolling(); // F3: restart idle discovery
    }
  }

  void _patchTab(
    int id, {
    ConnectionPhase? phase,
    DateTime? connectedAt,
    bool clearConnectedAt = false,
  }) {
    final index = _tabs.indexWhere((tab) => tab.id == id);
    if (index < 0 || !mounted) {
      return;
    }

    setState(() {
      _tabs[index] = _tabs[index].copyWith(
        phase: phase,
        connectedAt: connectedAt,
        clearConnectedAt: clearConnectedAt,
      );
    });
  }

  void _startTraffic() {
    _stopTraffic();
    _trafficMetricsService.reset();
    _clockTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted) {
        setState(() {});
      }
    });
    _trafficTimer = Timer.periodic(const Duration(seconds: 1), (_) async {
      if (!_connected || !mounted || _trafficSampleInFlight) {
        return;
      }

      _trafficSampleInFlight = true;
      try {
        final sample = await _trafficMetricsService.sample();
        if (!mounted || !_connected) {
          return;
        }

        setState(() {
          _download = [..._download.skip(1), sample.downloadMbps];
          _upload = [..._upload.skip(1), sample.uploadMbps];
        });
      } finally {
        _trafficSampleInFlight = false;
      }
    });
  }

  void _stopTraffic() {
    _trafficTimer?.cancel();
    _trafficTimer = null;
    _trafficSampleInFlight = false;
    _trafficMetricsService.reset();
    _clockTimer?.cancel();
    _clockTimer = null;
    _sessionStatusTimer?.cancel();
    _sessionStatusTimer = null;
    _sessionStatusInFlight = false;
  }

  // F3: polls status when connected (safety net for core crash) AND when
  // disconnected (catches external/orphaned sessions for reattach).
  void _startSessionStatusPolling() {
    _sessionStatusTimer?.cancel();
    _sessionStatusTimer = Timer.periodic(const Duration(seconds: 5), (_) async {
      if (!mounted || _sessionStatusInFlight) return;

      _sessionStatusInFlight = true;
      try {
        final status = await _backendSessionService.sessionStatus();
        if (!mounted) return;

        if (_connected && !status.running) {
          // Core exited while GUI thought we were connected.
          _clearTimers();
          _stopTraffic();
          _patchTab(_activeTabId,
              phase: ConnectionPhase.idle, clearConnectedAt: true);
          setState(() {
            _connectionMessage = '${_activeCore.name} core exited';
            _progress = 0;
            _download = List<double>.filled(16, 0);
            _upload = List<double>.filled(16, 0);
          });
          _startSessionStatusPolling(); // restart for idle discovery
        } else if (!_connected &&
            !_working &&
            status.running &&
            status.session != null) {
          // External/orphaned session detected. Update the session tab without
          // stealing focus from the core the user is currently inspecting.
          _adoptSession(status.session!, focus: false);
        }
      } catch (error) {
        if (mounted && _connected) {
          setState(() => _connectionMessage = error.toString());
        }
      } finally {
        _sessionStatusInFlight = false;
      }
    });
  }

  void _clearTimers() {
    for (final timer in _timers) {
      timer.cancel();
    }
    _timers.clear();
  }

  Future<void> _loadProfiles() async {
    final loaded = await _backendSessionService.listProfiles();
    if (!mounted) {
      return;
    }

    setState(() {
      final initialCoreId = _coreSpecs.any((core) => core.id == _debugCoreId)
          ? _debugCoreId
          : 'trusttunnel';
      _servers = loaded;
      final initialServerId = _firstServerIdForCore(initialCoreId);
      _profilesLoaded = true;
      _tabs
        ..clear()
        ..add(
          SessionTab(
            id: 1,
            coreId: initialCoreId,
            selectedServerId: initialServerId,
            phase: ConnectionPhase.idle,
            connectedAt: null,
          ),
        );
      _activeTabId = 1;
      _nextTabId = 1;
      if (Platform.environment['POH_DEBUG_OVERLAY'] == 'edit') {
        _editingServer = _firstServerForCore(initialCoreId);
        _overlay = _editingServer == null
            ? _DesktopOverlay.none
            : _DesktopOverlay.editProfile;
      }
    });

    // F1: reattach to any already-running session.
    await _checkExistingSession();

    // F3: start idle discovery polling if we didn't reattach.
    if (mounted && !_connected && !_working) {
      _startSessionStatusPolling();
    }
  }

  /// F1: on startup, query session status and adopt a running session so the GUI
  /// shows Connected immediately rather than requiring a manual Connect.
  Future<void> _checkExistingSession() async {
    try {
      final status = await _backendSessionService.sessionStatus();
      if (!mounted) return;
      if (status.running && status.session != null) {
        _adoptSession(status.session!);
      }
    } catch (_) {
      // poh_cli not found or session query failed — not a startup-blocking error.
    }
  }

  /// F1/F2/F3/F6: transition the GUI to Connected using data from an existing
  /// (possibly orphaned) session.  [_supervisorSession] is intentionally null —
  /// Disconnect will go through [stopSession()] instead.
  ///
  /// Rust's [desktop_session_status] already verified [creation_time_100ns], so
  /// the session PID is guaranteed to belong to our core process.
  void _adoptSession(BackendSession session, {bool focus = true}) {
    // Find or create a tab for the session's core.
    var tabIdx = _tabs.indexWhere((t) => t.coreId == session.coreId);
    if (tabIdx < 0) {
      final newId = ++_nextTabId;
      _tabs.add(SessionTab(
        id: newId,
        coreId: session.coreId,
        selectedServerId: session.profileId,
        phase: ConnectionPhase.idle,
        connectedAt: null,
      ));
      tabIdx = _tabs.length - 1;
      if (focus) {
        _activeTabId = newId;
      }
    }

    final tab = _tabs[tabIdx];
    setState(() {
      if (focus) {
        _activeTabId = tab.id;
      }
      _tabs[tabIdx] = tab.copyWith(
        selectedServerId: session.profileId,
        phase: ConnectionPhase.connected,
        connectedAt:
            DateTime.fromMillisecondsSinceEpoch(session.startedAtUnixMs),
      );
      _supervisorSession = null;
      _connectionMessage = null;
      _progress = focus ? 1.0 : _progress;
      _download = List<double>.filled(16, 0);
      _upload = List<double>.filled(16, 0);
    });
    _startTraffic();
    _startSessionStatusPolling();
  }

  String _firstServerIdForCore(String coreId) {
    for (final server in _servers) {
      if (server.coreId == coreId) {
        return server.id;
      }
    }

    return '';
  }

  ServerProfile? _firstServerForCore(String coreId) {
    for (final server in _servers) {
      if (server.coreId == coreId) {
        return server;
      }
    }

    return null;
  }
}

class _DesktopWindow extends StatelessWidget {
  const _DesktopWindow({
    required this.compact,
    required this.coreMenuOpen,
    required this.cores,
    required this.activeCore,
    required this.activeTab,
    required this.selectedServer,
    required this.servers,
    required this.profilesLoaded,
    required this.tabs,
    required this.download,
    required this.upload,
    required this.progress,
    required this.connected,
    required this.working,
    required this.connectionMessage,
    required this.themeMode,
    required this.activeRoute,
    required this.activeMode,
    required this.routePresets,
    required this.motionDuration,
    required this.expandReveal,
    required this.onToggleCompact,
    required this.onToggleTheme,
    required this.onToggleCoreMenu,
    required this.onDismissCoreMenu,
    required this.onSelectTab,
    required this.onAddCore,
    required this.onInstallCore,
    required this.onSelectServer,
    required this.onEditServer,
    required this.onToggleConnection,
    required this.onOpenSettings,
    required this.onSelectRouteMode,
    required this.onSelectRoutePreset,
    required this.onOpenRoutes,
    required this.onOpenLogs,
    required this.onOpenImportProfile,
  });

  final bool compact;
  final bool coreMenuOpen;
  final List<CoreSpec> cores;
  final CoreSpec activeCore;
  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<ServerProfile> servers;
  final bool profilesLoaded;
  final List<SessionTab> tabs;
  final List<double> download;
  final List<double> upload;
  final double progress;
  final bool connected;
  final bool working;
  final String? connectionMessage;
  final PohThemeMode themeMode;
  final String activeRoute;
  final String activeMode;
  final List<RoutePreset> routePresets;
  final Duration motionDuration;
  final int expandReveal;
  final VoidCallback onToggleCompact;
  final ValueChanged<Offset> onToggleTheme;
  final VoidCallback onToggleCoreMenu;
  final VoidCallback onDismissCoreMenu;
  final ValueChanged<int> onSelectTab;
  final ValueChanged<String> onAddCore;
  final ValueChanged<String> onInstallCore;
  final ValueChanged<String> onSelectServer;
  final ValueChanged<ServerProfile> onEditServer;
  final VoidCallback onToggleConnection;
  final VoidCallback onOpenSettings;
  final ValueChanged<String> onSelectRouteMode;
  final ValueChanged<RoutePreset> onSelectRoutePreset;
  final VoidCallback onOpenRoutes;
  final VoidCallback onOpenLogs;
  final VoidCallback onOpenImportProfile;

  @override
  Widget build(BuildContext context) {
    final target = compact ? kCompactWindow : kExpandedWindow;

    // Shutter/mask: the inner UI is always laid out at its final target size, so
    // the native window resize never squeezes it (no RenderFlex overflow, no
    // per-letter text). ClipRect masks it down to the live window size, and the
    // RepaintBoundary keeps the cached content on its own GPU layer so resizing
    // only recomposites instead of repainting the whole tree.
    return ClipRect(
      child: OverflowBox(
        alignment: Alignment.topLeft,
        minWidth: target.width,
        maxWidth: target.width,
        minHeight: target.height,
        maxHeight: target.height,
        child: RepaintBoundary(
          child: SizedBox(
            width: target.width,
            height: target.height,
            child: _content(context),
          ),
        ),
      ),
    );
  }

  Widget _content(BuildContext context) {
    return Stack(
      children: [
        Positioned.fill(
          child: Column(
            children: [
              _TitleBar(
                compact: compact,
                activeCore: activeCore,
                activeTab: activeTab,
                cores: cores,
                tabs: tabs,
                onToggleCompact: onToggleCompact,
                onToggleCoreMenu: onToggleCoreMenu,
                onSelectTab: onSelectTab,
              ),
              Expanded(
                child: _MorphingBody(
                  compact: compact,
                  activeCore: activeCore,
                  activeTab: activeTab,
                  selectedServer: selectedServer,
                  servers: servers,
                  profilesLoaded: profilesLoaded,
                  download: download,
                  upload: upload,
                  progress: progress,
                  connected: connected,
                  working: working,
                  connectionMessage: connectionMessage,
                  themeMode: themeMode,
                  activeRoute: activeRoute,
                  activeMode: activeMode,
                  routePresets: routePresets,
                  motionDuration: motionDuration,
                  expandReveal: expandReveal,
                  onSelectServer: onSelectServer,
                  onEditServer: onEditServer,
                  onToggleConnection: onToggleConnection,
                  onToggleTheme: onToggleTheme,
                  onOpenSettings: onOpenSettings,
                  onSelectRouteMode: onSelectRouteMode,
                  onSelectRoutePreset: onSelectRoutePreset,
                  onOpenRoutes: onOpenRoutes,
                  onOpenLogs: onOpenLogs,
                  onOpenImportProfile: onOpenImportProfile,
                ),
              ),
            ],
          ),
        ),
        if (coreMenuOpen)
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTap: onDismissCoreMenu,
              child: const SizedBox.expand(),
            ),
          ),
        if (coreMenuOpen)
          Positioned(
            left: compact ? 52 : 68,
            top: 48,
            child: _CoreMenu(
              cores: cores,
              activeCoreId: activeCore.id,
              openTabs: tabs,
              onAddCore: onAddCore,
              onInstallCore: onInstallCore,
            ),
          ),
      ],
    );
  }
}

class _TitleBar extends StatelessWidget {
  const _TitleBar({
    required this.compact,
    required this.activeCore,
    required this.activeTab,
    required this.cores,
    required this.tabs,
    required this.onToggleCompact,
    required this.onToggleCoreMenu,
    required this.onSelectTab,
  });

  final bool compact;
  final CoreSpec activeCore;
  final SessionTab activeTab;
  final List<CoreSpec> cores;
  final List<SessionTab> tabs;
  final VoidCallback onToggleCompact;
  final VoidCallback onToggleCoreMenu;
  final ValueChanged<int> onSelectTab;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final listMode = compact || tabs.length > 5;

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onPanStart: (_) => WindowControls.startDrag(),
      child: Container(
        height: 48,
        padding: const EdgeInsets.fromLTRB(10, 6, 10, 0),
        decoration: BoxDecoration(
          color: palette.subtle,
          border: Border(bottom: BorderSide(color: palette.border)),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Padding(
              padding: const EdgeInsets.only(bottom: 7),
              child: _AppMark(accent: palette.accent),
            ),
            const SizedBox(width: 8),
            if (listMode)
              Padding(
                padding: const EdgeInsets.only(bottom: 7),
                child: _CoreListButton(
                  core: activeCore,
                  count: tabs.length,
                  onPressed: onToggleCoreMenu,
                ),
              )
            else
              Flexible(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    for (final tab in tabs)
                      _CoreTabButton(
                        core: findCoreIn(cores, tab.coreId),
                        tab: tab,
                        active: tab.id == activeTab.id,
                        onPressed: () => onSelectTab(tab.id),
                      ),
                    _HeaderIconButton(
                      tooltip: 'Open cores',
                      icon: Icons.add_rounded,
                      onPressed: onToggleCoreMenu,
                      bottomPadding: 7,
                    ),
                  ],
                ),
              ),
            const Spacer(),
            _HeaderIconButton(
              tooltip: compact ? 'Expanded mode' : 'Compact mode',
              icon: compact
                  ? Icons.crop_square_rounded
                  : Icons.keyboard_double_arrow_right_rounded,
              onPressed: onToggleCompact,
              bottomPadding: 7,
            ),
            Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Container(width: 1, height: 18, color: palette.border),
            ),
            const _HeaderIconButton(
              tooltip: 'Minimize',
              icon: Icons.remove_rounded,
              onPressed: WindowControls.minimize,
              bottomPadding: 7,
            ),
            const _HeaderIconButton(
              tooltip: 'Close',
              icon: Icons.close_rounded,
              danger: true,
              onPressed: WindowControls.close,
              bottomPadding: 7,
            ),
          ],
        ),
      ),
    );
  }
}

class _MorphingBody extends StatelessWidget {
  const _MorphingBody({
    required this.compact,
    required this.activeCore,
    required this.activeTab,
    required this.selectedServer,
    required this.servers,
    required this.profilesLoaded,
    required this.download,
    required this.upload,
    required this.progress,
    required this.connected,
    required this.working,
    required this.connectionMessage,
    required this.themeMode,
    required this.activeRoute,
    required this.activeMode,
    required this.routePresets,
    required this.motionDuration,
    required this.expandReveal,
    required this.onSelectServer,
    required this.onEditServer,
    required this.onToggleConnection,
    required this.onToggleTheme,
    required this.onOpenSettings,
    required this.onSelectRouteMode,
    required this.onSelectRoutePreset,
    required this.onOpenRoutes,
    required this.onOpenLogs,
    required this.onOpenImportProfile,
  });

  final bool compact;
  final CoreSpec activeCore;
  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<ServerProfile> servers;
  final bool profilesLoaded;
  final List<double> download;
  final List<double> upload;
  final double progress;
  final bool connected;
  final bool working;
  final String? connectionMessage;
  final PohThemeMode themeMode;
  final String activeRoute;
  final String activeMode;
  final List<RoutePreset> routePresets;
  final Duration motionDuration;
  final int expandReveal;
  final ValueChanged<String> onSelectServer;
  final ValueChanged<ServerProfile> onEditServer;
  final VoidCallback onToggleConnection;
  final ValueChanged<Offset> onToggleTheme;
  final VoidCallback onOpenSettings;
  final ValueChanged<String> onSelectRouteMode;
  final ValueChanged<RoutePreset> onSelectRoutePreset;
  final VoidCallback onOpenRoutes;
  final VoidCallback onOpenLogs;
  final VoidCallback onOpenImportProfile;

  @override
  Widget build(BuildContext context) {
    final sidebarWidth = compact ? double.infinity : 316.0;
    const compactPanelHeight = 202.0;
    const expandedSidebarWidth = 316.0;
    final curve =
        motionDuration == Duration.zero ? Curves.linear : PohMotion.standard;

    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        final height = constraints.maxHeight;
        // Each panel is sized for its OWN mode using the shared window
        // constants, then only slid on/off screen. So the expanded detail pane
        // is always measured at its full width and the compact dock always at
        // its full width - neither ever reflows its text while the window
        // animates; the off-mode one is simply parked off-canvas and clipped.
        final detailWidth = kExpandedWindow.width - expandedSidebarWidth;
        final detailHeight = kExpandedWindow.height - 48;
        final dockWidth = kCompactWindow.width;
        final sidebarHeight =
            compact ? math.max(0.0, height - compactPanelHeight) : height;

        // Removed detailLeft. The detail pane stays in place horizontally and slides down.
        final detailTop = compact ? 40.0 : 0.0;

        return ClipRect(
          child: Stack(
            children: [
              AnimatedPositioned(
                duration: motionDuration,
                curve: curve,
                left: 0,
                top: 0,
                width: sidebarWidth.isInfinite ? width : sidebarWidth,
                height: sidebarHeight,
                child: _ServerSidebar(
                  activeTab: activeTab,
                  selectedServer: selectedServer,
                  servers: servers,
                  profilesLoaded: profilesLoaded,
                  connected: connected,
                  compact: compact,
                  onSelectServer: onSelectServer,
                  onEditServer: onEditServer,
                  onToggleTheme: onToggleTheme,
                  themeMode: themeMode,
                  onOpenSettings: onOpenSettings,
                  onOpenRoutes: onOpenRoutes,
                  onOpenLogs: onOpenLogs,
                  onOpenImportProfile: onOpenImportProfile,
                ),
              ),
              AnimatedPositioned(
                duration: motionDuration,
                curve: curve,
                left: expandedSidebarWidth,
                top: detailTop,
                width: detailWidth,
                height: detailHeight,
                child: IgnorePointer(
                  ignoring: compact,
                  child: AnimatedOpacity(
                    duration: motionDuration,
                    curve: Curves.easeOutCubic,
                    opacity: compact ? 0 : 1,
                    child: _ExpandedDetailPane(
                      key: ValueKey(expandReveal),
                      activeCore: activeCore,
                      activeTab: activeTab,
                      selectedServer: selectedServer,
                      download: download,
                      upload: upload,
                      connected: connected,
                      connectionMessage: connectionMessage,
                      activeRoute: activeRoute,
                      activeMode: activeMode,
                      routePresets: routePresets,
                      motionDuration: motionDuration,
                      onOpenRoutes: onOpenRoutes,
                      onSelectRouteMode: onSelectRouteMode,
                      onSelectRoutePreset: onSelectRoutePreset,
                      progress: progress,
                      working: working,
                      onToggleConnection: onToggleConnection,
                    ),
                  ),
                ),
              ),
              Positioned(
                left: 0,
                bottom: 0,
                width: dockWidth,
                height: compactPanelHeight,
                child: IgnorePointer(
                  ignoring: !compact,
                  // The dock pane manages its own staggered internal opacity,
                  // but we give it a master opacity so it fully fades when expanding.
                  child: AnimatedOpacity(
                    duration: motionDuration,
                    curve: Curves.easeOutCubic,
                    opacity: compact ? 1 : 0,
                    child: _CompactDockPane(
                      key: ValueKey(compact),
                      activeCore: activeCore,
                      activeTab: activeTab,
                      selectedServer: selectedServer,
                      connectionMessage: connectionMessage,
                      themeMode: themeMode,
                      onToggleTheme: onToggleTheme,
                      onOpenSettings: onOpenSettings,
                      onOpenRoutes: onOpenRoutes,
                      onOpenLogs: onOpenLogs,
                      motionDuration: motionDuration,
                      compact: compact,
                      progress: progress,
                      working: working,
                      onToggleConnection: onToggleConnection,
                    ),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _ExpandedDetailPane extends StatelessWidget {
  const _ExpandedDetailPane({
    super.key,
    required this.activeCore,
    required this.activeTab,
    required this.selectedServer,
    required this.download,
    required this.upload,
    required this.connected,
    required this.connectionMessage,
    required this.activeRoute,
    required this.activeMode,
    required this.routePresets,
    required this.motionDuration,
    required this.onOpenRoutes,
    required this.onSelectRouteMode,
    required this.onSelectRoutePreset,
    required this.progress,
    required this.working,
    required this.onToggleConnection,
  });

  final CoreSpec activeCore;
  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<double> download;
  final List<double> upload;
  final bool connected;
  final String? connectionMessage;
  final String activeRoute;
  final String activeMode;
  final List<RoutePreset> routePresets;
  final Duration motionDuration;
  final VoidCallback onOpenRoutes;
  final ValueChanged<String> onSelectRouteMode;
  final ValueChanged<RoutePreset> onSelectRoutePreset;
  final double progress;
  final bool working;
  final VoidCallback? onToggleConnection;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(26, 18, 26, 22),
      child: Column(
        children: [
          _AppearIn(
            motionDuration: motionDuration,
            child: _ServerStrip(server: selectedServer),
          ),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final ringSpace = math.min(204.0, constraints.maxHeight * 0.38);
                final ringDiameter = math.min(170.0, ringSpace);
                return Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    _AppearIn(
                      motionDuration: motionDuration,
                      child: SizedBox(
                        height: ringSpace,
                        child: Center(
                          child: SizedBox.square(
                            dimension: ringDiameter,
                            child: _MorphingConnectControl(
                              compact: false,
                              phase: activeTab.phase,
                              progress: progress,
                              duration: Duration.zero,
                              onPressed: working ? null : onToggleConnection,
                            ),
                          ),
                        ),
                      ),
                    ),
                    _AppearIn(
                      motionDuration: motionDuration,
                      delay: const Duration(milliseconds: 50),
                      child: _ConnectionStatus(
                        phase: activeTab.phase,
                        core: activeCore,
                        connectedAt: activeTab.connectedAt,
                        message: connectionMessage,
                      ),
                    ),
                    const SizedBox(height: 14),
                    _AppearIn(
                      motionDuration: motionDuration,
                      delay: const Duration(milliseconds: 100),
                      child: Wrap(
                        alignment: WrapAlignment.center,
                        spacing: 16,
                        runSpacing: 10,
                        children: [
                          _MetricCard(
                            title: 'DOWNLOAD',
                            value: connected ? download.last : 0,
                            sparkline: download,
                            primary: true,
                          ),
                          _MetricCard(
                            title: 'UPLOAD',
                            value: connected ? upload.last : 0,
                            sparkline: upload,
                            primary: false,
                          ),
                        ],
                      ),
                    ),
                  ],
                );
              },
            ),
          ),
          Container(height: 1, color: palette.border),
          const SizedBox(height: 14),
          _AppearIn(
            motionDuration: motionDuration,
            delay: const Duration(milliseconds: 150),
            child: _DetailBar(
              core: activeCore,
              server: selectedServer,
              activeRoute: activeRoute,
              activeMode: activeMode,
              routePresets: routePresets,
              onOpenRoutes: onOpenRoutes,
              onSelectRouteMode: onSelectRouteMode,
              onSelectRoutePreset: onSelectRoutePreset,
            ),
          ),
        ],
      ),
    );
  }
}

/// Cascade entrance animation: fade-in + slight upward slide.
/// Applied to each major section of [_ExpandedDetailPane] with staggered
/// [delay]s so the pane fills in sequentially when expanding.
///
/// When [motionDuration] is [Duration.zero] (animations disabled), the child
/// is rendered immediately without animation.
class _AppearIn extends StatefulWidget {
  const _AppearIn({
    required this.child,
    required this.motionDuration,
    this.delay = Duration.zero,
    this.slideOffset = 6.0,
  });

  final Widget child;
  final Duration motionDuration;
  final Duration delay;
  final double slideOffset;

  @override
  State<_AppearIn> createState() => _AppearInState();
}

class _AppearInState extends State<_AppearIn> {
  bool _visible = false;
  Timer? _delayTimer;

  @override
  void initState() {
    super.initState();
    if (widget.motionDuration == Duration.zero ||
        widget.delay == Duration.zero) {
      _visible = true;
    } else {
      _delayTimer = Timer(widget.delay, () {
        if (mounted) setState(() => _visible = true);
      });
    }
  }

  @override
  void dispose() {
    _delayTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.motionDuration == Duration.zero) return widget.child;
    if (!_visible) return Opacity(opacity: 0, child: widget.child);
    return TweenAnimationBuilder<double>(
      tween: Tween(begin: 0, end: 1),
      duration: widget.motionDuration,
      curve: PohMotion.decel,
      builder: (context, t, child) => Opacity(
        opacity: t,
        child: Transform.translate(
          offset: Offset(0, widget.slideOffset * (1 - t)),
          child: child,
        ),
      ),
      child: widget.child,
    );
  }
}

class _CompactDockPane extends StatelessWidget {
  const _CompactDockPane({
    super.key,
    required this.activeCore,
    required this.activeTab,
    required this.selectedServer,
    required this.connectionMessage,
    required this.themeMode,
    required this.onToggleTheme,
    required this.onOpenSettings,
    required this.onOpenRoutes,
    required this.onOpenLogs,
    required this.motionDuration,
    required this.compact,
    required this.progress,
    required this.working,
    required this.onToggleConnection,
  });

  final CoreSpec activeCore;
  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final String? connectionMessage;
  final PohThemeMode themeMode;
  final ValueChanged<Offset> onToggleTheme;
  final VoidCallback onOpenSettings;
  final VoidCallback onOpenRoutes;
  final VoidCallback onOpenLogs;
  final Duration motionDuration;
  final bool compact;
  final double progress;
  final bool working;
  final VoidCallback? onToggleConnection;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
        child: Column(
          children: [
            _AppearIn(
              motionDuration: motionDuration,
              delay: const Duration(milliseconds: 100),
              slideOffset: 30.0,
              child: _DockServerCard(core: activeCore, server: selectedServer),
            ),
            const SizedBox(height: 8),
            _AppearIn(
              motionDuration: motionDuration,
              delay: const Duration(milliseconds: 200),
              slideOffset: 40.0,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Expanded(
                    child: _ConnectionStatus(
                      phase: activeTab.phase,
                      core: activeCore,
                      connectedAt: activeTab.connectedAt,
                      message: connectionMessage,
                      compact: true,
                    ),
                  ),
                  _RoundIconButton(
                    tooltip: 'Routing',
                    icon: Icons.route_rounded,
                    onPressed: onOpenRoutes,
                    height: 34,
                    iconSize: 17,
                  ),
                  const SizedBox(width: 8),
                  _RoundIconButton(
                    tooltip: 'Logs',
                    icon: Icons.article_outlined,
                    onPressed: onOpenLogs,
                    height: 34,
                    iconSize: 17,
                  ),
                  const SizedBox(width: 8),
                  _RoundIconButton(
                    tooltip: 'Settings',
                    icon: Icons.tune_rounded,
                    onPressed: onOpenSettings,
                    height: 34,
                    iconSize: 17,
                  ),
                  const SizedBox(width: 8),
                  _RoundIconButton(
                    tooltip: themeMode == PohThemeMode.dark
                        ? 'Light theme'
                        : 'Dark theme',
                    icon: themeMode == PohThemeMode.dark
                        ? Icons.light_mode_rounded
                        : Icons.dark_mode_rounded,
                    onPressedAt: onToggleTheme,
                    height: 34,
                    iconSize: 17,
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
            _AppearIn(
              motionDuration: motionDuration,
              delay: const Duration(milliseconds: 300),
              slideOffset: 60.0,
              child: SizedBox(
                height: 56,
                child: _MorphingConnectControl(
                  compact: true,
                  phase: activeTab.phase,
                  progress: progress,
                  duration: Duration.zero,
                  onPressed: working ? null : onToggleConnection,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _MorphingConnectControl extends StatelessWidget {
  const _MorphingConnectControl({
    required this.compact,
    required this.phase,
    required this.progress,
    required this.duration,
    required this.onPressed,
  });

  final bool compact;
  final ConnectionPhase phase;
  final double progress;
  final Duration duration;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final active = phase == ConnectionPhase.connected;
    final working = phase != ConnectionPhase.idle && !active;
    final label = active
        ? 'DISCONNECT'
        : working
            ? phaseLabel(phase).toUpperCase()
            : 'CONNECT';
    final curve = duration == Duration.zero
        ? Curves.linear
        : Curves.fastLinearToSlowEaseIn;

    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(compact ? 14 : 999),
        onTap: onPressed,
        child: TweenAnimationBuilder<double>(
          tween: Tween<double>(end: compact ? 1 : 0),
          duration: duration,
          curve: curve,
          builder: (context, compactProgress, child) {
            final radius = 999 - (985 * compactProgress);
            final borderWidth = 5 * (1 - compactProgress);
            final showGlow = active || compactProgress > 0.01;

            return Container(
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: compact
                    ? palette.accent
                    : active
                        ? palette.accentSoft
                        : palette.surface,
                borderRadius: BorderRadius.circular(radius),
                border: Border.all(
                  color: compactProgress >= 0.98
                      ? Colors.transparent
                      : active
                          ? palette.accent
                          : palette.border,
                  width: borderWidth,
                ),
                boxShadow: [
                  if (showGlow)
                    BoxShadow(
                      color: palette.glow,
                      blurRadius: compact ? 18 : 28,
                      spreadRadius: compact ? 0 : 2,
                    ),
                ],
              ),
              child: child,
            );
          },
          child: compact
              ? Text(
                  active ? 'Disconnect' : 'Connect',
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 15,
                    fontWeight: FontWeight.w900,
                  ),
                )
              : Stack(
                  alignment: Alignment.center,
                  children: [
                    Positioned.fill(
                      child: CircularProgressIndicator(
                        value: working ? null : progress.clamp(0, 1),
                        strokeWidth: 4,
                        color: palette.accent,
                        backgroundColor: palette.border,
                      ),
                    ),
                    Container(
                      width: 88,
                      height: 88,
                      decoration: BoxDecoration(
                        color: active ? palette.accent : palette.surface,
                        shape: BoxShape.circle,
                        border: Border.all(color: palette.border),
                      ),
                      child: Column(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Icon(
                            Icons.power_settings_new_rounded,
                            color: active ? Colors.white : palette.muted,
                            size: 28,
                          ),
                          const SizedBox(height: 5),
                          Text(
                            label,
                            style: TextStyle(
                              color: active ? Colors.white : palette.muted,
                              fontSize: 10,
                              fontWeight: FontWeight.w900,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
        ),
      ),
    );
  }
}

/// Hosts an auxiliary overlay (settings / logs / import) over the main shell.
/// Expanded mode: a centered card that scales + fades in. Compact mode: a
/// bottom "sheet" that slides up. In both cases the main shell behind is
/// progressively blurred and dimmed. The hosted shells keep their own title bar
/// (with the top-right close), so this only handles placement + the backdrop.
class _OverlayHost extends StatefulWidget {
  const _OverlayHost({
    required this.compact,
    required this.motion,
    required this.onDismiss,
    required this.child,
  });

  final bool compact;
  final Duration motion;
  final VoidCallback onDismiss;
  final Widget child;

  @override
  State<_OverlayHost> createState() => _OverlayHostState();
}

class _OverlayHostState extends State<_OverlayHost>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: widget.motion == Duration.zero
        ? const Duration(milliseconds: 1)
        : widget.motion,
  )..forward();
  late final Animation<double> _t = CurvedAnimation(
    parent: _controller,
    curve: PohMotion.standard,
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Expanded keeps the original presentation: a centered card over a plain dim
    // scrim (no blur, no transform). The blur + slide-up sheet are compact-only.
    if (!widget.compact) {
      return Stack(
        children: [
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: widget.onDismiss,
              child: ColoredBox(
                color: Colors.black.withValues(alpha: 0.38),
              ),
            ),
          ),
          Center(child: widget.child),
        ],
      );
    }

    return AnimatedBuilder(
      animation: _t,
      builder: (context, _) {
        final p = _t.value.clamp(0.0, 1.0);
        return Stack(
          children: [
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: widget.onDismiss,
                child: ColoredBox(
                  color: Colors.black.withValues(alpha: 0.38 * p),
                ),
              ),
            ),
            Align(
              alignment: Alignment.bottomCenter,
              child: FractionalTranslation(
                translation: Offset(0, 1 - p),
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(8, 0, 8, 8),
                  child: widget.child,
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _ServerSidebar extends StatelessWidget {
  const _ServerSidebar({
    required this.activeTab,
    required this.selectedServer,
    required this.servers,
    required this.profilesLoaded,
    required this.connected,
    required this.onSelectServer,
    required this.onEditServer,
    required this.onToggleTheme,
    required this.themeMode,
    required this.onOpenSettings,
    required this.onOpenRoutes,
    required this.onOpenLogs,
    required this.onOpenImportProfile,
    this.compact = false,
  });

  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<ServerProfile> servers;
  final bool profilesLoaded;
  final bool connected;
  final ValueChanged<String> onSelectServer;
  final ValueChanged<ServerProfile> onEditServer;
  final ValueChanged<Offset> onToggleTheme;
  final PohThemeMode themeMode;
  final VoidCallback onOpenSettings;
  final VoidCallback onOpenRoutes;
  final VoidCallback onOpenLogs;
  final VoidCallback onOpenImportProfile;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.surface,
        border:
            compact ? null : Border(right: BorderSide(color: palette.border)),
      ),
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(18, 16, 18, 8),
            child: Row(
              children: [
                Text(
                  'SERVERS',
                  style: TextStyle(
                    color: palette.muted,
                    fontSize: 12,
                    fontWeight: FontWeight.w800,
                    letterSpacing: 0.8,
                  ),
                ),
                const SizedBox(width: 8),
                const Spacer(),
                Container(
                  height: 18,
                  constraints: const BoxConstraints(minWidth: 24),
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  decoration: BoxDecoration(
                    color: palette.subtle,
                    borderRadius: BorderRadius.circular(999),
                  ),
                  alignment: Alignment.center,
                  child: Text(
                    profilesLoaded ? '${servers.length}' : '...',
                    style: TextStyle(
                      color: palette.muted,
                      fontSize: 11,
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: _ServerList(
              activeTab: activeTab,
              selectedServer: selectedServer,
              servers: servers,
              profilesLoaded: profilesLoaded,
              connected: connected,
              compact: compact,
              onSelectServer: onSelectServer,
              onEditServer: onEditServer,
              onOpenImportProfile: onOpenImportProfile,
            ),
          ),
          if (compact)
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 6, 12, 8),
              child: _AddServerButton(
                compact: compact,
                onPressed: onOpenImportProfile,
              ),
            ),
          if (!compact)
            Padding(
              padding: const EdgeInsets.all(12),
              child: Row(
                children: [
                  Expanded(
                    child: _RoundIconButton(
                      tooltip: 'Routing profiles',
                      icon: Icons.alt_route_rounded,
                      onPressed: onOpenRoutes,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _RoundIconButton(
                      tooltip: 'Logs',
                      icon: Icons.article_outlined,
                      onPressed: onOpenLogs,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _RoundIconButton(
                      tooltip: 'Settings',
                      icon: Icons.tune_rounded,
                      onPressed: onOpenSettings,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _RoundIconButton(
                      tooltip: themeMode == PohThemeMode.dark
                          ? 'Light theme'
                          : 'Dark theme',
                      icon: themeMode == PohThemeMode.dark
                          ? Icons.light_mode_rounded
                          : Icons.dark_mode_rounded,
                      onPressedAt: onToggleTheme,
                    ),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _ServerList extends StatelessWidget {
  const _ServerList({
    required this.activeTab,
    required this.selectedServer,
    required this.servers,
    required this.profilesLoaded,
    required this.connected,
    required this.compact,
    required this.onSelectServer,
    required this.onEditServer,
    required this.onOpenImportProfile,
  });

  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<ServerProfile> servers;
  final bool profilesLoaded;
  final bool connected;
  final bool compact;
  final ValueChanged<String> onSelectServer;
  final ValueChanged<ServerProfile> onEditServer;
  final VoidCallback onOpenImportProfile;

  @override
  Widget build(BuildContext context) {
    if (!profilesLoaded) {
      return const _SidebarMessage(
        title: 'Loading profiles',
        subtitle: 'Reading saved Proxy Open Hub state',
      );
    }

    if (servers.isEmpty) {
      return SizedBox.expand(
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onSecondaryTapUp: (details) => _showEmptyServerListMenu(
            context,
            details.globalPosition,
            onOpenImportProfile,
          ),
          child: ListView(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
            children: [
              const _SidebarMessage(
                title: 'No servers for this core',
                subtitle: 'Import or add a profile for the active core',
                compact: true,
              ),
              if (!compact) ...[
                const SizedBox(height: 10),
                _AddServerButton(
                  compact: compact,
                  onPressed: onOpenImportProfile,
                ),
              ],
            ],
          ),
        ),
      );
    }

    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onSecondaryTapUp: (details) => _showEmptyServerListMenu(
        context,
        details.globalPosition,
        onOpenImportProfile,
      ),
      child: ListView.separated(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        itemCount: servers.length + (compact ? 0 : 1),
        separatorBuilder: (_, __) => const SizedBox(height: 6),
        itemBuilder: (context, index) {
          if (index == servers.length) {
            return _AddServerButton(
              compact: compact,
              onPressed: onOpenImportProfile,
            );
          }

          final server = servers[index];
          return _ServerCard(
            server: server,
            selected: server.id == selectedServer.id,
            connectedHere: connected && server.id == activeTab.selectedServerId,
            onPressed: () => onSelectServer(server.id),
            onEdit: () => onEditServer(server),
          );
        },
      ),
    );
  }
}

class _SidebarMessage extends StatelessWidget {
  const _SidebarMessage({
    required this.title,
    required this.subtitle,
    this.compact = false,
  });

  final String title;
  final String subtitle;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Padding(
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 6 : 18,
        vertical: compact ? 22 : 34,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: TextStyle(
              color: palette.text,
              fontSize: 15,
              fontWeight: FontWeight.w900,
            ),
          ),
          const SizedBox(height: 5),
          Text(
            subtitle,
            style: TextStyle(
              color: palette.muted,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
          ),
        ],
      ),
    );
  }
}

class _ServerCard extends StatelessWidget {
  const _ServerCard({
    required this.server,
    required this.selected,
    required this.connectedHere,
    required this.onPressed,
    required this.onEdit,
  });

  final ServerProfile server;
  final bool selected;
  final bool connectedHere;
  final VoidCallback onPressed;
  final VoidCallback onEdit;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return GestureDetector(
      onSecondaryTapUp: (details) => _showServerCardMenu(
        context,
        details.globalPosition,
        onEdit,
      ),
      child: Material(
        color: selected ? palette.accentSoft : Colors.transparent,
        borderRadius: BorderRadius.circular(14),
        child: InkWell(
          borderRadius: BorderRadius.circular(14),
          onTap: onPressed,
          child: Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(14),
              border: Border.all(
                color: selected
                    ? palette.accent.withValues(alpha: 0.38)
                    : Colors.transparent,
              ),
            ),
            child: Row(
              children: [
                _FlagChip(
                  code: server.countryCode,
                  selected: selected,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              server.name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: palette.text,
                                fontSize: 13.5,
                                fontWeight: FontWeight.w800,
                              ),
                            ),
                          ),
                          if (connectedHere) ...[
                            const SizedBox(width: 6),
                            Text(
                              'ACTIVE',
                              style: TextStyle(
                                color: palette.accent,
                                fontSize: 10,
                                fontWeight: FontWeight.w800,
                              ),
                            ),
                          ],
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        server.host,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: palette.muted,
                          fontSize: 11.5,
                          fontFamily: 'monospace',
                        ),
                      ),
                      if (server.tlsVerificationDisabled) ...[
                        const SizedBox(height: 5),
                        const _TlsWarningChip(compact: true),
                      ],
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    _PingLoad(server: server),
                    const SizedBox(height: 6),
                    Tooltip(
                      message: 'Edit server',
                      child: InkWell(
                        customBorder: const CircleBorder(),
                        onTap: onEdit,
                        child: Padding(
                          padding: const EdgeInsets.all(4),
                          child: Icon(
                            Icons.settings_rounded,
                            color: palette.muted,
                            size: 15,
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

enum _ServerCardMenuAction { edit }

enum _EmptyServerListMenuAction { pasteImport, importToml }

void _showServerCardMenu(
  BuildContext context,
  Offset globalPosition,
  VoidCallback onEdit,
) {
  unawaited(() async {
    final action = await _showPohMenu<_ServerCardMenuAction>(
      context: context,
      globalPosition: globalPosition,
      items: const [
        PopupMenuItem<_ServerCardMenuAction>(
          value: _ServerCardMenuAction.edit,
          height: 52,
          child: _PohContextMenuItem(
            icon: Icons.tune_rounded,
            title: 'Edit Configuration',
            subtitle:
                'Edit connection parameters, port, or encryption keys of the selected server.',
          ),
        ),
      ],
    );

    if (action == _ServerCardMenuAction.edit) {
      onEdit();
    }
  }());
}

void _showEmptyServerListMenu(
  BuildContext context,
  Offset globalPosition,
  VoidCallback onOpenImportProfile,
) {
  unawaited(() async {
    final action = await _showPohMenu<_EmptyServerListMenuAction>(
      context: context,
      globalPosition: globalPosition,
      items: const [
        PopupMenuItem<_EmptyServerListMenuAction>(
          value: _EmptyServerListMenuAction.pasteImport,
          height: 52,
          child: _PohContextMenuItem(
            icon: Icons.content_paste_rounded,
            title: 'Paste (Import)',
            subtitle:
                'Add a server by importing a connection link or JSON configuration from the clipboard.',
          ),
        ),
        PopupMenuItem<_EmptyServerListMenuAction>(
          value: _EmptyServerListMenuAction.importToml,
          height: 52,
          child: _PohContextMenuItem(
            icon: Icons.data_object_rounded,
            title: 'Import TOML',
            subtitle: 'Open the import dialog and paste a TOML profile.',
          ),
        ),
      ],
    );

    if (action != null) {
      onOpenImportProfile();
    }
  }());
}

Future<T?> _showPohMenu<T>({
  required BuildContext context,
  required Offset globalPosition,
  required List<PopupMenuEntry<T>> items,
}) {
  final palette = PohPalette.of(context);
  final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
  final position = RelativeRect.fromRect(
    Rect.fromLTWH(globalPosition.dx, globalPosition.dy, 0, 0),
    Offset.zero & overlay.size,
  );

  final dark = Theme.of(context).brightness == Brightness.dark;
  return showMenu<T>(
    context: context,
    position: position,
    color: palette.surface,
    elevation: 16,
    shadowColor: Colors.black.withValues(alpha: dark ? 0.45 : 0.18),
    surfaceTintColor: Colors.transparent,
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(18),
      side: BorderSide(color: palette.border),
    ),
    items: items,
  );
}

class _PohContextMenuItem extends StatelessWidget {
  const _PohContextMenuItem({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  final IconData icon;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return SizedBox(
      width: 260,
      child: Row(
        children: [
          Container(
            width: 30,
            height: 30,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: palette.accentSoft,
              borderRadius: BorderRadius.circular(9),
            ),
            child: Icon(icon, color: palette.accent, size: 16),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 13,
                    fontWeight: FontWeight.w900,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.muted,
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _CoreMenu extends StatelessWidget {
  const _CoreMenu({
    required this.cores,
    required this.activeCoreId,
    required this.openTabs,
    required this.onAddCore,
    required this.onInstallCore,
  });

  final List<CoreSpec> cores;
  final String activeCoreId;
  final List<SessionTab> openTabs;
  final ValueChanged<String> onAddCore;
  final ValueChanged<String> onInstallCore;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final openCoreIds = openTabs.map((tab) => tab.coreId).toSet();

    return Material(
      color: Colors.transparent,
      child: Container(
        width: 286,
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: palette.surface,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: palette.border),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(alpha: 0.20),
              blurRadius: 28,
              offset: const Offset(0, 12),
            ),
          ],
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(8, 4, 8, 6),
              child: Text(
                'OPEN CORES - ${openTabs.length}',
                style: TextStyle(
                  color: palette.muted,
                  fontSize: 10,
                  fontWeight: FontWeight.w800,
                  letterSpacing: 0.8,
                ),
              ),
            ),
            for (final core in cores)
              _CoreMenuItem(
                core: core,
                isOpen: openCoreIds.contains(core.id),
                isActive: core.id == activeCoreId,
                onPressed: () => onAddCore(core.id),
                onInstall:
                    core.installable ? () => onInstallCore(core.id) : null,
              ),
          ],
        ),
      ),
    );
  }
}

class _CoreMenuItem extends StatelessWidget {
  const _CoreMenuItem({
    required this.core,
    required this.isOpen,
    required this.isActive,
    required this.onPressed,
    this.onInstall,
  });

  final CoreSpec core;
  final bool isOpen;
  final bool isActive;
  final VoidCallback onPressed;
  final VoidCallback? onInstall;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final canOpen = core.status == CoreStatus.active || isOpen;
    final canInstall = onInstall != null && !core.installing;
    final tap = canOpen
        ? onPressed
        : canInstall
            ? onInstall
            : null;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Material(
        color: isActive ? palette.accentSoft : Colors.transparent,
        borderRadius: BorderRadius.circular(12),
        child: InkWell(
          borderRadius: BorderRadius.circular(12),
          onTap: tap,
          child: Opacity(
            opacity: (tap == null && !core.installing) ? 0.52 : 1,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
              child: Row(
                children: [
                  _CoreMark(core: core),
                  const SizedBox(width: 11),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          core.name,
                          style: TextStyle(
                            color: palette.text,
                            fontWeight: FontWeight.w800,
                            fontSize: 13.5,
                          ),
                        ),
                        Text(
                          core.tagline,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: palette.muted,
                            fontSize: 11,
                            fontFamily: 'monospace',
                          ),
                        ),
                      ],
                    ),
                  ),
                  _coreAction(palette),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _coreAction(PohPalette palette) {
    if (core.installing) {
      return SizedBox(
        width: 14,
        height: 14,
        child: CircularProgressIndicator(
          strokeWidth: 2,
          color: palette.accent,
        ),
      );
    }
    if (core.status == CoreStatus.active) {
      return Text(
        'Open',
        style: TextStyle(
          color: palette.accent,
          fontSize: 10,
          fontWeight: FontWeight.w800,
        ),
      );
    }
    if (core.installable) {
      return Text(
        'Install',
        style: TextStyle(
          color: palette.accent.withValues(alpha: 0.8),
          fontSize: 10,
          fontWeight: FontWeight.w800,
        ),
      );
    }
    return Text(
      'Soon',
      style: TextStyle(
        color: palette.muted,
        fontSize: 10,
        fontWeight: FontWeight.w800,
      ),
    );
  }
}

class _DetailBar extends StatelessWidget {
  const _DetailBar({
    required this.core,
    required this.server,
    required this.activeRoute,
    required this.activeMode,
    required this.routePresets,
    required this.onOpenRoutes,
    required this.onSelectRouteMode,
    required this.onSelectRoutePreset,
  });

  final CoreSpec core;
  final ServerProfile server;
  final String activeRoute;
  final String activeMode;
  final List<RoutePreset> routePresets;
  final VoidCallback onOpenRoutes;
  final ValueChanged<String> onSelectRouteMode;
  final ValueChanged<RoutePreset> onSelectRoutePreset;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);

    final pills = <Widget>[
      _DetailPill(label: 'PROTOCOL', value: core.protocol),
      _DetailPill(
        label: 'PING',
        value: server.pingMs <= 0 ? '-' : '${server.pingMs} ms',
      ),
    ];

    Widget routeModeSelector() {
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            'MODE',
            style: TextStyle(
              color: palette.muted,
              fontSize: 11,
              fontWeight: FontWeight.w800,
            ),
          ),
          const SizedBox(width: 12),
          PopupMenuButton<String>(
            tooltip: 'Choose routing mode',
            color: palette.surface,
            elevation: 10,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
              side: BorderSide(color: palette.border),
            ),
            offset: const Offset(0, -8),
            onSelected: onSelectRouteMode,
            itemBuilder: (context) {
              return const [
                _RouteModeMenuEntry(
                  value: 'tun',
                  title: 'TUN',
                  subtitle:
                      'Intercepts all system traffic through a virtual network adapter.',
                ),
                _RouteModeMenuEntry(
                  value: 'system_proxy',
                  title: 'System Proxy',
                  subtitle:
                      'Sets the Windows system proxy for applications that respect it.',
                ),
                _RouteModeMenuEntry(
                  value: 'local_proxy_gate',
                  title: 'Local Proxy Gate',
                  subtitle:
                      'Hosts a local proxy server without changing system settings.',
                ),
              ];
            },
            child: _RouteInlineButton(
              width: 138,
              text: _routeModeLabel(activeMode),
            ),
          ),
        ],
      );
    }

    Widget routePresetSelector() {
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            'ROUTE',
            style: TextStyle(
              color: palette.muted,
              fontSize: 11,
              fontWeight: FontWeight.w800,
            ),
          ),
          const SizedBox(width: 12),
          PopupMenuButton<RoutePreset>(
            tooltip: 'Choose route preset',
            color: palette.surface,
            elevation: 10,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
              side: BorderSide(color: palette.border),
            ),
            offset: const Offset(0, -8),
            onSelected: onSelectRoutePreset,
            itemBuilder: (context) {
              return routePresets
                  .map(
                    (preset) => PopupMenuItem<RoutePreset>(
                      value: preset,
                      child: _RoutePresetMenuItem(
                        title: _quickPresetLabel(preset),
                        subtitle: _quickPresetDescription(preset),
                        selected: _activeRouteMatchesPreset(
                          activeRoute,
                          preset,
                        ),
                      ),
                    ),
                  )
                  .toList(growable: false);
            },
            child: _RouteInlineButton(
              width: 148,
              text: _activeRouteLabel(activeRoute, routePresets),
            ),
          ),
        ],
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final isNarrow = constraints.maxWidth < 520;
        final pillWrap = Wrap(spacing: 8, runSpacing: 8, children: pills);

        if (isNarrow) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              pillWrap,
              const SizedBox(height: 10),
              Wrap(
                alignment: WrapAlignment.end,
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 14,
                runSpacing: 10,
                children: [
                  routeModeSelector(),
                  routePresetSelector(),
                ],
              ),
            ],
          );
        }

        return Row(
          children: [
            Expanded(child: pillWrap),
            const SizedBox(width: 12),
            routeModeSelector(),
            const SizedBox(width: 14),
            routePresetSelector(),
          ],
        );
      },
    );
  }
}

String _routeModeLabel(String mode) {
  return switch (mode) {
    'system_proxy' => 'System Proxy',
    'local_proxy_gate' => 'Local Proxy Gate',
    _ => 'TUN',
  };
}

String _quickPresetLabel(RoutePreset preset) {
  return switch (preset.id) {
    'proxy_all' => 'Proxy All',
    'bypass_ru' => 'Bypass RU',
    'list_only' => 'List Only',
    'direct' => 'Direct Connection',
    _ => preset.name,
  };
}

String _quickPresetDescription(RoutePreset preset) {
  return switch (preset.id) {
    'proxy_all' => 'Absolutely all device traffic is routed through the proxy.',
    'bypass_ru' =>
      'Traffic to Russian resources goes directly, foreign sites via proxy.',
    'list_only' =>
      'Only domains and IP addresses you manually added will be routed.',
    'direct' => 'Proxy is not used for routing; traffic goes directly.',
    _ => preset.description,
  };
}

String _activeRouteLabel(String activeRoute, List<RoutePreset> presets) {
  for (final preset in presets) {
    if (_activeRouteMatchesPreset(activeRoute, preset)) {
      return _quickPresetLabel(preset);
    }
  }

  return activeRoute.isEmpty ? 'Default' : activeRoute;
}

bool _activeRouteMatchesPreset(String activeRoute, RoutePreset preset) {
  return preset.id == activeRoute ||
      preset.name == activeRoute ||
      _quickPresetLabel(preset) == activeRoute;
}

class _RouteInlineButton extends StatelessWidget {
  const _RouteInlineButton({
    required this.width,
    required this.text,
  });

  final double width;
  final String text;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: width,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
      decoration: BoxDecoration(
        color: palette.input,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: palette.border),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: palette.text,
                fontSize: 13,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          Icon(Icons.keyboard_arrow_down_rounded,
              color: palette.muted, size: 18),
        ],
      ),
    );
  }
}

class _RouteModeMenuEntry extends PopupMenuEntry<String> {
  const _RouteModeMenuEntry({
    required this.value,
    required this.title,
    required this.subtitle,
  });

  final String value;
  final String title;
  final String subtitle;

  @override
  double get height => 64;

  @override
  bool represents(String? value) => value == this.value;

  @override
  State<_RouteModeMenuEntry> createState() => _RouteModeMenuEntryState();
}

class _RouteModeMenuEntryState extends State<_RouteModeMenuEntry> {
  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return InkWell(
      onTap: () => Navigator.pop<String>(context, widget.value),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(
              widget.title,
              style: TextStyle(
                color: palette.text,
                fontWeight: FontWeight.w900,
                fontSize: 13,
              ),
            ),
            const SizedBox(height: 3),
            Text(
              widget.subtitle,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: palette.muted, fontSize: 11.5),
            ),
          ],
        ),
      ),
    );
  }
}

class _RoutePresetMenuItem extends StatelessWidget {
  const _RoutePresetMenuItem({
    required this.title,
    required this.subtitle,
    required this.selected,
  });

  final String title;
  final String subtitle;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return SizedBox(
      width: 250,
      child: Row(
        children: [
          Icon(
            selected ? Icons.check_circle_rounded : Icons.route_rounded,
            color: selected ? palette.accent : palette.muted,
            size: 18,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: TextStyle(
                    color: palette.text,
                    fontWeight: FontWeight.w900,
                    fontSize: 13,
                  ),
                ),
                const SizedBox(height: 3),
                Text(
                  subtitle,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: palette.muted, fontSize: 11.5),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _MetricCard extends StatelessWidget {
  const _MetricCard({
    required this.title,
    required this.value,
    required this.sparkline,
    required this.primary,
  });

  final String title;
  final double value;
  final List<double> sparkline;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 166,
      height: 72,
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: palette.border),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.06),
            blurRadius: 14,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Row(
        children: [
          Sparkline(values: sparkline, isPrimary: primary),
          const Spacer(),
          Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(
                title,
                style: TextStyle(
                  color: palette.muted,
                  fontSize: 9,
                  fontWeight: FontWeight.w800,
                ),
              ),
              Text(
                value.toStringAsFixed(1),
                style: TextStyle(
                  color: palette.text,
                  fontSize: 19,
                  fontWeight: FontWeight.w900,
                  height: 1,
                ),
              ),
              Text(
                'Mbps',
                style: TextStyle(color: palette.muted, fontSize: 10),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _ConnectionStatus extends StatelessWidget {
  const _ConnectionStatus({
    required this.phase,
    required this.core,
    required this.connectedAt,
    this.message,
    this.compact = false,
  });

  final ConnectionPhase phase;
  final CoreSpec core;
  final DateTime? connectedAt;
  final String? message;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final duration = connectedAt == null
        ? null
        : DateTime.now().difference(connectedAt!).inSeconds;
    return Column(
      crossAxisAlignment:
          compact ? CrossAxisAlignment.start : CrossAxisAlignment.center,
      children: [
        Row(
          mainAxisAlignment:
              compact ? MainAxisAlignment.start : MainAxisAlignment.center,
          children: [
            Container(
              width: 8,
              height: 8,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: phase == ConnectionPhase.connected
                    ? palette.accent
                    : palette.muted,
              ),
            ),
            const SizedBox(width: 7),
            Flexible(
              child: Text(
                phaseLabel(phase),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: palette.text,
                  fontSize: compact ? 18 : 22,
                  fontWeight: FontWeight.w900,
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 3),
        Text(
          message ??
              (phase == ConnectionPhase.connected
                  ? 'via ${core.name} - ${_formatDuration(duration ?? 0)}'
                  : phase == ConnectionPhase.idle
                      ? 'Ready to connect'
                      : 'Setting up secure channel'),
          style: TextStyle(
            color: palette.muted,
            fontSize: compact ? 11.5 : 13,
          ),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ],
    );
  }
}

String _formatDuration(int seconds) {
  final hours = (seconds ~/ 3600).toString().padLeft(2, '0');
  final minutes = ((seconds % 3600) ~/ 60).toString().padLeft(2, '0');
  final secs = (seconds % 60).toString().padLeft(2, '0');
  return '$hours:$minutes:$secs';
}

class _ServerStrip extends StatelessWidget {
  const _ServerStrip({required this.server});

  final ServerProfile server;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final subtitle = server.city.trim().isEmpty
        ? server.host
        : '${server.city} - ${server.host}';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: palette.border),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.08),
            blurRadius: 18,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Row(
        children: [
          _FlagChip(code: server.countryCode),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  server.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.text,
                    fontWeight: FontWeight.w900,
                    fontSize: 15,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.muted,
                    fontSize: 12,
                    fontFamily: 'monospace',
                  ),
                ),
                if (server.tlsVerificationDisabled) ...[
                  const SizedBox(height: 6),
                  const _TlsWarningChip(),
                ],
              ],
            ),
          ),
          _PingLabel(pingMs: server.pingMs),
        ],
      ),
    );
  }
}

class _DockServerCard extends StatelessWidget {
  const _DockServerCard({required this.core, required this.server});

  final CoreSpec core;
  final ServerProfile server;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: palette.border),
      ),
      child: Row(
        children: [
          _FlagChip(code: server.countryCode),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  server.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.text,
                    fontWeight: FontWeight.w900,
                    fontSize: 14,
                  ),
                ),
                Text(
                  '${server.host} - ${core.protocol}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: palette.muted, fontSize: 11.5),
                ),
                if (server.tlsVerificationDisabled) ...[
                  const SizedBox(height: 6),
                  const _TlsWarningChip(compact: true),
                ],
              ],
            ),
          ),
          _PingLabel(pingMs: server.pingMs),
        ],
      ),
    );
  }
}

class _DetailPill extends StatelessWidget {
  const _DetailPill({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 13, vertical: 9),
      decoration: BoxDecoration(
        color: palette.subtle,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            label,
            style: TextStyle(
              color: palette.muted,
              fontSize: 9,
              fontWeight: FontWeight.w800,
            ),
          ),
          Text(
            value,
            style: TextStyle(
              color: palette.text,
              fontSize: 13,
              fontWeight: FontWeight.w900,
            ),
          ),
        ],
      ),
    );
  }
}

class _TlsWarningChip extends StatelessWidget {
  const _TlsWarningChip({this.compact = false});

  final bool compact;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    const warning = Color(0xFFE26060);
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 7 : 9,
        vertical: compact ? 3 : 5,
      ),
      decoration: BoxDecoration(
        color: warning.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: warning.withValues(alpha: 0.28)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.shield_outlined, color: warning, size: compact ? 11 : 13),
          SizedBox(width: compact ? 4 : 5),
          Text(
            'TLS verification off',
            style: TextStyle(
              color: palette.text,
              fontSize: compact ? 9.5 : 11,
              fontWeight: FontWeight.w800,
            ),
          ),
        ],
      ),
    );
  }
}

class _AddServerButton extends StatelessWidget {
  const _AddServerButton({
    required this.compact,
    required this.onPressed,
  });

  final bool compact;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
      color: Colors.transparent,
      borderRadius: BorderRadius.circular(14),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onPressed,
        borderRadius: BorderRadius.circular(14),
        child: Container(
          height: 42,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            border: Border.all(color: palette.border),
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.add_rounded, color: palette.muted, size: 17),
              const SizedBox(width: 6),
              Text(
                'Add Server',
                style: TextStyle(
                  color: palette.muted,
                  fontSize: 13,
                  fontWeight: FontWeight.w800,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _PingLoad extends StatelessWidget {
  const _PingLoad({required this.server});

  final ServerProfile server;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        _PingLabel(pingMs: server.pingMs),
        const SizedBox(height: 6),
        Container(
          width: 46,
          height: 4,
          decoration: BoxDecoration(
            color: palette.border,
            borderRadius: BorderRadius.circular(10),
          ),
          alignment: Alignment.centerLeft,
          child: FractionallySizedBox(
            widthFactor: server.load.clamp(0, 1),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: palette.muted,
                borderRadius: BorderRadius.circular(10),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _PingLabel extends StatelessWidget {
  const _PingLabel({required this.pingMs});

  final int pingMs;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final hasPing = pingMs > 0;
    final color = !hasPing
        ? palette.muted.withValues(alpha: 0.55)
        : pingMs < 35
            ? const Color(0xFF3A9B6E)
            : pingMs < 60
                ? const Color(0xFFC4A343)
                : const Color(0xFFC46A4F);

    return SizedBox(
      height: 18,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Container(
            width: 7,
            height: 7,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 5),
          Text.rich(
            TextSpan(
              text: hasPing ? '$pingMs' : '-',
              children: [
                TextSpan(
                  text: ' ms',
                  style: TextStyle(
                    color: palette.muted,
                    fontSize: 9.5,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
            strutStyle: const StrutStyle(
              forceStrutHeight: true,
              fontSize: 12.5,
              height: 1.0,
            ),
            style: TextStyle(
              color: hasPing ? color : palette.muted,
              fontSize: 12.5,
              fontWeight: FontWeight.w800,
              height: 1.0,
            ),
          ),
        ],
      ),
    );
  }
}

class _CoreTabButton extends StatelessWidget {
  const _CoreTabButton({
    required this.core,
    required this.tab,
    required this.active,
    required this.onPressed,
  });

  final CoreSpec core;
  final SessionTab tab;
  final bool active;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Padding(
      padding: const EdgeInsets.only(right: 4),
      child: Material(
        color: active ? palette.surface : Colors.transparent,
        borderRadius: const BorderRadius.vertical(top: Radius.circular(10)),
        child: InkWell(
          borderRadius: const BorderRadius.vertical(top: Radius.circular(10)),
          onTap: onPressed,
          child: Container(
            height: 36,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              borderRadius:
                  const BorderRadius.vertical(top: Radius.circular(10)),
              border: active
                  ? Border(
                      top: BorderSide(color: palette.border),
                      left: BorderSide(color: palette.border),
                      right: BorderSide(color: palette.border),
                      // No bottom border — the tab "sits" on the content container line
                      bottom: BorderSide(color: palette.surface, width: 1),
                    )
                  : Border.all(color: Colors.transparent),
              boxShadow: active
                  ? [
                      BoxShadow(
                        color: Colors.black.withValues(alpha: 0.10),
                        blurRadius: 10,
                        offset: const Offset(0, -1),
                      ),
                    ]
                  : null,
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                _CoreMark(core: core, compact: true),
                const SizedBox(width: 8),
                Text(
                  core.name,
                  style: TextStyle(
                    color: active ? palette.text : palette.muted,
                    fontSize: 12.5,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _CoreListButton extends StatelessWidget {
  const _CoreListButton({
    required this.core,
    required this.count,
    required this.onPressed,
  });

  final CoreSpec core;
  final int count;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
      color: palette.surface,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: onPressed,
        child: Container(
          height: 34,
          padding: const EdgeInsets.symmetric(horizontal: 7),
          decoration: BoxDecoration(
            border: Border.all(color: palette.border),
            borderRadius: BorderRadius.circular(12),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _CoreMark(core: core, compact: true),
              const SizedBox(width: 7),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
                decoration: BoxDecoration(
                  color: core.accent,
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  '$count',
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 11,
                    fontWeight: FontWeight.w900,
                  ),
                ),
              ),
              Icon(Icons.keyboard_arrow_down_rounded,
                  color: palette.muted, size: 18),
            ],
          ),
        ),
      ),
    );
  }
}

class _CoreMark extends StatelessWidget {
  const _CoreMark({required this.core, this.compact = false});

  final CoreSpec core;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final size = compact ? 20.0 : 26.0;
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: core.accent,
        borderRadius: BorderRadius.circular(compact ? 6 : 8),
      ),
      child: Text(
        core.letter,
        style: TextStyle(
          color: Colors.white,
          fontSize: compact ? 11 : 12,
          fontWeight: FontWeight.w900,
          fontFamily: 'monospace',
        ),
      ),
    );
  }
}

class _FlagChip extends StatelessWidget {
  const _FlagChip({
    required this.code,
    this.selected = false,
  });

  final String code;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 38,
      height: 28,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: selected ? palette.accent : palette.subtle,
        borderRadius: BorderRadius.circular(7),
        border: Border.all(
          color: selected ? Colors.transparent : palette.border,
        ),
      ),
      child: Text(
        code,
        style: TextStyle(
          color: selected ? Colors.white : palette.muted,
          fontSize: 12,
          fontFamily: 'monospace',
          fontWeight: FontWeight.w800,
        ),
      ),
    );
  }
}

class _RoundIconButton extends StatelessWidget {
  const _RoundIconButton({
    required this.tooltip,
    required this.icon,
    this.onPressed,
    this.onPressedAt,
    this.height = 36,
    this.iconSize = 18,
  }) : assert(onPressed != null || onPressedAt != null);

  final String tooltip;
  final IconData icon;
  final VoidCallback? onPressed;
  final ValueChanged<Offset>? onPressedAt;
  final double height;
  final double iconSize;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Tooltip(
      message: tooltip,
      child: Material(
        color: palette.surface,
        borderRadius: BorderRadius.circular(10),
        child: InkWell(
          borderRadius: BorderRadius.circular(10),
          onTap: onPressedAt == null ? onPressed : null,
          onTapDown: onPressedAt == null
              ? null
              : (details) => onPressedAt!(details.globalPosition),
          child: Container(
            height: height,
            constraints: BoxConstraints(minWidth: height),
            alignment: Alignment.center,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: palette.border),
            ),
            child: Icon(icon, color: palette.text, size: iconSize),
          ),
        ),
      ),
    );
  }
}

class _HeaderIconButton extends StatelessWidget {
  const _HeaderIconButton({
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    this.bottomPadding = 0,
    this.danger = false,
  });

  final String tooltip;
  final IconData icon;
  final VoidCallback onPressed;
  final double bottomPadding;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Tooltip(
      message: tooltip,
      child: Padding(
        padding: EdgeInsets.only(bottom: bottomPadding),
        child: SizedBox(
          width: 34,
          height: 34,
          child: IconButton(
            padding: EdgeInsets.zero,
            visualDensity: VisualDensity.compact,
            splashRadius: 17,
            tooltip: tooltip,
            color: palette.muted,
            hoverColor: danger
                ? const Color(0xFFB94444).withValues(alpha: 0.16)
                : palette.hover,
            onPressed: onPressed,
            icon: Icon(icon, size: 18),
          ),
        ),
      ),
    );
  }
}

class _AppMark extends StatelessWidget {
  const _AppMark({required this.accent});

  final Color accent;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 24,
      height: 24,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: accent,
        borderRadius: BorderRadius.circular(8),
        boxShadow: [
          BoxShadow(
            color: accent.withValues(alpha: 0.22),
            spreadRadius: 3,
            blurRadius: 0,
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(7),
        child: Image.asset(
          'assets/logo/proxy-open-hub-128.png',
          width: 24,
          height: 24,
          fit: BoxFit.cover,
          filterQuality: FilterQuality.high,
        ),
      ),
    );
  }
}
