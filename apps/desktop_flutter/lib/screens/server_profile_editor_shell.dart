import 'package:flutter/material.dart';

import '../models/app_models.dart';
import '../services/backend_session_service.dart';
import '../services/window_controls.dart';
import '../theme/poh_theme.dart';

class ServerProfileEditorShell extends StatefulWidget {
  const ServerProfileEditorShell({
    super.key,
    required this.sessionService,
    required this.core,
    required this.profile,
    required this.onSaved,
    this.onClose,
  });

  final BackendSessionService sessionService;
  final CoreSpec core;
  final ServerProfile profile;
  final ValueChanged<BackendProfileUpdateResult> onSaved;
  final VoidCallback? onClose;

  @override
  State<ServerProfileEditorShell> createState() =>
      _ServerProfileEditorShellState();
}

class _ServerProfileEditorShellState extends State<ServerProfileEditorShell> {
  late final Future<BackendCoreSchema> _schemaFuture;
  late final TextEditingController _displayNameController;
  final _textControllers = <String, TextEditingController>{};
  final _boolValues = <String, bool>{};
  final _intValues = <String, int>{};
  final _selectValues = <String, String>{};
  var _initialized = false;
  var _busy = false;
  String? _message;
  String? _error;
  var _warnings = const <BackendValidationWarning>[];

  @override
  void initState() {
    super.initState();
    _schemaFuture = widget.sessionService.coreSchema(widget.core.id);
    _displayNameController = TextEditingController(text: widget.profile.name);
  }

