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

class BackendSessionService {
  const BackendSessionService({
    this.stateLoader = const DesktopStateLoader(),
  });

  final DesktopStateLoader stateLoader;

  Future<BackendSessionPlan> prepareTrustTunnelSession(
    ServerProfile server,
  ) async {
    if (server.id.isEmpty) {
      throw const BackendSessionException('No server profile is selected');
    }

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

    final result = await Process.run(
      cli.path,
      ['desktop-session-plan', stateFile.path, server.id],
      runInShell: false,
    );
    if (result.exitCode != 0) {
      final stderr = result.stderr.toString().trim();
      throw BackendSessionException(
        stderr.isEmpty ? 'Rust backend rejected the session plan' : stderr,
      );
    }

    final decoded = jsonDecode(result.stdout.toString());
    if (decoded is! Map<String, dynamic>) {
      throw const BackendSessionException('Rust backend returned invalid JSON');
    }

    return BackendSessionPlan.fromJson(decoded);
  }

  Future<BackendSession> startTrustTunnelSession(ServerProfile server) async {
    final decoded = await _runSessionCommand(
      server,
      command: 'desktop-session-start',
    );

    return BackendSession.fromJson(decoded);
  }

  Future<BackendSessionStatus> stopTrustTunnelSession() async {
    final decoded = await _runBackendCommand(['desktop-session-stop']);
    return BackendSessionStatus.fromJson(decoded);
  }

  Future<BackendSessionStatus> trustTunnelSessionStatus() async {
    final decoded = await _runBackendCommand(['desktop-session-status']);
    return BackendSessionStatus.fromJson(decoded);
  }

  Future<BackendSessionLog> trustTunnelSessionLog() async {
    final decoded = await _runBackendCommand(['desktop-session-log']);
    return BackendSessionLog.fromJson(decoded);
  }

  Future<BackendImportResult> importTrustTunnelProfile(String input) async {
    if (input.trim().isEmpty) {
      throw const BackendSessionException('Import text is empty');
    }

    final decoded = await _runBackendCommandWithStdin(
      ['desktop-import-profile', '-'],
      input,
    );

    return BackendImportResult.fromJson(decoded);
  }

  Future<BackendImportPreview> previewTrustTunnelProfile(String input) async {
    if (input.trim().isEmpty) {
      throw const BackendSessionException('Import text is empty');
    }

    final decoded = await _runBackendCommandWithStdin(
      ['desktop-preview-profile', '-'],
      input,
    );

    return BackendImportPreview.fromJson(decoded);
  }

  Future<Map<String, dynamic>> _runSessionCommand(
    ServerProfile server, {
    required String command,
  }) async {
    if (server.id.isEmpty) {
      throw const BackendSessionException('No server profile is selected');
    }

    final stateFile = await stateLoader.stateFile();
    if (stateFile == null || !await stateFile.exists()) {
      throw const BackendSessionException('Saved desktop state was not found');
    }

    return _runBackendCommand([command, stateFile.path, server.id]);
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

class BackendSessionException implements Exception {
  const BackendSessionException(this.message);

  final String message;

  @override
  String toString() => message;
}
