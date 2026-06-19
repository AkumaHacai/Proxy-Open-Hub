import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/backend_session_service.dart';
import '../services/window_controls.dart';
import '../theme/poh_theme.dart';

class LogsShell extends StatefulWidget {
  const LogsShell({
    super.key,
    required this.sessionService,
    this.onClose,
  });

  final BackendSessionService sessionService;
  final VoidCallback? onClose;

  @override
  State<LogsShell> createState() => _LogsShellState();
}

class _LogsShellState extends State<LogsShell> {
  late Future<BackendSessionLog> _future;

  @override
  void initState() {
    super.initState();
    _future = widget.sessionService.sessionLog();
  }

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 760,
      height: 560,
      clipBehavior: Clip.antiAlias,
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
      child: Column(
        children: [
          _LogsTitleBar(onClose: widget.onClose),
          Expanded(
            child: FutureBuilder<BackendSessionLog>(
              future: _future,
              builder: (context, snapshot) {
                if (snapshot.connectionState != ConnectionState.done) {
                  return const Center(child: CircularProgressIndicator());
                }

                final log = snapshot.data;
                final error = snapshot.error;
                final content = error != null
                    ? error.toString()
                    : log?.content.trim().isNotEmpty == true
                        ? log!.content
                        : 'No log output yet.';
                final subtitle = error == null
                    ? _subtitle(log)
                    : 'Unable to read backend log.';
                return Padding(
                  padding: const EdgeInsets.fromLTRB(24, 22, 24, 18),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Logs',
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 26,
                          fontWeight: FontWeight.w900,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Text(
                        subtitle,
                        style: TextStyle(color: palette.muted, fontSize: 13),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: 16),
                      Expanded(
                        child: Container(
                          width: double.infinity,
                          padding: const EdgeInsets.all(18),
                          decoration: BoxDecoration(
                            color: palette.input,
                            borderRadius: BorderRadius.circular(18),
                            border: Border.all(color: palette.border),
                          ),
                          child: SingleChildScrollView(
                            child: SelectableText(
                              content,
                              style: TextStyle(
                                color: palette.text,
                                fontSize: 12.5,
                                height: 1.45,
                                fontFamily: 'Consolas',
                              ),
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
          _LogsFooter(
            onRefresh: () {
              setState(() {
                _future = widget.sessionService.sessionLog();
              });
            },
            onCopy: () async {
              final log = await _future;
              await Clipboard.setData(ClipboardData(text: log.content));
            },
            onClose: widget.onClose,
          ),
        ],
      ),
    );
  }

  static String _subtitle(BackendSessionLog? log) {
    if (log == null) {
      return 'Reading core runtime log.';
    }

    final path = log.logPath;
    if (path == null || path.isEmpty) {
      return log.running ? 'Core is running.' : 'No active core session.';
    }

    return '${log.running ? "Active" : "Stopped"} session log: $path';
  }
}

class _LogsTitleBar extends StatelessWidget {
  const _LogsTitleBar({this.onClose});

  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onPanStart: (_) => WindowControls.startDrag(),
      child: Container(
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
                'T',
                style: TextStyle(
                  color: Colors.white,
                  fontWeight: FontWeight.w900,
                ),
              ),
            ),
            const SizedBox(width: 11),
            Text(
              'Core logs',
              style: TextStyle(
                color: palette.text,
                fontSize: 15,
                fontWeight: FontWeight.w900,
              ),
            ),
            const Spacer(),
            IconButton(
              tooltip: 'Close',
              visualDensity: VisualDensity.compact,
              color: palette.muted,
              onPressed: onClose,
              icon: const Icon(Icons.close_rounded, size: 18),
            ),
          ],
        ),
      ),
    );
  }
}

class _LogsFooter extends StatelessWidget {
  const _LogsFooter({
    required this.onRefresh,
    required this.onCopy,
    this.onClose,
  });

  final VoidCallback onRefresh;
  final VoidCallback onCopy;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final ghostStyle = TextButton.styleFrom(foregroundColor: palette.accent);
    return Container(
      height: 72,
      padding: const EdgeInsets.symmetric(horizontal: 24),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          TextButton(
            onPressed: onRefresh,
            style: ghostStyle,
            child: const Text('Refresh'),
          ),
          const Spacer(),
          TextButton(
            onPressed: onCopy,
            style: ghostStyle,
            child: const Text('Copy'),
          ),
          const SizedBox(width: 10),
          FilledButton(
            onPressed: onClose,
            style: FilledButton.styleFrom(
              backgroundColor: palette.accent,
              foregroundColor: Colors.white,
            ),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }
}
