import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'preferences_model.dart';

const protocolVersion = 4;
const maxFrameBytes = 262144;
const resourceNames = ['microphone', 'camera'];
const activityNames = ['active', 'ready', 'none_observed', 'unknown'];
const healthNames = ['healthy', 'degraded', 'unavailable'];

// Rust owns the versioned observation and narrowly typed command boundary.
class Snapshot {
  Snapshot._(this.data, this.preferences);
  final Map<String, dynamic> data;
  final PreferenceView preferences;
  int get sequence => data['sequence'] as int;
  String get locale => data['locale'] as String;
  DateTime get observedAt => DateTime.parse(data['observed_at'] as String);
  Map<String, dynamic> get engine => data['engine'] as Map<String, dynamic>;
  List<Map<String, dynamic>> get resources => _maps(data['resources']);
  List<Map<String, dynamic>> get accesses => _maps(data['accesses']);
  List<Map<String, dynamic>> get collectors => _maps(data['collectors']);
  List<String> get gaps => (data['observation_gaps'] as List).cast<String>();
  List<String> get diagnostics => (data['diagnostics'] as List).cast<String>();
  Map<String, dynamic> protection(String resource) =>
      (data['protection'] as Map<String, dynamic>)[resource] as Map<String, dynamic>;
  bool get truncated => data['truncated'] as bool;
  Map<String, dynamic> get controls => data['controls'] as Map<String, dynamic>;

  static List<Map<String, dynamic>> _maps(dynamic value) =>
      (value as List).cast<Map<String, dynamic>>();

