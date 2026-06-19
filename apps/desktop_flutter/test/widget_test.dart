import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:proxy_open_hub/main.dart';
import 'package:proxy_open_hub/models/app_models.dart';
import 'package:proxy_open_hub/screens/routes_shell.dart';
import 'package:proxy_open_hub/screens/settings_shell.dart';
import 'package:proxy_open_hub/services/app_settings_store.dart';
import 'package:proxy_open_hub/services/backend_session_service.dart';
import 'package:proxy_open_hub/theme/poh_theme.dart';

Widget _settingsHarness() {
  return MaterialApp(
    debugShowCheckedModeBanner: false,
    theme: buildPohTheme(mode: PohThemeMode.dark, accent: PohAccent.forest),
    home: Scaffold(
      // Center() mirrors the real overlay host, which caps the shell at the
      // window size - the condition that used to crush it to one letter per line.
      body: Center(
        child: SettingsShell(
          themeMode: PohThemeMode.dark,
          accent: PohAccent.forest,
          accentFollowsCore: true,
          animationsEnabled: true,
          animationDurationMs: 320,
          density: 'comfortable',
          startupMode: 'expanded',
          defaultCore: 'TrustTunnel',
          autoConnect: false,
          socksLan: false,
          socksAddress: '127.0.0.1:1080',
          httpEnabled: true,
          httpAddress: '127.0.0.1:8080',
          pingHost: '8.8.8.8',
          httpsUrl: 'https://example.com/generate_204',
          timeoutSeconds: 5,
          onThemeModeChanged: (_, __) {},
          onAccentChanged: (_) {},
          onAccentFollowsCoreChanged: (_) {},
          onAnimationsEnabledChanged: (_) {},
          onAnimationDurationChanged: (_) {},
          onDensityChanged: (_) {},
          onStartupModeChanged: (_) {},
          onDefaultCoreChanged: (_) {},
          onAutoConnectChanged: (_) {},
          onSocksLanChanged: (_) {},
          onSocksAddressChanged: (_) {},
          onHttpEnabledChanged: (_) {},
          onHttpAddressChanged: (_) {},
          onPingHostChanged: (_) {},
          onHttpsUrlChanged: (_) {},
          onTimeoutChanged: (_) {},
          onReset: (_) {},
          onSave: () {},
        ),
      ),
    ),
  );
}

Widget _routesHarness({RoutesChanged? onRoutesChanged}) {
  return MaterialApp(
    debugShowCheckedModeBanner: false,
    theme: buildPohTheme(mode: PohThemeMode.dark, accent: PohAccent.forest),
    home: Scaffold(
      body: Center(
        child: RoutesShell(
          activeCoreId: 'trusttunnel',
          sessionService: const BackendSessionService(),
          cores: coreSpecs,
          routesByCore: const {'trusttunnel': 'proxy_all'},
          routeRulesByCore: const {'trusttunnel': RouteRules.empty},
          activeModeByCore: const {'trusttunnel': 'tun'},
          routePresetsByCore: const {
            'trusttunnel': RoutePreset.defaults,
          },
          onRoutesChanged: onRoutesChanged ?? (_, __, ___, ____) {},
        ),
      ),
    ),
  );
}

