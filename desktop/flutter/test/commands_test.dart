import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/commands.dart';
import 'package:miccamwatch_desktop/snapshot.dart' show Snapshot;

import 'snapshot_test.dart' as snapshots show frame, encode;

const sessionA = '0123456789abcdef0123456789abcdef';
const sessionB = 'abcdef0123456789abcdef0123456789';

final _clients = <MicrophoneCommandClient>{};
void disposeClient(MicrophoneCommandClient client) {
  if (_clients.remove(client)) client.dispose();
}

Map<String, dynamic> controls({
  String session = sessionA,
  bool enable = true,
  bool release = true,
  bool? requested = false,
  int generation = 0,
  String? reason,
  Map<String, dynamic>? command,
}) => {
  'session': session,
  'microphone': <String, dynamic>{
    'can_enable': enable,
    'can_release': release,
    'current_requested': requested,
    'generation': generation,
    'reason': reason,
  },
  'command': command,
};

Map<String, dynamic> result(String state, {
  int id = 1,
  bool requested = true,
  String? code,
  String? detail,
}) => {
  'id': id, 'requested': requested, 'state': state,
  'code': code, 'detail': detail,
};

MicrophoneCommandClient makeClient(List<Map<String, dynamic>> sent, {
  Duration timeout = const Duration(seconds: 5),
  Future<void> Function(Map<String, dynamic>)? send,
}) {
  final client = MicrophoneCommandClient(
    send: send ?? (message) async { sent.add(message); },
    timeout: timeout,
  );
  _clients.add(client);
  addTearDown(() => disposeClient(client));
  return client;
}

Snapshot commandSnapshot({
  String platform = 'windows',
  String locale = 'en',
  String mode = 'controlled_development',
  int sequence = 1,
  Map<String, dynamic>? commandControls,
}) {
  final value = snapshots.frame(sequence: sequence);
  value['locale'] = locale;
  value['engine']['platform'] = platform;
  value['engine']['mode'] = mode;
  value['controls'] = commandControls ?? controls();
  return Snapshot.decode(snapshots.encode(value));
}

Widget host(MicrophoneCommandClient client, {
  String locale = 'en',
  Snapshot? snapshot,
}) =>
    MaterialApp(theme: ThemeData(useMaterial3: true), home: Scaffold(
      body: SingleChildScrollView(child: MicrophoneCommandPanel(
        client: client, locale: locale,
        snapshot: snapshot ?? commandSnapshot(locale: locale))),
    ));


Finder actionButton(bool requested) => find.widgetWithText(
    requested ? FilledButton : OutlinedButton,
    requested ? 'Mute inputs (SDK)' : 'Release all active inputs');