  static Snapshot decode(List<int> bytes, {int? afterSequence}) {
    if (bytes.length > maxFrameBytes) _invalid('Frame exceeds byte limit');
    final value = jsonDecode(utf8.decode(bytes));
    final root = _object(value, const ['type', 'protocol_version', 'sequence',
      'observed_at', 'engine', 'locale', 'resources', 'accesses', 'collectors',
      'observation_gaps', 'protection', 'diagnostics', 'truncated', 'controls', 'preferences']);
    if (root['type'] != 'snapshot' || root['protocol_version'] is! int ||
        root['protocol_version'] != protocolVersion) {
      _invalid('Unsupported snapshot protocol');
    }
    _integer(root['sequence']);
    if (afterSequence != null && (root['sequence'] as int) <= afterSequence) {
      _invalid('Snapshot sequence did not increase');
    }
    _timestamp(root['observed_at']);
    final engine = _object(root['engine'], const ['version', 'platform', 'mode', 'state', 'detail']);
    _text(engine['version']);
    _enum(engine['platform'], const ['windows', 'linux', 'macos']);
    _enum(engine['mode'], const ['read_only_development', 'controlled_development', 'settings_development', 'operational']);
    _enum(engine['state'], healthNames);
    _nullableText(engine['detail']);
    _enum(root['locale'], const ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']);
    final resources = _list(root['resources'], 2);
    if (resources.length != 2) _invalid('Exactly two resources required');
    for (var i = 0; i < resources.length; i++) {
      final resource = _object(resources[i], const ['resource', 'activity', 'coverage',
        'confidence', 'active_count', 'ready_count']);
      if (resource['resource'] != resourceNames[i]) _invalid('Invalid resource order');
      _enum(resource['activity'], activityNames);
      _enum(resource['coverage'], const ['complete', 'incomplete', 'unavailable']);
      _confidence(resource['confidence']);
      _integer(resource['active_count']);
      _integer(resource['ready_count']);
    }
    for (final item in _list(root['accesses'], 128)) {
      final access = _object(item, const ['resource', 'activity', 'confidence',
        'application', 'pid', 'device', 'started_at']);
      _enum(access['resource'], resourceNames);
      _enum(access['activity'], activityNames);
      _confidence(access['confidence']);
      _text(access['application']);
      if (access['pid'] != null) _integer(access['pid'], maximum: 4294967295);
      _nullableText(access['device']);
      if (access['started_at'] != null) _timestamp(access['started_at']);
    }
    for (final item in _list(root['collectors'], 64)) {
      final collector = _object(item, const ['collector', 'state', 'detail']);
      _text(collector['collector']);
      _enum(collector['state'], healthNames);
      _nullableText(collector['detail']);
    }
    for (final gap in _list(root['observation_gaps'], 2)) {
      _enum(gap, resourceNames);
    }
    final protection = _object(root['protection'], resourceNames);
    for (final resource in resourceNames) {
      final status = _object(protection[resource], const ['state', 'requested', 'service_active', 'detail']);
      _enum(status['state'], resource == 'microphone'
          ? const ['muted', 'unmuted', 'mixed', 'unknown']
          : const ['allowed', 'blocked', 'system_managed', 'unknown']);
      _nullableBool(status['requested']);
      _nullableBool(status['service_active']);
      _nullableText(status['detail']);
    }
    for (final diagnostic in _list(root['diagnostics'], 64)) {
      _text(diagnostic);
    }
    if (root['truncated'] is! bool) _invalid('Invalid truncated flag');
    final controls = _object(root['controls'], const ['session', 'microphone', 'command']);
    final token = controls['session'];
    if (token is! String || (token.isNotEmpty && !RegExp(r'^[0-9a-f]{32}$').hasMatch(token))) {
      _invalid('Invalid command session');
    }
    final capability = _object(controls['microphone'], const [
      'can_enable', 'can_release', 'current_requested', 'generation', 'reason']);
    if (capability['can_enable'] is! bool || capability['can_release'] is! bool) {
      _invalid('Invalid command capability');
    }
    _nullableBool(capability['current_requested']);
    _integer(capability['generation']);
    _nullableText(capability['reason']);
    if ((capability['can_enable'] == true || capability['can_release'] == true) &&
        (token.isEmpty || capability['current_requested'] == null)) {
      _invalid('Capability lacks authenticated current state');
    }
    if (controls['command'] != null) {
      final result = _object(controls['command'], const ['id', 'requested', 'state', 'code', 'detail']);
      _integer(result['id'], maximum: 0xffffffff);
      if (result['id'] == 0 || result['requested'] is! bool || token.isEmpty) _invalid('Invalid command result');
      _enum(result['state'], const ['accepted', 'succeeded', 'failed', 'rejected', 'uncertain']);
      _nullableText(result['code']);
      if (result['code'] != null && utf8.encode(result['code'] as String).length > 64) _invalid('Invalid result code');
      _nullableText(result['detail']);
    }
    final preferences = PreferenceView.fromJson(root['preferences']);
    if ((preferences.writable || preferences.command != null) && token.isEmpty) {
      _invalid('Preferences lack an authenticated command session');
    }
    return Snapshot._(root, preferences);
  }

  // Rust owns collector-specific interpretation on every platform. The view
  // preserves positive observations and checks only portable wire invariants.
  String activity(Map<String, dynamic> resource) {
    final observed = resource['activity'] as String;
    if (observed == 'active' || observed == 'ready') return observed;
    if (resource['coverage'] != 'complete' ||
        gaps.contains(resource['resource']) || collectors.isEmpty ||
        collectors.every((c) => c['state'] != 'healthy')) {
      return 'unknown';
    }
    return observed;
  }
}

Never _invalid(String message) => throw FormatException(message);
Map<String, dynamic> _object(dynamic value, List<String> keys) {
  if (value is! Map<String, dynamic> || value.length != keys.length ||
      keys.any((key) => !value.containsKey(key))) _invalid('Invalid object shape');
  return value;
}
List<dynamic> _list(dynamic value, int maximum) {
  if (value is! List || value.length > maximum) _invalid('Invalid bounded list');
  return value;
}
void _text(dynamic value) {
  if (value is! String || utf8.encode(value).length > 1024) _invalid('Invalid bounded string');
}
void _nullableText(dynamic value) { if (value != null) _text(value); }
void _integer(dynamic value, {int maximum = 0x7fffffffffffffff}) {
  if (value is! int || value < 0 || value > maximum) _invalid('Invalid unsigned integer');
}
void _enum(dynamic value, List<String> allowed) {
  if (value is! String || !allowed.contains(value)) _invalid('Unknown enum value');
}
void _confidence(dynamic value) {
  if (value != null) _enum(value, const ['high', 'medium', 'low']);
}
void _nullableBool(dynamic value) {
  if (value != null && value is! bool) _invalid('Invalid nullable boolean');
}
void _timestamp(dynamic value) {
  _text(value);
  final match = RegExp(r'^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(\.\d+)?(Z|[+-](\d{2}):(\d{2}))$')
      .firstMatch(value as String);
  if (match == null || DateTime.tryParse(value) == null) _invalid('Invalid timestamp');
  final year = int.parse(match.group(1)!);
  final month = int.parse(match.group(2)!);
  final day = int.parse(match.group(3)!);
  if (month < 1 || month > 12 || day < 1 ||
      day > DateTime.utc(year, month + 1, 0).day ||
      int.parse(match.group(4)!) > 23 || int.parse(match.group(5)!) > 59 ||
      int.parse(match.group(6)!) > 59 ||
      (match.group(9) != null && int.parse(match.group(9)!) > 23) ||
      (match.group(10) != null && int.parse(match.group(10)!) > 59)) {
    _invalid('Invalid calendar timestamp');
  }
}

enum ConnectionState { connecting, live, stale, error, disconnected }

class SnapshotSession extends ChangeNotifier {
  SnapshotSession({Duration Function()? elapsed, DateTime Function()? now})
      : _elapsed = elapsed ?? (Stopwatch()..start()).elapsedDuration,
        _now = now ?? DateTime.now;

