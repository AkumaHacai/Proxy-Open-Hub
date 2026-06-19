import 'dart:async';
import 'dart:convert';
import 'dart:io';

import '../models/app_models.dart';
import 'desktop_state_loader.dart';

class BackendSessionPlan {
  const BackendSessionPlan({
    required this.profileId,
    required this.profileName,
    required this.coreId,
    required this.commandArgs,
    required this.redactedPreview,
  });

  factory BackendSessionPlan.fromJson(Map<String, dynamic> json) {
    return BackendSessionPlan(
      profileId: json['profile_id']?.toString() ?? '',
      profileName: json['profile_name']?.toString() ?? '',
      coreId: json['core_id']?.toString() ?? '',
      commandArgs: (json['command_args'] as List? ?? const [])
          .map((value) => value.toString())
          .toList(growable: false),
      redactedPreview: json['redacted_preview']?.toString() ?? '',
    );
  }

  final String profileId;
  final String profileName;
  final String coreId;
  final List<String> commandArgs;
  final String redactedPreview;
}

class BackendSession {
  const BackendSession({
    required this.profileId,
    required this.profileName,
    required this.coreId,
    required this.pid,
    required this.executablePath,
    required this.configPath,
    required this.logPath,
    required this.runtimeDir,
    required this.commandArgs,
    required this.redactedPreview,
    required this.startedAtUnixMs,
  });

  factory BackendSession.fromJson(Map<String, dynamic> json) {
    return BackendSession(
      profileId: json['profile_id']?.toString() ?? '',
      profileName: json['profile_name']?.toString() ?? '',
      coreId: json['core_id']?.toString() ?? '',
      pid: (json['pid'] as num?)?.toInt() ?? 0,
      executablePath: json['executable_path']?.toString() ?? '',
      configPath: json['config_path']?.toString() ?? '',
      logPath: json['log_path']?.toString() ?? '',
      runtimeDir: json['runtime_dir']?.toString() ?? '',
      commandArgs: (json['command_args'] as List? ?? const [])
          .map((value) => value.toString())
          .toList(growable: false),
      redactedPreview: json['redacted_preview']?.toString() ?? '',
      startedAtUnixMs: (json['started_at_unix_ms'] as num?)?.toInt() ?? 0,
    );
  }

  final String profileId;
  final String profileName;
  final String coreId;
  final int pid;
  final String executablePath;
  final String configPath;
  final String logPath;
  final String runtimeDir;
  final List<String> commandArgs;
  final String redactedPreview;
  final int startedAtUnixMs;
}

class BackendSessionStatus {
  const BackendSessionStatus({
    required this.running,
    required this.session,
  });

  factory BackendSessionStatus.fromJson(Map<String, dynamic> json) {
    final sessionJson = json['session'];
    return BackendSessionStatus(
      running: json['running'] == true,
      session: sessionJson is Map<String, dynamic>
          ? BackendSession.fromJson(sessionJson)
          : null,
    );
  }

  final bool running;
  final BackendSession? session;
}

class BackendSessionLog {
  const BackendSessionLog({
    required this.running,
    required this.logPath,
    required this.content,
  });

  factory BackendSessionLog.fromJson(Map<String, dynamic> json) {
    return BackendSessionLog(
      running: json['running'] == true,
      logPath: json['log_path']?.toString(),
      content: json['content']?.toString() ?? '',
    );
  }

  final bool running;
  final String? logPath;
  final String content;
}

class BackendImportResult {
  const BackendImportResult({
    required this.profileId,
    required this.profileName,
    required this.coreId,
    required this.statePath,
    required this.secretsImported,
    required this.warnings,
  });

