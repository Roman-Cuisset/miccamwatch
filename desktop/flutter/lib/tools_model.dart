import 'package:flutter/foundation.dart' show mapEquals;

/// These are intentions. Only Rust can authorize them and confirm their effect.
class NativeControl {
  const NativeControl._(this.kind, this.parameters);
  factory NativeControl.action(String kind) {
    if (!const {'microphone_mute', 'microphone_restore', 'camera_block',
      'camera_allow', 'camera_resume', 'camera_inspect'}.contains(kind)) {
      throw ArgumentError.value(kind, 'kind');
    }
    return NativeControl._(kind, const {});
  }
  factory NativeControl.lock({required bool microphone, required bool camera,
      required bool restore}) => NativeControl._('lock_policy', Map.unmodifiable({
    'microphone': microphone, 'camera': camera, 'restore': restore,
  }));
  factory NativeControl.terminate(int pid) {
    if (pid <= 4 || pid > 0xffffffff) throw ArgumentError.value(pid, 'pid');
    return NativeControl._('terminate', Map.unmodifiable({'pid': pid}));
  }
  factory NativeControl.enforcement(bool enabled) =>
      NativeControl._('automatic_enforcement', Map.unmodifiable({'enabled': enabled}));
  factory NativeControl.restoreLegacyCamera(String instanceId) {
    if (instanceId.trim().isEmpty || instanceId.length > 2048 ||
        RegExp(r'[\x00-\x1f\x7f]').hasMatch(instanceId)) {
      throw ArgumentError.value(instanceId, 'instanceId');
    }
    return NativeControl._('camera_restore_legacy', Map.unmodifiable({'instance_id': instanceId}));
  }
  final String kind;
  final Map<String, dynamic> parameters;
  Map<String, dynamic> toJson() => {'kind': kind, ...parameters};
  factory NativeControl.fromJson(dynamic value) {
    if (value is! Map<String, dynamic> || value['kind'] is! String) {
      throw const FormatException('Invalid native control');
    }
    final kind = value['kind'] as String;
    if (kind == 'lock_policy') {
      final data = _object(value, {'kind', 'microphone', 'camera', 'restore'});
      return NativeControl.lock(microphone: _bool(data['microphone']),
        camera: _bool(data['camera']), restore: _bool(data['restore']));
    }
    if (kind == 'terminate') {
      final data = _object(value, {'kind', 'pid'});
      return NativeControl.terminate(_int(data['pid'], 5, 0xffffffff));
    }
    if (kind == 'automatic_enforcement') {
      final data = _object(value, {'kind', 'enabled'});
      return NativeControl.enforcement(_bool(data['enabled']));
    }
    if (kind == 'camera_restore_legacy') {
      final data = _object(value, {'kind', 'instance_id'});
      try { return NativeControl.restoreLegacyCamera(_text(data['instance_id'], 2048)); }
      on ArgumentError { throw const FormatException('Invalid camera instance'); }
    }
    _object(value, {'kind'});
    try { return NativeControl.action(kind); }
    on ArgumentError { throw const FormatException('Unknown native control'); }
  }
  @override
  bool operator ==(Object other) => other is NativeControl &&
      kind == other.kind && mapEquals(parameters, other.parameters);
  @override
  int get hashCode => Object.hash(kind, Object.hashAllUnordered(parameters.entries
      .map((entry) => Object.hash(entry.key, entry.value))));
}

class ToolsRequest {
  const ToolsRequest._(this.action, this.parameters, {this.control});
  const ToolsRequest.controls() : this._('controls', const {});
  const ToolsRequest.devices() : this._('devices', const {});
  const ToolsRequest.paths() : this._('paths', const {});
  const ToolsRequest.policy() : this._('policy', const {});
  const ToolsRequest.maintenance() : this._('maintenance', const {});
  const ToolsRequest.clearHistory() : this._('clear_history', const {
    'confirmation': 'clear_only_recorded_history',
  });
  factory ToolsRequest.application(int pid) {
    _int(pid, 2, 0xffffffff);
    return ToolsRequest._('application', Map.unmodifiable({'pid': pid}));
  }
  factory ToolsRequest.validatePolicy(String? path) {
    if (path != null && (path.isEmpty || path.length > 4096 ||
        RegExp(r'[\x00-\x1f\x7f]').hasMatch(path))) {
      throw ArgumentError.value(path, 'path');
    }
    return ToolsRequest._('validate_policy', Map.unmodifiable({'path': path}));
  }
  factory ToolsRequest.prepare(NativeControl control) =>
      ToolsRequest._('prepare', Map<String, dynamic>.unmodifiable({
        'control': Map<String, dynamic>.unmodifiable(control.toJson()),
      }), control: control);
  factory ToolsRequest.commit(NativeReport prepared) {
    if (prepared.outcome != 'prepared' || prepared.token == null) {
      throw ArgumentError('A current native confirmation is required');
    }
    return ToolsRequest._('commit', Map.unmodifiable({'token': prepared.token,
      'confirmation': 'apply_prepared_native_action'}), control: prepared.control);
  }
  final String action;
  final Map<String, dynamic> parameters;
  final NativeControl? control;
  Map<String, dynamic> toJson() => {'action': action, ...parameters};
}

