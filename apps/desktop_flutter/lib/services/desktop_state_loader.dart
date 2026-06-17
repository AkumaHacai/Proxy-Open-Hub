import 'dart:convert';
import 'dart:io';

import '../models/app_models.dart';

class DesktopStateLoader {
  const DesktopStateLoader();

  Future<List<ServerProfile>> loadProfiles() async {
    final file = await stateFile();
    if (file == null || !await file.exists()) {
      return const [];
    }

    try {
      final json = jsonDecode(await file.readAsString());
      final profiles = (json as Map<String, dynamic>)['Profiles'];
      if (profiles is! List) {
        return const [];
      }

      return profiles
          .whereType<Map>()
          .map((profile) => ServerProfile.fromDesktopStateJson(
                profile.cast<String, dynamic>(),
              ))
          .where((profile) => profile.host.isNotEmpty)
          .toList(growable: false);
    } on FormatException {
      return const [];
    } on FileSystemException {
      return const [];
    }
  }

  Future<File?> stateFile() async {
    final localAppData = Platform.environment['LOCALAPPDATA'];
    if (localAppData == null || localAppData.isEmpty) {
      return null;
    }

    final primary = File('$localAppData${Platform.pathSeparator}ProxyOpenHub'
        '${Platform.pathSeparator}desktop-state.json');
    if (await primary.exists()) {
      return primary;
    }

    return File('$localAppData${Platform.pathSeparator}TrustTunnel'
        '${Platform.pathSeparator}desktop-state.json');
  }
}
