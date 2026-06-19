import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../models/app_models.dart';
import '../services/backend_session_service.dart';
import '../services/window_controls.dart';
import '../theme/poh_theme.dart';

class ImportProfileShell extends StatefulWidget {
  const ImportProfileShell({
    super.key,
    required this.sessionService,
    required this.cores,
    required this.activeCoreId,
    required this.onImported,
    this.onClose,
  });

  final BackendSessionService sessionService;
  final List<CoreSpec> cores;
  final String activeCoreId;
  final ValueChanged<BackendImportResult> onImported;
  final VoidCallback? onClose;

  @override
  State<ImportProfileShell> createState() => _ImportProfileShellState();
}

class _ImportProfileShellState extends State<ImportProfileShell> {
  final _controller = TextEditingController();
  var _busy = false;
  late var _selectedCoreId = widget.activeCoreId;
  var _coreChangedByUser = false;
  String? _message;
  String? _error;

  @override
  void didUpdateWidget(covariant ImportProfileShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_coreChangedByUser && oldWidget.activeCoreId != widget.activeCoreId) {
      _selectedCoreId = widget.activeCoreId;
    }
  }

  CoreSpec? get _selectedCore {
    if (_selectedCoreId == 'auto') {
      return null;
    }

    for (final core in widget.cores) {
      if (core.id == _selectedCoreId) {
        return core;
      }
    }

    return null;
  }

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
          _ImportTitleBar(
            selectedCore: _selectedCore,
            autoDetect: _selectedCoreId == 'auto',
            onClose: widget.onClose,
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(24, 22, 24, 18),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    _selectedCore == null
                        ? 'Import server profile'
                        : 'Import ${_selectedCore!.name} profile',
                    style: TextStyle(
                      color: palette.text,
                      fontSize: 26,
                      fontWeight: FontWeight.w900,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    'Choose a target core, then paste a profile. Secrets are protected by the Rust backend and risky fields are shown before save.',
                    style: TextStyle(color: palette.muted, fontSize: 13),
                  ),
                  const SizedBox(height: 16),
                  _CoreImportPicker(
                    cores: widget.cores,
                    selectedCoreId: _selectedCoreId,
                    onChanged: (value) {
                      setState(() {
                        _selectedCoreId = value;
                        _coreChangedByUser = true;
                        _message = null;
                        _error = null;
                      });
                    },
                  ),
                  const SizedBox(height: 14),
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
      _message = 'Checking profile...';
      _error = null;
    });

    try {
      final preview = await widget.sessionService.previewProfile(
        coreId: _selectedCoreId,
        input: text,
      );
      if (!mounted) {
        return;
      }

      if (preview.warnings.isNotEmpty) {
        final confirmed = await _confirmWarnings(preview);
        if (!confirmed) {
          if (mounted) {
            setState(() {
              _message = 'Import cancelled before saving.';
              _error = null;
            });
          }
          return;
        }
      }

      if (mounted) {
        setState(() => _message = 'Importing profile...');
      }

      final result = await widget.sessionService.importProfile(
        coreId: _selectedCoreId,
        input: text,
      );
      if (!mounted) {
        return;
      }

      setState(() {
        _message = result.warnings.isEmpty
            ? 'Imported ${result.profileName}.'
            : 'Imported ${result.profileName}; risky settings were confirmed.';
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

  Future<bool> _confirmWarnings(BackendImportPreview preview) async {
    final palette = PohPalette.of(context);
    final result = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) {
        return Dialog(
          backgroundColor: Colors.transparent,
          child: Container(
            width: 480,
            padding: const EdgeInsets.all(22),
            decoration: BoxDecoration(
              color: palette.surface,
              borderRadius: BorderRadius.circular(22),
              border: Border.all(color: palette.border),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withValues(alpha: 0.24),
                  blurRadius: 34,
                  offset: const Offset(0, 16),
                ),
              ],
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Container(
                      width: 36,
                      height: 36,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: const Color(0xFFE26060).withValues(alpha: 0.14),
                        borderRadius: BorderRadius.circular(14),
                      ),
                      child: const Icon(
                        Icons.warning_rounded,
                        color: Color(0xFFE26060),
                        size: 21,
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        'Security warning',
                        style: TextStyle(
                          color: palette.text,
                          fontSize: 20,
                          fontWeight: FontWeight.w900,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Text(
                  'Profile "${preview.profileName}" contains risky settings. Review them before saving.',
                  style: TextStyle(
                    color: palette.muted,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 14),
                Container(
                  constraints: const BoxConstraints(maxHeight: 190),
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: palette.background,
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: palette.border),
                  ),
                  child: ListView.separated(
                    shrinkWrap: true,
                    itemCount: preview.warnings.length,
                    separatorBuilder: (_, __) => Divider(color: palette.border),
                    itemBuilder: (context, index) {
                      final warning = preview.warnings[index];
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            warning.field,
                            style: TextStyle(
                              color: palette.text,
                              fontSize: 12,
                              fontWeight: FontWeight.w900,
                            ),
                          ),
                          const SizedBox(height: 3),
                          Text(
                            warning.message,
                            style: TextStyle(
                              color: palette.muted,
                              fontSize: 12,
                              height: 1.3,
                            ),
                          ),
                        ],
                      );
                    },
                  ),
                ),
                const SizedBox(height: 18),
                Row(
                  children: [
                    const Spacer(),
                    OutlinedButton(
                      onPressed: () => Navigator.of(context).pop(false),
                      child: const Text('Cancel'),
                    ),
                    const SizedBox(width: 10),
                    FilledButton(
                      style: FilledButton.styleFrom(
                        backgroundColor: const Color(0xFFE26060),
                        foregroundColor: Colors.white,
                      ),
                      onPressed: () => Navigator.of(context).pop(true),
                      child: const Text('Save anyway'),
                    ),
                  ],
                ),
              ],
            ),
          ),
        );
      },
    );

    return result == true;
  }
}

