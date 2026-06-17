import 'package:flutter/material.dart';

import '../theme/poh_theme.dart';

class Sparkline extends StatelessWidget {
  const Sparkline({
    super.key,
    required this.values,
    required this.isPrimary,
  });

  final List<double> values;
  final bool isPrimary;

  @override
  Widget build(BuildContext context) {
    final palette = PohPalette.of(context);
    return CustomPaint(
      size: const Size(58, 26),
      painter: _SparklinePainter(
        values: values,
        line: isPrimary ? palette.accent : palette.muted,
        fill: isPrimary
            ? palette.accent.withValues(alpha: 0.18)
            : palette.muted.withValues(alpha: 0.12),
      ),
    );
  }
}

class _SparklinePainter extends CustomPainter {
  const _SparklinePainter({
    required this.values,
    required this.line,
    required this.fill,
  });

  final List<double> values;
  final Color line;
  final Color fill;

  @override
  void paint(Canvas canvas, Size size) {
    if (values.length < 2) {
      return;
    }

    final maxValue = values.reduce((a, b) => a > b ? a : b).clamp(1, 9999);
    final points = <Offset>[
      for (var index = 0; index < values.length; index++)
        Offset(
          index / (values.length - 1) * size.width,
          size.height - (values[index] / maxValue) * (size.height - 4) - 2,
        ),
    ];

    final linePath = Path()..moveTo(points.first.dx, points.first.dy);
    for (final point in points.skip(1)) {
      linePath.lineTo(point.dx, point.dy);
    }

    final areaPath = Path.from(linePath)
      ..lineTo(size.width, size.height)
      ..lineTo(0, size.height)
      ..close();

    canvas.drawPath(areaPath, Paint()..color = fill);
    canvas.drawPath(
      linePath,
      Paint()
        ..color = line
        ..strokeWidth = 2
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round,
    );
  }

  @override
  bool shouldRepaint(_SparklinePainter oldDelegate) {
    return values != oldDelegate.values ||
        line != oldDelegate.line ||
        fill != oldDelegate.fill;
  }
}
