import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../models/app_models.dart';
import '../services/app_settings_store.dart';
import '../services/backend_session_service.dart';
import '../services/window_controls.dart';
import '../theme/poh_theme.dart';

typedef RoutesChanged = void Function(
  Map<String, String> routesByCore,
  Map<String, RouteRules> routeRulesByCore,
  Map<String, String> activeModeByCore,
  Map<String, List<RoutePreset>> routePresetsByCore,
);

class RoutesShell extends StatefulWidget {
  const RoutesShell({
    super.key,
    required this.activeCoreId,
    required this.sessionService,
    required this.cores,
    required this.routesByCore,
    required this.routeRulesByCore,
    required this.activeModeByCore,
    required this.routePresetsByCore,
    required this.onRoutesChanged,
    this.onClose,
  });

  final String activeCoreId;
  final BackendSessionService sessionService;
  final List<CoreSpec> cores;
  final Map<String, String> routesByCore;
  final Map<String, RouteRules> routeRulesByCore;
  final Map<String, String> activeModeByCore;
  final Map<String, List<RoutePreset>> routePresetsByCore;
  final RoutesChanged onRoutesChanged;
  final VoidCallback? onClose;

  @override
  State<RoutesShell> createState() => _RoutesShellState();
}

class _RoutesShellState extends State<RoutesShell> {
  final _scrollController = ScrollController();
  final _modeFutures = <String, Future<BackendCoreModes>>{};
  late String _selectedCoreId = _initialCoreId(widget.activeCoreId);
  late final Map<String, String> _routes = Map.of(widget.routesByCore);
  late final Map<String, RouteRules> _rules = Map.of(widget.routeRulesByCore);
  late final Map<String, String> _modes = Map.of(widget.activeModeByCore);
  late final Map<String, List<RoutePreset>> _presets = {
    for (final entry in widget.routePresetsByCore.entries)
      entry.key: List<RoutePreset>.of(entry.value),
  };

  CoreSpec get _selectedCore => findCoreIn(widget.cores, _selectedCoreId);

  List<RoutePreset> get _selectedPresets {
    return _presets.putIfAbsent(_selectedCoreId, () => RoutePreset.defaults);
  }

  String get _selectedRoute {
    final presets = _selectedPresets;
    return _routes[_selectedCoreId] ??
        (presets.isEmpty ? '' : presets.first.id);
  }

  RouteRules get _selectedRules {
    return _rules[_selectedCoreId] ?? RouteRules.empty;
  }

  Future<BackendCoreModes> _coreModes(String coreId) {
    return _modeFutures.putIfAbsent(
      coreId,
      () => widget.sessionService.coreModes(coreId),
    );
  }

  String _initialCoreId(String coreId) {
    for (final core in widget.cores) {
      if (core.id == coreId) {
        return core.id;
      }
    }
    return 'trusttunnel';
  }

