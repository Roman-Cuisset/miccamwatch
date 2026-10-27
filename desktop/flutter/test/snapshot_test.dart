import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/snapshot.dart';

Map<String, dynamic> frame({int sequence = 1}) => {
  'type': 'snapshot', 'protocol_version': 1, 'sequence': sequence,
  'observed_at': '2026-10-27T08:00:00Z', 'locale': 'en',
  'engine': {'version': '0.21.1', 'platform': 'windows',
    'mode': 'read_only_development', 'state': 'healthy', 'detail': null},
  'resources': [
    {'resource': 'microphone', 'activity': 'none_observed', 'coverage': 'complete',
      'confidence': null, 'active_count': 0, 'ready_count': 0},
    {'resource': 'camera', 'activity': 'unknown', 'coverage': 'incomplete',
      'confidence': null, 'active_count': 0, 'ready_count': 0},
  ],
  'accesses': <Map<String, dynamic>>[],
  'collectors': [
    {'collector': 'wasapi', 'state': 'healthy', 'detail': null},
    {'collector': 'privacy_store', 'state': 'degraded', 'detail': 'Access unavailable'},
  ],
  'observation_gaps': ['camera'],
  'protection': {
    for (final resource in resourceNames) resource:
      {'state': 'unknown', 'requested': null, 'service_active': null, 'detail': null},
  },
  'diagnostics': <String>[], 'truncated': false,
};
List<int> encode(Map<String, dynamic> value) => utf8.encode(jsonEncode(value));

void main() {
  test('valid snapshot preserves unknown versus none observed', () {
    final snapshot = Snapshot.decode(encode(frame()));
    expect(snapshot.activity(snapshot.resources[0]), 'none_observed');
    expect(snapshot.activity(snapshot.resources[1]), 'unknown');
    expect(snapshot.protection('microphone')['state'], 'unknown');
  });

  test('known active survives incomplete and degraded collector coverage', () {
    final value = frame();
    value['resources'][0]['activity'] = 'active';
    value['resources'][0]['coverage'] = 'incomplete';
    value['resources'][0]['active_count'] = 1;
    value['resources'][0]['confidence'] = 'high';
    value['collectors'][0]['state'] = 'degraded';
    value['observation_gaps'] = ['microphone', 'camera'];
    value['accesses'] = [{'resource': 'microphone', 'activity': 'active',
      'confidence': 'high', 'application': 'Observed application', 'pid': 123,
      'device': null, 'started_at': null}];
    final snapshot = Snapshot.decode(encode(value));
    expect(snapshot.activity(snapshot.resources[0]), 'active');
    expect(snapshot.resources[0]['coverage'], 'incomplete');
    expect(snapshot.accesses.single['confidence'], 'high');
  });

  test('portable resource interpretation does not require Windows collectors', () {
    final value = frame();
    value['engine']['platform'] = 'linux';
    value['collectors'] = [
      {'collector': 'pipewire', 'state': 'healthy', 'detail': null}
    ];
    value['resources'][1]['activity'] = 'none_observed';
    value['resources'][1]['coverage'] = 'complete';
    value['observation_gaps'] = [];
    final snapshot = Snapshot.decode(encode(value));
    expect(snapshot.activity(snapshot.resources[0]), 'none_observed');
    expect(snapshot.activity(snapshot.resources[1]), 'none_observed');
  });

  test('empty or failed evidence never implies no activity', () {
    final value = frame();
    value['collectors'] = [];
    var snapshot = Snapshot.decode(encode(value));
    expect(snapshot.activity(snapshot.resources[0]), 'unknown');
    value['collectors'] = [{'collector': 'new_collector', 'state': 'unavailable', 'detail': null}];
    snapshot = Snapshot.decode(encode(value));
    expect(snapshot.activity(snapshot.resources[0]), 'unknown');
  });

  test('schema, types, enum, version and monotonic sequence are required', () {
    final changes = <void Function(Map<String, dynamic>)>[
      (v) => v.remove('protection'),
      (v) => v['protocol_version'] = 2,
      (v) => v['protocol_version'] = 1.0,
      (v) => v['type'] = 'command',
      (v) => v['sequence'] = '1',
      (v) => v['resources'][0]['activity'] = 'safe',
      (v) => v['resources'][0]['coverage'] = 'unbounded',
      (v) => v['resources'][0]['active_count'] = -1,
      (v) => v['collectors'][0]['state'] = 'fine',
      (v) => v['truncated'] = 0,
      (v) => v['observed_at'] = 'yesterday',
      (v) => v['protection']['camera']['requested'] = 'true',
      (v) => v['locale'] = 'unsupported',
      (v) => v['unexpected'] = true,
    ];
    for (final change in changes) {
      final value = frame();
      change(value);
      expect(() => Snapshot.decode(encode(value)), throwsFormatException);
    }
    expect(() => Snapshot.decode(encode(frame()), afterSequence: 1), throwsFormatException);
    expect(() => Snapshot.decode(encode(frame()), afterSequence: 2), throwsFormatException);
    expect(() => Snapshot.decode([0xff]), throwsFormatException);
    expect(() => Snapshot.decode(List.filled(maxFrameBytes + 1, 32)), throwsFormatException);
    final longText = frame();
    longText['diagnostics'] = [List.filled(513, 'é').join()];
    expect(() => Snapshot.decode(encode(longText)), throwsFormatException);
  });

  test('stale uses elapsed receipt time, errors retain only last snapshot', () {
    var elapsed = Duration.zero;
    final session = SnapshotSession(elapsed: () => elapsed);
    session.accept(encode(frame()));
    expect(session.state, ConnectionState.live);
    elapsed = const Duration(seconds: 5);
    session.tick();
    expect(session.state, ConnectionState.live);
    elapsed += const Duration(milliseconds: 1);
    session.tick();
    expect(session.state, ConnectionState.stale);
    session.accept(encode(frame()));
    expect(session.state, ConnectionState.error);
    expect(session.sequence, 1);
    session.accept(encode(frame(sequence: 2)));
    expect(session.state, ConnectionState.live);
    session.disconnect();
    expect(session.state, ConnectionState.disconnected);
    session.dispose();
  });

  test('pipe handles split UTF8, rejects overlong lines and reports EOF', () async {
    final stream = StreamController<List<int>>();
    final session = SnapshotSession();
    var receipts = 0;
    final done = Completer<void>();
    final input = SnapshotInput(stream.stream, session,
      onSnapshot: () => receipts++, onDone: () => done.complete());
    final value = frame();
    value['diagnostics'] = ['é'];
    final bytes = encode(value);
    final split = bytes.indexOf(0xc3) + 1;
    stream.add(bytes.sublist(0, split));
    stream.add([...bytes.sublist(split), 10]);
    await Future<void>.delayed(Duration.zero);
    expect(receipts, 1);
    expect(session.snapshot!.diagnostics.single, 'é');
    stream.add(List.filled(maxFrameBytes + 1, 32));
    await Future<void>.delayed(Duration.zero);
    expect(session.state, ConnectionState.error);
    stream.add([10, ...encode(frame(sequence: 2)), 10]);
    await Future<void>.delayed(Duration.zero);
    expect(receipts, 2);
    await stream.close();
    await done.future;
    expect(session.state, ConnectionState.disconnected);
    await input.close();
    session.dispose();
  });
}
