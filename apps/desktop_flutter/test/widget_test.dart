import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';

import 'package:proxy_open_hub/main.dart';

void main() {
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
}
