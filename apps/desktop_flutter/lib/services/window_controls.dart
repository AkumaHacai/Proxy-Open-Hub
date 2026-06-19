import 'package:flutter/animation.dart';
import 'package:flutter/services.dart';

class WindowControls {
  const WindowControls._();

  static const _channel = MethodChannel('proxy_open_hub/window');

  static Future<void> minimize() async {
    await _channel.invokeMethod<void>('minimize');
  }

  static Future<void> close() async {
    await _channel.invokeMethod<void>('close');
  }

  static Future<void> startDrag() async {
    await _channel.invokeMethod<void>('startDrag');
  }

  static Future<({double width, double height})> getWindowSize() async {
    final result = await _channel.invokeMapMethod<String, Object?>(
      'getWindowSize',
    );
    final width = (result?['width'] as num?)?.toDouble() ?? 0;
    final height = (result?['height'] as num?)?.toDouble() ?? 0;
    return (width: width, height: height);
  }

  static Future<void> setWindowSize({
    required double width,
    required double height,
  }) async {
    await _channel.invokeMethod<void>('setWindowSize', {
      'width': width,
      'height': height,
    });
  }

  static Future<void> animateWindowSize({
    required double width,
    required double height,
    Duration duration = const Duration(milliseconds: 220),
  }) async {
    if (duration == Duration.zero) {
      await setWindowSize(width: width, height: height);
      return;
    }

    final start = await getWindowSize();
    if (start.width <= 0 || start.height <= 0) {
      await setWindowSize(width: width, height: height);
      return;
    }

    final stopwatch = Stopwatch()..start();

    while (stopwatch.elapsed < duration) {
      final elapsed = stopwatch.elapsedMilliseconds;
      final progress = (elapsed / duration.inMilliseconds).clamp(0.0, 1.0);

      // Same curve the in-window layout uses (PohMotion.standard ==
      // cubic-bezier(.4,0,.2,1)) so the window edges and the content morph in
      // lockstep instead of drifting apart.
      final eased = Curves.fastOutSlowIn.transform(progress);
      await setWindowSize(
        width: start.width + (width - start.width) * eased,
        height: start.height + (height - start.height) * eased,
      );
      // Yield briefly to event loop to allow UI rendering and avoid clogging
      // the MethodChannel.
      await Future<void>.delayed(const Duration(milliseconds: 1));
    }

    stopwatch.stop();
    // Final snap to target
    await setWindowSize(width: width, height: height);
  }
}
