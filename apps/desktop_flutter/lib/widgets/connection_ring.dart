import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../models/app_models.dart';
import '../theme/poh_theme.dart';

class ConnectionRingButton extends StatelessWidget {
  const ConnectionRingButton({
    super.key,
    required this.phase,
    required this.progress,
    required this.onPressed,
  });

  final ConnectionPhase phase;
  final double progress;
  final VoidCallback? onPressed;

  bool get _isConnected => phase == ConnectionPhase.connected;

  bool get _isWorking {
    return phase == ConnectionPhase.preparing ||
        phase == ConnectionPhase.connecting ||
        phase == ConnectionPhase.authenticating;
  }

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    final active = _isConnected || _isWorking;
    final fill = active ? palette.accent : palette.surface;
    final foreground = active ? Colors.white : palette.muted;

    return TweenAnimationBuilder<double>(
      tween: Tween(begin: 0, end: progress.clamp(0, 1)),
      duration: const Duration(milliseconds: 260),
      curve: Curves.easeOutCubic,
      builder: (context, animatedProgress, child) {
        return SizedBox(
          width: 194,
          height: 194,
          child: Stack(
            alignment: Alignment.center,
            children: [
              CustomPaint(
                size: const Size.square(194),
                painter: _ConnectionRingPainter(
                  progress: animatedProgress,
                  track: palette.border.withValues(alpha: 0.72),
                  accent: palette.accent,
                  glow: palette.glow,
                  isConnected: _isConnected,
                  isWorking: _isWorking,
                ),
              ),
              AnimatedScale(
                scale: active ? 1.0 : 0.96,
                duration: const Duration(milliseconds: 220),
                curve: Curves.easeOutCubic,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: fill,
                    shape: BoxShape.circle,
                    border: Border.all(
                      color: palette.border.withValues(alpha: 0.55),
                    ),
                    boxShadow: active
                        ? [
                            BoxShadow(
                              color: palette.glow,
                              blurRadius: 32,
                              spreadRadius: 3,
                            ),
                          ]
                        : null,
                  ),
                  child: Material(
                    color: Colors.transparent,
                    shape: const CircleBorder(),
                    child: InkWell(
                      customBorder: const CircleBorder(),
                      onTap: onPressed,
                      child: SizedBox.square(
                        dimension: 100,
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Icon(Icons.power_settings_new_rounded,
                                color: foreground, size: 34),
                            const SizedBox(height: 8),
                            Text(
                              _isConnected ? 'DISCONNECT' : 'CONNECT',
                              style: TextStyle(
                                color: foreground,
                                fontSize: 11,
                                fontWeight: FontWeight.w800,
                                letterSpacing: 0.6,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _ConnectionRingPainter extends CustomPainter {
  const _ConnectionRingPainter({
    required this.progress,
    required this.track,
    required this.accent,
    required this.glow,
    required this.isConnected,
    required this.isWorking,
  });

  final double progress;
  final Color track;
  final Color accent;
  final Color glow;
  final bool isConnected;
  final bool isWorking;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = math.min(size.width, size.height) / 2 - 16;
    final rect = Rect.fromCircle(center: center, radius: radius);

    final trackPaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 5
      ..strokeCap = StrokeCap.round
      ..color = track;
    canvas.drawCircle(center, radius, trackPaint);

    if (progress <= 0 && !isConnected && !isWorking) {
      return;
    }

    final accentPaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = isConnected ? 6 : 5
      ..strokeCap = StrokeCap.round
      ..color = accent;

    if (isConnected) {
      final glowPaint = Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 16
        ..strokeCap = StrokeCap.round
        ..color = glow.withValues(alpha: 0.34)
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 10);
      canvas.drawArc(rect, -math.pi / 2, math.pi * 2, false, glowPaint);
      canvas.drawCircle(center, radius, accentPaint);
      return;
    }

    final sweep = math.pi * 2 * progress.clamp(0, 1);
    canvas.drawArc(rect, -math.pi / 2, sweep, false, accentPaint);
  }

  @override
  bool shouldRepaint(_ConnectionRingPainter oldDelegate) {
    return progress != oldDelegate.progress ||
        track != oldDelegate.track ||
        accent != oldDelegate.accent ||
        glow != oldDelegate.glow ||
        isConnected != oldDelegate.isConnected ||
        isWorking != oldDelegate.isWorking;
  }
}
