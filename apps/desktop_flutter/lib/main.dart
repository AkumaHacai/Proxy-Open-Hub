import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'models/app_models.dart';
import 'screens/import_profile_shell.dart';
import 'screens/logs_shell.dart';
import 'screens/settings_shell.dart';
import 'services/app_settings_store.dart';
import 'services/backend_session_service.dart';
import 'services/desktop_state_loader.dart';
import 'services/traffic_metrics_service.dart';
import 'services/window_controls.dart';
import 'theme/poh_theme.dart';
import 'widgets/sparkline.dart';
import 'widgets/theme_reveal_wrapper.dart';

void main() {
  runApp(const ProxyOpenHubApp());
}

enum _DesktopOverlay { none, settings, logs, importProfile }

class ProxyOpenHubApp extends StatefulWidget {
  const ProxyOpenHubApp({super.key});

  @override
  State<ProxyOpenHubApp> createState() => _ProxyOpenHubAppState();
}

class _ProxyOpenHubAppState extends State<ProxyOpenHubApp> {
  var _themeMode = PohThemeMode.dark;
  var _accent = PohAccent.forest;
  var _compact = false;
  var _animationsEnabled = true;
  var _animationDurationMs = 520;
  var _density = 'comfortable';
  var _startupMode = 'expanded';
  var _defaultCore = 'TrustTunnel';
  var _defaultRoute = 'Default';
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
  var _servers = <ServerProfile>[];
  var _profilesLoaded = false;
  var _overlay = _DesktopOverlay.none;
  String? _connectionMessage;
  final _settingsStore = const AppSettingsStore();
  final _backendSessionService = const BackendSessionService();
  final _trafficMetricsService = TrafficMetricsService();
  final _themeRevealKey = GlobalKey<ThemeRevealWrapperState>();
  final _tabs = <SessionTab>[];
  final _timers = <Timer>[];
  Timer? _trafficTimer;
  Timer? _clockTimer;
  Timer? _sessionStatusTimer;
  var _trafficSampleInFlight = false;
  var _sessionStatusInFlight = false;

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

  CoreSpec get _activeCore => findCore(_activeTab.coreId);

  ServerProfile get _selectedServer {
    if (_servers.isEmpty) {
      return const ServerProfile(
        id: '',
        name: 'No server selected',
        host: 'Import a TrustTunnel profile',
        countryCode: 'TT',
        city: '',
        pingMs: 0,
        dns: 'HTTP/2 - TUN',
        load: 0,
      );
    }

    return _servers.firstWhere(
      (server) => server.id == _activeTab.selectedServerId,
      orElse: () => _servers.first,
    );
  }

  bool get _connected => _activeTab.phase == ConnectionPhase.connected;

  Duration get _motionDuration {
    return _animationsEnabled
        ? Duration(milliseconds: _animationDurationMs)
        : Duration.zero;
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
      animationsEnabled: _animationsEnabled,
      animationDurationMs: _animationDurationMs,
      density: _density,
      startupMode: _startupMode,
      defaultCore: _defaultCore,
      defaultRoute: _defaultRoute,
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
    _loadSettings();
    _loadProfiles();
  }