  @override
  void didUpdateWidget(covariant RoutesShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.activeCoreId != widget.activeCoreId) {
      _selectedCoreId = _initialCoreId(widget.activeCoreId);
    }
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return LayoutBuilder(
      builder: (context, constraints) {
        final maxW =
            constraints.maxWidth.isFinite ? constraints.maxWidth : 820.0;
        final maxH =
            constraints.maxHeight.isFinite ? constraints.maxHeight : 600.0;
        final width = math.min(820.0, maxW);
        final height = math.min(600.0, maxH);

        return Container(
          width: width,
          height: height,
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
              _RoutesTitleBar(onClose: widget.onClose),
              Expanded(
                child: Scrollbar(
                  controller: _scrollController,
                  thumbVisibility: true,
                  child: SingleChildScrollView(
                    controller: _scrollController,
                    padding: const EdgeInsets.fromLTRB(28, 24, 28, 28),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Route profiles',
                          style: TextStyle(
                            color: palette.text,
                            fontSize: 26,
                            fontWeight: FontWeight.w900,
                          ),
                        ),
                        const SizedBox(height: 6),
                        Text(
                          'Routing is configured per core.',
                          style: TextStyle(color: palette.muted, fontSize: 13),
                        ),
                        const SizedBox(height: 18),
                        _CoreTabs(
                          cores: widget.cores,
                          selectedCoreId: _selectedCoreId,
                          onSelected: (id) {
                            setState(() => _selectedCoreId = id);
                          },
                        ),
                        const SizedBox(height: 18),
                        _CoreSummary(core: _selectedCore),
                        const SizedBox(height: 16),
                        FutureBuilder<BackendCoreModes>(
                          future: _coreModes(_selectedCoreId),
                          builder: (context, snapshot) {
                            final modes = snapshot.data;
                            return _RouteModeSelector(
                              core: _selectedCore,
                              modes: modes,
                              selectedMode: _modes[_selectedCoreId] ??
                                  modes?.defaultMode ??
                                  'local_proxy_gate',
                              onChanged: (mode) {
                                setState(() {
                                  _modes[_selectedCoreId] = mode;
                                });
                              },
                            );
                          },
                        ),
                        const SizedBox(height: 18),
                        _PresetManager(
                          presets: _selectedPresets,
                          selectedPresetId: _selectedRoute,
                          rules: _selectedRules,
                          onApply: _applyPreset,
                          onCreate: _createPreset,
                          onRename: _renamePreset,
                          onDelete: _deletePreset,
                        ),
                        const SizedBox(height: 18),
                        _RulesEditor(
                          key: ValueKey(_selectedCoreId),
                          core: _selectedCore,
                          rules: _selectedRules,
                          onChanged: (rules) {
                            setState(() => _rules[_selectedCoreId] = rules);
                          },
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              _RoutesFooter(
                onSave: () {
                  widget.onRoutesChanged(
                    Map.unmodifiable(_routes),
                    Map.unmodifiable(_rules),
                    Map.unmodifiable(_modes),
                    Map.unmodifiable({
                      for (final entry in _presets.entries)
                        entry.key: List<RoutePreset>.unmodifiable(entry.value),
                    }),
                  );
                  widget.onClose?.call();
                },
                onClose: widget.onClose,
              ),
            ],
          ),
        );
      },
    );
  }

  void _applyPreset(RoutePreset preset) {
    setState(() {
      _routes[_selectedCoreId] = preset.id;
      _rules[_selectedCoreId] = preset.rules;
    });
  }

  void _createPreset() {
    final presets = _selectedPresets;
    final id = _uniquePresetId(_selectedCoreId, presets);
    setState(() {
      presets.add(RoutePreset(
        id: id,
        name: 'Custom preset ${presets.length + 1}',
        description: 'User route preset.',
        rules: _selectedRules,
      ));
      _routes[_selectedCoreId] = id;
    });
  }

  void _renamePreset(RoutePreset preset) {
    final controller = TextEditingController(text: preset.name);
    showDialog<void>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: const Text('Rename preset'),
          content: TextField(
            controller: controller,
            autofocus: true,
            decoration: const InputDecoration(labelText: 'Preset name'),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final name = controller.text.trim();
                if (name.isNotEmpty) {
                  setState(() {
                    final presets = _selectedPresets;
                    final index =
                        presets.indexWhere((item) => item.id == preset.id);
                    if (index >= 0) {
                      presets[index] = preset.copyWith(name: name);
                    }
                  });
                }
                Navigator.of(context).pop();
              },
              child: const Text('Save'),
            ),
          ],
        );
      },
    );
  }

  void _deletePreset(RoutePreset preset) {
    setState(() {
      final presets = _selectedPresets;
      if (presets.length <= 1) {
        return;
      }
      presets.removeWhere((item) => item.id == preset.id);
      if (_routes[_selectedCoreId] == preset.id) {
        _routes[_selectedCoreId] = presets.first.id;
        _rules[_selectedCoreId] = presets.first.rules;
      }
    });
  }
}

String _uniquePresetId(String coreId, List<RoutePreset> presets) {
  var counter = presets.length + 1;
  while (true) {
    final id = '${coreId}_custom_$counter';
    if (!presets.any((preset) => preset.id == id)) {
      return id;
    }
    counter++;
  }
}

class _RoutesTitleBar extends StatelessWidget {
  const _RoutesTitleBar({this.onClose});

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
              ),
              child: const Icon(
                Icons.alt_route_rounded,
                color: Colors.white,
                size: 17,
              ),
            ),
            const SizedBox(width: 11),
            Text(
              'Routes',
              style: TextStyle(
                color: palette.text,
                fontSize: 15,
                fontWeight: FontWeight.w900,
              ),
            ),
            const Spacer(),
            IconButton(
              tooltip: 'Minimize',
              visualDensity: VisualDensity.compact,
              color: palette.muted,
              onPressed: WindowControls.minimize,
              icon: const Icon(Icons.remove_rounded, size: 18),
            ),
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

