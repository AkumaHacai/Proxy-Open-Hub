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
      final state = json as Map<String, dynamic>;
      final legacyProfiles = state['Profiles'];
      final contractProfiles = state['profiles'];
      final profiles = <ServerProfile>[
        if (legacyProfiles is List)
          for (final profile in legacyProfiles.whereType<Map>())
            ServerProfile.fromDesktopStateJson(profile.cast<String, dynamic>()),
        if (contractProfiles is List)
          for (final profile in contractProfiles.whereType<Map>())
            ServerProfile.fromCliJson(profile.cast<String, dynamic>()),
      ];
      if (profiles.isEmpty) {
        return const [];
      }

      return profiles
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