  @override
  void dispose() {
    _clearTimers();
    _stopTraffic();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final accent = _accent;

    return MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: buildPohTheme(mode: PohThemeMode.light, accent: accent),
      darkTheme: buildPohTheme(mode: PohThemeMode.dark, accent: accent),
      themeMode: _themeMode.materialMode,
      home: ThemeRevealWrapper(
        key: _themeRevealKey,
        themeMode: _themeMode,
        accent: accent,
        duration: _motionDuration,
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
                      activeCore: _activeCore,
                      activeTab: _activeTab,
                      selectedServer: _selectedServer,
                      servers: _servers,
                      profilesLoaded: _profilesLoaded,
                      tabs: _tabs,
                      download: _download,
                      upload: _upload,
                      progress: _progress,
                      connected: _connected,
                      working: _working,
                      connectionMessage: _connectionMessage,
                      themeMode: _themeMode,
                      motionDuration: _motionDuration,
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
                      onSelectServer: _selectServer,
                      onToggleConnection: _toggleConnection,
                      onOpenSettings: () {
                        setState(() => _overlay = _DesktopOverlay.settings);
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
                    Positioned.fill(
                      child: GestureDetector(
                        behavior: HitTestBehavior.opaque,
                        onTap: () {
                          setState(() => _overlay = _DesktopOverlay.none);
                        },
                        child: ColoredBox(
                          color: Colors.black.withValues(alpha: 0.38),
                        ),
                      ),
                    ),
                  if (_overlay != _DesktopOverlay.none)
                    Center(
                      child: _overlay == _DesktopOverlay.settings
                          ? SettingsShell(
                              themeMode: _themeMode,
                              accent: _accent,
                              animationsEnabled: _animationsEnabled,
                              animationDurationMs: _animationDurationMs,
                              density: _density,
                              startupMode: _startupMode,
                              defaultCore: _defaultCore,
                              defaultRoute: _defaultRoute,
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
                              onDefaultRouteChanged: (value) {
                                _applySettings(_settings.copyWith(
                                  defaultRoute: value,
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
                          : _overlay == _DesktopOverlay.importProfile
                              ? ImportProfileShell(
                                  sessionService: _backendSessionService,
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
                              : LogsShell(
                                  sessionService: _backendSessionService,
                                  onClose: () {
                                    setState(
                                      () => _overlay = _DesktopOverlay.none,
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

  void _applySettings(
    AppSettings settings, {
    bool persist = true,
    bool applyStartupModeToCurrentWindow = false,
  }) {
    setState(() {
      _themeMode = settings.themeMode;
      _accent = settings.accent;
      _animationsEnabled = settings.animationsEnabled;
      _animationDurationMs = settings.animationDurationMs;
      _density = settings.density;
      _startupMode = settings.startupMode;
      _defaultCore = settings.defaultCore;
      _defaultRoute = settings.defaultRoute;
      _autoConnect = settings.autoConnect;
      _socksLan = settings.socksLan;
      _socksAddress = settings.socksAddress;
      _httpEnabled = settings.httpEnabled;
      _httpAddress = settings.httpAddress;
      _pingHost = settings.pingHost;
      _httpsUrl = settings.httpsUrl;
      _timeoutSeconds = settings.timeoutSeconds;
      if (applyStartupModeToCurrentWindow) {
        _compact = settings.startupMode == 'compact';
      }
    });

    if (persist) {
      unawaited(_settingsStore.save(settings));
    }
  }

  Future<void> _saveSettings() async {
    await _settingsStore.save(_settings);
  }

  void _addCore(String coreId) {
    final alreadyOpen = _tabs.any((tab) => tab.coreId == coreId);
    if (alreadyOpen) {
      final existing = _tabs.firstWhere((tab) => tab.coreId == coreId);
      _selectTab(existing.id);
      return;
    }

    final core = findCore(coreId);
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
          selectedServerId: _servers.isEmpty ? '' : _servers.first.id,
          phase: ConnectionPhase.idle,
          connectedAt: null,
        ),
      );
      _activeTabId = id;
      _coreMenuOpen = false;
      _progress = 0;
    });
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
  }

  void _toggleConnection() {
    if (_servers.isEmpty) {
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

  void _toggleCompact() {
    final compact = !_compact;
    setState(() {
      _compact = compact;
      _coreMenuOpen = false;
    });

    unawaited(
      WindowControls.animateWindowSize(
        width: compact ? 360 : 960,
        height: compact ? 650 : 660,
        duration: _motionDuration,
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
    final accent = _accent;
    if (reveal == null || _motionDuration == Duration.zero) {
      setState(() => _themeMode = mode);
      unawaited(_saveSettings());
      return;
    }

    await reveal.revealTo(
      themeMode: mode,
      accent: accent,
      globalOrigin: origin,
    );
    if (!mounted) {
      return;
    }

    setState(() => _themeMode = mode);
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
      final session =
          await _backendSessionService.startTrustTunnelSession(server);
      if (!mounted || id != _activeTabId) {
        return;
      }

      _patchTab(id, phase: ConnectionPhase.authenticating);
      setState(() {
        _connectionMessage = 'TrustTunnel core started: PID ${session.pid}';
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
          session.startedAtUnixMs,
        ),
      );
      setState(() {
        _connectionMessage = null;
        _progress = 1;
      });
      _startTraffic();
      _startSessionStatusPolling();
    } catch (error) {
      if (!mounted) {
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

  Future<void> _disconnect() async {
    final id = _activeTabId;
    _clearTimers();
    _stopTraffic();
    setState(() => _connectionMessage = 'Disconnecting...');
    _patchTab(id, phase: ConnectionPhase.disconnecting);
    setState(() => _progress = 0.18);

    try {
      await _backendSessionService.stopTrustTunnelSession();
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

  void _startSessionStatusPolling() {
    _sessionStatusTimer?.cancel();
    _sessionStatusTimer = Timer.periodic(const Duration(seconds: 2), (_) async {
      if (!_connected || !mounted || _sessionStatusInFlight) {
        return;
      }

      _sessionStatusInFlight = true;
      try {
        final status = await _backendSessionService.trustTunnelSessionStatus();
        if (!mounted || !_connected || status.running) {
          return;
        }

        _clearTimers();
        _stopTraffic();
        _patchTab(_activeTabId,
            phase: ConnectionPhase.idle, clearConnectedAt: true);
        setState(() {
          _connectionMessage = 'TrustTunnel core exited';
          _progress = 0;
          _download = List<double>.filled(16, 0);
          _upload = List<double>.filled(16, 0);
        });
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
    final loaded = await const DesktopStateLoader().loadProfiles();
    if (!mounted) {
      return;
    }

    setState(() {
      _servers = loaded;
      _profilesLoaded = true;
      _tabs
        ..clear()
        ..add(
          SessionTab(
            id: 1,
            coreId: 'trusttunnel',
            selectedServerId: loaded.isEmpty ? '' : loaded.first.id,
            phase: ConnectionPhase.idle,
            connectedAt: null,
          ),
        );
      _activeTabId = 1;
      _nextTabId = 1;
    });
  }
}

class _DesktopWindow extends StatelessWidget {
  const _DesktopWindow({
    required this.compact,
    required this.coreMenuOpen,
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
    required this.motionDuration,
    required this.onToggleCompact,
    required this.onToggleTheme,
    required this.onToggleCoreMenu,
    required this.onDismissCoreMenu,
    required this.onSelectTab,
    required this.onAddCore,
    required this.onSelectServer,
    required this.onToggleConnection,
    required this.onOpenSettings,
    required this.onOpenLogs,
    required this.onOpenImportProfile,
  });

  final bool compact;
  final bool coreMenuOpen;
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
  final Duration motionDuration;
  final VoidCallback onToggleCompact;
  final ValueChanged<Offset> onToggleTheme;
  final VoidCallback onToggleCoreMenu;
  final VoidCallback onDismissCoreMenu;
  final ValueChanged<int> onSelectTab;
  final ValueChanged<String> onAddCore;
  final ValueChanged<String> onSelectServer;
  final VoidCallback onToggleConnection;
  final VoidCallback onOpenSettings;
  final VoidCallback onOpenLogs;
  final VoidCallback onOpenImportProfile;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final compactLayout = compact || constraints.maxWidth < 720;

        return Stack(
          children: [
            Positioned.fill(
              child: Column(
                children: [
                  _TitleBar(
                    compact: compactLayout,
                    activeCore: activeCore,
                    activeTab: activeTab,
                    tabs: tabs,
                    onToggleCompact: onToggleCompact,
                    onToggleCoreMenu: onToggleCoreMenu,
                    onSelectTab: onSelectTab,
                  ),
                  Expanded(
                    child: _MorphingBody(
                      compact: compactLayout,
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
                      motionDuration: motionDuration,
                      onSelectServer: onSelectServer,
                      onToggleConnection: onToggleConnection,
                      onToggleTheme: onToggleTheme,
                      onOpenSettings: onOpenSettings,
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
                left: compactLayout ? 52 : 68,
                top: 48,
                child: _CoreMenu(
                  activeCoreId: activeCore.id,
                  openTabs: tabs,
                  onAddCore: onAddCore,
                ),
              ),
          ],
        );
      },
    );
  }
}

class _TitleBar extends StatelessWidget {
  const _TitleBar({
    required this.compact,
    required this.activeCore,
    required this.activeTab,
    required this.tabs,
    required this.onToggleCompact,
    required this.onToggleCoreMenu,
    required this.onSelectTab,
  });

  final bool compact;
  final CoreSpec activeCore;
  final SessionTab activeTab;
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
    required this.motionDuration,
    required this.onSelectServer,
    required this.onToggleConnection,
    required this.onToggleTheme,
    required this.onOpenSettings,
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
  final Duration motionDuration;
  final ValueChanged<String> onSelectServer;
  final VoidCallback onToggleConnection;
  final ValueChanged<Offset> onToggleTheme;
  final VoidCallback onOpenSettings;
  final VoidCallback onOpenLogs;
  final VoidCallback onOpenImportProfile;

  @override
  Widget build(BuildContext context) {
    final sidebarWidth = compact ? double.infinity : 316.0;
    const compactPanelHeight = 216.0;
    const expandedSidebarWidth = 316.0;
    const ringSize = 170.0;
    const compactButtonHeight = 56.0;
    final curve = motionDuration == Duration.zero
        ? Curves.linear
        : Curves.fastLinearToSlowEaseIn;

    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        final height = constraints.maxHeight;
        final rightWidth = math.max(0.0, width - expandedSidebarWidth);
        final sidebarHeight =
            compact ? math.max(0.0, height - compactPanelHeight) : height;
        final detailLeft = compact ? width + 24 : expandedSidebarWidth;
        final detailWidth = compact ? rightWidth : rightWidth;
        final ringLeft = compact
            ? 12.0
            : expandedSidebarWidth + math.max(0.0, (rightWidth - ringSize) / 2);
        final ringTop = compact
            ? math.max(0.0, height - 12 - compactButtonHeight)
            : math.max(88.0, (height - ringSize) / 2 - 88);
        final ringWidth = compact ? math.max(0.0, width - 24) : ringSize;
        final ringHeight = compact ? compactButtonHeight : ringSize;

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
                  onToggleTheme: onToggleTheme,
                  themeMode: themeMode,
                  onOpenSettings: onOpenSettings,
                  onOpenLogs: onOpenLogs,
                  onOpenImportProfile: onOpenImportProfile,
                ),
              ),
              AnimatedPositioned(
                duration: motionDuration,
                curve: curve,
                left: detailLeft,
                top: 0,
                width: detailWidth,
                height: height,
                child: IgnorePointer(
                  ignoring: compact,
                  child: AnimatedOpacity(
                    duration: motionDuration,
                    curve: Curves.easeOutCubic,
                    opacity: compact ? 0 : 1,
                    child: _ExpandedDetailPane(
                      activeCore: activeCore,
                      activeTab: activeTab,
                      selectedServer: selectedServer,
                      download: download,
                      upload: upload,
                      connected: connected,
                      connectionMessage: connectionMessage,
                    ),
                  ),
                ),
              ),
              AnimatedPositioned(
                duration: motionDuration,
                curve: curve,
                left: compact ? 0 : -width,
                bottom: 0,
                width: width,
                height: compactPanelHeight,
                child: IgnorePointer(
                  ignoring: !compact,
                  child: AnimatedOpacity(
                    duration: motionDuration,
                    curve: Curves.easeOutCubic,
                    opacity: compact ? 1 : 0,
                    child: _CompactDockPane(
                      activeCore: activeCore,
                      activeTab: activeTab,
                      selectedServer: selectedServer,
                      connectionMessage: connectionMessage,
                      themeMode: themeMode,
                      onToggleTheme: onToggleTheme,
                      onOpenSettings: onOpenSettings,
                      onOpenLogs: onOpenLogs,
                    ),
                  ),
                ),
              ),
              AnimatedPositioned(
                duration: motionDuration,
                curve: curve,
                left: ringLeft,
                top: ringTop,
                width: ringWidth,
                height: ringHeight,
                child: _MorphingConnectControl(
                  compact: compact,
                  phase: activeTab.phase,
                  progress: progress,
                  duration: motionDuration,
                  onPressed: working ? null : onToggleConnection,
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
    required this.activeCore,
    required this.activeTab,
    required this.selectedServer,
    required this.download,
    required this.upload,
    required this.connected,
    required this.connectionMessage,
  });

  final CoreSpec activeCore;
  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<double> download;
  final List<double> upload;
  final bool connected;
  final String? connectionMessage;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(26, 18, 26, 22),
      child: Column(
        children: [
          _ServerStrip(server: selectedServer),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final ringSpace = math.min(204.0, constraints.maxHeight * 0.38);
                return Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    SizedBox(height: ringSpace),
                    _ConnectionStatus(
                      phase: activeTab.phase,
                      core: activeCore,
                      connectedAt: activeTab.connectedAt,
                      message: connectionMessage,
                    ),
                    const SizedBox(height: 14),
                    Wrap(
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
                  ],
                );
              },
            ),
          ),
          Container(height: 1, color: palette.border),
          const SizedBox(height: 14),
          _DetailBar(core: activeCore, server: selectedServer),
        ],
      ),
    );
  }
}

class _CompactDockPane extends StatelessWidget {
  const _CompactDockPane({
    required this.activeCore,
    required this.activeTab,
    required this.selectedServer,
    required this.connectionMessage,
    required this.themeMode,
    required this.onToggleTheme,
    required this.onOpenSettings,
    required this.onOpenLogs,
  });

  final CoreSpec activeCore;
  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final String? connectionMessage;
  final PohThemeMode themeMode;
  final ValueChanged<Offset> onToggleTheme;
  final VoidCallback onOpenSettings;
  final VoidCallback onOpenLogs;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 8, 12, 78),
        child: Column(
          children: [
            _DockServerCard(core: activeCore, server: selectedServer),
            const SizedBox(height: 10),
            Row(
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
                  onPressed: onOpenSettings,
                ),
                const SizedBox(width: 8),
                _RoundIconButton(
                  tooltip: 'Logs',
                  icon: Icons.article_outlined,
                  onPressed: onOpenLogs,
                ),
                const SizedBox(width: 8),
                _RoundIconButton(
                  tooltip: 'Settings',
                  icon: Icons.tune_rounded,
                  onPressed: onOpenSettings,
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
                ),
              ],
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

class _ServerSidebar extends StatelessWidget {
  const _ServerSidebar({
    required this.activeTab,
    required this.selectedServer,
    required this.servers,
    required this.profilesLoaded,
    required this.connected,
    required this.onSelectServer,
    required this.onToggleTheme,
    required this.themeMode,
    required this.onOpenSettings,
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
  final ValueChanged<Offset> onToggleTheme;
  final PohThemeMode themeMode;
  final VoidCallback onOpenSettings;
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
              onOpenImportProfile: onOpenImportProfile,
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
                      onPressed: onOpenSettings,
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
    required this.onOpenImportProfile,
  });

  final SessionTab activeTab;
  final ServerProfile selectedServer;
  final List<ServerProfile> servers;
  final bool profilesLoaded;
  final bool connected;
  final bool compact;
  final ValueChanged<String> onSelectServer;
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
      return ListView(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        children: [
          const _SidebarMessage(
            title: 'No servers yet',
            subtitle: 'Import or add a TrustTunnel profile',
            compact: true,
          ),
          const SizedBox(height: 10),
          _AddServerButton(
            compact: compact,
            onPressed: onOpenImportProfile,
          ),
        ],
      );
    }

    return ListView.separated(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      itemCount: servers.length + 1,
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
        );
      },
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
  });

  final ServerProfile server;
  final bool selected;
  final bool connectedHere;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
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
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.muted,
                        fontSize: 11.5,
                        fontFamily: 'monospace',
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              _PingLoad(server: server),
            ],
          ),
        ),
      ),
    );
  }
}

class _CoreMenu extends StatelessWidget {
  const _CoreMenu({
    required this.activeCoreId,
    required this.openTabs,
    required this.onAddCore,
  });

  final String activeCoreId;
  final List<SessionTab> openTabs;
  final ValueChanged<String> onAddCore;

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
            for (final core in coreSpecs)
              _CoreMenuItem(
                core: core,
                isOpen: openCoreIds.contains(core.id),
                isActive: core.id == activeCoreId,
                onPressed: () => onAddCore(core.id),
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
  });

  final CoreSpec core;
  final bool isOpen;
  final bool isActive;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final disabled = core.status != CoreStatus.active && !isOpen;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Material(
        color: isActive ? palette.accentSoft : Colors.transparent,
        borderRadius: BorderRadius.circular(12),
        child: InkWell(
          borderRadius: BorderRadius.circular(12),
          onTap: disabled ? null : onPressed,
          child: Opacity(
            opacity: disabled ? 0.52 : 1,
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
                  Text(
                    core.status == CoreStatus.active ? 'Open' : 'Soon',
                    style: TextStyle(
                      color: core.status == CoreStatus.active
                          ? palette.accent
                          : palette.muted,
                      fontSize: 10,
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DetailBar extends StatelessWidget {
  const _DetailBar({required this.core, required this.server});

  final CoreSpec core;
  final ServerProfile server;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);

    final pills = <Widget>[
      _DetailPill(label: 'PROTOCOL', value: core.protocol),
      _DetailPill(label: 'PING', value: '${server.pingMs} ms'),
      const _DetailPill(label: 'SESSION', value: '-'),
    ];

    Widget routeSelector() {
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
          Container(
            width: 128,
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
                    'Default',
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
              Align(
                alignment: Alignment.centerRight,
                child: routeSelector(),
              ),
            ],
          );
        }

        return Row(
          children: [
            Expanded(child: pillWrap),
            const SizedBox(width: 12),
            routeSelector(),
          ],
        );
      },
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
            Text(
              phaseLabel(phase),
              style: TextStyle(
                color: palette.text,
                fontSize: compact ? 20 : 22,
                fontWeight: FontWeight.w900,
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
            fontSize: compact ? 12 : 13,
          ),
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
                  style: TextStyle(
                    color: palette.text,
                    fontWeight: FontWeight.w900,
                    fontSize: 15,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  '${server.city} - ${server.host}',
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.muted,
                    fontSize: 12,
                    fontFamily: 'monospace',
                  ),
                ),
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
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: palette.text,
                    fontWeight: FontWeight.w900,
                    fontSize: 14,
                  ),
                ),
                Text(
                  '${server.host} - ${core.protocol}',
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: palette.muted, fontSize: 11.5),
                ),
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
    final color = pingMs < 35
        ? const Color(0xFF3A9B6E)
        : pingMs < 60
            ? const Color(0xFFC4A343)
            : const Color(0xFFC46A4F);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 7,
          height: 7,
          decoration: BoxDecoration(color: color, shape: BoxShape.circle),
        ),
        const SizedBox(width: 5),
        Text(
          '$pingMs',
          style: TextStyle(
            color: color,
            fontSize: 12.5,
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(width: 2),
        Text(
          'ms',
          style: TextStyle(
            color: PohPalette.of(context).muted,
            fontSize: 10,
          ),
        ),
      ],
    );
  }
}

class _CoreTabButton extends StatelessWidget {
  const _CoreTabButton({
    required this.tab,
    required this.active,
    required this.onPressed,
  });