  final DateTime Function() _now;
  final Duration Function() _elapsed;
  Snapshot? snapshot;
  Duration? _received;
  DateTime? receivedAt;
  String? error;
  ConnectionState state = ConnectionState.connecting;
  int? get sequence => snapshot?.sequence;
  bool get isLive => state == ConnectionState.live;
  bool get transportLive => state != ConnectionState.error &&
      state != ConnectionState.disconnected && _received != null &&
      _elapsed() - _received! <= const Duration(seconds: 5);
  bool get _observationFresh {
    final observed = snapshot?.observedAt;
    if (observed == null) return false;
    final age = _now().toUtc().difference(observed);
    return age >= const Duration(seconds: -2) && age <= const Duration(seconds: 5);
  }

  void accept(List<int> bytes) {
    try {
      final accepted = Snapshot.decode(bytes, afterSequence: sequence);
      snapshot = accepted;
      _received = _elapsed();
      receivedAt = _now();
      error = null;
      state = _observationFresh ? ConnectionState.live : ConnectionState.stale;
    } on Object {
      fail('Invalid snapshot / incompatible protocol');
      return;
    }
    notifyListeners();
  }

  void tick() {
    if (state == ConnectionState.live &&
        (!transportLive || !_observationFresh)) {
      state = ConnectionState.stale;
    }
    notifyListeners();
  }

  void fail(String message) {
    state = ConnectionState.error;
    error = message;
    notifyListeners();
  }

  void disconnect() {
    state = ConnectionState.disconnected;
    notifyListeners();
  }
}

// Getter tear-off is not available for Stopwatch.elapsed.
extension on Stopwatch {
  Duration elapsedDuration() => elapsed;
}

class SnapshotInput {
  SnapshotInput(Stream<List<int>> stream, this.session, {required this.onSnapshot, required this.onDone}) {
    _subscription = stream.listen(_chunk, onError: (Object error) {
      session.fail('Snapshot pipe read failed');
    }, onDone: () {
      if (_pending.isNotEmpty && !_discarding) session.fail('Incomplete snapshot at EOF');
      session.disconnect();
      onDone();
    });
  }
  final SnapshotSession session;
  final void Function() onSnapshot;
  final void Function() onDone;
  late final StreamSubscription<List<int>> _subscription;
  final List<int> _pending = [];
  bool _discarding = false;

  void _chunk(List<int> bytes) {
    for (final byte in bytes) {
      if (byte == 10) {
        if (!_discarding) {
          final before = session.sequence;
          session.accept(_pending);
          if (before != session.sequence) onSnapshot();
        }
        _pending.clear();
        _discarding = false;
      } else if (!_discarding) {
        if (_pending.length == maxFrameBytes) {
          _pending.clear();
          _discarding = true;
          session.fail('Snapshot exceeds byte limit');
        } else {
          _pending.add(byte);
        }
      }
    }
  }
  Future<void> close() => _subscription.cancel();
}
