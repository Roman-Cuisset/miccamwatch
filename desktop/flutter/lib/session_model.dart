class ObservationCadence {
  const ObservationCadence._(
    this.nativeMs,
    this.cameraInventoryMs,
    this.requestedMs,
    this.presentationMs,
  );

  final int nativeMs;
  final int? cameraInventoryMs;
  final int? requestedMs;
  final int presentationMs;

  factory ObservationCadence.fromJson(Object? value) {
    final data = _object(value, {
      'native_ms',
      'camera_inventory_ms',
      'requested_ms',
      'presentation_ms',
    });
    final native = _integer(data['native_ms'], 100, 5000);
    final camera = data['camera_inventory_ms'];
    final requested = data['requested_ms'];
    final interval = requested == null ? null : _integer(requested, 100, 5000);
    final effective = _integer(data['presentation_ms'], native, 5000);
    if (effective !=
        (interval != null && interval > native ? interval : native)) {
      throw const FormatException('Inconsistent presentation cadence');
    }
    return ObservationCadence._(
      native,
      camera == null ? null : _integer(camera, 100, 60000),
      interval,
      effective,
    );
  }

  bool matches(ObservationCadence other) =>
      nativeMs == other.nativeMs &&
      cameraInventoryMs == other.cameraInventoryMs &&
      requestedMs == other.requestedMs &&
      presentationMs == other.presentationMs;
}

enum ObservationPhase { idle, recording, stopping, failed }

class ObservationSession {
  const ObservationSession._(
    this.phase,
    this.id,
    this.startedAt,
    this.path,
    this.events,
    this.bytes,
    this.maxBytes,
    this.error,
    this.cadence,
  );

  final ObservationPhase phase;
  final String? id;
  final DateTime? startedAt;
  final String? path;
  final int events;
  final int bytes;
  final int maxBytes;
  final String? error;
  final ObservationCadence cadence;

  factory ObservationSession.fromJson(Object? value) {
    final data = _object(value, {
      'phase',
      'session_id',
      'started_at',
      'path',
      'events_written',
      'bytes_written',
      'last_error',
      'max_bytes',
      'origin',
      'cadence',
    });
    final phase = ObservationPhase.values
        .where((phase) => phase.name == data['phase'])
        .firstOrNull;
    final id = _text(data['session_id'], 32);
    final path = _text(data['path'], 3072);
    final at = _text(data['started_at'], 64);
    final startedAt = at == null ? null : DateTime.tryParse(at);
    final error = _text(data['last_error'], 1024);
    final maxBytes = _integer(data['max_bytes'], 1, 32 * 1024 * 1024);
    final events = _integer(data['events_written'], 0, maxBytes);
    final bytes = _integer(data['bytes_written'], events, maxBytes);
    if (phase == null ||
        data['origin'] != 'native_owner' ||
        (id != null && !RegExp(r'^[a-f0-9]{32}$').hasMatch(id)) ||
        (id == null) != (path == null) ||
        (id == null) != (at == null) ||
        (at != null && (startedAt == null || !startedAt.isUtc)) ||
        (phase == ObservationPhase.failed && error == null) ||
        (id == null &&
            (bytes != 0 ||
                phase == ObservationPhase.recording ||
                phase == ObservationPhase.stopping))) {
      throw const FormatException('Invalid native observation session');
    }
    return ObservationSession._(
      phase,
      id,
      startedAt,
      path,
      events,
      bytes,
      maxBytes,
      error,
      ObservationCadence.fromJson(data['cadence']),
    );
  }
}

Map<String, dynamic> _object(Object? value, Set<String> keys) {
  if (value is! Map<String, dynamic> ||
      value.length != keys.length ||
      !keys.every(value.containsKey)) {
    throw const FormatException('Invalid observation session object');
  }
  return value;
}

int _integer(Object? value, int min, int max) {
  if (value is! int || value < min || value > max) {
    throw const FormatException('Invalid observation session integer');
  }
  return value;
}

String? _text(Object? value, int max) {
  if (value == null) return null;
  if (value is! String || value.isEmpty || value.length > max) {
    throw const FormatException('Invalid observation session text');
  }
  return value;
}