  final SessionTab tab;
  final bool active;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final core = findCore(tab.coreId);
    return Padding(
      padding: const EdgeInsets.only(right: 4),
      child: Transform.translate(
        offset: Offset(0, active ? 1 : 0),
        child: Material(
          color: active ? palette.surface : Colors.transparent,
          borderRadius: const BorderRadius.vertical(top: Radius.circular(10)),
          child: InkWell(
            borderRadius: const BorderRadius.vertical(top: Radius.circular(10)),
            onTap: onPressed,
            child: Container(
              height: active ? 41 : 34,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              decoration: BoxDecoration(
                borderRadius:
                    const BorderRadius.vertical(top: Radius.circular(10)),
                border: Border(
                  top: BorderSide(
                      color: active ? palette.border : Colors.transparent),
                  left: BorderSide(
                      color: active ? palette.border : Colors.transparent),
                  right: BorderSide(
                      color: active ? palette.border : Colors.transparent),
                  bottom: BorderSide(
                    color: active ? palette.surface : Colors.transparent,
                  ),
                ),
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
  }) : assert(onPressed != null || onPressedAt != null);

  final String tooltip;
  final IconData icon;
  final VoidCallback? onPressed;
  final ValueChanged<Offset>? onPressedAt;

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
            height: 36,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: palette.border),
            ),
            child: Icon(icon, color: palette.text, size: 18),
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
        child: IconButton(
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
