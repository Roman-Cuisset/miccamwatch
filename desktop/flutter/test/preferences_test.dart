import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/preferences.dart';
import 'package:miccamwatch_desktop/preferences_model.dart';
import 'package:miccamwatch_desktop/snapshot.dart' show protocolVersion;

const sessionA = '0123456789abcdef0123456789abcdef';
const sessionB = 'abcdef0123456789abcdef0123456789';

Map<String, dynamic> preferenceJson({
  bool available = true,
  bool writable = true,
  bool profileWritable = true,
  bool notifications = true,
  String? pauseUntil,
  bool paused = false,
  bool sound = false,
  bool history = true,
  String profile = 'balanced',
  String? applied = 'balanced',
  bool deliverySupported = false,
  bool historySupported = false,
  bool historyActive = false,
  Map<String, dynamic>? command,
}) => <String, dynamic>{
  'available': available,
  'writable': writable,
  'profile_writable': profileWritable,
  'values': available ? <String, dynamic>{
    'notifications_enabled': notifications,
    'sound_enabled': sound,
    'history_enabled': history,
    'profile': profile,
    'pause_notifications_until': pauseUntil,
    'notifications_paused': paused,
  } : null,
  'applied_profile': applied,
  'effects': <String, dynamic>{
    'capture_notifications_supported': deliverySupported,
    'capture_sounds_supported': deliverySupported,
    'history_recording_supported': historySupported,
    'history_recording_active': historyActive,
  },
  'reason': null,
  'command': command,
};

Map<String, dynamic> outcome(String state, {
  int id = 1,
  PreferenceKey key = PreferenceKey.historyEnabled,
  Object value = false,
}) => <String, dynamic>{
  'id': id, 'key': key.wireName, 'value': value,
  'state': state, 'code': null, 'detail': null,
};

PreferenceView view({
  bool history = true,
  bool notifications = true,
  String? pauseUntil,
  bool paused = false,
  String profile = 'balanced',
  String? applied = 'balanced',
  Map<String, dynamic>? command,
}) => PreferenceView.fromJson(preferenceJson(
  history: history, notifications: notifications, pauseUntil: pauseUntil,
  paused: paused, profile: profile, applied: applied, command: command));

final _clients = <PreferenceCommandClient>{};
void disposeClient(PreferenceCommandClient client) {
  if (_clients.remove(client)) client.dispose();
}

PreferenceCommandClient makeClient(List<Map<String, dynamic>> sent, {
  Future<void> Function(Map<String, dynamic>)? send,
  Duration timeout = const Duration(seconds: 5),
}) {
  final client = PreferenceCommandClient(
    send: send ?? (message) async { sent.add(message); }, timeout: timeout);
  _clients.add(client);
  addTearDown(() => disposeClient(client));
  return client;
}

Widget host(PreferenceCommandClient client, {String locale = 'en'}) => MaterialApp(
  theme: ThemeData(useMaterial3: true),
  home: Scaffold(body: SingleChildScrollView(child: PreferencesPanel(
    client: client, locale: locale))),
);