  factory BackendImportResult.fromJson(Map<String, dynamic> json) {
    return BackendImportResult(
      profileId: json['profile_id']?.toString() ?? '',
      profileName: json['profile_name']?.toString() ?? '',
      coreId: json['core_id']?.toString() ?? '',
      statePath: json['state_path']?.toString() ?? '',
      secretsImported: (json['secrets_imported'] as num?)?.toInt() ?? 0,
      warnings: (json['warnings'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendValidationWarning.fromJson)
          .toList(growable: false),
    );
  }

  final String profileId;
  final String profileName;
  final String coreId;
  final String statePath;
  final int secretsImported;
  final List<BackendValidationWarning> warnings;
}

class BackendImportPreview {
  const BackendImportPreview({
    required this.profileName,
    required this.coreId,
    required this.secretsDetected,
    required this.warnings,
  });

  factory BackendImportPreview.fromJson(Map<String, dynamic> json) {
    return BackendImportPreview(
      profileName: json['profile_name']?.toString() ?? '',
      coreId: json['core_id']?.toString() ?? '',
      secretsDetected: (json['secrets_detected'] as num?)?.toInt() ?? 0,
      warnings: (json['warnings'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendValidationWarning.fromJson)
          .toList(growable: false),
    );
  }

  final String profileName;
  final String coreId;
  final int secretsDetected;
  final List<BackendValidationWarning> warnings;
}

class BackendValidationWarning {
  const BackendValidationWarning({
    required this.field,
    required this.message,
  });

  factory BackendValidationWarning.fromJson(Map<String, dynamic> json) {
    return BackendValidationWarning(
      field: json['field']?.toString() ?? '',
      message: json['message']?.toString() ?? '',
    );
  }

  final String field;
  final String message;
}

class BackendCoreCatalog {
  const BackendCoreCatalog({
    required this.cores,
  });

  factory BackendCoreCatalog.fromJson(Map<String, dynamic> json) {
    return BackendCoreCatalog(
      cores: (json['cores'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendCoreCatalogEntry.fromJson)
          .toList(growable: false),
    );
  }

  final List<BackendCoreCatalogEntry> cores;
}

class BackendCoreCatalogEntry {
  const BackendCoreCatalogEntry({
    required this.coreId,
    required this.displayName,
    required this.sourceType,
    required this.sourceStatus,
    required this.installEnabled,
    required this.installable,
    required this.installed,
    required this.activeVersion,
    required this.installedVersions,
    required this.executableRelativePath,
    required this.homepage,
    required this.license,
    required this.notes,
  });

  factory BackendCoreCatalogEntry.fromJson(Map<String, dynamic> json) {
    return BackendCoreCatalogEntry(
      coreId: json['core_id']?.toString() ?? '',
      displayName: json['display_name']?.toString() ?? '',
      sourceType: json['source_type']?.toString() ?? '',
      sourceStatus: json['source_status']?.toString() ?? '',
      installEnabled: json['install_enabled'] == true,
      installable: json['installable'] == true,
      installed: json['installed'] == true,
      activeVersion: json['active_version']?.toString(),
      installedVersions: (json['installed_versions'] as List? ?? const [])
          .map((value) => value.toString())
          .toList(growable: false),
      executableRelativePath: json['executable_relative_path']?.toString(),
      homepage: json['homepage']?.toString(),
      license: json['license']?.toString(),
      notes: json['notes']?.toString(),
    );
  }

  final String coreId;
  final String displayName;
  final String sourceType;
  final String sourceStatus;
  final bool installEnabled;
  final bool installable;
  final bool installed;
  final String? activeVersion;
  final List<String> installedVersions;
  final String? executableRelativePath;
  final String? homepage;
  final String? license;
  final String? notes;
}

class BackendCoreSchema {
  const BackendCoreSchema({
    required this.coreId,
    required this.sections,
  });

  factory BackendCoreSchema.fromJson(Map<String, dynamic> json) {
    return BackendCoreSchema(
      coreId: json['core_id']?.toString() ?? '',
      sections: (json['sections'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendCoreSchemaSection.fromJson)
          .toList(growable: false),
    );
  }

  final String coreId;
  final List<BackendCoreSchemaSection> sections;
}

class BackendCoreSchemaSection {
  const BackendCoreSchemaSection({
    required this.key,
    required this.title,
    required this.fields,
  });

