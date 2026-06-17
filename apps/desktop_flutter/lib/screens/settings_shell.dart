import 'package:flutter/material.dart';

import '../services/app_settings_store.dart';
import '../services/window_controls.dart';
import '../theme/poh_theme.dart';

enum SettingsPageId { appearance, connection, proxy, diagnostics, about }

typedef ThemeModeChangedAt = void Function(PohThemeMode mode, Offset origin);
typedef ValueChangedAt<T> = void Function(T value, Offset origin);

class SettingsShell extends StatefulWidget {
  const SettingsShell({
    super.key,
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
    required this.onThemeModeChanged,
    required this.onAccentChanged,
    required this.onAnimationsEnabledChanged,
    required this.onAnimationDurationChanged,
    required this.onDensityChanged,
    required this.onStartupModeChanged,
    required this.onDefaultCoreChanged,
    required this.onDefaultRouteChanged,
    required this.onAutoConnectChanged,
    required this.onSocksLanChanged,
    required this.onSocksAddressChanged,
    required this.onHttpEnabledChanged,
    required this.onHttpAddressChanged,
    required this.onPingHostChanged,
    required this.onHttpsUrlChanged,
    required this.onTimeoutChanged,
    required this.onReset,
    required this.onSave,
    this.onClose,
  });

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
  final ThemeModeChangedAt onThemeModeChanged;
  final ValueChanged<PohAccent> onAccentChanged;
  final ValueChanged<bool> onAnimationsEnabledChanged;
  final ValueChanged<int> onAnimationDurationChanged;
  final ValueChanged<String> onDensityChanged;
  final ValueChanged<String> onStartupModeChanged;
  final ValueChanged<String> onDefaultCoreChanged;
  final ValueChanged<String> onDefaultRouteChanged;
  final ValueChanged<bool> onAutoConnectChanged;
  final ValueChanged<bool> onSocksLanChanged;
  final ValueChanged<String> onSocksAddressChanged;
  final ValueChanged<bool> onHttpEnabledChanged;
  final ValueChanged<String> onHttpAddressChanged;
  final ValueChanged<String> onPingHostChanged;
  final ValueChanged<String> onHttpsUrlChanged;
  final ValueChanged<int> onTimeoutChanged;
  final ValueChanged<AppSettings> onReset;
  final VoidCallback onSave;
  final VoidCallback? onClose;

  @override
  State<SettingsShell> createState() => _SettingsShellState();
}

class _SettingsShellState extends State<SettingsShell> {
  var _active = SettingsPageId.appearance;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 760,
      height: 600,
      decoration: BoxDecoration(
        color: palette.background,
        borderRadius: BorderRadius.circular(26),
        border: Border.all(color: palette.border),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.28),
            blurRadius: 42,
            offset: const Offset(0, 18),
          ),
        ],
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        children: [
          _SettingsTitleBar(onClose: widget.onClose),
          Expanded(
            child: Row(
              children: [
                _SettingsNav(
                  active: _active,
                  onChanged: (value) => setState(() => _active = value),
                ),
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.fromLTRB(30, 26, 30, 30),
                    child: _buildPage(),
                  ),
                ),
              ],
            ),
          ),
          _SettingsFooter(
            onReset: _reset,
            onSave: widget.onSave,
            onClose: widget.onClose,
          ),
        ],
      ),
    );
  }

  Widget _buildPage() {
    return switch (_active) {
      SettingsPageId.appearance => _AppearancePage(
          themeMode: widget.themeMode,
          accent: widget.accent,
          animationsEnabled: widget.animationsEnabled,
          animationDurationMs: widget.animationDurationMs,
          density: widget.density,
          startupMode: widget.startupMode,
          onThemeModeChanged: widget.onThemeModeChanged,
          onAccentChanged: widget.onAccentChanged,
          onAnimationsEnabledChanged: widget.onAnimationsEnabledChanged,
          onAnimationDurationChanged: widget.onAnimationDurationChanged,
          onDensityChanged: widget.onDensityChanged,
          onStartupModeChanged: widget.onStartupModeChanged,
        ),
      SettingsPageId.connection => _ConnectionPage(
          defaultCore: widget.defaultCore,
          defaultRoute: widget.defaultRoute,
          autoConnect: widget.autoConnect,
          onDefaultCoreChanged: widget.onDefaultCoreChanged,
          onDefaultRouteChanged: widget.onDefaultRouteChanged,
          onAutoConnectChanged: widget.onAutoConnectChanged,
        ),
      SettingsPageId.proxy => _ProxyPage(
          socksLan: widget.socksLan,
          socksAddress: widget.socksAddress,
          httpEnabled: widget.httpEnabled,
          httpAddress: widget.httpAddress,
          onSocksLanChanged: widget.onSocksLanChanged,
          onSocksAddressChanged: widget.onSocksAddressChanged,
          onHttpEnabledChanged: widget.onHttpEnabledChanged,
          onHttpAddressChanged: widget.onHttpAddressChanged,
        ),
      SettingsPageId.diagnostics => _DiagnosticsPage(
          pingHost: widget.pingHost,
          httpsUrl: widget.httpsUrl,
          timeoutSeconds: widget.timeoutSeconds,
          onPingHostChanged: widget.onPingHostChanged,
          onHttpsUrlChanged: widget.onHttpsUrlChanged,
          onTimeoutChanged: widget.onTimeoutChanged,
        ),
      SettingsPageId.about => const _AboutPage(),
    };
  }

  void _reset() {
    widget.onReset(AppSettings.defaults());
  }
}