class _CoreTabs extends StatelessWidget {
  const _CoreTabs({
    required this.cores,
    required this.selectedCoreId,
    required this.onSelected,
  });

  final List<CoreSpec> cores;
  final String selectedCoreId;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final core in cores)
          _CoreTab(
            core: core,
            selected: core.id == selectedCoreId,
            onPressed: () => onSelected(core.id),
          ),
      ],
    );
  }
}

class _CoreTab extends StatelessWidget {
  const _CoreTab({
    required this.core,
    required this.selected,
    required this.onPressed,
  });

  final CoreSpec core;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
      color: selected ? core.accent.withValues(alpha: 0.14) : palette.surface,
      borderRadius: BorderRadius.circular(14),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onPressed,
        child: Container(
          height: 40,
          padding: const EdgeInsets.symmetric(horizontal: 12),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            border: Border.all(
              color: selected
                  ? core.accent.withValues(alpha: 0.55)
                  : palette.border,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _CoreBadge(core: core),
              const SizedBox(width: 8),
              Text(
                core.name,
                style: TextStyle(
                  color: selected ? palette.text : palette.muted,
                  fontSize: 13,
                  fontWeight: FontWeight.w900,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CoreBadge extends StatelessWidget {
  const _CoreBadge({required this.core});

  final CoreSpec core;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 23,
      height: 23,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: core.accent,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        core.letter,
        style: const TextStyle(
          color: Colors.white,
          fontSize: 12,
          fontWeight: FontWeight.w900,
        ),
      ),
    );
  }
}

class _CoreSummary extends StatelessWidget {
  const _CoreSummary({required this.core});

  final CoreSpec core;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: palette.border),
      ),
      child: Row(
        children: [
          _CoreBadge(core: core),
          const SizedBox(width: 12),
          Expanded(
            child: Text(
              '${core.protocol} - ${core.listener} - ${core.tagline}',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: palette.muted, fontSize: 12.5),
            ),
          ),
        ],
      ),
    );
  }
}

class _RouteModeSelector extends StatelessWidget {
  const _RouteModeSelector({
    required this.core,
    required this.modes,
    required this.selectedMode,
    required this.onChanged,
  });

  final CoreSpec core;
  final BackendCoreModes? modes;
  final String selectedMode;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final available = modes?.available.toSet() ?? const <String>{};
    final disabled = {
      for (final item in modes?.disabled ?? const <BackendDisabledRouteMode>[])
        item.mode: item.reason,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Routing mode',
          style: TextStyle(
            color: palette.muted,
            fontSize: 12,
            fontWeight: FontWeight.w900,
            letterSpacing: 0.6,
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 10,
          runSpacing: 10,
          children: [
            for (final mode in _routeModes)
              _RouteModeChip(
                mode: mode,
                selected: selectedMode == mode.id,
                enabled: available.contains(mode.id),
                disabledReason: disabled[mode.id],
                onPressed: () => onChanged(mode.id),
              ),
          ],
        ),
      ],
    );
  }
}

const _routeModes = [
  _RouteModeCopy(
    id: 'tun',
    title: 'TUN',
    description:
        'Intercepts all system traffic through a virtual network adapter.',
    hint:
        'Use when applications do not support system proxy. Requires Administrator privileges.',
  ),
  _RouteModeCopy(
    id: 'system_proxy',
    title: 'System Proxy',
    description:
        'Sets the Windows system proxy for applications that respect it.',
    hint: 'Good for everyday web browsing without Administrator privileges.',
  ),
  _RouteModeCopy(
    id: 'local_proxy_gate',
    title: 'Local Proxy Gate',
    description: 'Hosts a local proxy server without changing system settings.',
    hint: 'Safest mode; each application must be configured manually.',
  ),
];

class _RouteModeCopy {
  const _RouteModeCopy({
    required this.id,
    required this.title,
    required this.description,
    required this.hint,
  });

  final String id;
  final String title;
  final String description;
  final String hint;
}

class _RouteModeChip extends StatelessWidget {
  const _RouteModeChip({
    required this.mode,
    required this.selected,
    required this.enabled,
    required this.disabledReason,
    required this.onPressed,
  });