  factory BackendCoreSchemaSection.fromJson(Map<String, dynamic> json) {
    return BackendCoreSchemaSection(
      key: json['key']?.toString() ?? '',
      title: json['title']?.toString() ?? '',
      fields: (json['fields'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendCoreSchemaField.fromJson)
          .toList(growable: false),
    );
  }

  final String key;
  final String title;
  final List<BackendCoreSchemaField> fields;
}

class BackendCoreSchemaField {
  const BackendCoreSchemaField({
    required this.key,
    required this.title,
    required this.required,
    required this.kind,
    required this.description,
  });

  factory BackendCoreSchemaField.fromJson(Map<String, dynamic> json) {
    return BackendCoreSchemaField(
      key: json['key']?.toString() ?? '',
      title: json['title']?.toString() ?? '',
      required: json['required'] == true,
      kind: BackendCoreFieldKind.fromJson(json['kind']),
      description: json['description']?.toString(),
    );
  }

  final String key;
  final String title;
  final bool required;
  final BackendCoreFieldKind kind;
  final String? description;
}

class BackendCoreFieldKind {
  const BackendCoreFieldKind({
    required this.kind,
    required this.options,
    this.min,
    this.max,
  });

  factory BackendCoreFieldKind.fromJson(Object? value) {
    if (value is! Map) {
      return const BackendCoreFieldKind(kind: 'text', options: []);
    }

    return BackendCoreFieldKind(
      kind: value['kind']?.toString() ?? 'text',
      min: (value['min'] as num?)?.toInt(),
      max: (value['max'] as num?)?.toInt(),
      options: (value['options'] as List? ?? const [])
          .map((option) => option.toString())
          .toList(growable: false),
    );
  }

  final String kind;
  final int? min;
  final int? max;
  final List<String> options;
}

class BackendProfileValidationResult {
  const BackendProfileValidationResult({
    required this.ok,
    required this.error,
    required this.warnings,
  });

  factory BackendProfileValidationResult.fromJson(Map<String, dynamic> json) {
    return BackendProfileValidationResult(
      ok: json['ok'] == true,
      error: json['error']?.toString(),
      warnings: (json['warnings'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendValidationWarning.fromJson)
          .toList(growable: false),
    );
  }

  final bool ok;
  final String? error;
  final List<BackendValidationWarning> warnings;
}

class BackendProfileUpdateResult {
  const BackendProfileUpdateResult({
    required this.ok,
    required this.profileId,
  });

  factory BackendProfileUpdateResult.fromJson(Map<String, dynamic> json) {
    return BackendProfileUpdateResult(
      ok: json['ok'] == true,
      profileId: json['profile_id']?.toString() ?? '',
    );
  }

  final bool ok;
  final String profileId;
}

class BackendCoreModes {
  const BackendCoreModes({
    required this.coreId,
    required this.available,
    required this.defaultMode,
    required this.disabled,
  });

  factory BackendCoreModes.fromJson(Map<String, dynamic> json) {
    return BackendCoreModes(
      coreId: json['core_id']?.toString() ?? '',
      available: (json['available'] as List? ?? const [])
          .map((value) => value.toString())
          .toList(growable: false),
      defaultMode: json['default']?.toString() ?? 'local_proxy_gate',
      disabled: (json['disabled'] as List? ?? const [])
          .whereType<Map<String, dynamic>>()
          .map(BackendDisabledRouteMode.fromJson)
          .toList(growable: false),
    );
  }

  final String coreId;
  final List<String> available;
  final String defaultMode;
  final List<BackendDisabledRouteMode> disabled;
}

class BackendDisabledRouteMode {
  const BackendDisabledRouteMode({
    required this.mode,
    required this.reason,
  });

