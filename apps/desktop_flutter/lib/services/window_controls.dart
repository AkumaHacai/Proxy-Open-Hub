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
    final start = await getWindowSize();
    if (start.width <= 0 || start.height <= 0) {
      await setWindowSize(width: width, height: height);
      return;
    }

    const frames = 14;
    final frameDelay = duration ~/ frames;

    for (var frame = 1; frame <= frames; frame++) {
      final t = frame / frames;
      final eased = 1 - (1 - t) * (1 - t) * (1 - t);
      await setWindowSize(
        width: start.width + (width - start.width) * eased,
        height: start.height + (height - start.height) * eased,
      );
      await Future<void>.delayed(frameDelay);
    }
  }
}