  @override
  void dispose() {
    _displayNameController.dispose();
    for (final controller in _textControllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      width: 780,
      height: 620,
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
          _EditorTitleBar(core: widget.core, onClose: widget.onClose),
          Expanded(
            child: FutureBuilder<BackendCoreSchema>(
              future: _schemaFuture,
              builder: (context, snapshot) {
                if (snapshot.connectionState != ConnectionState.done) {
                  return const Center(child: CircularProgressIndicator());
                }
                if (snapshot.hasError) {
                  return _EditorError(message: snapshot.error.toString());
                }

                final schema = snapshot.data!;
                _initialize(schema);
                return _buildForm(schema);
              },
            ),
          ),
          _EditorFooter(
            busy: _busy,
            onSave: _save,
            onClose: widget.onClose,
          ),
        ],
      ),
    );
  }

  Widget _buildForm(BackendCoreSchema schema) {
    final palette = PohPalette.of(context);
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(24, 22, 24, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            '${widget.core.name} server settings',
            style: TextStyle(
              color: palette.text,
              fontSize: 26,
              fontWeight: FontWeight.w900,
            ),
          ),
          const SizedBox(height: 6),
          Text(
            '${widget.profile.host} - ${widget.profile.dns}',
            style: TextStyle(color: palette.muted, fontSize: 13),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          const SizedBox(height: 18),
          _DisplayNameField(controller: _displayNameController),
          const SizedBox(height: 18),
          for (final section in schema.sections) ...[
            _SchemaSection(
              section: section,
              fieldBuilder: _buildField,
            ),
            const SizedBox(height: 14),
          ],
          if (_warnings.isNotEmpty) ...[
            const SizedBox(height: 2),
            _MessageBox(
              tone: _MessageTone.warning,
              messages: [
                for (final warning in _warnings)
                  warning.field.isEmpty
                      ? warning.message
                      : '${warning.field}: ${warning.message}',
              ],
            ),
          ],
          if (_message != null) ...[
            const SizedBox(height: 10),
            _MessageBox(tone: _MessageTone.info, messages: [_message!]),
          ],
          if (_error != null) ...[
            const SizedBox(height: 10),
            _MessageBox(tone: _MessageTone.error, messages: [_error!]),
          ],
        ],
      ),
    );
  }

  Widget _buildField(BackendCoreSchemaField field) {
    final kind = field.kind.kind;
    final label = field.title.isEmpty ? field.key : field.title;
    final hint = field.description ?? '';

    return switch (kind) {
      'boolean' => _FieldRow(
          label: label,
          hint: hint,
          control: Switch(
            value: _boolValues[field.key] ?? false,
            onChanged: (value) {
              setState(() {
                _boolValues[field.key] = value;
                _clearMessagesState();
              });
            },
          ),
        ),
      'integer' => _FieldRow(
          label: label,
          hint: hint,
          control: _IntegerStepper(
            value: _intValues[field.key] ?? field.kind.min ?? 0,
            min: field.kind.min,
            max: field.kind.max,
            onChanged: (value) {
              setState(() {
                _intValues[field.key] = value;
                _clearMessagesState();
              });
            },
          ),
        ),
      'secret' => _FieldRow(
          label: label,
          hint: hint,
          control: TextField(
            controller: _textControllers[field.key],
            obscureText: true,
            onChanged: (_) => _clearMessages(),
            decoration: const InputDecoration(hintText: 'Stored secret'),
          ),
        ),
      'multiline' => _FieldRow(
          label: label,
          hint: hint,
          control: TextField(
            controller: _textControllers[field.key],
            minLines: 3,
            maxLines: 5,
            onChanged: (_) => _clearMessages(),
          ),
        ),
      'select' => field.kind.options.isEmpty
          ? _FieldRow(
              label: label,
              hint: hint,
              control: TextField(
                controller: _textControllers[field.key],
                onChanged: (_) => _clearMessages(),
              ),
            )
          : _FieldRow(
              label: label,
              hint: hint,
              control: DropdownButtonFormField<String>(
                initialValue: _selectValues[field.key],
                items: [
                  for (final option in field.kind.options)
                    DropdownMenuItem(value: option, child: Text(option)),
                ],
                onChanged: (value) {
                  if (value == null) {
                    return;
                  }
                  setState(() {
                    _selectValues[field.key] = value;
                    _clearMessagesState();
                  });
                },
              ),
            ),
      _ => _FieldRow(
          label: label,
          hint: hint,
          control: TextField(
            controller: _textControllers[field.key],
            onChanged: (_) => _clearMessages(),
          ),
        ),
    };
  }

  void _initialize(BackendCoreSchema schema) {
    if (_initialized) {
      return;
    }

    for (final section in schema.sections) {
      for (final field in section.fields) {
        switch (field.kind.kind) {
          case 'boolean':
            _boolValues[field.key] = _initialBool(field);
          case 'integer':
            _intValues[field.key] = _initialInt(field);
          case 'select':
            if (field.kind.options.isEmpty) {
              _textControllers[field.key] = TextEditingController(
                text: _initialText(field),
              );
            } else {
              _selectValues[field.key] = _initialSelect(field);
            }
          default:
            _textControllers[field.key] = TextEditingController(
              text: _initialText(field),
            );
        }
      }
    }

    _initialized = true;
  }

  String _initialText(BackendCoreSchemaField field) {
    final key = field.key.toLowerCase();
    if (key == 'hostname' || key.endsWith('.host') || key == 'host') {
      return widget.profile.host;
    }
    if (key.contains('password')) {
      return '';
    }
    return '';
  }

  bool _initialBool(BackendCoreSchemaField field) {
    final key = field.key.toLowerCase();
    if (key.contains('skip') || key.contains('tls')) {
      return widget.profile.tlsVerificationDisabled;
    }
    return false;
  }

  int _initialInt(BackendCoreSchemaField field) {
    final key = field.key.toLowerCase();
    final min = field.kind.min;
    final max = field.kind.max;
    final value = key.contains('listen') ? 1080 : 443;
    return value.clamp(min ?? value, max ?? value);
  }

  String _initialSelect(BackendCoreSchemaField field) {
    if (field.kind.options.isEmpty) {
      return '';
    }
    final key = field.key.toLowerCase();
    final dns = widget.profile.dns.toLowerCase();
    for (final option in field.kind.options) {
      if (dns.contains(option.toLowerCase())) {
        return option;
      }
    }
    if (key.contains('listen') && field.kind.options.contains('socks')) {
      return 'socks';
    }
    return field.kind.options.first;
  }

  Map<String, Object?> _collectFields(BackendCoreSchema schema) {
    final result = <String, Object?>{};
    for (final section in schema.sections) {
      for (final field in section.fields) {
        switch (field.kind.kind) {
          case 'boolean':
            result[field.key] = _boolValues[field.key] ?? false;
          case 'integer':
            result[field.key] = _intValues[field.key] ?? field.kind.min ?? 0;
          case 'select':
            result[field.key] = field.kind.options.isEmpty
                ? _textControllers[field.key]?.text ?? ''
                : _selectValues[field.key] ?? '';
          default:
            result[field.key] = _textControllers[field.key]?.text ?? '';
        }
      }
    }
    return result;
  }

  Future<void> _save() async {
    if (_busy) {
      return;
    }
    setState(() {
      _busy = true;
      _message = 'Validating profile...';
      _error = null;
      _warnings = const [];
    });

    try {
      final schema = await _schemaFuture;
      final fields = _collectFields(schema);
      final validation = await widget.sessionService.validateProfile(
        coreId: widget.core.id,
        profileId: widget.profile.id,
        fields: fields,
      );
      if (!mounted) {
        return;
      }
      if (!validation.ok) {
        setState(() {
          _busy = false;
          _message = null;
          _error = validation.error ?? 'Profile validation failed.';
          _warnings = validation.warnings;
        });
        return;
      }

      setState(() {
        _message = 'Saving profile...';
        _warnings = validation.warnings;
      });
      final result = await widget.sessionService.updateProfile(
        coreId: widget.core.id,
        profileId: widget.profile.id,
        displayName: _displayNameController.text.trim(),
        fields: fields,
      );
      if (!mounted) {
        return;
      }
      setState(() {
        _busy = false;
        _message = result.ok ? 'Profile saved.' : null;
        _error = result.ok ? null : 'Profile update failed.';
      });
      if (result.ok) {
        widget.onSaved(result);
      }
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _busy = false;
        _message = null;
        _error = error.toString();
      });
    }
  }

  void _clearMessages() {
    if (_message == null && _error == null && _warnings.isEmpty) {
      return;
    }
    setState(_clearMessagesState);
  }

  void _clearMessagesState() {
    _message = null;
    _error = null;
    _warnings = const [];
  }
}