  factory BackendDisabledRouteMode.fromJson(Map<String, dynamic> json) {
    return BackendDisabledRouteMode(
      mode: json['mode']?.toString() ?? '',
      reason: json['reason']?.toString() ?? '',
    );
  }

  final String mode;
  final String reason;
}

/// A running supervised session.  Holds the supervisor [Process] and the
/// initial [BackendSession] data returned by the supervisor's `started` event.
///
/// Call [stop] to request a graceful shutdown.  If the app exits without
/// calling [stop], the OS pipe closure triggers the supervisor's stdin-EOF
/// handler, which stops the core automatically.
class SupervisedSession {
  SupervisedSession({required this.session, required this.process});

  final BackendSession session;
  final Process process;

  /// Sends a stop command to the supervisor and waits for it to exit.
  Future<void> stop() async {
    try {
      process.stdin.writeln('{"command":"stop"}');
      await process.stdin.flush();
    } catch (_) {
      // stdin may already be closed if the supervisor exited on its own.
    }
    try {
      await process.stdin.close();
    } catch (_) {}
    try {
      await process.exitCode.timeout(const Duration(seconds: 10));
    } on TimeoutException {
      process.kill();
    }
  }
}

class BackendSessionService {
  const BackendSessionService({
    this.stateLoader = const DesktopStateLoader(),
  });

  final DesktopStateLoader stateLoader;

  Future<BackendCoreCatalog> coreCatalog() async {
    final decoded = await _runBackendCommand(['catalog-list']);
    return BackendCoreCatalog.fromJson(decoded);
  }

  /// Download and install a core from its pinned GitHub release. Returns the
  /// install result JSON on success. May take tens of seconds while downloading.
  Future<Map<String, dynamic>> coreInstall(String coreId) async {
    return _runBackendCommand(['core-install', coreId]);
  }

  Future<BackendCoreSchema> coreSchema(String coreId) async {
    try {
      final decoded = await _runBackendCommand(['desktop-core-schema', coreId]);
      return BackendCoreSchema.fromJson(decoded);
    } on Object {
      return _mockCoreSchema(coreId);
    }
  }

  Future<BackendCoreModes> coreModes(String coreId) async {
    try {
      final decoded = await _runBackendCommand(['desktop-core-modes', coreId]);
      return BackendCoreModes.fromJson(decoded);
    } on Object {
      return _mockCoreModes(coreId);
    }
  }

  Future<BackendProfileValidationResult> validateProfile({
    required String coreId,
    required String profileId,
    required Map<String, Object?> fields,
  }) async {
    final stateFile = await stateLoader.stateFile();
    if (stateFile == null || !await stateFile.exists()) {
      return const BackendProfileValidationResult(
        ok: true,
        error: null,
        warnings: [],
      );
    }

    try {
      final decoded = await _runBackendCommandWithStdin(
        ['desktop-validate-profile', stateFile.path],
        jsonEncode({
          'core_id': coreId,
          if (profileId.isNotEmpty) 'profile_id': profileId,
          'fields': fields,
        }),
      );
      return BackendProfileValidationResult.fromJson(decoded);
    } on Object {
      return const BackendProfileValidationResult(
        ok: true,
        error: null,
        warnings: [],
      );
    }
  }

  Future<BackendProfileUpdateResult> updateProfile({
    required String coreId,
    required String profileId,
    required String displayName,
    required Map<String, Object?> fields,
  }) async {
    final stateFile = await stateLoader.stateFile();
    if (stateFile == null || !await stateFile.exists()) {
      return BackendProfileUpdateResult(
        ok: true,
        profileId: profileId.isNotEmpty ? profileId : '$coreId:draft',
      );
    }

    try {
      final decoded = await _runBackendCommandWithStdin(
        ['desktop-update-profile', stateFile.path],
        jsonEncode({
          'core_id': coreId,
          if (profileId.isNotEmpty) 'profile_id': profileId,
          'display_name': displayName,
          'fields': fields,
        }),
      );
      return BackendProfileUpdateResult.fromJson(decoded);
    } on Object {
      return BackendProfileUpdateResult(
        ok: true,
        profileId: profileId.isNotEmpty ? profileId : '$coreId:draft',
      );
    }
  }

