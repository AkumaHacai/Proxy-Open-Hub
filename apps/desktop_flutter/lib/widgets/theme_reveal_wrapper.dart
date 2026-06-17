import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../theme/poh_theme.dart';

class ThemeRevealWrapper extends StatefulWidget {
  const ThemeRevealWrapper({
    super.key,
    required this.themeMode,
    required this.accent,
    required this.duration,
    required this.builder,
  });

  final PohThemeMode themeMode;
  final PohAccent accent;
  final Duration duration;
  final Widget Function(BuildContext context, PohThemeMode themeMode) builder;

  @override
  State<ThemeRevealWrapper> createState() => ThemeRevealWrapperState();
}

class ThemeRevealWrapperState extends State<ThemeRevealWrapper>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _radiusProgress;
  Offset? _origin;
  PohThemeMode? _revealedThemeMode;
  PohAccent? _revealedAccent;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: widget.duration,
    );
    _radiusProgress = CurvedAnimation(
      parent: _controller,
      curve: Curves.easeOutCubic,
    );
  }

  @override
  void didUpdateWidget(covariant ThemeRevealWrapper oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.duration != widget.duration) {
      _controller.duration = widget.duration;
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> revealTo({
    required PohThemeMode themeMode,
    required PohAccent accent,
    required Offset globalOrigin,
  }) async {
    final box = context.findRenderObject() as RenderBox?;
    final fallbackOrigin =
        box == null ? Offset.zero : box.size.center(Offset.zero);

    setState(() {
      _origin = box?.globalToLocal(globalOrigin) ?? fallbackOrigin;
      _revealedThemeMode = themeMode;
      _revealedAccent = accent;
    });

    _controller.stop();
    await _controller.forward(from: 0);
  }

  void clearReveal() {
    if (!mounted) {
      return;
    }

    setState(() {
      _origin = null;
      _revealedThemeMode = null;
      _revealedAccent = null;
    });
    _controller.reset();
  }

  @override
  Widget build(BuildContext context) {
    final activeTheme = buildPohTheme(
      mode: widget.themeMode,
      accent: widget.accent,
    );
    final revealedThemeMode = _revealedThemeMode;
    final revealedAccent = _revealedAccent;
    final origin = _origin;

    return Theme(
      data: activeTheme,
      child: Builder(
        builder: (activeContext) {
          return Stack(
            fit: StackFit.expand,
            children: [
              widget.builder(activeContext, widget.themeMode),
              if (revealedThemeMode != null &&
                  revealedAccent != null &&
                  origin != null)
                IgnorePointer(
                  child: AnimatedBuilder(
                    animation: _radiusProgress,
                    builder: (context, child) {
                      return ClipPath(
                        clipper: _CircularRevealClipper(
                          origin: origin,
                          progress: _radiusProgress.value,
                        ),
                        child: child,
                      );
                    },
                    child: Theme(
                      data: buildPohTheme(
                        mode: revealedThemeMode,
                        accent: revealedAccent,
                      ),
                      child: Builder(
                        builder: (revealedContext) {
                          return widget.builder(
                            revealedContext,
                            revealedThemeMode,
                          );
                        },
                      ),
                    ),
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}

class _CircularRevealClipper extends CustomClipper<Path> {
  const _CircularRevealClipper({
    required this.origin,
    required this.progress,
  });

  final Offset origin;
  final double progress;

  @override
  Path getClip(Size size) {
    final corners = [
      Offset.zero,
      Offset(size.width, 0),
      Offset(0, size.height),
      Offset(size.width, size.height),
    ];

    // The circle must cover the farthest screen corner from the click point.
    // For each corner we calculate the hypotenuse sqrt(dx^2 + dy^2), then use
    // the largest distance as the final radius so any click origin can reveal
    // the whole window without leaving uncovered corners.
    final maxRadius = corners
        .map((corner) => (corner - origin).distance)
        .fold<double>(0, math.max);

    return Path()
      ..addOval(Rect.fromCircle(
        center: origin,
        radius: maxRadius * progress,
      ));
  }

  @override
  bool shouldReclip(_CircularRevealClipper oldClipper) {
    return oldClipper.origin != origin || oldClipper.progress != progress;
  }
}
