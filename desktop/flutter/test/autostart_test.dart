import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/consultation.dart';
import 'package:miccamwatch_desktop/consultation_model.dart';
import 'package:miccamwatch_desktop/consultation_panels.dart';
import 'package:miccamwatch_desktop/snapshot.dart' show Snapshot;

import 'consultation_test.dart' as fixtures;
import 'snapshot_test.dart' as snapshots show frame, encode;

Map<String, dynamic> startup({bool? enabled = false, bool writable = true, String? reason}) => {
  'enabled': enabled, 'writable': writable, 'reason': reason,
};
ConsultationView completion({int id = 1, String operation = 'autostart_status',
  String state = 'succeeded', Map<String, dynamic>? actual, String? code, String? detail}) =>
    fixtures.view(completion: fixtures.result(id: id, operation: operation, state: state,
      autostart: actual ?? startup(), code: code, detail: detail));

Future<void> readStatus(ConsultationClient client, {bool? enabled = false, bool writable = true,
  bool operational = true}) async {
  client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: operational);
  await client.refreshAutostart();
  client.update(completion(actual: startup(enabled: enabled, writable: writable)),
    fixtures.sessionA, 2, true, operational: operational);
}

Widget host(ConsultationClient client, {String locale = 'en', Brightness brightness = Brightness.light}) =>
  MaterialApp(theme: ThemeData(useMaterial3: true, brightness: brightness),
    home: Scaffold(body: SingleChildScrollView(padding: const EdgeInsets.all(16),
      child: AutostartPanel(client: client, locale: locale))));