  Future<List<ServerProfile>> listProfiles() async {
    final stateFile = await stateLoader.stateFile();
    if (stateFile == null || !await stateFile.exists()) {
      return stateLoader.loadProfiles();
    }

    try {
      final decoded =
          await _runBackendCommand(['desktop-list-profiles', stateFile.path]);
      final profilesJson = decoded['profiles'];
      if (profilesJson is! List) {
        return stateLoader.loadProfiles();
      }

      final profiles = profilesJson
          .whereType<Map<String, dynamic>>()
          .map(ServerProfile.fromCliJson)
          .where((profile) => profile.host.isNotEmpty)
          .toList(growable: false);
      if (profiles.isEmpty) {
        return stateLoader.loadProfiles();
      }

      return profiles;
    } on Object {
      return stateLoader.loadProfiles();
    }
  }

  Future<BackendSessionPlan> prepareSession(
    String coreId,
    ServerProfile server,
  ) async {
    final decoded = await _runSessionCommand(
      coreId,
      server,
      command: 'desktop-session-plan',
    );

    return BackendSessionPlan.fromJson(decoded);
  }

  Future<BackendSession> startSession(
    String coreId,
    ServerProfile server,
  ) async {
    final decoded = await _runSessionCommand(
      coreId,
      server,
      command: 'desktop-session-start',
    );

    return BackendSession.fromJson(decoded);
  }

  Future<BackendSessionStatus> stopSession() async {
    final decoded = await _runBackendCommand(['desktop-session-stop']);
    return BackendSessionStatus.fromJson(decoded);
  }

  Future<BackendSessionStatus> sessionStatus() async {
    final decoded = await _runBackendCommand(['desktop-session-status']);
    return BackendSessionStatus.fromJson(decoded);
  }

  Future<BackendSessionLog> sessionLog() async {
    final decoded = await _runBackendCommand(['desktop-session-log']);
    return BackendSessionLog.fromJson(decoded);
  }

  Future<BackendImportResult> importProfile({
    required String coreId,
    required String input,
  }) async {
    if (input.trim().isEmpty) {
      throw const BackendSessionException('Import text is empty');
    }

    _ensureImportSupported(coreId);
    final decoded = await _runBackendCommandWithStdin(
      ['desktop-import-profile', '-'],
      input,
    );

    return BackendImportResult.fromJson(decoded);
  }

  Future<BackendImportPreview> previewProfile({
    required String coreId,
    required String input,
  }) async {
    if (input.trim().isEmpty) {
      throw const BackendSessionException('Import text is empty');
    }

    _ensureImportSupported(coreId);
    final decoded = await _runBackendCommandWithStdin(
      ['desktop-preview-profile', '-'],
      input,
    );

    return BackendImportPreview.fromJson(decoded);
  }

  /// Starts the core via the long-lived supervisor process.  The supervisor
  /// starts the core, holds a Windows Job Object around it, and then monitors
  /// it for the lifetime of the returned [SupervisedSession].
  ///
  /// The caller MUST hold the returned [SupervisedSession] and call
  /// [SupervisedSession.stop] when disconnecting.  Dropping the session (or
  /// closing the Flutter app) closes the supervisor's stdin pipe, which the
  /// supervisor interprets as a stop signal.
  Future<SupervisedSession> startSupervisedSession(
    String coreId,
    ServerProfile server,
  ) async {
    if (server.id.isEmpty) {
      throw const BackendSessionException('No server profile is selected');
    }
    _ensureProfileMatchesCore(coreId, server);

    final stateFile = await stateLoader.stateFile();
    if (stateFile == null || !await stateFile.exists()) {
      throw const BackendSessionException('Saved desktop state was not found');
    }

    final cli = await _findPohCli();
    if (cli == null) {
      throw const BackendSessionException(
        'poh_cli.exe was not found. Build Rust workspace first.',
      );
    }

    final process = await Process.start(
      cli.path,
      [
        'desktop-session-supervise',
        stateFile.path,
        server.id,
        '--gui-pid',
        pid.toString(),
      ],
      runInShell: false,
    );

    // Wait for the first stdout line; it must be {"type":"started",...}.
    // If the supervisor exits before emitting it, something went wrong.
    final startedEvent = await _waitForSupervisorStart(process);
    return SupervisedSession(
      session: BackendSession.fromJson(startedEvent),
      process: process,
    );
  }

