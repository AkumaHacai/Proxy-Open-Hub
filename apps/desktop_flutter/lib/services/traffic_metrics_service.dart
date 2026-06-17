import 'package:flutter/services.dart';

class TrafficSample {
  const TrafficSample({
    required this.downloadMbps,
    required this.uploadMbps,
    required this.interfaces,
  });

  final double downloadMbps;
  final double uploadMbps;
  final int interfaces;
}

class TrafficMetricsService {
  TrafficMetricsService();

  static const _channel = MethodChannel('proxy_open_hub/window');

  _TrafficSnapshot? _previous;

  void reset() {
    _previous = null;
  }

  Future<TrafficSample> sample() async {
    final snapshot = await _snapshot();
    final previous = _previous;
    _previous = snapshot;
    if (previous == null || snapshot.timestampMs <= previous.timestampMs) {
      return TrafficSample(
        downloadMbps: 0,
        uploadMbps: 0,
        interfaces: snapshot.interfaces,
      );
    }

    final elapsedSeconds = (snapshot.timestampMs - previous.timestampMs) / 1000;
    final rxDelta =
        (snapshot.rxBytes - previous.rxBytes).clamp(0, 1 << 62).toDouble();
    final txDelta =
        (snapshot.txBytes - previous.txBytes).clamp(0, 1 << 62).toDouble();
    return TrafficSample(
      downloadMbps: rxDelta * 8 / elapsedSeconds / 1000000,
      uploadMbps: txDelta * 8 / elapsedSeconds / 1000000,
      interfaces: snapshot.interfaces,
    );
  }

  Future<_TrafficSnapshot> _snapshot() async {
    final result = await _channel.invokeMapMethod<String, Object?>(
      'networkSnapshot',
    );
    final now = DateTime.now().millisecondsSinceEpoch;
    return _TrafficSnapshot(
      rxBytes: _readInt(result?['rxBytes']),
      txBytes: _readInt(result?['txBytes']),
      interfaces: _readInt(result?['interfaces']).toInt(),
      timestampMs: now,
    );
  }

  int _readInt(Object? value) {
    return switch (value) {
      int number => number,
      double number => number.toInt(),
      String text => int.tryParse(text) ?? 0,
      _ => 0,
    };
  }
}

class _TrafficSnapshot {
  const _TrafficSnapshot({
    required this.rxBytes,
    required this.txBytes,
    required this.interfaces,
    required this.timestampMs,
  });

  final int rxBytes;
  final int txBytes;
  final int interfaces;
  final int timestampMs;
}