void main() {
  test('AppSettings defaults follow core accents and preserves legacy override',
      () {
    expect(AppSettings.defaults().accentFollowsCore, isTrue);
    expect(AppSettings.fromJson(const {}).accentFollowsCore, isTrue);
    expect(
      AppSettings.fromJson(const {'accent': 'ocean'}).accentFollowsCore,
      isFalse,
    );
    expect(
      AppSettings.fromJson(
        const {'accent': 'ocean', 'accentFollowsCore': true},
      ).accentFollowsCore,
      isTrue,
    );
  });

  test('Poh theme accepts active core accent colors', () {
    final theme = buildPohTheme(
      mode: PohThemeMode.dark,
      accentColor: findCore('naiveproxy').accent,
    );
    final palette = theme.extension<PohPalette>()!;

    expect(palette.accent, findCore('naiveproxy').accent);
  });

  test('ServerProfile keeps core id from contract and legacy state', () {
    final cliProfile = ServerProfile.fromCliJson(const {
      'id': 'naiveproxy:demo',
      'name': 'np.demo',
      'core_id': 'naiveproxy',
      'host': 'np.example.test',
      'summary': 'H2 CONNECT',
    });
    final legacyProfile = ServerProfile.fromDesktopStateJson(const {
      'Id': 'legacy-tt',
      'DisplayName': 'tt.demo',
      'Endpoint': {'Hostname': 'tt.example.test'},
    });

    expect(cliProfile.coreId, 'naiveproxy');
    expect(cliProfile.dns, 'H2 CONNECT');
    expect(legacyProfile.coreId, 'trusttunnel');
  });

  test('Backend core schema parses field kinds from contract JSON', () {
    final schema = BackendCoreSchema.fromJson(const {
      'core_id': 'naiveproxy',
      'sections': [
        {
          'key': 'proxy',
          'title': 'Proxy',
          'fields': [
            {
              'key': 'proxy.port',
              'title': 'Port',
              'required': true,
              'kind': {'kind': 'integer', 'min': 1, 'max': 65535},
            },
            {
              'key': 'proxy.scheme',
              'title': 'Scheme',
              'required': true,
              'kind': {
                'kind': 'select',
                'options': ['https', 'http'],
              },
            },
          ],
        },
      ],
    });

    expect(schema.coreId, 'naiveproxy');
    expect(schema.sections.single.fields.first.kind.kind, 'integer');
    expect(schema.sections.single.fields.first.kind.max, 65535);
    expect(schema.sections.single.fields.last.kind.options, contains('https'));
  });

  testWidgets('Proxy Open Hub shell renders main workflow', (tester) async {
    tester.view.physicalSize = const Size(1100, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(const ProxyOpenHubApp());
    await tester.pumpAndSettle();

    expect(find.text('SERVERS'), findsOneWidget);
    expect(find.text('CONNECT'), findsOneWidget);
  });

  testWidgets('Main shell does not overflow at an intermediate resize width',
      (tester) async {
    // A width between compact (360) and expanded (960): the window passes
    // through sizes like this during the resize animation. The OverflowBox keeps
    // the content laid out at its final size, so nothing reflows or overflows.
    tester.view.physicalSize = const Size(600, 655);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(const ProxyOpenHubApp());
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
  });

  testWidgets('Settings shell stays laid out at compact width', (tester) async {
    // The compact window size from _toggleCompact(): a fixed 760-wide settings
    // panel used to be clamped to this and wrap text one letter per line.
    tester.view.physicalSize = const Size(360, 650);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(_settingsHarness());
    await tester.pumpAndSettle();

    // No RenderFlex overflow was thrown during layout, and the previously broken
    // labels render.
    expect(tester.takeException(), isNull);
    expect(find.text('Appearance'), findsWidgets);
    expect(find.text('Color scheme'), findsOneWidget);
  });

  testWidgets('Settings shell keeps routing out of global settings',
      (tester) async {
    tester.view.physicalSize = const Size(900, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(_settingsHarness());
    await tester.pumpAndSettle();

    expect(find.text('Connection'), findsOneWidget);
    expect(find.text('Routing'), findsNothing);
    expect(find.text('Default route'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Routes shell renders core tabs and TrustTunnel presets',
      (tester) async {
    tester.view.physicalSize = const Size(900, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(_routesHarness());
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.text('Route profiles'), findsOneWidget);
    expect(find.text('TrustTunnel'), findsWidgets);
    expect(find.text('Bypass RU'), findsOneWidget);
    expect(find.text('Custom rules'), findsOneWidget);
    expect(find.text('Applications'), findsOneWidget);
  });

  testWidgets('Routes shell wraps core tabs at compact width', (tester) async {
    tester.view.physicalSize = const Size(360, 650);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(_routesHarness());
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(find.text('Hysteria2'), findsOneWidget);
    expect(find.text('Applications'), findsOneWidget);
  });

  testWidgets('Routes shell saves custom application rules', (tester) async {
    tester.view.physicalSize = const Size(900, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    Map<String, RouteRules>? savedRules;

    await tester.pumpWidget(
      _routesHarness(
        onRoutesChanged: (_, rules, __, ___) {
          savedRules = rules;
        },
      ),
    );
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byType(TextFormField).first,
      'chrome.exe\ntelegram.exe',
    );
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
    expect(savedRules, isNotNull);
    expect(savedRules!['trusttunnel']!.appExeNames, contains('chrome.exe'));
    expect(savedRules!['trusttunnel']!.appExeNames, contains('telegram.exe'));
  });
}