  /// Parses the supervisor's stdout until a `{"type":"started"}` event arrives
  /// or the process exits.  Throws [BackendSessionException] on failure.
  Future<Map<String, dynamic>> _waitForSupervisorStart(Process process) async {
    final stdoutLines =
        process.stdout.transform(utf8.decoder).transform(const LineSplitter());
    final stderrFuture = process.stderr.transform(utf8.decoder).join();

    await for (final line in stdoutLines) {
      if (line.trim().isEmpty) continue;
      final Map<String, dynamic> event;
      try {
        final decoded = jsonDecode(line);
        if (decoded is! Map<String, dynamic>) continue;
        event = decoded;
      } catch (_) {
        continue;
      }

      final type = event['type'] as String?;
      if (type == 'started') {
        return event;
      }
      // Ignore non-started events (e.g. unexpected early stopping/faulted).
    }

    // stdout stream ended without a "started" event — supervisor exited early.
    final exitCode = await process.exitCode;
    final stderr = await stderrFuture;
    final detail = stderr.trim();
    throw BackendSessionException(
      detail.isNotEmpty
          ? detail
          : 'Supervisor exited (code $exitCode) before core became ready',
    );
  }

  Future<Map<String, dynamic>> _runSessionCommand(
    String coreId,
    ServerProfile server, {
    required String command,
  }) async {
    if (server.id.isEmpty) {
      throw const BackendSessionException('No server profile is selected');
    }
    _ensureProfileMatchesCore(coreId, server);

    final stateFile = await stateLoader.stateFile();
    if (stateFile == null || !await stateFile.exists()) {
      throw const BackendSessionException('Saved desktop state was not found');
    }

    return _runBackendCommand([command, stateFile.path, server.id]);
  }

  void _ensureProfileMatchesCore(String coreId, ServerProfile server) {
    if (server.coreId != coreId) {
      throw BackendSessionException(
        'Selected profile belongs to ${server.coreId}, not $coreId',
      );
    }
  }

  void _ensureImportSupported(String coreId) {
    if (coreId == 'auto' || coreId == 'trusttunnel') {
      return;
    }

    throw BackendSessionException(
      'Import for $coreId is waiting for the Rust backend contract.',
    );
  }

  Future<Map<String, dynamic>> _runBackendCommand(List<String> args) async {
    final cli = await _findPohCli();
    if (cli == null) {
      throw const BackendSessionException(
        'poh_cli.exe was not found. Build Rust workspace first.',
      );
    }

    final result = await Process.run(
      cli.path,
      args,
      runInShell: false,
    );
    if (result.exitCode != 0) {
      final stderr = result.stderr.toString().trim();
      throw BackendSessionException(
        stderr.isEmpty ? 'Rust backend rejected the command' : stderr,
      );
    }

    final decoded = jsonDecode(result.stdout.toString());
    if (decoded is! Map<String, dynamic>) {
      throw const BackendSessionException('Rust backend returned invalid JSON');
    }

    return decoded;
  }