class _SettingsTitleBar extends StatelessWidget {
  const _SettingsTitleBar({this.onClose});

  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      height: 52,
      padding: const EdgeInsets.symmetric(horizontal: 18),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(bottom: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          Container(
            width: 26,
            height: 26,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: palette.accent,
              borderRadius: BorderRadius.circular(9),
              boxShadow: [
                BoxShadow(
                  color: palette.accentSoft,
                  spreadRadius: 4,
                  blurRadius: 0,
                ),
              ],
            ),
            child: const Text(
              'C',
              style: TextStyle(
                color: Colors.white,
                fontWeight: FontWeight.w900,
              ),
            ),
          ),
          const SizedBox(width: 11),
          Text(
            'Settings',
            style: TextStyle(
              color: palette.text,
              fontSize: 15,
              fontWeight: FontWeight.w900,
            ),
          ),
          const Spacer(),
          IconButton(
            tooltip: 'Minimize',
            visualDensity: VisualDensity.compact,
            color: palette.muted,
            onPressed: WindowControls.minimize,
            icon: const Icon(Icons.remove_rounded, size: 18),
          ),
          IconButton(
            tooltip: 'Close',
            visualDensity: VisualDensity.compact,
            color: palette.muted,
            onPressed: onClose,
            icon: const Icon(Icons.close_rounded, size: 18),
          ),
        ],
      ),
    );
  }
}

class _SettingsNav extends StatelessWidget {
  const _SettingsNav({required this.active, required this.onChanged});

  final SettingsPageId active;
  final ValueChanged<SettingsPageId> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 210,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(right: BorderSide(color: palette.border)),
      ),
      child: Column(
        children: [
          _NavButton(
            id: SettingsPageId.appearance,
            active: active,
            icon: Icons.language_rounded,
            label: 'Appearance',
            onChanged: onChanged,
          ),
          _NavButton(
            id: SettingsPageId.connection,
            active: active,
            icon: Icons.power_settings_new_rounded,
            label: 'Connection',
            onChanged: onChanged,
          ),
          _NavButton(
            id: SettingsPageId.proxy,
            active: active,
            icon: Icons.alt_route_rounded,
            label: 'Proxy',
            onChanged: onChanged,
          ),
          _NavButton(
            id: SettingsPageId.diagnostics,
            active: active,
            icon: Icons.article_outlined,
            label: 'Diagnostics',
            onChanged: onChanged,
          ),
          _NavButton(
            id: SettingsPageId.about,
            active: active,
            icon: Icons.info_outline_rounded,
            label: 'About',
            onChanged: onChanged,
          ),
        ],
      ),
    );
  }
}

class _NavButton extends StatelessWidget {
  const _NavButton({
    required this.id,
    required this.active,
    required this.icon,
    required this.label,
    required this.onChanged,
  });