class _CoreImportPicker extends StatelessWidget {
  const _CoreImportPicker({
    required this.cores,
    required this.selectedCoreId,
    required this.onChanged,
  });

  final List<CoreSpec> cores;
  final String selectedCoreId;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        _CoreImportChip(
          label: 'Auto',
          color: palette.accent,
          selected: selectedCoreId == 'auto',
          onPressed: () => onChanged('auto'),
        ),
        for (final core in cores)
          _CoreImportChip(
            label: core.name,
            color: core.accent,
            selected: selectedCoreId == core.id,
            onPressed: () => onChanged(core.id),
          ),
      ],
    );
  }
}

class _CoreImportChip extends StatelessWidget {
  const _CoreImportChip({
    required this.label,
    required this.color,
    required this.selected,
    required this.onPressed,
  });

  final String label;
  final Color color;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
      color: selected ? color.withValues(alpha: 0.14) : palette.surface,
      borderRadius: BorderRadius.circular(999),
      child: InkWell(
        borderRadius: BorderRadius.circular(999),
        onTap: onPressed,
        child: Container(
          height: 32,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          alignment: Alignment.center,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(999),
            border: Border.all(
              color: selected ? color.withValues(alpha: 0.55) : palette.border,
            ),
          ),
          child: Text(
            label,
            style: TextStyle(
              color: selected ? palette.text : palette.muted,
              fontSize: 12,
              fontWeight: FontWeight.w800,
            ),
          ),
        ),
      ),
    );
  }
}

class _ImportTitleBar extends StatelessWidget {
  const _ImportTitleBar({
    required this.selectedCore,
    required this.autoDetect,
    this.onClose,
  });

  final CoreSpec? selectedCore;
  final bool autoDetect;

  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final core = selectedCore;
    final markerColor = core?.accent ?? palette.accent;
    final markerLetter = autoDetect ? 'A' : core?.letter ?? '?';
    final title =
        autoDetect || core == null ? 'Add server' : 'Add ${core.name} server';
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
                color: markerColor,
                borderRadius: BorderRadius.circular(9),
                boxShadow: [
                  BoxShadow(
                    color: markerColor.withValues(alpha: 0.22),
                    spreadRadius: 4,
                    blurRadius: 0,
                  ),
                ],
              ),
              child: Text(
                markerLetter,
                style: const TextStyle(
                  color: Colors.white,
                  fontWeight: FontWeight.w900,
                ),
              ),
            ),
            const SizedBox(width: 11),
            Text(
              title,
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