class NativeState {
  NativeState._(this.values);
  final Map<String, dynamic> values;
  String text(String key) => values[key] as String;
  bool flag(String key) => values[key] as bool;
  String? issue(String key) => values[key] as String?;
  factory NativeState.fromJson(dynamic value) {
    const texts = {'microphone', 'microphone_scope', 'camera', 'camera_scope'};
    const issues = {'camera_error', 'microphone_error'};
    const flags = {'can_mute', 'can_restore', 'can_camera', 'can_resume',
      'can_inspect', 'lock_microphone', 'lock_camera', 'restore_on_unlock',
      'lock_supported', 'camera_lock_supported'};
    final data = _object(value, {...texts, ...issues, ...flags,
      'camera_requested', 'camera_service', 'camera_present', 'camera_pending'});
    for (final key in texts) { _text(data[key], 1024); }
    for (final key in issues) {
      if (data[key] != null) _text(data[key], 1024);
    }
    for (final key in flags) { _bool(data[key]); }
    for (final key in ['camera_requested', 'camera_service']) {
      if (data[key] != null) _bool(data[key]);
    }
    for (final key in ['camera_present', 'camera_pending']) {
      if (data[key] != null) _int(data[key], 0, 0x7fffffff);
    }
    return NativeState._(Map.unmodifiable(data));
  }
}

class EnforcementStatus {
  EnforcementStatus._(this.enabled, this.generation, this.accepted, this.failed, this.error);
  final bool enabled;
  final int generation;
  final int accepted;
  final int failed;
  final String? error;
  factory EnforcementStatus.fromJson(dynamic value) {
    final data = _object(value, {'enabled', 'generation', 'accepted_requests',
      'failed_requests', 'last_error', 'scope'});
    _text(data['scope'], 1024);
    return EnforcementStatus._(_bool(data['enabled']), _int(data['generation'], 0, 0x7fffffffffffffff),
      _int(data['accepted_requests'], 0, 0x7fffffffffffffff),
      _int(data['failed_requests'], 0, 0x7fffffffffffffff),
      data['last_error'] == null ? null : _text(data['last_error'], 1024));
  }
}

class NativeReport {
  const NativeReport._(this.outcome, this.state, this.token, this.control,
      this.data, this.error);
  final String outcome;
  final NativeState? state;
  final String? token;
  final NativeControl? control;
  final Map<String, dynamic>? data;
  final String? error;
  EnforcementStatus? get enforcement => data?['enforcement'] == null
      ? null : EnforcementStatus.fromJson(data!['enforcement']);
  factory NativeReport.fromJson(dynamic value) {
    final map = _object(value, {'outcome', 'state', 'token', 'control', 'data', 'error'});
    final outcome = _text(map['outcome'], 32);
    if (!const {'prepared', 'completed', 'not_confirmed'}.contains(outcome)) {
      throw const FormatException('Unknown native result');
    }
    final token = map['token'];
    if (token != null && (token is! String ||
        !RegExp(r'^[0-9a-f]{32}$').hasMatch(token))) {
      throw const FormatException('Invalid native confirmation');
    }
    final state = map['state'] == null ? null : NativeState.fromJson(map['state']);
    final control = map['control'] == null ? null : NativeControl.fromJson(map['control']);
    final error = map['error'] == null ? null : _text(map['error'], 1024);
    if (outcome == 'prepared' && (token == null || state == null ||
        control == null || error != null)) {
      throw const FormatException('Incomplete native preparation');
    }
    if (outcome != 'prepared' && token != null) {
      throw const FormatException('Completed response retains a confirmation');
    }
    if (outcome == 'not_confirmed' && (control == null || error == null)) {
      throw const FormatException('Unconfirmed action lacks its explanation');
    }
    final rawData = map['data'];
    if (rawData != null && rawData is! Map<String, dynamic>) {
      throw const FormatException('Invalid native report data');
    }
    _boundedData(rawData, 0);
    if (rawData is Map<String, dynamic> && rawData['enforcement'] != null) {
      EnforcementStatus.fromJson(rawData['enforcement']);
    }
    return NativeReport._(outcome, state, token, control,
      rawData == null ? null : Map.unmodifiable(rawData as Map<String, dynamic>), error);
  }
}

Map<String, dynamic> _object(dynamic value, Set<String> keys) {
  if (value is! Map<String, dynamic> || value.length != keys.length ||
      !keys.every(value.containsKey)) {
    throw const FormatException('Invalid native object');
  }
  return value;
}
bool _bool(dynamic value) {
  if (value is! bool) throw const FormatException('Invalid native flag');
  return value;
}
int _int(dynamic value, int minimum, int maximum) {
  if (value is! int || value < minimum || value > maximum) {
    throw const FormatException('Invalid native integer');
  }
  return value;
}
String _text(dynamic value, int maximum) {
  if (value is! String || value.length > maximum) {
    throw const FormatException('Invalid native text');
  }
  return value;
}
void _boundedData(dynamic value, int depth) {
  if (depth > 10) throw const FormatException('Native report nesting too deep');
  if (value == null || value is bool || value is num) return;
  if (value is String) { _text(value, 4096); return; }
  if (value is List && value.length <= 128) {
    for (final item in value) { _boundedData(item, depth + 1); }
    return;
  }
  if (value is Map<String, dynamic> && value.length <= 64) {
    for (final entry in value.entries) {
      _text(entry.key, 1024); _boundedData(entry.value, depth + 1);
    }
    return;
  }
  throw const FormatException('Native report exceeds presentation bounds');
}