  final _RouteModeCopy mode;
  final bool selected;
  final bool enabled;
  final String? disabledReason;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Tooltip(
      message: enabled ? mode.hint : disabledReason ?? mode.hint,
      child: Opacity(
        opacity: enabled ? 1 : 0.48,
        child: Material(
          color: selected ? palette.accentSoft : palette.surface,
          borderRadius: BorderRadius.circular(14),
          child: InkWell(
            borderRadius: BorderRadius.circular(14),
            onTap: enabled ? onPressed : null,
            child: Container(
              width: 224,
              constraints: const BoxConstraints(minHeight: 92),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(14),
                border: Border.all(
                  color: selected
                      ? palette.accent.withValues(alpha: 0.55)
                      : palette.border,
                ),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(
                        selected
                            ? Icons.radio_button_checked_rounded
                            : Icons.radio_button_unchecked_rounded,
                        color: selected ? palette.accent : palette.muted,
                        size: 17,
                      ),
                      const SizedBox(width: 7),
                      Expanded(
                        child: Text(
                          mode.title,
                          style: TextStyle(
                            color: palette.text,
                            fontSize: 13,
                            fontWeight: FontWeight.w900,
                          ),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 7),
                  Text(
                    mode.description,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: palette.muted,
                      fontSize: 11.5,
                      height: 1.25,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _PresetManager extends StatelessWidget {
  const _PresetManager({
    required this.presets,
    required this.selectedPresetId,
    required this.rules,
    required this.onApply,
    required this.onCreate,
    required this.onRename,
    required this.onDelete,
  });

  final List<RoutePreset> presets;
  final String selectedPresetId;
  final RouteRules rules;
  final ValueChanged<RoutePreset> onApply;
  final VoidCallback onCreate;
  final ValueChanged<RoutePreset> onRename;
  final ValueChanged<RoutePreset> onDelete;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(
              'Presets',
              style: TextStyle(
                color: palette.muted,
                fontSize: 12,
                fontWeight: FontWeight.w900,
                letterSpacing: 0.6,
              ),
            ),
            const Spacer(),
            TextButton.icon(
              onPressed: onCreate,
              icon: const Icon(Icons.add_rounded, size: 16),
              label: const Text('New'),
            ),
          ],
        ),
        const SizedBox(height: 8),
        for (final preset in presets) ...[
          _PresetCard(
            preset: preset,
            selected: preset.id == selectedPresetId,
            onApply: () => onApply(preset),
            onRename: () => onRename(preset),
            onDelete: presets.length <= 1 ? null : () => onDelete(preset),
          ),
          const SizedBox(height: 8),
        ],
      ],
    );
  }
}

class _PresetCard extends StatelessWidget {
  const _PresetCard({
    required this.preset,
    required this.selected,
    required this.onApply,
    required this.onRename,
    required this.onDelete,
  });

  final RoutePreset preset;
  final bool selected;
  final VoidCallback onApply;
  final VoidCallback onRename;
  final VoidCallback? onDelete;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Material(
      color: selected ? palette.accentSoft : palette.surface,
      borderRadius: BorderRadius.circular(14),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onApply,
        child: Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            border: Border.all(
              color: selected
                  ? palette.accent.withValues(alpha: 0.55)
                  : palette.border,
            ),
          ),
          child: Row(
            children: [
              Icon(
                selected ? Icons.check_circle_rounded : Icons.route_rounded,
                color: selected ? palette.accent : palette.muted,
                size: 19,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      preset.name,
                      style: TextStyle(
                        color: palette.text,
                        fontSize: 13.5,
                        fontWeight: FontWeight.w900,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      preset.description,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: palette.muted,
                        fontSize: 12,
                        height: 1.25,
                      ),
                    ),
                  ],
                ),
              ),
              IconButton(
                tooltip: 'Rename',
                onPressed: onRename,
                icon: const Icon(Icons.edit_rounded, size: 16),
              ),
              IconButton(
                tooltip: 'Delete',
                onPressed: onDelete,
                icon: const Icon(Icons.delete_outline_rounded, size: 16),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _RulesEditor extends StatelessWidget {
  const _RulesEditor({
    super.key,
    required this.core,
    required this.rules,
    required this.onChanged,
  });

  final CoreSpec core;
  final RouteRules rules;
  final ValueChanged<RouteRules> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Custom rules',
          style: TextStyle(
            color: palette.muted,
            fontSize: 12,
            fontWeight: FontWeight.w900,
            letterSpacing: 0.6,
          ),
        ),
        const SizedBox(height: 10),
        Container(
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: palette.surface,
            borderRadius: BorderRadius.circular(18),
            border: Border.all(color: palette.border),
          ),
          child: Column(
            children: [
              _RouteTextArea(
                fieldKey: ValueKey('${core.id}-apps'),
                label: 'Applications',
                helper: '.exe names to proxy, one per line',
                hint: 'chrome.exe\ntelegram.exe',
                value: rules.appExeNames,
                onChanged: (value) {
                  onChanged(rules.copyWith(appExeNames: value));
                },
              ),
              const SizedBox(height: 14),
              _RouteTextArea(
                fieldKey: ValueKey('${core.id}-include-domains'),
                label: 'Domains via proxy',
                helper: 'Domains routed through tunnel, one per line',
                hint: 'example.com\n*.example.org',
                value: rules.includedDomains,
                onChanged: (value) {
                  onChanged(rules.copyWith(includedDomains: value));
                },
              ),
              const SizedBox(height: 14),
              _RouteTextArea(
                fieldKey: ValueKey('${core.id}-exclude-domains'),
                label: 'Domains bypassed',
                helper: 'Domains that bypass proxy, one per line',
                hint: 'local\ncompany.lan',
                value: rules.excludedDomains,
                onChanged: (value) {
                  onChanged(rules.copyWith(excludedDomains: value));
                },
              ),
              const SizedBox(height: 14),
              _RouteTextArea(
                fieldKey: ValueKey('${core.id}-include-cidrs'),
                label: 'CIDRs via proxy',
                helper: 'IP ranges routed through tunnel',
                hint: '0.0.0.0/0\n::/0',
                value: rules.includedCidrs,
                onChanged: (value) {
                  onChanged(rules.copyWith(includedCidrs: value));
                },
              ),
              const SizedBox(height: 14),
              _RouteTextArea(
                fieldKey: ValueKey('${core.id}-exclude-cidrs'),
                label: 'CIDRs bypassed',
                helper: 'IP ranges that bypass proxy',
                hint: '10.0.0.0/8\n192.168.0.0/16',
                value: rules.excludedCidrs,
                onChanged: (value) {
                  onChanged(rules.copyWith(excludedCidrs: value));
                },
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _RouteTextArea extends StatelessWidget {
  const _RouteTextArea({
    required this.fieldKey,
    required this.label,
    required this.helper,
    required this.hint,
    required this.value,
    required this.onChanged,
  });

  final Key fieldKey;
  final String label;
  final String helper;
  final String hint;
  final String value;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: TextStyle(
            color: palette.text,
            fontSize: 12.5,
            fontWeight: FontWeight.w900,
          ),
        ),
        const SizedBox(height: 4),
        Text(
          helper,
          style: TextStyle(
            color: palette.muted,
            fontSize: 11.5,
          ),
        ),
        const SizedBox(height: 8),
        TextFormField(
          key: fieldKey,
          initialValue: value,
          minLines: 2,
          maxLines: 4,
          keyboardType: TextInputType.multiline,
          textInputAction: TextInputAction.newline,
          onChanged: onChanged,
          cursorColor: palette.accent,
          decoration: InputDecoration(hintText: hint),
        ),
      ],
    );
  }
}

class _RoutesFooter extends StatelessWidget {
  const _RoutesFooter({
    required this.onSave,
    this.onClose,
  });

  final VoidCallback onSave;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return Container(
      height: 76,
      padding: const EdgeInsets.symmetric(horizontal: 24),
      decoration: BoxDecoration(
        color: palette.surface,
        border: Border(top: BorderSide(color: palette.border)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final narrow = constraints.maxWidth < 520;
          return Row(
            children: [
              if (!narrow)
                Text(
                  'Mode and presets are saved per core.',
                  style: TextStyle(color: palette.muted, fontSize: 12.5),
                ),
              const Spacer(),
              OutlinedButton(
                onPressed: onClose,
                child: const Text('Cancel'),
              ),
              const SizedBox(width: 10),
              FilledButton(
                onPressed: onSave,
                child: const Text('Save'),
              ),
            ],
          );
        },
      ),
    );
  }
}