void main() {
  test('autostart DTOs use exact variants and native result exclusivity', () {
    expect(const AutostartStatusQuery().toJson(), {'kind': 'autostart_status'});
    expect(const SetAutostartQuery(requested: true, expectedEnabled: false).toJson(), {
      'kind': 'set_autostart', 'requested': true, 'expected_enabled': false,
      'confirmation': 'change_autostart_for_this_installation',
    });
    final frame = snapshots.frame();
    frame['controls']['session'] = fixtures.sessionA;
    frame['consultation'] = fixtures.viewJson(completion: fixtures.result(
      operation: 'autostart_status', autostart: startup(enabled: null, writable: false)));
    expect(Snapshot.decode(snapshots.encode(frame)).consultation.result!.autostart!.enabled, isNull);
    final mutations = <void Function(Map<String, dynamic>)>[
      (v) => v.remove('autostart'),
      (v) => v['history'] = fixtures.page(),
      (v) => v['diagnostics'] = fixtures.report(),
      (v) => v['autostart'] = null,
      (v) => v['autostart']['enabled'] = 1,
      (v) => v['autostart']['writable'] = 'true',
      (v) => v['autostart']['reason'] = List.filled(513, 'é').join(),
      (v) => v['autostart']['path'] = 'not permitted',
      (v) => v['state'] = 'rejected',
      (v) => v['operation'] = 'autostart',
    ];
    for (final mutation in mutations) {
      final value = fixtures.clone(fixtures.result(operation: 'autostart_status', autostart: startup()));
      mutation(value);
      expect(() => ConsultationResult.fromJson(value), throwsFormatException);
    }
    final unknownWrite = fixtures.result(operation: 'set_autostart',
      autostart: startup(enabled: null));
    expect(() => ConsultationResult.fromJson(unknownWrite), throwsFormatException);
  });

  test('development, native read-only, unknown and stale states forbid startup writes', () async {
    for (final scenario in [
      (operational: false, writable: true, enabled: false),
      (operational: true, writable: false, enabled: false),
      (operational: true, writable: true, enabled: null),
    ]) {
      final sent = <Map<String, dynamic>>[];
      final client = fixtures.clientFor(sent);
      await readStatus(client, enabled: scenario.enabled, writable: scenario.writable,
        operational: scenario.operational);
      expect(sent.single['request']['operation'], {'kind': 'autostart_status'});
      expect(client.canSetAutostart, isFalse);
      await client.setAutostart(true, expectedEnabled: false);
      expect(sent, hasLength(1));
    }
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent);
    await readStatus(client);
    expect(client.canSetAutostart, isTrue);
    client.update(fixtures.view(), fixtures.sessionA, 2, false, operational: true);
    await client.setAutostart(true, expectedEnabled: false);
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isTrue);
    expect(client.canSetAutostart, isFalse);
    expect(sent, hasLength(1));
    client.update(fixtures.view(), fixtures.sessionA, 3, true, operational: true);
    expect(client.canSetAutostart, isFalse);
    await client.refreshAutostart();
    client.update(completion(id: 2), fixtures.sessionA, 4, true, operational: true);
    expect(client.canSetAutostart, isTrue);
  });

  test('status and writes share lane IDs but never replace history or diagnostics data', () async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent)
      ..update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    await client.refreshHistory();
    client.update(fixtures.view(completion: fixtures.result(history: fixtures.page(cursor: 'next'))),
      fixtures.sessionA, 2, true, operational: true);
    final records = client.entries;
    await client.refreshDiagnostics();
    client.update(fixtures.view(completion: fixtures.result(id: 2, operation: 'diagnostics')),
      fixtures.sessionA, 3, true, operational: true);
    final report = client.report;
    await client.refreshAutostart();
    expect(sent.last['request']['id'], 3);
    await client.refreshHistory();
    expect(sent, hasLength(3));
    client.update(completion(id: 3), fixtures.sessionA, 4, true, operational: true);
    expect(identical(client.entries, records), isTrue);
    expect(identical(client.report, report), isTrue);
    expect(client.nextCursor, 'next');
    expect(client.historyStale, isFalse);
    expect(client.diagnosticsStale, isFalse);
    await client.setAutostart(true, expectedEnabled: false);
    expect(sent.last, {
      'type': 'consultation_request', 'protocol_version': 5,
      'request': {'session': fixtures.sessionA, 'id': 4, 'observed_sequence': 4,
        'operation': {'kind': 'set_autostart', 'requested': true, 'expected_enabled': false,
          'confirmation': 'change_autostart_for_this_installation'}},
    });
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isTrue);
    client.update(completion(id: 4, operation: 'set_autostart', state: 'accepted'),
      fixtures.sessionA, 5, true, operational: true);
    expect(client.autostart!.enabled, isFalse);
    client.update(completion(id: 4, operation: 'set_autostart', actual: startup(enabled: true)),
      fixtures.sessionA, 6, true, operational: true);
    expect(client.autostart!.enabled, isTrue);
    expect(client.autostartStale, isFalse);
    expect(client.autostartStatus.state, QueryState.succeeded);
    expect(client.nextCursor, 'next');
  });

  test('CAS rejection leaves stale prior readback and requires a real status requery', () async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent);
    await readStatus(client);
    await client.setAutostart(true, expectedEnabled: true);
    expect(sent, hasLength(1));
    await client.setAutostart(true, expectedEnabled: false);
    client.update(completion(id: 2, operation: 'set_autostart', state: 'rejected',
      code: 'state_changed', detail: 'Native startup changed before admission'),
      fixtures.sessionA, 3, true, operational: true);
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isTrue);
    expect(client.autostartStatus.state, QueryState.rejected);
    expect(client.canSetAutostart, isFalse);
    await client.setAutostart(true, expectedEnabled: false);
    expect(sent, hasLength(2));
    await client.refreshAutostart();
    client.update(completion(id: 3, actual: startup(enabled: true)), fixtures.sessionA, 4, true, operational: true);
    expect(client.autostart!.enabled, isTrue);
    expect(client.autostartStale, isFalse);
    expect(client.canSetAutostart, isTrue);
    await client.setAutostart(false, expectedEnabled: true);
    expect(sent.last['request']['operation']['expected_enabled'], isTrue);
  });

  test('mismatched success is not trusted and cannot become optimistic state', () async {
    final client = fixtures.clientFor([]);
    await readStatus(client);
    await client.setAutostart(true, expectedEnabled: false);
    client.update(completion(id: 2, operation: 'set_autostart', actual: startup(enabled: false)),
      fixtures.sessionA, 3, true, operational: true);
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isTrue);
    expect(client.autostartStatus.state, QueryState.failed);
    expect(client.autostartStatus.localIssue, 'autostartReadbackMismatch');
    expect(client.canSetAutostart, isFalse);
  });

  testWidgets('timeout keeps prior readback stale until correlated native completion or requery', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent, timeout: const Duration(seconds: 1));
    await readStatus(client);
    await client.setAutostart(true, expectedEnabled: false);
    await tester.pump(const Duration(seconds: 2));
    expect(client.autostartStatus.state, QueryState.uncertain);
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isTrue);
    expect(client.canSetAutostart, isFalse);
    client.update(completion(id: 2, operation: 'set_autostart', actual: startup(enabled: true)),
      fixtures.sessionA, 3, true, operational: true);
    expect(client.autostart!.enabled, isTrue);
    expect(client.autostartStale, isFalse);
    await client.setAutostart(false, expectedEnabled: true);
    await tester.pump(const Duration(seconds: 2));
    await client.refreshAutostart();
    client.update(completion(id: 3, operation: 'set_autostart', actual: startup(enabled: false)),
      fixtures.sessionA, 4, true, operational: true);
    expect(client.autostart!.enabled, isTrue);
    client.update(completion(id: 4, actual: startup(enabled: false)), fixtures.sessionA, 5, true, operational: true);
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isFalse);
  });

  testWidgets('confirmation is explicit, cancel sends nothing and success uses native readback', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent);
    await readStatus(client);
    await tester.pumpWidget(host(client));
    final change = find.byKey(const ValueKey('autostart.change'));
    await tester.tap(change);
    await tester.pumpAndSettle();
    expect(sent, hasLength(1));
    await tester.tap(find.byKey(const ValueKey('autostart.cancel')));
    await tester.pumpAndSettle();
    expect(sent, hasLength(1));
    await tester.tap(change);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('autostart.confirm')));
    await tester.pump();
    expect(sent, hasLength(2));
    expect(sent.last['request']['operation']['confirmation'], 'change_autostart_for_this_installation');
    expect(client.autostart!.enabled, isFalse);
    expect(tester.widget<OutlinedButton>(change).onPressed, isNull);
    expect(find.byKey(const ValueKey('autostart.stale')), findsOneWidget);
    client.update(completion(id: 2, operation: 'set_autostart', actual: startup(enabled: true)),
      fixtures.sessionA, 3, true, operational: true);
    await tester.pumpAndSettle();
    expect(client.autostart!.enabled, isTrue);
    expect(tester.widget<OutlinedButton>(change).onPressed, isNotNull);
    expect(find.byKey(const ValueKey('autostart.stale')), findsNothing);
  });

  testWidgets('losing freshness or installation session during confirmation sends no change', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent);
    await readStatus(client);
    await tester.pumpWidget(host(client));
    await tester.tap(find.byKey(const ValueKey('autostart.change')));
    await tester.pumpAndSettle();
    client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('autostart.confirm')));
    await tester.pumpAndSettle();
    expect(sent, hasLength(1));
    expect(client.autostart!.enabled, isFalse);
    expect(client.autostartStale, isTrue);
    expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('autostart.change'))).onPressed, isNull);
  });

  testWidgets('readonly status refresh works without activating any write', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent)
      ..update(fixtures.view(), fixtures.sessionA, 1, true);
    await tester.pumpWidget(host(client));
    await tester.pump(const Duration(seconds: 1));
    expect(sent, isEmpty);
    final refresh = find.byKey(const ValueKey('autostart.refresh'));
    await tester.tap(refresh);
    await tester.pump();
    expect(sent.single['request']['operation'], {'kind': 'autostart_status'});
    client.update(completion(actual: startup(enabled: true, writable: false,
      reason: 'Development session does not own startup changes')),
      fixtures.sessionA, 2, true);
    await tester.pumpAndSettle();
    final change = find.byKey(const ValueKey('autostart.change'));
    expect(tester.widget<OutlinedButton>(change).onPressed, isNull);
    expect(client.autostart!.enabled, isTrue);
    await tester.tap(change);
    await tester.pump();
    expect(find.byType(AlertDialog), findsNothing);
    expect(sent, hasLength(1));
  });

  testWidgets('keyboard opens confirmation without sending a startup mutation', (tester) async {
    final sent = <Map<String, dynamic>>[];
    final client = fixtures.clientFor(sent);
    await readStatus(client);
    await tester.pumpWidget(host(client));
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('autostart.confirm')), findsOneWidget);
    expect(sent, hasLength(1));
    await tester.tap(find.byKey(const ValueKey('autostart.cancel')));
    await tester.pumpAndSettle();
  });

  testWidgets('seven languages fit compact themes with stale native startup status', (tester) async {
    tester.view.physicalSize = const Size(360, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final client = fixtures.clientFor([]);
    await readStatus(client, enabled: true, writable: false, operational: false);
    client.update(fixtures.view(), fixtures.sessionA, 2, false);
    for (final locale in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
      for (final brightness in Brightness.values) {
        await tester.pumpWidget(host(client, locale: locale, brightness: brightness));
        expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('autostart.change'))).onPressed, isNull);
        expect(find.byKey(const ValueKey('autostart.stale')), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    }
  });
}