  Future<Map<String, dynamic>> _runBackendCommandWithStdin(
    List<String> args,
    String stdinText,
  ) async {
    final cli = await _findPohCli();
    if (cli == null) {
      throw const BackendSessionException(
        'poh_cli.exe was not found. Build Rust workspace first.',
      );
    }

    final process = await Process.start(
      cli.path,
      args,
      runInShell: false,
    );
    process.stdin.write(stdinText);
    await process.stdin.close();

    final stdoutFuture = process.stdout.transform(utf8.decoder).join();
    final stderrFuture = process.stderr.transform(utf8.decoder).join();
    final exitCode = await process.exitCode;
    final stdout = await stdoutFuture;
    final stderr = await stderrFuture;

    if (exitCode != 0) {
      final trimmedStderr = stderr.trim();
      throw BackendSessionException(
        trimmedStderr.isEmpty
            ? 'Rust backend rejected the command'
            : trimmedStderr,
      );
    }

    final decoded = jsonDecode(stdout);
    if (decoded is! Map<String, dynamic>) {
      throw const BackendSessionException('Rust backend returned invalid JSON');
    }

    return decoded;
  }

  Future<File?> _findPohCli() async {
    final envPath = Platform.environment['POH_CLI_PATH'];
    if (envPath != null && envPath.isNotEmpty) {
      final file = File(envPath);
      if (await file.exists()) {
        return file;
      }
    }

    final executableName = Platform.isWindows ? 'poh_cli.exe' : 'poh_cli';
    final roots = <Directory>{
      Directory.current,
      File(Platform.resolvedExecutable).parent,
    };

    for (final root in roots) {
      for (final parent in _walkParents(root)) {
        for (final candidate in [
          File('${parent.path}${Platform.pathSeparator}$executableName'),
          File('${parent.path}${Platform.pathSeparator}target'
              '${Platform.pathSeparator}debug'
              '${Platform.pathSeparator}$executableName'),
          File('${parent.path}${Platform.pathSeparator}target'
              '${Platform.pathSeparator}release'
              '${Platform.pathSeparator}$executableName'),
        ]) {
          if (await candidate.exists()) {
            return candidate;
          }
        }
      }
    }

    return null;
  }