void main() {
  group('strict native preference boundary', () {
    test('malformed and extra fields never become editable defaults', () {
      final mutations = <void Function(Map<String, dynamic>)>[
        (json) => json['unknown_setting'] = false,
        (json) => json.remove('reason'),
        (json) => json['available'] = 1,
        (json) => json['writable'] = 'true',
        (json) => json['profile_writable'] = null,
        (json) => json['values'] = null,
        (json) => (json['values'] as Map)['history_enabled'] = 'false',
        (json) => (json['values'] as Map)['sound_enabled'] = 0,
        (json) => (json['values'] as Map)['notifications_enabled'] = null,
        (json) => (json['values'] as Map)['notifications_paused'] = 'true',
        (json) => (json['values'] as Map).remove('notifications_paused'),
        (json) => (json['values'] as Map).remove('pause_notifications_until'),
        (json) => (json['values'] as Map)['pause_notifications_until'] = 30,
        (json) => (json['values'] as Map)['pause_notifications_until'] = 'tomorrow',
        (json) => (json['values'] as Map)['pause_notifications_until'] = '2026-10-30T10:00:00',
        (json) => (json['values'] as Map)['pause_notifications_until'] = '2026-10-30T10:00:00+01:00',
        (json) => (json['values'] as Map)['pause_notifications_until'] = '2026-02-30T10:00:00Z',
        (json) => (json['values'] as Map)['pause_notifications_until'] = '2026-10-30T24:00:00Z',
        (json) => (json['values'] as Map)['profile'] = true,
        (json) => (json['values'] as Map)['profile'] = 'aggressive',
        (json) => (json['values'] as Map)['autostart'] = true,
        (json) => (json['values'] as Map).remove('sound_enabled'),
        (json) => json['effects'] = <String, dynamic>{},
        (json) => (json['effects'] as Map)['capture_sounds_supported'] = 1,
        (json) => (json['effects'] as Map)['camera_supported'] = true,
        (json) => json['applied_profile'] = 'unknown',
        (json) => json['reason'] = 42,
        (json) => json['reason'] = List.filled(513, 'é').join(),
      ];
      for (final mutate in mutations) {
        final json = preferenceJson();
        mutate(json);
        expect(() => PreferenceView.fromJson(json), throwsFormatException);
      }
      for (final value in [null, [], false, 'settings']) {
        expect(() => PreferenceView.fromJson(value), throwsFormatException);
      }
    });

    test('inconsistent native capability claims are rejected', () {
      final invalid = [
        preferenceJson(available: false),
        preferenceJson(writable: false, profileWritable: true),
        preferenceJson(historySupported: false, historyActive: true),
        {...preferenceJson(), 'available': false},
      ];
      for (final json in invalid) {
        expect(() => PreferenceView.fromJson(json), throwsFormatException);
      }
    });

    test('cross-key command values and malformed results cannot complete work', () {
      for (final key in PreferenceKey.values) {
        final bad = key == PreferenceKey.profile ? true : 'balanced';
        expect(() => PreferenceView.fromJson(preferenceJson(command:
          outcome('succeeded', key: key, value: bad))), throwsFormatException);
      }
      final mutations = <void Function(Map<String, dynamic>)>[
        (result) => result['id'] = 0,
        (result) => result['id'] = 0x100000000,
        (result) => result['id'] = 1.0,
        (result) => result['key'] = 'camera_enabled',
        (result) => result['state'] = 'done',
        (result) => result['value'] = null,
        (result) => result['extra'] = true,
        (result) => result.remove('code'),
        (result) => result['code'] = List.filled(33, 'é').join(),
        (result) => result['detail'] = List.filled(513, 'é').join(),
      ];
      for (final mutate in mutations) {
        final result = outcome('succeeded');
        mutate(result);
        expect(() => PreferenceView.fromJson(preferenceJson(command: result)),
          throwsFormatException);
      }
      expect(() => PreferenceView.fromJson(preferenceJson(command:
        outcome('accepted', key: PreferenceKey.profile, value: 'default'))),
        throwsFormatException);
    });
  });

  test('unknown, unavailable, read-only and invalid inputs never send', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(view(), '', 1, true);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(view(), sessionA, 0, true);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(const PreferenceView.unavailable(), sessionA, 1, true);
    expect(client.canChange(PreferenceKey.historyEnabled), isFalse);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(PreferenceView.fromJson(preferenceJson(
      writable: false, profileWritable: false)), sessionA, 2, true);
    for (final key in PreferenceKey.values) {
      expect(client.canChange(key), isFalse);
      await client.request(key, key == PreferenceKey.profile ? 'private' : false);
    }
    client.update(view(), sessionA, 3, false);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(view(), sessionA, 3, true);
    await client.request(PreferenceKey.historyEnabled, 'false');
    await client.request(PreferenceKey.profile, true);
    await client.request(PreferenceKey.profile, 'unknown');
    for (final value in [null, true, 30, 'pause', 'pause_60_minutes',
        '2026-10-30T10:00:00Z']) {
      if (value != null) {
        await client.request(PreferenceKey.notificationPause, value);
      }
      expect(isPreferenceValue(PreferenceKey.notificationPause, value), isFalse);
    }
    expect(client.pending, isFalse);
    expect(sent, isEmpty);
    expect(client.view!.values!.historyEnabled, isTrue);
  });

  test('native profile override gates only profile; one pending gates all fields', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(PreferenceView.fromJson(preferenceJson(profileWritable: false)),
      sessionA, 1, true);
    expect(client.canChange(PreferenceKey.profile), isFalse);
    expect(client.canChange(PreferenceKey.soundEnabled), isTrue);
    await client.request(PreferenceKey.profile, 'private');
    expect(sent, isEmpty);
    await client.request(PreferenceKey.soundEnabled, true);
    for (final key in PreferenceKey.values) {
      expect(client.canChange(key), isFalse);
    }
    await client.request(PreferenceKey.historyEnabled, false);
    expect(sent, hasLength(1));
    expect(client.view!.values!.soundEnabled, isFalse);
    expect(client.pendingKey, PreferenceKey.soundEnabled);
  });

  test('acceptance and success do not fabricate saved or applied values', () async {
    final client = makeClient([]);
    client.update(view(), sessionA, 1, true);
    await client.request(PreferenceKey.profile, 'private');
    expect(client.view!.values!.profile, 'balanced');
    client.update(view(command: outcome('accepted',
      key: PreferenceKey.profile, value: 'private')), sessionA, 2, true);
    expect(client.pending, isTrue);
    expect(client.state, 'accepted');
    expect(client.view!.values!.profile, 'balanced');
    client.update(view(profile: 'private', applied: 'balanced', command:
      outcome('succeeded', key: PreferenceKey.profile, value: 'private')),
      sessionA, 3, true);
    expect(client.pending, isFalse);
    expect(client.state, 'succeeded');
    expect(client.view!.values!.profile, 'private');
    expect(client.view!.appliedProfile, 'balanced');
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(view(profile: 'private', command:
      outcome('succeeded', id: 2)), sessionA, 4, true);
    expect(client.view!.values!.historyEnabled, isTrue);
  });

  test('CAS uses the newest authoritative value despite reordered snapshots', () async {
    // A minimal native compare-and-set peer rejects stale expected values.
    // This tests the outcome of concurrent storage changes, not field echoes.
    var nativeHistory = false;
    var sequence = 12;
    late PreferenceCommandClient client;
    client = makeClient([], send: (message) async {
      final request = message['request'] as Map<String, dynamic>;
      final matched = request['expected_value'] == nativeHistory;
      if (matched) nativeHistory = request['value'] as bool;
      client.update(view(history: nativeHistory, command: outcome(
        matched ? 'succeeded' : 'rejected', id: request['id'] as int,
        value: request['value'] as Object)), sessionA, ++sequence, true);
    });
    client.update(view(history: true), sessionA, 10, true);
    client.update(view(history: false), sessionA, 12, true);
    client.update(view(history: true), sessionA, 11, true);
    await client.request(PreferenceKey.historyEnabled, true);
    expect(client.state, 'succeeded');
    expect(nativeHistory, isTrue);
    // Storage changed without a snapshot: never blind-overwrite that field.
    nativeHistory = false;
    await client.request(PreferenceKey.historyEnabled, false);
    expect(client.state, 'rejected');
    expect(client.view!.values!.historyEnabled, isFalse);
    expect(nativeHistory, isFalse);
    await client.request(PreferenceKey.historyEnabled, true);
    expect(client.state, 'succeeded');
    expect(nativeHistory, isTrue);
  });

  test('ID, key, value and newer sequence must all match before completion', () async {
    final client = makeClient([]);
    client.update(view(), sessionA, 5, true);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(view(history: false, command: outcome('succeeded')),
      sessionA, 5, true);
    expect(client.view!.values!.historyEnabled, isTrue);
    expect(client.pending, isTrue);
    client.update(view(command: outcome('succeeded', id: 2)), sessionA, 6, true);
    client.update(view(command: outcome('succeeded',
      key: PreferenceKey.notificationsEnabled)), sessionA, 7, true);
    client.update(view(command: outcome('succeeded', value: true)),
      sessionA, 8, true);
    expect(client.pending, isTrue);
    expect(client.state, 'pending');
    client.update(view(history: false, command: outcome('succeeded')),
      sessionA, 9, true);
    expect(client.pending, isFalse);
    expect(client.view!.values!.historyEnabled, isFalse);
    await client.request(PreferenceKey.historyEnabled, true);
    expect(client.pendingId, 2);
    // A replayed completed result cannot complete the next request.
    client.update(view(command: outcome('succeeded')), sessionA, 10, true);
    expect(client.pending, isTrue);
  });

  for (final state in ['failed', 'rejected', 'uncertain']) {
    test('native $state leaves values authoritative and releases only its ticket', () async {
      final client = makeClient([]);
      client.update(view(), sessionA, 1, true);
      await client.request(PreferenceKey.historyEnabled, false);
      client.update(view(command: outcome(state)), sessionA, 2, true);
      expect(client.state, state);
      expect(client.pending, isFalse);
      expect(client.view!.values!.historyEnabled, isTrue);
      await client.request(PreferenceKey.historyEnabled, false);
      expect(client.pendingId, 2);
    });
  }

  test('an inherited native result cannot masquerade as a new completion', () async {
    final client = makeClient([]);
    client.update(view(command: outcome('succeeded', id: 7)), sessionA, 1, true);
    expect(client.state, isNull);
    await client.request(PreferenceKey.historyEnabled, false);
    client.update(view(command: outcome('succeeded', id: 7)), sessionA, 2, true);
    expect(client.pending, isTrue);
    expect(client.state, 'pending');
    client.update(view(history: false, command: outcome('succeeded', id: 8)),
      sessionA, 3, true);
    expect(client.pending, isFalse);
    expect(client.view!.values!.historyEnabled, isFalse);
  });

  test('exhausted session cannot wrap IDs; a new session restores admission', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(view(command: outcome('succeeded', id: 0xffffffff)),
      sessionA, 1, true);
    await client.request(PreferenceKey.historyEnabled, false);
    expect(client.canChange(PreferenceKey.historyEnabled), isFalse);
    expect(sent, isEmpty);
    client.update(view(), sessionB, 1, true);
    await client.request(PreferenceKey.historyEnabled, false);
    expect(client.pending, isTrue);
    expect(sent, hasLength(1));
  });

  test('stale disconnect disables controls; stale reconnect cannot revive them', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(view(), sessionA, 20, true);
    client.update(view(), sessionA, 19, false);
    expect(client.canChange(PreferenceKey.historyEnabled), isFalse);
    client.update(view(), sessionA, 19, true);
    await client.request(PreferenceKey.historyEnabled, false);
    expect(sent, isEmpty);
    client.update(view(history: false), sessionA, 20, true);
    expect(client.view!.values!.historyEnabled, isTrue);
    expect(client.canChange(PreferenceKey.historyEnabled), isTrue);
  });

  testWidgets('timeout retains ticket, ignores acceptance and never retries', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent, timeout: const Duration(seconds: 2));
    client.update(view(), sessionA, 1, true);
    await client.request(PreferenceKey.historyEnabled, false);
    await tester.pump(const Duration(seconds: 2));
    expect(client.state, 'uncertain');
    expect(client.pending, isTrue);
    expect(client.view!.values!.historyEnabled, isTrue);
    client.update(view(command: outcome('accepted')), sessionA, 2, true);
    expect(client.state, 'uncertain');
    await client.request(PreferenceKey.soundEnabled, true);
    await tester.pump(const Duration(seconds: 30));
    expect(sent, hasLength(1));
    expect(client.pending, isTrue);
    client.update(view(history: false, command: outcome('succeeded')),
      sessionA, 3, true);
    expect(client.state, 'succeeded');
    expect(client.pending, isFalse);
    expect(client.error, isNull);
  });

  testWidgets('transport uncertainty survives navigation without resend', (tester) async {
    var deliveries = 0;
    final client = makeClient([], send: (_) async {
      deliveries++;
      throw StateError('pipe disconnected after write');
    });
    client.update(view(), sessionA, 1, true);
    await tester.pumpWidget(host(client));
    await client.request(PreferenceKey.historyEnabled, false);
    expect(client.state, 'uncertain');
    expect(client.pending, isTrue);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 30));
    await tester.pumpWidget(host(client));
    expect(deliveries, 1);
    expect(client.pending, isTrue);
    expect(tester.widgetList<SwitchListTile>(find.byType(SwitchListTile))
      .every((tile) => tile.onChanged == null), isTrue);
    client.update(view(history: false, command: outcome('succeeded')),
      sessionA, 2, true);
    await tester.pump();
    expect(client.pending, isFalse);
    expect(deliveries, 1);
  });

  testWidgets('new session retires old results and old asynchronous send failures', (tester) async {
    final delivery = Completer<void>();
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent, send: (message) {
      sent.add(message);
      return sent.length == 1 ? delivery.future : Future<void>.value();
    });
    client.update(view(), sessionA, 9, true);
    final oldRequest = client.request(PreferenceKey.historyEnabled, false);
    client.update(view(), sessionB, 1, true);
    await client.request(PreferenceKey.historyEnabled, false);
    expect(client.pendingId, 1);
    delivery.completeError(StateError('old transport'));
    await oldRequest;
    expect(client.state, 'pending');
    client.update(view(history: false, command: outcome('succeeded')),
      sessionA, 99, true);
    expect(client.session, sessionB);
    expect(client.pending, isTrue);
    expect(client.view!.values!.historyEnabled, isTrue);
    client.update(view(history: false, command: outcome('succeeded')),
      sessionB, 2, true);
    expect(client.pending, isFalse);
    await tester.pump(const Duration(seconds: 30));
    expect(client.state, 'succeeded');
    expect(sent, hasLength(2));
  });

  testWidgets('disposal cancels timeout and ignores late transport completion', (tester) async {
    final delivery = Completer<void>();
    var notifications = 0;
    var deliveries = 0;
    final client = makeClient([], send: (_) {
      deliveries++;
      return delivery.future;
    });
    client.update(view(), sessionA, 1, true);
    client.addListener(() { notifications++; });
    final request = client.request(PreferenceKey.historyEnabled, false);
    final beforeDispose = notifications;
    disposeClient(client);
    delivery.completeError(StateError('late error'));
    await request;
    await tester.pump(const Duration(seconds: 30));
    client.update(view(history: false, command: outcome('succeeded')),
      sessionA, 2, true);
    await client.request(PreferenceKey.soundEnabled, true);
    expect(notifications, beforeDispose);
    expect(deliveries, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('standard controls keep confirmed switch and profile values while pending', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(view(), sessionA, 1, true);
    await tester.pumpWidget(host(client));
    final toggles = tester.widgetList<SwitchListTile>(find.byType(SwitchListTile));
    expect(toggles, hasLength(3));
    expect(toggles.every((tile) => tile.onChanged != null), isTrue);
    final picker = find.byType(DropdownButton<String>);
    await tester.ensureVisible(picker);
    await tester.tap(picker);
    await tester.pumpAndSettle();
    final privateItem = find.byWidgetPredicate((widget) =>
      widget is DropdownMenuItem<String> && widget.value == 'private');
    await tester.tap(find.descendant(of: privateItem,
      matching: find.byType(Text)).hitTestable().last);
    await tester.pumpAndSettle();
    expect(tester.widget<DropdownButton<String>>(picker).value, 'balanced');
    expect(tester.widget<DropdownButton<String>>(picker).onChanged, isNull);
    expect(tester.widgetList<SwitchListTile>(find.byType(SwitchListTile))
      .every((tile) => tile.onChanged == null), isTrue);
    expect(sent, hasLength(1));
    client.update(view(profile: 'private', applied: 'balanced', command:
      outcome('succeeded', key: PreferenceKey.profile, value: 'private')),
      sessionA, 2, true);
    await tester.pump();
    expect(tester.widget<DropdownButton<String>>(picker).value, 'private');
    expect(client.view!.appliedProfile, 'balanced');
  });

  testWidgets('all locales expose unsupported delivery and native capability gates', (tester) async {
    final client = makeClient([]);
    client.update(PreferenceView.fromJson(preferenceJson(profileWritable: false)),
      sessionA, 1, true);
    for (final locale in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
      await tester.pumpWidget(host(client, locale: locale));
      expect(find.byKey(const ValueKey('preferences.notificationsUnsupported')),
        findsOneWidget);
      expect(find.byKey(const ValueKey('preferences.soundsUnsupported')),
        findsOneWidget);
      expect(find.byKey(const ValueKey('preferences.historyStatus')), findsOneWidget);
      expect(tester.widget<DropdownButton<String>>(
        find.byType(DropdownButton<String>)).onChanged, isNull);
      expect(tester.widgetList<SwitchListTile>(find.byType(SwitchListTile))
        .every((tile) => tile.onChanged != null), isTrue);
      expect(tester.takeException(), isNull);
    }
    client.update(PreferenceView.fromJson(preferenceJson(deliverySupported: true,
      historySupported: true, historyActive: true)), sessionA, 2, true);
    await tester.pump();
    expect(find.byKey(const ValueKey('preferences.notificationsUnsupported')),
      findsNothing);
    expect(find.byKey(const ValueKey('preferences.soundsUnsupported')), findsNothing);
  });

  group('native notification pause contract', () {
    const original = '2026-10-30T10:30:00.123456789+00:00';
    const changed = '2026-10-30T11:00:00Z';

    test('pause and resume send actions and exact nullable native CAS values', () async {
      final sent = <Map<String, dynamic>>[];
      final client = makeClient(sent);
      client.update(view(), sessionA, 1, true);
      await client.request(PreferenceKey.notificationPause, 'pause_30_minutes');
      expect(sent.single, {
        'type': 'preferences_command',
        'protocol_version': protocolVersion,
        'request': {
          'session': sessionA, 'id': 1, 'action': 'set_preference',
          'observed_sequence': 1, 'key': 'notification_pause',
          'value': 'pause_30_minutes', 'expected_value': null,
        },
      });
      expect(client.view!.values!.pauseNotificationsUntil, isNull);
      expect(client.view!.values!.notificationsPaused, isFalse);
      client.update(view(command: outcome('accepted',
        key: PreferenceKey.notificationPause, value: 'pause_30_minutes')),
        sessionA, 2, true);
      expect(client.pending, isTrue);
      expect(client.view!.values!.notificationsPaused, isFalse);
      client.update(view(pauseUntil: original, paused: true, command:
        outcome('succeeded', key: PreferenceKey.notificationPause,
          value: 'pause_30_minutes')), sessionA, 3, true);
      expect(client.pending, isFalse);
      expect(client.view!.values!.notificationsPaused, isTrue);
      await client.request(PreferenceKey.notificationPause, 'resume');
      expect(sent.last['request'], {
        'session': sessionA, 'id': 2, 'action': 'set_preference',
        'observed_sequence': 3, 'key': 'notification_pause',
        'value': 'resume', 'expected_value': original,
      });
      // Even a matching success result cannot synthesize cleared native values.
      client.update(view(pauseUntil: original, paused: true, command:
        outcome('succeeded', id: 2, key: PreferenceKey.notificationPause,
          value: 'resume')), sessionA, 4, true);
      expect(client.view!.values!.pauseNotificationsUntil, original);
      expect(client.view!.values!.notificationsPaused, isTrue);
      client.update(view(), sessionA, 5, true);
      expect(client.view!.values!.pauseNotificationsUntil, isNull);
      expect(client.view!.values!.notificationsPaused, isFalse);
    });

    test('concurrent native pause is not cleared or extended by stale requests', () async {
      for (final action in ['resume', 'pause_30_minutes']) {
        var nativeDeadline = changed;
        var sequence = 3;
        late PreferenceCommandClient client;
        client = makeClient([], send: (message) async {
          final request = message['request'] as Map<String, dynamic>;
          final matched = request['expected_value'] == nativeDeadline;
          if (matched) {
            nativeDeadline = action == 'resume' ? '' : original;
          }
          client.update(view(
            pauseUntil: nativeDeadline.isEmpty ? null : nativeDeadline,
            paused: nativeDeadline.isNotEmpty,
            command: outcome(matched ? 'succeeded' : 'rejected',
              id: request['id'] as int, key: PreferenceKey.notificationPause,
              value: request['value'] as Object)),
            sessionA, ++sequence, true);
        });
        client.update(view(pauseUntil: original, paused: true), sessionA, 2, true);
        client.update(view(), sessionA, 1, true);
        await client.request(PreferenceKey.notificationPause, action);
        expect(client.state, 'rejected');
        expect(nativeDeadline, changed);
        expect(client.view!.values!.pauseNotificationsUntil, changed);
        // The next explicit action uses the newly observed timestamp.
        await client.request(PreferenceKey.notificationPause, action);
        expect(client.state, 'succeeded');
        expect(client.view!.values!.pauseNotificationsUntil,
          action == 'resume' ? isNull : original);
      }
    });

    test('new and retired sessions cannot inherit pause completion', () async {
      final client = makeClient([]);
      client.update(view(pauseUntil: original, paused: true), sessionA, 8, true);
      await client.request(PreferenceKey.notificationPause, 'resume');
      client.update(view(pauseUntil: changed, paused: true), sessionB, 1, true);
      expect(client.state, isNull);
      await client.request(PreferenceKey.notificationPause, 'resume');
      client.update(view(command: outcome('succeeded',
        key: PreferenceKey.notificationPause, value: 'resume')),
        sessionA, 99, true);
      expect(client.session, sessionB);
      expect(client.pending, isTrue);
      expect(client.view!.values!.pauseNotificationsUntil, changed);
      // Wrong action from the current session is not a matching result either.
      client.update(view(pauseUntil: changed, paused: true, command:
        outcome('succeeded', key: PreferenceKey.notificationPause,
          value: 'pause_30_minutes')), sessionB, 2, true);
      expect(client.pending, isTrue);
      client.update(view(command: outcome('succeeded',
        key: PreferenceKey.notificationPause, value: 'resume')),
        sessionB, 3, true);
      expect(client.pending, isFalse);
      expect(client.view!.values!.pauseNotificationsUntil, isNull);
    });

    for (final state in ['failed', 'rejected', 'uncertain']) {
      testWidgets('$state is visible without optimistic resume', (tester) async {
        final client = makeClient([]);
        client.update(view(pauseUntil: original, paused: true), sessionA, 1, true);
        await tester.pumpWidget(host(client));
        await tester.tap(find.byKey(const ValueKey('preferences.resume')));
        await tester.pump();
        expect(client.view!.values!.notificationsPaused, isTrue);
        expect(tester.widget<OutlinedButton>(
          find.byKey(const ValueKey('preferences.resume'))).onPressed, isNull);
        client.update(view(pauseUntil: original, paused: true, command:
          outcome(state, key: PreferenceKey.notificationPause, value: 'resume')),
          sessionA, 2, true);
        await tester.pump();
        expect(client.state, state);
        expect(find.textContaining(original), findsOneWidget);
        final status = tester.widget<Semantics>(
          find.byKey(const ValueKey('preferences.commandStatus')));
        expect(status.child, isA<Text>());
        expect((status.child! as Text).style!.color,
          Theme.of(tester.element(find.byType(PreferencesPanel))).colorScheme.error);
        expect(client.view!.values!.notificationsPaused, isTrue);
      });
    }

    testWidgets('pause timeout and delivery failure stay uncertain without retry',
        (tester) async {
      for (final fails in [false, true]) {
        var deliveries = 0;
        final client = makeClient([], timeout: const Duration(seconds: 2),
          send: (_) async {
            deliveries++;
            if (fails) throw StateError('closed pipe');
          });
        client.update(view(), sessionA, 1, true);
        await tester.pumpWidget(host(client));
        await tester.tap(find.byKey(const ValueKey('preferences.pause')));
        await tester.pump(const Duration(seconds: 3));
        expect(client.state, 'uncertain');
        expect(client.pending, isTrue);
        expect(client.view!.values!.notificationsPaused, isFalse);
        expect(tester.widget<FilledButton>(
          find.byKey(const ValueKey('preferences.pause'))).onPressed, isNull);
        await tester.pump(const Duration(minutes: 31));
        expect(deliveries, 1);
        client.update(view(command: outcome('accepted',
          key: PreferenceKey.notificationPause, value: 'pause_30_minutes')),
          sessionA, 2, true);
        expect(client.state, 'uncertain');
        disposeClient(client);
        await tester.pumpWidget(const SizedBox());
      }
    });

    testWidgets('only native snapshots expire pauses, including restored old timestamps',
        (tester) async {
      final client = makeClient([]);
      client.update(view(pauseUntil: original, paused: true), sessionA, 1, true);
      await tester.pumpWidget(host(client));
      final activeStatus = tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.pauseStatus')),
        matching: find.byType(Text))).data;
      await tester.pump(const Duration(days: 365));
      expect(client.view!.values!.notificationsPaused, isTrue);
      expect(tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.pauseStatus')),
        matching: find.byType(Text))).data, activeStatus);
      client.update(view(pauseUntil: original, paused: false), sessionA, 2, true);
      await tester.pump();
      expect(tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.pauseStatus')),
        matching: find.byType(Text))).data, isNot(activeStatus));
      // Expired persisted timestamps can still be explicitly cleared.
      expect(tester.widget<OutlinedButton>(
        find.byKey(const ValueKey('preferences.resume'))).onPressed, isNotNull);
      expect(find.textContaining(original), findsOneWidget);
    });

    testWidgets('master off takes precedence and resume never enables alerts',
        (tester) async {
      final sent = <Map<String, dynamic>>[];
      final client = makeClient(sent);
      client.update(view(notifications: false, pauseUntil: original, paused: true),
        sessionA, 1, true);
      await tester.pumpWidget(host(client));
      expect(tester.widget<FilledButton>(
        find.byKey(const ValueKey('preferences.pause'))).onPressed, isNull);
      final offStatus = tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.pauseStatus')),
        matching: find.byType(Text))).data;
      await tester.tap(find.byKey(const ValueKey('preferences.resume')));
      await tester.pump();
      client.update(view(notifications: false, command: outcome('succeeded',
        key: PreferenceKey.notificationPause, value: 'resume')),
        sessionA, 2, true);
      await tester.pump();
      expect(sent.single['request']['value'], 'resume');
      expect(client.view!.values!.notificationsEnabled, isFalse);
      expect(tester.widgetList<SwitchListTile>(find.byType(SwitchListTile))
        .first.value, isFalse);
      expect(tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.pauseStatus')),
        matching: find.byType(Text))).data, offStatus);
      expect(tester.widget<OutlinedButton>(
        find.byKey(const ValueKey('preferences.resume'))).onPressed, isNull);
    });

    testWidgets('seven translated narrow layouts preserve native state and gates',
        (tester) async {
      tester.view.physicalSize = const Size(320, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final client = makeClient([]);
      var sequence = 0;
      for (final locale in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
        client.update(view(pauseUntil: original, paused: true),
          sessionA, ++sequence, true);
        await tester.pumpWidget(host(client, locale: locale));
        final pause = find.byKey(const ValueKey('preferences.pause'));
        final resume = find.byKey(const ValueKey('preferences.resume'));
        await tester.ensureVisible(resume);
        expect(tester.widget<OutlinedButton>(resume).onPressed, isNotNull);
        client.update(view(pauseUntil: original, paused: true),
          sessionA, ++sequence, false);
        await tester.pump();
        expect(tester.widget<OutlinedButton>(resume).onPressed, isNull);
        expect(tester.widget<FilledButton>(pause).onPressed, isNull);
        expect(find.byKey(const ValueKey('preferences.notificationsUnsupported')),
          findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    });

    testWidgets('keyboard activates pause once and pending disables both actions',
        (tester) async {
      final sent = <Map<String, dynamic>>[];
      final client = makeClient(sent);
      client.update(view(), sessionA, 1, true);
      await tester.pumpWidget(host(client));
      bool pauseFocused() {
        var focused = false;
        FocusManager.instance.primaryFocus?.context?.visitAncestorElements((element) {
          if (element.widget is FilledButton &&
              element.widget.key == const ValueKey('preferences.pause')) {
            focused = true;
          }
          return !focused;
        });
        return focused;
      }
      for (var step = 0; step < 12 && !pauseFocused(); step++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
      }
      expect(pauseFocused(), isTrue);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(sent, hasLength(1));
      expect(sent.single['request']['value'], 'pause_30_minutes');
      expect(client.view!.values!.notificationsPaused, isFalse);
      expect(tester.widget<FilledButton>(
        find.byKey(const ValueKey('preferences.pause'))).onPressed, isNull);
      expect(tester.widget<OutlinedButton>(
        find.byKey(const ValueKey('preferences.resume'))).onPressed, isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      disposeClient(client);
      await tester.pump();
    });
  });
}