class _EditorTitleBar extends StatelessWidget {
  const _EditorTitleBar({
    required this.core,
    this.onClose,
  });

  final CoreSpec core;
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
                color: core.accent,
                borderRadius: BorderRadius.circular(9),
                boxShadow: [
                  BoxShadow(
                    color: core.accent.withValues(alpha: 0.22),
                    spreadRadius: 4,
                    blurRadius: 0,
                  ),
                ],
              ),
              child: Text(
                core.letter,
                style: const TextStyle(
                  color: Colors.white,
                  fontWeight: FontWeight.w900,
                ),
              ),
            ),
            const SizedBox(width: 11),
            Text(
              'Server settings',
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

class _SchemaSection extends StatelessWidget {
  const _SchemaSection({
    required this.section,
    required this.fieldBuilder,
  });

  final BackendCoreSchemaSection section;
  final Widget Function(BackendCoreSchemaField field) fieldBuilder;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: palette.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            section.title.isEmpty ? section.key : section.title,
            style: TextStyle(
              color: palette.text,
              fontSize: 14,
              fontWeight: FontWeight.w900,
            ),
          ),
          const SizedBox(height: 14),
          for (final field in section.fields) ...[
            fieldBuilder(field),
            if (field != section.fields.last)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 12),
                child: Divider(color: palette.border, height: 1),
              ),
          ],
        ],
      ),
    );
  }
}

class _DisplayNameField extends StatelessWidget {
  const _DisplayNameField({required this.controller});

  final TextEditingController controller;

  @override
  Widget build(BuildContext context) {
    return _FieldRow(
      label: 'Display name',
      hint: 'Shown name for this server',
      control: TextField(controller: controller),
    );
  }
}

class _FieldRow extends StatelessWidget {
  const _FieldRow({
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
    return LayoutBuilder(
      builder: (context, constraints) {
        final compact = constraints.maxWidth < 560;
        final labelBlock = Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              label,
              style: TextStyle(
                color: palette.text,
                fontSize: 13,
                fontWeight: FontWeight.w900,
              ),
            ),
            if (hint.trim().isNotEmpty) ...[
              const SizedBox(height: 3),
              Text(
                hint,
                style: TextStyle(
                  color: palette.muted,
                  fontSize: 12,
                  height: 1.25,
                ),
              ),
            ],
          ],
        );

        if (compact) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              labelBlock,
              const SizedBox(height: 8),
              control,
            ],
          );
        }

        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            SizedBox(width: 230, child: labelBlock),
            const SizedBox(width: 18),
            Expanded(child: control),
          ],
        );
      },
    );
  }
}

class _IntegerStepper extends StatelessWidget {
  const _IntegerStepper({
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
  });

  final int value;
  final int? min;
  final int? max;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final canDec = min == null || value > min!;
    final canInc = max == null || value < max!;
    return Container(
      height: 44,
      decoration: BoxDecoration(
        color: palette.input,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: palette.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            tooltip: 'Decrease',
            onPressed: canDec ? () => onChanged(value - 1) : null,
            icon: const Icon(Icons.remove_rounded, size: 18),
          ),
          Expanded(
            child: Text(
              '$value',
              textAlign: TextAlign.center,
              style: TextStyle(
                color: palette.text,
                fontSize: 14,
                fontWeight: FontWeight.w900,
              ),
            ),
          ),
          IconButton(
            tooltip: 'Increase',
            onPressed: canInc ? () => onChanged(value + 1) : null,
            icon: const Icon(Icons.add_rounded, size: 18),
          ),
        ],
      ),
    );
  }
}

enum _MessageTone { info, warning, error }

class _MessageBox extends StatelessWidget {
  const _MessageBox({
    required this.tone,
    required this.messages,
  });

  final _MessageTone tone;
  final List<String> messages;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final color = switch (tone) {
      _MessageTone.info => palette.accent,
      _MessageTone.warning => const Color(0xFFC4A343),
      _MessageTone.error => const Color(0xFFE26060),
    };
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withValues(alpha: 0.32)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final message in messages)
            Text(
              message,
              style: TextStyle(
                color: palette.text,
                fontSize: 12,
                fontWeight: FontWeight.w700,
              ),
            ),
        ],
      ),
    );
  }
}

class _EditorFooter extends StatelessWidget {
  const _EditorFooter({
    required this.busy,
    required this.onSave,
    this.onClose,
  });

  final bool busy;
  final VoidCallback onSave;
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
          const Spacer(),
          OutlinedButton(
            onPressed: busy ? null : onClose,
            child: const Text('Cancel'),
          ),
          const SizedBox(width: 10),
          FilledButton(
            onPressed: busy ? null : onSave,
            child: busy
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text('Save'),
          ),
        ],
      ),
    );
  }
}

class _EditorError extends StatelessWidget {
  const _EditorError({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          message,
          textAlign: TextAlign.center,
          style: const TextStyle(
            color: Color(0xFFE26060),
            fontSize: 13,
            fontWeight: FontWeight.w800,
            height: 1.35,
          ),
        ),
      ),
    );
  }
}