Future<void> confirmDialog(WidgetTester tester) async {
  final dialog = find.byType(AlertDialog);
  await tester.tap(find.descendant(of: dialog, matching: find.byType(FilledButton)));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('retained capabilities cannot authorize an unqualified engine context',
      (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    expect(client.canEnable, isTrue);
    expect(client.canRelease, isTrue);
    for (final snapshot in <Snapshot?>[
      null,
      commandSnapshot(platform: 'linux'),
      commandSnapshot(platform: 'macos'),
    ]) {
      await tester.pumpWidget(MaterialApp(home: Scaffold(
        body: MicrophoneCommandPanel(client: client, locale: 'en', snapshot: snapshot))));
      expect(tester.widget<FilledButton>(actionButton(true)).onPressed, isNull);
      expect(tester.widget<OutlinedButton>(actionButton(false)).onPressed, isNull);
      await tester.tap(actionButton(true));
      await tester.tap(actionButton(false));
      await tester.pump();
      expect(find.byType(AlertDialog), findsNothing);
      expect(sent, isEmpty);
    }
  });

  testWidgets('losing the engine context invalidates consent until a new confirmation',
      (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    await tester.pumpWidget(host(client, snapshot: commandSnapshot(platform: 'linux')));
    await tester.pump();
    await confirmDialog(tester);
    expect(sent, isEmpty);
    expect(tester.widget<FilledButton>(actionButton(true)).onPressed, isNull);

    await tester.pumpWidget(host(client));
    await tester.pumpAndSettle();
    expect(sent, isEmpty);
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    expect(sent, isEmpty);
    await confirmDialog(tester);
    expect(sent, hasLength(1));
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    await tester.pump();
    expect(client.pending, isFalse);
  });

  test('disconnected, invalid, unknown and read-only snapshots never send', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    await client.request(true);
    expect(client.error, 'disconnected');
    client.update(controls(), 0, true);
    await client.request(true);
    expect(client.error, 'unavailable');
    client.update(controls(session: ''), 1, true);
    await client.request(true);
    expect(client.session, isEmpty);
    client.update(controls(requested: null), 1, true);
    await client.request(true);
    expect(client.currentRequested, isNull);
    expect(client.error, 'unavailable');
    client.update(controls(enable: false, release: false,
      reason: 'Read-only development connection'), 2, true);
    await client.request(false);
    expect(client.error, 'capability');
    expect(client.canEnable, isFalse);
    expect(client.canRelease, isFalse);
    expect(client.pending, isFalse);
    expect(sent, isEmpty);
  });

  test('accepted command never optimistically changes requested state', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(generation: 7), 41, true);
    await client.request(true);
    expect(client.state, 'pending');
    expect(client.currentRequested, isFalse);
    expect(client.pendingId, 1);
    expect(client.canEnable, isFalse);
    client.update(controls(enable: false, release: false, generation: 7,
      command: result('accepted')), 42, true);
    expect(client.state, 'accepted');
    expect(client.currentRequested, isFalse);
    expect(client.pending, isTrue);
    client.update(controls(requested: true, generation: 8,
      command: result('succeeded')), 43, true);
    expect(client.state, 'succeeded');
    expect(client.currentRequested, isTrue);
    expect(client.generation, 8);
    expect(client.pending, isFalse);
    expect(client.canRelease, isTrue);
  });

  test('release is explicit and uses the latest native generation', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(requested: true, generation: 4), 11, true);
    await client.request(false);
    expect(client.currentRequested, isTrue);
    client.update(controls(requested: false, generation: 5,
      command: result('succeeded', requested: false)), 12, true);
    expect(client.currentRequested, isFalse);
    expect(client.pending, isFalse);
  });

  testWidgets('timeout stays uncertain and busy until a matching terminal result', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent, timeout: const Duration(seconds: 2));
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await client.request(true);
    await tester.pump(const Duration(seconds: 2));
    expect(client.state, 'uncertain');
    expect(client.code, 'client_timeout');
    expect(client.pendingId, 1);
    await client.request(false);
    expect(client.error, 'busy');
    expect(sent, hasLength(1));
    client.update(controls(command: result('accepted')), 2, true);
    expect(client.state, 'uncertain');
    expect(client.code, 'client_timeout');
    client.update(controls(command: result('succeeded', id: 2)), 3, true);
    client.update(controls(command: result('succeeded', requested: false)), 4, true);
    expect(client.pending, isTrue);
    expect(client.state, 'uncertain');
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 5, true);
    await tester.pump();
    expect(client.pending, isFalse);
    expect(client.state, 'succeeded');
    expect(client.error, isNull);
    await tester.pump(const Duration(seconds: 20));
    expect(client.state, 'succeeded');
    expect(sent, hasLength(1));
  });

  test('failed native readback does not infer restored state', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await client.request(true);
    client.update(controls(requested: null, generation: 1,
      enable: false, release: false,
      command: result('failed', code: 'partial_readback', detail: 'Some inputs could not be read.')),
      2, true);
    expect(client.state, 'failed');
    expect(client.code, 'partial_readback');
    expect(client.detail, 'Some inputs could not be read.');
    expect(client.currentRequested, isNull);
    expect(client.pending, isFalse);
    expect(client.canRelease, isFalse);
    expect(sent, hasLength(1));
  });

  test('native rejection and native final uncertainty release only their ticket', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await client.request(true);
    client.update(controls(command: result('rejected', code: 'stale')),
      2, true);
    expect(client.state, 'rejected');
    expect(client.pending, isFalse);
    await client.request(true);
    expect((sent.last['request'] as Map)['id'], 2);
    client.update(controls(requested: null, enable: false, release: false,
      command: result('uncertain', id: 2, detail: 'Readback unavailable.')), 3, true);
    expect(client.state, 'uncertain');
    expect(client.pending, isFalse);
    expect(client.detail, 'Readback unavailable.');
    expect(client.currentRequested, isNull);
  });

  test('old results and same-sequence payloads cannot finish a newer request', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 10, true);
    await client.request(true);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 11, true);
    await client.request(false);
    expect(client.pendingId, 2);
    client.update(controls(requested: true, generation: 1,
      command: result('failed')), 12, true);
    expect(client.state, 'pending');
    client.update(controls(requested: false, generation: 2,
      command: result('succeeded', id: 2, requested: false)), 11, true);
    expect(client.pendingId, 2);
    expect(client.currentRequested, isTrue);
    client.update(controls(requested: false, generation: 2,
      command: result('succeeded', id: 2, requested: false)), 12, true);
    expect(client.pendingId, 2);
    expect(client.currentRequested, isTrue);
    client.update(controls(requested: false, generation: 2,
      command: result('succeeded', id: 2, requested: false)), 13, true);
    expect(client.pending, isFalse);
    expect(client.state, 'succeeded');
    expect(sent.map((message) => (message['request'] as Map)['id']), [1, 2]);
  });

  testWidgets('transport failure cannot imply cancellation or cause retry', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent, send: (message) async {
      sent.add(message);
      throw StateError('Pipe write acknowledgement lost');
    });
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await client.request(true);
    await tester.pump();
    expect(client.state, 'uncertain');
    expect(client.code, 'client_transport_error');
    expect(client.error, 'sendFailed');
    expect(client.pending, isTrue);
    await tester.pump(const Duration(seconds: 30));
    expect(sent, hasLength(1));
    expect(client.pending, isTrue);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    expect(client.pending, isFalse);
    expect(client.state, 'succeeded');
    expect(client.error, isNull);
  });

  test('pending gate is synchronous even while the send future is unresolved', () async {
    final sent = <Map<String, dynamic>>[];
    final delivery = Completer<void>();
    final client = makeClient(sent, send: (message) {
      sent.add(message);
      return delivery.future;
    });
    client.update(controls(), 1, true);
    final first = client.request(true);
    await client.request(true);
    await client.request(false);
    expect(sent, hasLength(1));
    expect(client.pendingId, 1);
    expect(client.error, 'busy');
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    delivery.completeError(StateError('Late send failure'));
    await first;
    expect(client.state, 'succeeded');
    expect(client.error, isNull);
    expect(client.pending, isFalse);
  });

  testWidgets('disconnect and reconnect retain pending identity and timeout', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent, timeout: const Duration(seconds: 2));
    client.update(controls(), 1, true);
    await client.request(true);
    client.update(controls(), 1, false);
    expect(client.connected, isFalse);
    expect(client.pendingId, 1);
    await client.request(false);
    expect(client.error, 'disconnected');
    await tester.pump(const Duration(seconds: 2));
    expect(client.state, 'uncertain');
    client.update(controls(), 1, true);
    expect(client.connected, isTrue);
    expect(client.pendingId, 1);
    expect(client.canEnable, isFalse);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    await client.request(false);
    expect(sent.map((message) => (message['request'] as Map)['id']), [1, 2]);
    await tester.pumpWidget(const SizedBox.shrink());
    disposeClient(client);
  });

  testWidgets('new session resets IDs; retired sessions and late sends cannot roll it back', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final delivery = Completer<void>();
    final client = makeClient(sent, send: (message) {
      sent.add(message);
      return sent.length == 1 ? delivery.future : Future<void>.value();
    });
    client.update(controls(), 9, true);
    final first = client.request(true);
    await tester.pump(const Duration(seconds: 1));
    client.update(controls(session: sessionB, requested: true, generation: 20), 1, true);
    expect(client.session, sessionB);
    expect(client.state, isNull);
    expect(client.pending, isFalse);
    await client.request(false);
    expect(client.pendingId, 1);
    delivery.completeError(StateError('Old session transport failed'));
    await first;
    expect(client.state, 'pending');
    expect(client.error, isNull);
    await tester.pump(const Duration(seconds: 4));
    expect(client.state, 'pending');
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 100, true);
    expect(client.session, sessionB);
    expect(client.generation, 20);
    expect(client.pendingId, 1);
    expect(client.observedSequence, 1);
    client.update(controls(session: sessionB, requested: false, generation: 21,
      command: result('succeeded', requested: false)), 2, true);
    await tester.pump(const Duration(seconds: 20));
    expect(client.pending, isFalse);
    expect(client.state, 'succeeded');
    expect((sent[0]['request'] as Map)['id'], 1);
    expect((sent[1]['request'] as Map)['id'], 1);
    expect((sent[1]['request'] as Map)['session'], sessionB);
  });

  test('empty or malformed session does not reset IDs or discard native work', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await client.request(true);
    client.update(controls(session: ''), 0, false);
    client.update(controls(session: 'not-a-session'), 2, true);
    expect(client.session, sessionA);
    expect(client.pendingId, 1);
    expect(client.canRelease, isFalse);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    await client.request(false);
    expect((sent.last['request'] as Map)['id'], 2);
  });

  testWidgets('both actions require explicit human confirmation', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    expect(sent, isEmpty);
    expect(find.byType(AlertDialog), findsOneWidget);
    await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
    await tester.pumpAndSettle();
    expect(sent, isEmpty);
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    await confirmDialog(tester);
    expect(sent, hasLength(1));
    expect(client.state, 'pending');
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    await tester.pump();
    await tester.tap(actionButton(false));
    await tester.pumpAndSettle();
    expect(sent, hasLength(1));
    expect(find.byType(AlertDialog), findsOneWidget);
    await confirmDialog(tester);
    expect(sent, hasLength(2));
    final request = sent.last['request'] as Map;
    expect(request['requested'], isFalse);
    expect(request['confirmation'], 'release_all_active_inputs');
    await tester.pumpWidget(const SizedBox.shrink());
    disposeClient(client);
  });

  testWidgets('navigation and rebuild never replay a pending or completed command', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    await confirmDialog(tester);
    expect(sent, hasLength(1));
    await tester.pumpWidget(const MaterialApp(home: Text('Other page')));
    await tester.pumpWidget(host(client));
    await tester.pumpWidget(host(client));
    expect(sent, hasLength(1));
    expect(client.pendingId, 1);
    expect(tester.widget<FilledButton>(actionButton(true)).onPressed, isNull);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    await tester.pump();
    await tester.pumpWidget(const MaterialApp(home: Text('Other page')));
    await tester.pumpWidget(host(client));
    expect(sent, hasLength(1));
    await tester.tap(actionButton(false));
    await tester.pumpAndSettle();
    await confirmDialog(tester);
    expect((sent.last['request'] as Map)['id'], 2);
    expect(sent, hasLength(2));
    await tester.pumpWidget(const SizedBox.shrink());
    disposeClient(client);
  });

  testWidgets('session rollover during confirmation cannot authorize a new session', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    client.update(controls(session: sessionB), 1, true);
    await confirmDialog(tester);
    expect(sent, isEmpty);
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    await confirmDialog(tester);
    expect((sent.single['request'] as Map)['session'], sessionB);
    expect((sent.single['request'] as Map)['id'], 1);
    await tester.pumpWidget(const SizedBox.shrink());
    disposeClient(client);
  });

  testWidgets('generation changes invalidate confirmation, capability loss gates sending', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    client.update(controls(generation: 1), 2, true);
    await confirmDialog(tester);
    expect(sent, isEmpty);
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    client.update(controls(generation: 1, enable: false, release: false,
      reason: 'Worker busy'), 3, true);
    await confirmDialog(tester);
    expect(sent, isEmpty);
    expect(client.error, 'capability');
    expect(tester.widget<FilledButton>(actionButton(true)).onPressed, isNull);
    expect(tester.widget<OutlinedButton>(actionButton(false)).onPressed, isNull);
  });


  test('validated reconnect restores gates without resetting the request counter', () async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await client.request(true);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    client.update(controls(session: ''), 0, false);
    expect(client.canRelease, isFalse);
    client.update(controls(requested: true, generation: 1,
      command: result('succeeded')), 2, true);
    expect(client.canRelease, isTrue);
    await client.request(false);
    expect((sent.last['request'] as Map)['id'], 2);
  });

  testWidgets('removing the panel during confirmation cannot send or dispose its client', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    await tester.pumpWidget(MaterialApp(theme: ThemeData(useMaterial3: true),
      home: const Scaffold(body: Text('Other page'))));
    await confirmDialog(tester);
    expect(sent, isEmpty);
    expect(client.canEnable, isTrue);
    await tester.pumpWidget(host(client));
    await tester.tap(actionButton(true));
    await tester.pumpAndSettle();
    await confirmDialog(tester);
    expect((sent.single['request'] as Map)['id'], 1);
    await tester.pumpWidget(const SizedBox.shrink());
    disposeClient(client);
  });

  testWidgets('failed readback disables actions until fresh native capabilities', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = makeClient(sent);
    client.update(controls(), 1, true);
    await tester.pumpWidget(host(client));
    await client.request(true);
    client.update(controls(requested: null, enable: false, release: false,
      command: result('failed', detail: 'Readback is mixed.')), 2, true);
    await tester.pump();
    expect(client.state, 'failed');
    expect(client.currentRequested, isNull);
    expect(tester.widget<FilledButton>(actionButton(true)).onPressed, isNull);
    client.update(controls(), 3, true);
    await client.request(true);
    client.update(controls(command: result('rejected', id: 2)), 4, true);
    await tester.pump();
    expect(client.state, 'rejected');
    expect(client.currentRequested, isFalse);
    expect(sent, hasLength(2));
  });
}