  final SettingsPageId id;
  final SettingsPageId active;
  final IconData icon;
  final String label;
  final ValueChanged<SettingsPageId> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final selected = id == active;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Material(
        color: selected ? palette.accentSoft : Colors.transparent,
        borderRadius: BorderRadius.circular(10),
        child: InkWell(
          borderRadius: BorderRadius.circular(10),
          onTap: () => onChanged(id),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
            child: Row(
              children: [
                Icon(icon,
                    size: 18, color: selected ? palette.accent : palette.muted),
                const SizedBox(width: 11),
                Text(
                  label,
                  style: TextStyle(
                    color: selected ? palette.accent : palette.muted,
                    fontSize: 13.5,
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

class _AppearancePage extends StatelessWidget {
  const _AppearancePage({
    required this.themeMode,
    required this.accent,
    required this.animationsEnabled,
    required this.animationDurationMs,
    required this.density,
    required this.startupMode,
    required this.onThemeModeChanged,
    required this.onAccentChanged,
    required this.onAnimationsEnabledChanged,
    required this.onAnimationDurationChanged,
    required this.onDensityChanged,
    required this.onStartupModeChanged,
  });

  final PohThemeMode themeMode;
  final PohAccent accent;
  final bool animationsEnabled;
  final int animationDurationMs;
  final String density;
  final String startupMode;
  final ThemeModeChangedAt onThemeModeChanged;
  final ValueChanged<PohAccent> onAccentChanged;
  final ValueChanged<bool> onAnimationsEnabledChanged;
  final ValueChanged<int> onAnimationDurationChanged;
  final ValueChanged<String> onDensityChanged;
  final ValueChanged<String> onStartupModeChanged;

  @override
  Widget build(BuildContext context) {
    return _Page(
      title: 'Appearance',
      sections: [
        _Section(
          title: 'Theme',
          children: [
            _Field(
              label: 'Color scheme',
              hint: 'Light or dark interface theme',
              control: _Segmented<PohThemeMode>(
                value: themeMode,
                values: const {
                  PohThemeMode.light: 'Light',
                  PohThemeMode.dark: 'Dark',
                },
                onChangedAt: onThemeModeChanged,
              ),
            ),
            _DividerLine(),
            _Field(
              label: 'Accent color',
              hint: 'Used for buttons, ring and selection',
              control: _AccentSwatches(
                value: accent,
                onChanged: onAccentChanged,
              ),
            ),
            _DividerLine(),
            _Field(
              label: 'Density',
              hint: 'Spacing and control sizes',
              control: _Segmented<String>(
                value: density,
                values: const {
                  'comfortable': 'Expanded',
                  'compact': 'Compact',
                },
                onChanged: onDensityChanged,
              ),
            ),
          ],
        ),
        _Section(
          title: 'Window',
          children: [
            _Field(
              label: 'Startup mode',
              hint: 'Open expanded window or compact dock',
              control: _Segmented<String>(
                value: startupMode,
                values: const {
                  'expanded': 'Expanded',
                  'compact': 'Compact',
                },
                onChanged: onStartupModeChanged,
              ),
            ),
          ],
        ),
        _Section(
          title: 'Motion',
          children: [
            _Field(
              label: 'Enable animations',
              hint: 'Smooth theme reveal and layout morphing',
              control: _Switch(
                value: animationsEnabled,
                onChanged: onAnimationsEnabledChanged,
              ),
            ),
            _DividerLine(),
            _Field(
              label: 'Animation speed',
              hint: 'Duration for window morph and color reveal',
              control: _AnimationSpeedSlider(
                value: animationDurationMs,
                enabled: animationsEnabled,
                onChanged: onAnimationDurationChanged,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _ConnectionPage extends StatelessWidget {
  const _ConnectionPage({
    required this.defaultCore,
    required this.defaultRoute,
    required this.autoConnect,
    required this.onDefaultCoreChanged,
    required this.onDefaultRouteChanged,
    required this.onAutoConnectChanged,
  });

  final String defaultCore;
  final String defaultRoute;
  final bool autoConnect;
  final ValueChanged<String> onDefaultCoreChanged;
  final ValueChanged<String> onDefaultRouteChanged;
  final ValueChanged<bool> onAutoConnectChanged;

  @override
  Widget build(BuildContext context) {
    return _Page(
      title: 'Connection',
      sections: [
        _Section(
          title: 'Defaults',
          children: [
            _Field(
              label: 'Default core',
              hint: 'Core opened on app start',
              control: _Select(
                value: defaultCore,
                values: const ['TrustTunnel', 'sing-box', 'NaiveProxy'],
                onChanged: onDefaultCoreChanged,
              ),
            ),
            _DividerLine(),
            _Field(
              label: 'Default route',
              hint: 'Traffic routing preset',
              control: _Select(
                value: defaultRoute,
                values: const ['Default', 'Local bypass', 'Selective'],
                onChanged: onDefaultRouteChanged,
              ),
            ),
          ],
        ),
        _Section(
          title: 'Behavior',
          children: [
            _Field(
              label: 'Auto-connect on startup',
              hint: 'Connect to the last server automatically',
              control: _Switch(
                value: autoConnect,
                onChanged: onAutoConnectChanged,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _ProxyPage extends StatelessWidget {
  const _ProxyPage({
    required this.socksLan,
    required this.socksAddress,
    required this.httpEnabled,
    required this.httpAddress,
    required this.onSocksLanChanged,
    required this.onSocksAddressChanged,
    required this.onHttpEnabledChanged,
    required this.onHttpAddressChanged,
  });

  final bool socksLan;
  final String socksAddress;
  final bool httpEnabled;
  final String httpAddress;
  final ValueChanged<bool> onSocksLanChanged;
  final ValueChanged<String> onSocksAddressChanged;
  final ValueChanged<bool> onHttpEnabledChanged;
  final ValueChanged<String> onHttpAddressChanged;

  @override
  Widget build(BuildContext context) {
    return _Page(
      title: 'Proxy',
      sections: [
        _Section(
          title: 'SOCKS',
          children: [
            _Field(
              label: 'Allow LAN connections',
              hint: 'Accept connections from devices in local network',
              control: _Switch(value: socksLan, onChanged: onSocksLanChanged),
            ),
            _DividerLine(),
            _Field(
              label: 'SOCKS address',
              hint: 'Local listen address and port',
              control: _Input(
                value: socksAddress,
                onChanged: onSocksAddressChanged,
              ),
            ),
          ],
        ),
        _Section(
          title: 'HTTP proxy',
          children: [
            _Field(
              label: 'Enable HTTP proxy',
              hint: 'Start local HTTP proxy when connected',
              control:
                  _Switch(value: httpEnabled, onChanged: onHttpEnabledChanged),
            ),
            _DividerLine(),
            _Field(
              label: 'HTTP address',
              hint: 'Local listen address and port',
              control: _Input(
                value: httpAddress,
                onChanged: onHttpAddressChanged,
                enabled: httpEnabled,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _DiagnosticsPage extends StatelessWidget {
  const _DiagnosticsPage({
    required this.pingHost,
    required this.httpsUrl,
    required this.timeoutSeconds,
    required this.onPingHostChanged,
    required this.onHttpsUrlChanged,
    required this.onTimeoutChanged,
  });

  final String pingHost;
  final String httpsUrl;
  final int timeoutSeconds;
  final ValueChanged<String> onPingHostChanged;
  final ValueChanged<String> onHttpsUrlChanged;
  final ValueChanged<int> onTimeoutChanged;

  @override
  Widget build(BuildContext context) {
    return _Page(
      title: 'Diagnostics',
      sections: [
        _Section(
          title: 'Connection checks',
          children: [
            _Field(
              label: 'Ping host',
              hint: 'Latency check address',
              control: _Input(value: pingHost, onChanged: onPingHostChanged),
            ),
            _DividerLine(),
            _Field(
              label: 'HTTPS test URL',
              hint: 'Internet access check URL',
              control: _Input(value: httpsUrl, onChanged: onHttpsUrlChanged),
            ),
            _DividerLine(),
            _Field(
              label: 'Diagnostics timeout',
              hint: 'Maximum response wait',
              control: _Stepper(
                value: timeoutSeconds,
                onChanged: onTimeoutChanged,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _AboutPage extends StatelessWidget {
  const _AboutPage();

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return _Page(
      title: 'About',
      sections: [
        _Section(
          title: 'Proxy Open Hub',
          children: [
            Padding(
              padding: const EdgeInsets.all(18),
              child: Row(
                children: [
                  Container(
                    width: 52,
                    height: 52,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: palette.accent,
                      borderRadius: BorderRadius.circular(15),
                    ),
                    child: const Text(
                      'C',
                      style: TextStyle(
                        color: Colors.white,
                        fontSize: 24,
                        fontWeight: FontWeight.w900,
                      ),
                    ),
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Proxy Open Hub',
                          style: TextStyle(
                            color: palette.text,
                            fontSize: 17,
                            fontWeight: FontWeight.w900,
                          ),
                        ),
                        Text(
                          'Version 0.1 - Alpha',
                          style: TextStyle(
                            color: palette.accent,
                            fontSize: 12,
                            fontWeight: FontWeight.w800,
                          ),
                        ),
                        Text(
                          'Modular desktop hub for trusted network cores',
                          style: TextStyle(color: palette.muted, fontSize: 12),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _Page extends StatelessWidget {
  const _Page({required this.title, required this.sections});

  final String title;
  final List<Widget> sections;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: TextStyle(
            color: palette.text,
            fontSize: 24,
            fontWeight: FontWeight.w900,
            letterSpacing: -0.4,
          ),
        ),
        const SizedBox(height: 22),
        ...sections,
      ],
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(left: 2, bottom: 9),
            child: Text(
              title,
              style: TextStyle(
                color: palette.muted,
                fontSize: 12,
                fontWeight: FontWeight.w900,
                letterSpacing: 0.8,
              ),
            ),
          ),
          Container(
            decoration: BoxDecoration(
              color: palette.surface,
              borderRadius: BorderRadius.circular(14),
              border: Border.all(color: palette.border),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.08),
                  blurRadius: 18,
                  offset: const Offset(0, 8),
                ),
              ],
            ),
            clipBehavior: Clip.antiAlias,
            child: Column(children: children),
          ),
        ],
      ),
    );
  }
}

class _Field extends StatelessWidget {
  const _Field({
    required this.label,
    required this.hint,
    required this.control,
  });

  final String label;
  final String hint;
  final Widget control;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 15),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: TextStyle(
                    color: palette.text,
                    fontSize: 14,
                    fontWeight: FontWeight.w900,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  hint,
                  style: TextStyle(color: palette.muted, fontSize: 12),
                ),
              ],
            ),
          ),
          const SizedBox(width: 20),
          control,
        ],
      ),
    );
  }
}

class _DividerLine extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      height: 1,
      margin: const EdgeInsets.symmetric(horizontal: 18),
      color: PohPalette.of(context).border,
    );
  }
}

class _Segmented<T> extends StatelessWidget {
  const _Segmented({
    required this.value,
    required this.values,
    this.onChanged,
    this.onChangedAt,
  }) : assert(onChanged != null || onChangedAt != null);

  final T value;
  final Map<T, String> values;
  final ValueChanged<T>? onChanged;
  final ValueChangedAt<T>? onChangedAt;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: palette.subtle,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final entry in values.entries)
            Material(
              color: entry.key == value ? palette.surface : Colors.transparent,
              borderRadius: BorderRadius.circular(8),
              child: InkWell(
                borderRadius: BorderRadius.circular(8),
                onTap: onChangedAt == null ? () => onChanged!(entry.key) : null,
                onTapDown: onChangedAt == null
                    ? null
                    : (details) {
                        if (entry.key != value) {
                          onChangedAt!(entry.key, details.globalPosition);
                        }
                      },
                child: Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 15, vertical: 7),
                  child: Text(
                    entry.value,
                    style: TextStyle(
                      color: entry.key == value ? palette.text : palette.muted,
                      fontSize: 13,
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _AccentSwatches extends StatelessWidget {
  const _AccentSwatches({required this.value, required this.onChanged});

  final PohAccent value;
  final ValueChanged<PohAccent> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final accent in PohAccent.values)
          Padding(
            padding: const EdgeInsets.only(left: 9),
            child: InkWell(
              customBorder: const CircleBorder(),
              onTap: () => onChanged(accent),
              child: Container(
                width: 28,
                height: 28,
                decoration: BoxDecoration(
                  color: accent.color,
                  shape: BoxShape.circle,
                  border: Border.all(
                    color:
                        value == accent ? palette.surface : Colors.transparent,
                    width: 3,
                  ),
                  boxShadow: [
                    BoxShadow(
                      color: value == accent
                          ? palette.accent.withValues(alpha: 0.60)
                          : palette.border,
                      spreadRadius: 1,
                      blurRadius: 0,
                    ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

class _Switch extends StatelessWidget {
  const _Switch({required this.value, required this.onChanged});

  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Switch(
      value: value,
      onChanged: onChanged,
      activeThumbColor: Colors.white,
      activeTrackColor: palette.accent,
      inactiveThumbColor: Colors.white,
      inactiveTrackColor: palette.border,
    );
  }
}

class _Select extends StatelessWidget {
  const _Select({
    required this.value,
    required this.values,
    required this.onChanged,
  });

  final String value;
  final List<String> values;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 190,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: BoxDecoration(
        color: palette.input,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: palette.border),
      ),
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String>(
          value: value,
          isExpanded: true,
          dropdownColor: palette.surface,
          style: TextStyle(color: palette.text, fontWeight: FontWeight.w700),
          items: [
            for (final item in values)
              DropdownMenuItem(value: item, child: Text(item)),
          ],
          onChanged: (value) {
            if (value != null) {
              onChanged(value);
            }
          },
        ),
      ),
    );
  }
}

class _Input extends StatelessWidget {
  const _Input({
    required this.value,
    required this.onChanged,
    this.enabled = true,
  });

  final String value;
  final ValueChanged<String> onChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return SizedBox(
      width: 260,
      child: TextFormField(
        initialValue: value,
        enabled: enabled,
        style: TextStyle(color: palette.text, fontFamily: 'monospace'),
        decoration: InputDecoration(
          isDense: true,
          filled: true,
          fillColor: palette.input,
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(10),
            borderSide: BorderSide(color: palette.border),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(10),
            borderSide: BorderSide(color: palette.accent),
          ),
        ),
        onChanged: onChanged,
      ),
    );
  }
}

class _Stepper extends StatelessWidget {
  const _Stepper({required this.value, required this.onChanged});

  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(3),
      decoration: BoxDecoration(
        color: palette.subtle,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          _StepButton(
            icon: Icons.remove_rounded,
            onPressed: () => onChanged((value - 1).clamp(1, 30)),
          ),
          SizedBox(
            width: 54,
            child: Text(
              '$value s',
              textAlign: TextAlign.center,
              style: TextStyle(
                color: palette.text,
                fontSize: 14,
                fontWeight: FontWeight.w900,
              ),
            ),
          ),
          _StepButton(
            icon: Icons.add_rounded,
            onPressed: () => onChanged((value + 1).clamp(1, 30)),
          ),
        ],
      ),
    );
  }
}

class _AnimationSpeedSlider extends StatelessWidget {
  const _AnimationSpeedSlider({
    required this.value,
    required this.enabled,
    required this.onChanged,
  });

  final int value;
  final bool enabled;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final activeColor = enabled ? palette.accent : palette.muted;
    return SizedBox(
      width: 260,
      child: Row(
        children: [
          Expanded(
            child: SliderTheme(
              data: SliderTheme.of(context).copyWith(
                activeTrackColor: activeColor,
                inactiveTrackColor: palette.border,
                thumbColor: activeColor,
                overlayColor: activeColor.withValues(alpha: 0.14),
                trackHeight: 4,
                thumbShape: const RoundSliderThumbShape(
                  enabledThumbRadius: 8,
                  disabledThumbRadius: 8,
                ),
                overlayShape: const RoundSliderOverlayShape(
                  overlayRadius: 16,
                ),
              ),
              child: Slider(
                value: value.toDouble(),
                min: 150,
                max: 1500,
                divisions: 27,
                onChanged: enabled ? (next) => onChanged(next.round()) : null,
              ),
            ),
          ),
          const SizedBox(width: 10),
          Container(
            width: 66,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
            decoration: BoxDecoration(
              color: palette.subtle,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(color: palette.border),
            ),
            alignment: Alignment.center,
            child: Text(
              '${value}ms',
              style: TextStyle(
                color: enabled ? palette.text : palette.muted,
                fontSize: 12,
                fontWeight: FontWeight.w900,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _StepButton extends StatelessWidget {
  const _StepButton({required this.icon, required this.onPressed});

  final IconData icon;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
      color: palette.surface,
      borderRadius: BorderRadius.circular(7),
      child: InkWell(
        borderRadius: BorderRadius.circular(7),
        onTap: onPressed,
        child: SizedBox(
          width: 30,
          height: 30,
          child: Icon(icon, color: palette.text, size: 18),
        ),
      ),
    );
  }
}

class _SettingsFooter extends StatelessWidget {
  const _SettingsFooter({
    required this.onReset,
    required this.onSave,
    this.onClose,
  });

  final VoidCallback onReset;
  final VoidCallback onSave;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      height: 64,
      padding: const EdgeInsets.symmetric(horizontal: 18),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          TextButton(
            onPressed: onReset,
            child: Text(
              'Reset',
              style: TextStyle(
                color: palette.muted,
                fontWeight: FontWeight.w800,
              ),
            ),
          ),
          const Spacer(),
          OutlinedButton(onPressed: onClose, child: const Text('Cancel')),
          const SizedBox(width: 10),
          FilledButton(onPressed: onSave, child: const Text('Save')),
        ],
      ),
    );
  }
}
