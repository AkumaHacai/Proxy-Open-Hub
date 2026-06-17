import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/backend_session_service.dart';
import '../theme/poh_theme.dart';

class ImportProfileShell extends StatefulWidget {
  const ImportProfileShell({
    super.key,
    required this.sessionService,
    required this.onImported,
    this.onClose,
  });

  final BackendSessionService sessionService;
  final ValueChanged<BackendImportResult> onImported;
  final VoidCallback? onClose;

  @override
  State<ImportProfileShell> createState() => _ImportProfileShellState();
}

class _ImportProfileShellState extends State<ImportProfileShell> {
  final _controller = TextEditingController();
  var _busy = false;
  String? _message;
  String? _error;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 680,
      height: 540,
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
          _ImportTitleBar(onClose: widget.onClose),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(24, 22, 24, 18),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'Import TrustTunnel profile',
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 26,
                      fontWeight: FontWeight.w900,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Paste a tt:// link or a TrustTunnel TOML config. Secrets are stored in the local state map and profiles keep only secret references.',
                    style: TextStyle(color: palette.muted, fontSize: 13),
                  ),
                  const SizedBox(height: 16),
                  Expanded(
                    child: TextField(
                      controller: _controller,
                      expands: true,
                      minLines: null,
                      maxLines: null,
                      textAlignVertical: TextAlignVertical.top,
                      style: TextStyle(
                        color: palette.text,
                        fontFamily: 'Consolas',
                        fontSize: 13,
                        height: 1.42,
                      ),
                      decoration: InputDecoration(
                        hintText: 'tt://... or [endpoint]\\nhostname = "..."\n',
                        contentPadding: const EdgeInsets.all(16),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(18),
                          borderSide: BorderSide(color: palette.border),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(18),
                          borderSide: BorderSide(color: palette.border),
                        ),
                        focusedBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(18),
                          borderSide: BorderSide(color: palette.accent),
                        ),
                      ),
                    ),
                  ),
                  if (_message != null || _error != null) ...[
                    const SizedBox(height: 12),
                    Text(
                      _error ?? _message!,
                      style: TextStyle(
                        color: _error == null
                            ? palette.accent
                            : const Color(0xFFE26060),
                        fontSize: 13,
                        fontWeight: FontWeight.w800,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ],
              ),
            ),
          ),
          _ImportFooter(
            busy: _busy,
            onPaste: _paste,
            onImport: _import,
            onClose: widget.onClose,
          ),
        ],
      ),
    );
  }

  Future<void> _paste() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text ?? '';
    if (text.trim().isEmpty) {
      setState(() {
        _message = null;
        _error = 'Clipboard does not contain text.';
      });
      return;
    }

    _controller.text = text;
    setState(() {
      _message = 'Clipboard text inserted.';
      _error = null;
    });
  }

  Future<void> _import() async {
    if (_busy) {
      return;
    }

    final text = _controller.text.trim();
    if (text.isEmpty) {
      setState(() {
        _message = null;
        _error = 'Paste a profile before importing.';
      });
      return;
    }

    setState(() {
      _busy = true;
      _message = 'Importing profile...';
      _error = null;
    });

    try {
      final result = await widget.sessionService.importTrustTunnelProfile(text);
      if (!mounted) {
        return;
      }

      setState(() {
        _message = result.warnings.isEmpty
            ? 'Imported ${result.profileName}.'
            : 'Imported ${result.profileName} with ${result.warnings.length} security warning(s).';
        _error = null;
      });
      widget.onImported(result);
    } catch (error) {
      if (!mounted) {
        return;
      }

      setState(() {
        _message = null;
        _error = error.toString();
      });
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }
}

class _ImportTitleBar extends StatelessWidget {
  const _ImportTitleBar({this.onClose});

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
              'T',
              style: TextStyle(
                color: Colors.white,
                fontWeight: FontWeight.w900,
              ),
            ),
          ),
          const SizedBox(width: 11),
          Text(
            'Add server',
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
    );
  }
}

class _ImportFooter extends StatelessWidget {
  const _ImportFooter({
    required this.busy,
    required this.onPaste,
    required this.onImport,
    this.onClose,
  });

  final bool busy;
  final VoidCallback onPaste;
  final VoidCallback onImport;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      height: 72,
      padding: const EdgeInsets.symmetric(horizontal: 24),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: Row(
        children: [
          TextButton.icon(
            onPressed: busy ? null : onPaste,
            icon: const Icon(Icons.content_paste_rounded, size: 17),
            label: const Text('Paste from clipboard'),
          ),
          const Spacer(),
          OutlinedButton(
            onPressed: busy ? null : onClose,
            child: const Text('Cancel'),
          ),
          const SizedBox(width: 10),
          FilledButton(
            onPressed: busy ? null : onImport,
            style: FilledButton.styleFrom(
              backgroundColor: palette.accent,
              foregroundColor: Colors.white,
            ),
            child: busy
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text('Import'),
          ),
        ],
      ),
    );
  }
}