  Iterable<Directory> _walkParents(Directory start) sync* {
    var current = start.absolute;
    while (true) {
      yield current;
      final parent = current.parent;
      if (parent.path == current.path) {
        return;
      }

      current = parent;
    }
  }
}

BackendCoreSchema _mockCoreSchema(String coreId) {
  return switch (coreId) {
    'naiveproxy' => const BackendCoreSchema(
        coreId: 'naiveproxy',
        sections: [
          BackendCoreSchemaSection(
            key: 'proxy',
            title: 'Proxy',
            fields: [
              BackendCoreSchemaField(
                key: 'proxy.scheme',
                title: 'Scheme',
                required: true,
                kind: BackendCoreFieldKind(
                  kind: 'select',
                  options: ['https', 'http'],
                ),
                description: 'Upstream proxy scheme',
              ),
              BackendCoreSchemaField(
                key: 'proxy.host',
                title: 'Server host',
                required: true,
                kind: BackendCoreFieldKind(kind: 'text', options: []),
                description: 'IP address or domain',
              ),
              BackendCoreSchemaField(
                key: 'proxy.port',
                title: 'Port',
                required: true,
                kind: BackendCoreFieldKind(
                  kind: 'integer',
                  min: 1,
                  max: 65535,
                  options: [],
                ),
                description: 'Upstream proxy port',
              ),
              BackendCoreSchemaField(
                key: 'proxy.username',
                title: 'Username',
                required: false,
                kind: BackendCoreFieldKind(kind: 'text', options: []),
                description: 'Optional authentication username',
              ),
              BackendCoreSchemaField(
                key: 'proxy.password',
                title: 'Password',
                required: false,
                kind: BackendCoreFieldKind(kind: 'secret', options: []),
                description: 'Stored through protected secrets',
              ),
            ],
          ),
          BackendCoreSchemaSection(
            key: 'listen',
            title: 'Listen',
            fields: [
              BackendCoreSchemaField(
                key: 'listen.scheme',
                title: 'Local proxy type',
                required: true,
                kind: BackendCoreFieldKind(
                  kind: 'select',
                  options: ['socks', 'http'],
                ),
                description: 'Local listener protocol',
              ),
              BackendCoreSchemaField(
                key: 'listen.host',
                title: 'Local host',
                required: true,
                kind: BackendCoreFieldKind(kind: 'text', options: []),
                description: 'Local listen address',
              ),
              BackendCoreSchemaField(
                key: 'listen.port',
                title: 'Local port',
                required: true,
                kind: BackendCoreFieldKind(
                  kind: 'integer',
                  min: 1,
                  max: 65535,
                  options: [],
                ),
                description: 'Local listen port',
              ),
            ],
          ),
          BackendCoreSchemaSection(
            key: 'advanced',
            title: 'Advanced',
            fields: [
              BackendCoreSchemaField(
                key: 'no_post_quantum',
                title: 'Disable post-quantum',
                required: false,
                kind: BackendCoreFieldKind(kind: 'boolean', options: []),
                description: 'Compatibility switch for upstream TLS',
              ),
              BackendCoreSchemaField(
                key: 'extra_headers',
                title: 'Extra headers',
                required: false,
                kind: BackendCoreFieldKind(kind: 'multiline', options: []),
                description: 'Additional headers, one per line',
              ),
            ],
          ),
        ],
      ),
    _ => const BackendCoreSchema(
        coreId: 'trusttunnel',
        sections: [
          BackendCoreSchemaSection(
            key: 'endpoint',
            title: 'Endpoint',
            fields: [
              BackendCoreSchemaField(
                key: 'hostname',
                title: 'Server host',
                required: true,
                kind: BackendCoreFieldKind(kind: 'text', options: []),
                description: 'IP address or domain',
              ),
              BackendCoreSchemaField(
                key: 'password',
                title: 'Password',
                required: true,
                kind: BackendCoreFieldKind(kind: 'secret', options: []),
                description: 'Stored through protected secrets',
              ),
              BackendCoreSchemaField(
                key: 'upstream_protocol',
                title: 'Upstream protocol',
                required: true,
                kind: BackendCoreFieldKind(
                  kind: 'select',
                  options: ['HTTP/2', 'HTTP/3'],
                ),
                description: 'Protocol used to reach the server',
              ),
              BackendCoreSchemaField(
                key: 'skip_verification',
                title: 'TLS verification disabled',
                required: false,
                kind: BackendCoreFieldKind(kind: 'boolean', options: []),
                description: 'Certificate check is off - insecure',
              ),
            ],
          ),
          BackendCoreSchemaSection(
            key: 'listener',
            title: 'Listener',
            fields: [
              BackendCoreSchemaField(
                key: 'listener_mode',
                title: 'Listener mode',
                required: true,
                kind: BackendCoreFieldKind(
                  kind: 'select',
                  options: ['TUN', 'SOCKS'],
                ),
                description: 'Local traffic capture mode',
              ),
            ],
          ),
        ],
      ),
  };
}

BackendCoreModes _mockCoreModes(String coreId) {
  if (coreId == 'trusttunnel' || coreId == 'sing-box') {
    return BackendCoreModes(
      coreId: coreId,
      available: const ['tun', 'system_proxy', 'local_proxy_gate'],
      defaultMode: 'tun',
      disabled: const [],
    );
  }

  return BackendCoreModes(
    coreId: coreId,
    available: const ['system_proxy', 'local_proxy_gate'],
    defaultMode: 'local_proxy_gate',
    disabled: const [
      BackendDisabledRouteMode(
        mode: 'tun',
        reason: 'Requires sing-box helper (not installed)',
      ),
    ],
  );
}

class BackendSessionException implements Exception {
  const BackendSessionException(this.message);

  final String message;

  @override
  String toString() => message;
}
