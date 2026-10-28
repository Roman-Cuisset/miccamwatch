import 'dart:convert';

/// The complete preference surface. No arbitrary settings keys cross this lane.
enum PreferenceKey {
  notificationsEnabled,
  soundEnabled,
  historyEnabled,
  profile;

  String get wireName => switch (this) {
    notificationsEnabled => 'notifications_enabled',
    soundEnabled => 'sound_enabled',
    historyEnabled => 'history_enabled',
    profile => 'profile',
  };
}

const assessmentProfiles = ['private', 'meeting', 'development', 'balanced'];

bool isPreferenceValue(PreferenceKey key, Object? value) =>
    key == PreferenceKey.profile
        ? value is String && assessmentProfiles.contains(value)
        : value is bool;

/// Immutable native state; construction validates the entire wire object.
class PreferenceView {
  const PreferenceView._({
    required this.available,
    required this.writable,
    required this.profileWritable,
    required this.values,
    required this.appliedProfile,
    required this.effects,
    required this.reason,
    required this.command,
  });

  /// No native preference observation yet; does not imply saved values.
  const PreferenceView.unavailable()
      : available = false,
        writable = false,
        profileWritable = false,
        values = null,
        appliedProfile = null,
        effects = const PreferenceEffects._(false, false, false, false),
        reason = null,
        command = null;

  final bool available;
  final bool writable;
  final bool profileWritable;
  final PreferenceValues? values;
  final String? appliedProfile;
  final PreferenceEffects effects;
  final String? reason;
  final PreferenceOutcome? command;

  factory PreferenceView.fromJson(Object? value) {
    final root = _object(value, const [
      'available', 'writable', 'profile_writable', 'values',
      'applied_profile', 'effects', 'reason', 'command',
    ]);
    final available = _boolean(root['available']);
    final writable = _boolean(root['writable']);
    final profileWritable = _boolean(root['profile_writable']);
    if ((!available && (writable || profileWritable)) ||
        (profileWritable && !writable) ||
        (available != (root['values'] != null))) {
      throw const FormatException('Inconsistent preference availability');
    }
    final applied = root['applied_profile'];
    if (applied != null) _profile(applied);
    return PreferenceView._(
      available: available,
      writable: writable,
      profileWritable: profileWritable,
      values: available ? PreferenceValues._decode(root['values']) : null,
      appliedProfile: applied as String?,
      effects: PreferenceEffects._decode(root['effects']),
      reason: _nullableText(root['reason'], 1024),
      command: root['command'] == null
          ? null : PreferenceOutcome._decode(root['command']),
    );
  }
}

class PreferenceValues {
  const PreferenceValues._(this.notificationsEnabled, this.soundEnabled,
      this.historyEnabled, this.profile);

  final bool notificationsEnabled;
  final bool soundEnabled;
  final bool historyEnabled;
  final String profile;

  factory PreferenceValues._decode(Object? value) {
    final values = _object(value, const [
      'notifications_enabled', 'sound_enabled', 'history_enabled', 'profile',
    ]);
    return PreferenceValues._(
      _boolean(values['notifications_enabled']),
      _boolean(values['sound_enabled']),
      _boolean(values['history_enabled']),
      _profile(values['profile']),
    );
  }

  Object valueFor(PreferenceKey key) => switch (key) {
    PreferenceKey.notificationsEnabled => notificationsEnabled,
    PreferenceKey.soundEnabled => soundEnabled,
    PreferenceKey.historyEnabled => historyEnabled,
    PreferenceKey.profile => profile,
  };
}

class PreferenceEffects {
  const PreferenceEffects._(this.captureNotificationsSupported,
      this.captureSoundsSupported, this.historyRecordingSupported,
      this.historyRecordingActive);

  final bool captureNotificationsSupported;
  final bool captureSoundsSupported;
  final bool historyRecordingSupported;
  final bool historyRecordingActive;

  factory PreferenceEffects._decode(Object? value) {
    final effects = _object(value, const [
      'capture_notifications_supported', 'capture_sounds_supported',
      'history_recording_supported', 'history_recording_active',
    ]);
    final supported = _boolean(effects['history_recording_supported']);
    final active = _boolean(effects['history_recording_active']);
    if (active && !supported) {
      throw const FormatException('History active without recording support');
    }
    return PreferenceEffects._(
      _boolean(effects['capture_notifications_supported']),
      _boolean(effects['capture_sounds_supported']),
      supported,
      active,
    );
  }
}

class PreferenceOutcome {
  const PreferenceOutcome._(this.id, this.key, this.value, this.state,
      this.code, this.detail);

  final int id;
  final PreferenceKey key;
  final Object value;
  final String state;
  final String? code;
  final String? detail;

  factory PreferenceOutcome._decode(Object? value) {
    final result = _object(value, const [
      'id', 'key', 'value', 'state', 'code', 'detail',
    ]);
    final id = result['id'];
    if (id is! int || id < 1 || id > 0xffffffff) {
      throw const FormatException('Invalid preference command ID');
    }
    final keys = PreferenceKey.values.where(
        (key) => key.wireName == result['key']);
    if (keys.isEmpty) throw const FormatException('Unknown preference key');
    final key = keys.single;
    if (!isPreferenceValue(key, result['value'])) {
      throw const FormatException('Invalid preference command value');
    }
    final state = result['state'];
    if (state is! String || !const [
      'accepted', 'succeeded', 'failed', 'rejected', 'uncertain',
    ].contains(state)) {
      throw const FormatException('Unknown preference command state');
    }
    return PreferenceOutcome._(id, key, result['value'] as Object, state,
      _nullableText(result['code'], 64),
      _nullableText(result['detail'], 1024));
  }
}

Map<String, dynamic> _object(Object? value, List<String> keys) {
  if (value is! Map<String, dynamic> || value.length != keys.length ||
      keys.any((key) => !value.containsKey(key))) {
    throw const FormatException('Invalid preference object shape');
  }
  return value;
}

bool _boolean(Object? value) {
  if (value is! bool) throw const FormatException('Invalid preference boolean');
  return value;
}

String _profile(Object? value) {
  if (value is! String || !assessmentProfiles.contains(value)) {
    throw const FormatException('Unknown assessment profile');
  }
  return value;
}

String? _nullableText(Object? value, int maximum) {
  if (value == null) return null;
  if (value is! String || utf8.encode(value).length > maximum) {
    throw const FormatException('Invalid bounded preference text');
  }
  return value;
}
