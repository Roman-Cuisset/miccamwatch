import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/consultation.dart';
import 'package:miccamwatch_desktop/consultation_model.dart';
import 'package:miccamwatch_desktop/consultation_panels.dart';
import 'package:miccamwatch_desktop/snapshot.dart' show Snapshot;

import 'snapshot_test.dart' as snapshots show frame, encode;

const sessionA = '0123456789abcdef0123456789abcdef';
const sessionB = 'abcdef0123456789abcdef0123456789';
const nativeTime = '2026-10-29T14:35:12.123456789+09:00';

Map<String, dynamic> entry({String application = 'Native application', String observedAt = nativeTime}) => {
  'observed_at': observedAt, 'action': 'observed', 'resource': 'microphone',
  'activity': 'active', 'confidence': 'high', 'application': application, 'pid': 123,
};
Map<String, dynamic> page({List<Map<String, dynamic>>? events, String? cursor,
  bool incomplete = false, List<String> issues = const []}) => {
  'events': events ?? [entry()], 'next_cursor': cursor, 'incomplete': incomplete,
  'issues': issues, 'ordering': 'recorded_newest_first',
};
Map<String, dynamic> report() => {
  'checks': [
    {'name': 'Native collector', 'status': 'warning', 'detail': 'Native evidence',
      'remedy': 'Native remedy'},
  ],
  'limitations': ['Native limitation'], 'engine_version': '0.21.1',
  'platform': 'windows', 'observed_at': nativeTime, 'incomplete': true,
};
Map<String, dynamic> result({int id = 1, String state = 'succeeded',
  String operation = 'history', Map<String, dynamic>? history,
  Map<String, dynamic>? diagnostics, Map<String, dynamic>? autostart,
  String? code, String? detail}) => {
  'id': id, 'state': state, 'code': code, 'detail': detail, 'operation': operation,
  'history': state == 'succeeded' && operation == 'history' ? history ?? page() : null,
  'diagnostics': state == 'succeeded' && operation == 'diagnostics' ? diagnostics ?? report() : null,
  'autostart': state == 'succeeded' &&
      ['autostart_status', 'set_autostart'].contains(operation) ? autostart : null,
};
Map<String, dynamic> viewJson({bool available = true, String? reason,
  Map<String, dynamic>? completion}) => {
  'available': available, 'reason': reason, 'result': completion,
};
ConsultationView view({bool available = true, String? reason,
  Map<String, dynamic>? completion}) => ConsultationView.fromJson(
    viewJson(available: available, reason: reason, completion: completion));
Map<String, dynamic> clone(Map<String, dynamic> value) =>
    jsonDecode(jsonEncode(value)) as Map<String, dynamic>;

ConsultationClient clientFor(List<Map<String, dynamic>> sent, {
  Duration timeout = const Duration(seconds: 5),
  Future<void> Function(Map<String, dynamic>)? send,
}) {
  final client = ConsultationClient(send: send ?? (message) async { sent.add(message); }, timeout: timeout);
  addTearDown(client.dispose);
  return client;
}

Widget host(ConsultationClient client, {bool diagnostics = false, String locale = 'en',
  Brightness brightness = Brightness.light}) => MaterialApp(
    theme: ThemeData(brightness: brightness, useMaterial3: true),
    home: Scaffold(body: SingleChildScrollView(padding: const EdgeInsets.all(16),
      child: diagnostics ? DetailedDiagnosticsPanel(client: client, locale: locale)
        : HistoryPanel(client: client, locale: locale))));

void main() {
  group('strict native DTOs', () {
    test('request models expose only the bounded finite consultation operations', () {
      expect(ConsultationRequest(session: sessionA, id: 0xffffffff,
        observedSequence: 0x7fffffffffffffff, operation: HistoryQuery(limit: 50)).toJson(), {
          'session': sessionA, 'id': 0xffffffff, 'observed_sequence': 0x7fffffffffffffff,
          'operation': {'kind': 'history', 'cursor': null, 'limit': 50},
        });
      expect(const DiagnosticsQuery().toJson(), {'kind': 'diagnostics'});
      expect(() => HistoryQuery(limit: 0), throwsRangeError);
      expect(() => HistoryQuery(limit: 51), throwsRangeError);
      expect(() => HistoryQuery(cursor: ''), throwsFormatException);
      expect(() => HistoryQuery(cursor: List.filled(129, 'é').join()), throwsFormatException);
      for (final id in [0, 0x100000000]) {
        expect(() => ConsultationRequest(session: sessionA, id: id,
          observedSequence: 1, operation: const DiagnosticsQuery()), throwsFormatException);
      }
      for (final sequence in [0, -1]) {
        expect(() => ConsultationRequest(session: sessionA, id: 1,
          observedSequence: sequence, operation: const DiagnosticsQuery()), throwsFormatException);
      }
      expect(() => ConsultationRequest(session: 'foreign', id: 1,
        observedSequence: 1, operation: const DiagnosticsQuery()), throwsFormatException);
    });

    test('protocol five decodes real payloads and missing consultation disables the lane', () {
      final frame = snapshots.frame();
      frame['controls']['session'] = sessionA;
      frame['consultation'] = viewJson(completion: result());
      final snapshot = Snapshot.decode(snapshots.encode(frame));
      expect(snapshot.consultation.result!.history!.events.single.observedAt, nativeTime);
      expect(snapshot.consultation.result!.history!.events.single.pid, 123);
      frame['consultation'] = viewJson(completion: result(operation: 'diagnostics'));
      expect(Snapshot.decode(snapshots.encode(frame)).consultation.result!.diagnostics!.checks.single.remedy,
        'Native remedy');
      frame.remove('consultation');
      expect(Snapshot.decode(snapshots.encode(frame)).consultation.available, isFalse);
      frame['protocol_version'] = 4;
      expect(() => Snapshot.decode(snapshots.encode(frame)), throwsFormatException);
    });

    test('authenticated session is required for availability and results', () {
      for (final value in [viewJson(), viewJson(available: false, completion: result())]) {
        final frame = snapshots.frame()..['consultation'] = value;
        expect(() => Snapshot.decode(snapshots.encode(frame)), throwsFormatException);
      }
    });

    test('malformed shapes, types, exclusivity, timestamps and bounds fail closed', () {
      final mutations = <void Function(Map<String, dynamic>)>[
        (v) => v['unknown'] = true,
        (v) => v.remove('reason'),
        (v) => v['available'] = 1,
        (v) => v['result']['id'] = 0,
        (v) => v['result']['id'] = 0x100000000,
        (v) => v['result']['id'] = 1.0,
        (v) => v['result']['state'] = 'done',
        (v) => v['result']['operation'] = 'export',
        (v) => v['result']['unexpected'] = null,
        (v) => v['result']['code'] = List.filled(65, 'x').join(),
        (v) => v['result']['diagnostics'] = report(),
        (v) => v['result']['state'] = 'failed',
        (v) => v['result']['history'] = null,
        (v) => v['result']['history']['ordering'] = 'observed_newest_first',
        (v) => v['result']['history']['next_cursor'] = '',
        (v) => v['result']['history']['next_cursor'] = List.filled(257, 'x').join(),
        (v) => v['result']['history']['incomplete'] = 'false',
        (v) => v['result']['history']['events'] = List.generate(51, (_) => entry()),
        (v) => v['result']['history']['events'][0]['application'] = List.filled(257, 'é').join(),
        (v) => v['result']['history']['events'][0]['observed_at'] = '2026-02-30T10:00:00Z',
        (v) => v['result']['history']['events'][0]['observed_at'] = '2026-10-29T10:00:00',
        (v) => v['result']['history']['events'][0]['observed_at'] = '2026-10-29T24:00:00Z',
        (v) => v['result']['history']['events'][0]['pid'] = -1,
        (v) => v['result']['history']['events'][0]['pid'] = 0x100000000,
        (v) => v['result']['history']['events'][0]['confidence'] = null,
        (v) => v['result']['history']['events'][0]['path'] = 'not allowed',
        (v) => v['result']['history']['issues'] = List.filled(17, 'issue'),
        (v) => v['result']['history']['issues'] = [List.filled(129, 'é').join()],
      ];
      for (final mutation in mutations) {
        final value = clone(viewJson(completion: result()));
        mutation(value);
        expect(() => ConsultationView.fromJson(value), throwsFormatException);
      }
      final diagnosticsMutations = <void Function(Map<String, dynamic>)>[
        (v) => v['checks'][0]['status'] = 'healthy',
        (v) => v['checks'][0]['remedy'] = false,
        (v) => v['checks'][0]['detail'] = List.filled(513, 'é').join(),
        (v) => v['checks'] = List.filled(65, report()['checks'][0]),
        (v) => v['limitations'] = List.filled(65, 'limit'),
        (v) => v['platform'] = 'unknown',
        (v) => v['observed_at'] = 'yesterday',
        (v) => v['incomplete'] = null,
        (v) => v['command'] = 'not allowed',
      ];
      for (final mutation in diagnosticsMutations) {
        final value = clone(report());
        mutation(value);
        expect(() => DetailedDiagnostics.fromJson(value), throwsFormatException);
      }
    });

    test('DTO boundary preserves technical strings and immutable bounded collections', () {
      final nativeEntry = entry();
      nativeEntry['application'] = null;
      nativeEntry['pid'] = null;
      final value = page(events: [nativeEntry],
        cursor: List.filled(256, 'x').join(), issues: [List.filled(128, 'é').join()]);
      value['events'][0]['action'] = 'native_future_action';
      final decoded = HistoryPage.fromJson(value);
      expect(decoded.events.single.application, isNull);
      expect(decoded.events.single.pid, isNull);
      expect(decoded.events.single.action, 'native_future_action');
      expect(() => decoded.events.clear(), throwsUnsupportedError);
      expect(() => decoded.issues.clear(), throwsUnsupportedError);
      final limits = report()..['checks'][0]['detail'] = List.filled(512, 'é').join();
      expect(DetailedDiagnostics.fromJson(limits).checks.single.detail.length, 512);
    });
  });

  group('correlated read-only lane', () {
    test('absent, invalid, stale and unavailable sessions cannot send', () async {
      final sent = <Map<String, dynamic>>[];
      final client = clientFor(sent);
      await client.refreshHistory();
      client.update(view(), '', 1, true);
      await client.refreshDiagnostics();
      client.update(view(), sessionA, 0, true);
      await client.refreshHistory();
      client.update(view(), sessionA, 1, false);
      await client.refreshHistory();
      client.update(view(available: false, reason: 'Native unsupported'), sessionA, 2, true);
      expect(client.disabledReason, 'queryUnavailable');
      expect(client.reason, 'Native unsupported');
      await client.refreshHistory();
      expect(client.historyLoaded, isFalse);
      expect(client.report, isNull);
      expect(sent, isEmpty);
    });

    test('request envelope is exact and results require latest ID, operation and newer sequence', () async {
      final sent = <Map<String, dynamic>>[];
      final client = clientFor(sent)..update(view(), sessionA, 10, true);
      await client.refreshHistory();
      expect(sent.single, {
        'type': 'consultation_request', 'protocol_version': 5,
        'request': {'session': sessionA, 'id': 1, 'observed_sequence': 10,
          'operation': {'kind': 'history', 'cursor': null, 'limit': 30}},
      });
      await client.refreshDiagnostics();
      expect(sent, hasLength(1));
      client.update(view(completion: result()), sessionA, 10, true);
      client.update(view(completion: result()), sessionA, 9, true);
      client.update(view(completion: result(id: 99)), sessionA, 11, true);
      client.update(view(completion: result(operation: 'diagnostics')), sessionA, 12, true);
      expect(client.historyLoaded, isFalse);
      expect(client.report, isNull);
      expect(client.busy, isTrue);
      client.update(view(completion: result(state: 'accepted')), sessionA, 13, true);
      expect(client.historyStatus.state, QueryState.accepted);
      client.update(view(completion: result()), sessionA, 14, true);
      expect(client.historyLoaded, isTrue);
      expect(client.entries.single.observedAt, nativeTime);
      expect(client.busy, isFalse);
      client.update(view(completion: result(history: page(events: []))), sessionA, 15, true);
      expect(client.entries, hasLength(1));
      await client.refreshDiagnostics();
      expect(sent.last['request']['operation'], {'kind': 'diagnostics'});
      client.update(view(completion: result(id: 2, operation: 'diagnostics')), sessionA, 16, true);
      expect(client.report!.incomplete, isTrue);
      expect(client.report!.checks.single.remedy, 'Native remedy');
    });

    test('disconnect and session change invalidate pending work and page cursors', () async {
      final client = clientFor([])..update(view(), sessionA, 1, true);
      await client.refreshHistory();
      client.update(view(completion: result(history: page(cursor: 'opaque:next'))), sessionA, 2, true);
      await client.loadMore();
      client.update(view(), sessionA, 2, false);
      expect(client.canRequest, isFalse);
      expect(client.nextCursor, isNull);
      expect(client.historyStale, isTrue);
      client.update(view(completion: result(id: 2, history: page(events: []))), sessionA, 3, true);
      expect(client.entries, hasLength(1));
      expect(client.historyStale, isTrue);
      await client.refreshHistory();
      client.update(view(), sessionB, 1, true);
      expect(client.nextCursor, isNull);
      expect(client.historyStale, isTrue);
      expect(client.busy, isFalse);
      client.update(view(completion: result(id: 3, history: page(events: []))), sessionA, 999, true);
      expect(client.entries, hasLength(1));
      await client.refreshHistory();
      client.update(view(completion: result(history: page(events: []))), sessionB, 2, true);
      expect(client.entries, isEmpty);
      expect(client.historyStale, isFalse);
    });

    testWidgets('timeout remains uncertain and a correlated late terminal result may resolve it', (tester) async {
      final client = clientFor([], timeout: const Duration(seconds: 2))
        ..update(view(), sessionA, 1, true);
      await client.refreshHistory();
      await tester.pump(const Duration(seconds: 3));
      expect(client.historyStatus.state, QueryState.uncertain);
      expect(client.historyStatus.localIssue, 'queryTimeout');
      expect(client.canRequest, isTrue);
      client.update(view(completion: result(state: 'accepted')), sessionA, 2, true);
      expect(client.historyStatus.state, QueryState.uncertain);
      client.update(view(completion: result()), sessionA, 3, true);
      expect(client.historyStatus.state, QueryState.succeeded);
      expect(client.historyStatus.localIssue, isNull);
      expect(client.historyLoaded, isTrue);
    });

    testWidgets('refresh supersedes a timed-out request and ignores late completion and send failure', (tester) async {
      final sends = <Completer<void>>[];
      final client = clientFor([], timeout: const Duration(seconds: 1), send: (_) {
        final completion = Completer<void>();
        sends.add(completion);
        return completion.future;
      })..update(view(), sessionA, 1, true);
      final first = client.refreshHistory();
      await tester.pump(const Duration(seconds: 2));
      final second = client.refreshHistory();
      client.update(view(completion: result()), sessionA, 2, true);
      expect(client.historyLoaded, isFalse);
      sends.first.completeError(StateError('pipe delivery unknown'));
      await first;
      expect(client.historyStatus.state, QueryState.pending);
      client.update(view(completion: result(id: 2, history: page(events: []))), sessionA, 3, true);
      sends.last.complete();
      await second;
      expect(client.historyLoaded, isTrue);
      expect(client.entries, isEmpty);
      expect(client.historyStatus.state, QueryState.succeeded);
    });

    test('native failure and unavailable transport retain only explicitly stale data', () async {
      final client = clientFor([])..update(view(), sessionA, 1, true);
      await client.refreshHistory();
      client.update(view(completion: result(history: page(cursor: 'next'))), sessionA, 2, true);
      await client.refreshHistory();
      expect(client.historyStale, isTrue);
      client.update(view(completion: result(id: 2, state: 'failed', code: 'read_failed',
        detail: 'Native storage inaccessible')), sessionA, 3, true);
      expect(client.entries, hasLength(1));
      expect(client.historyStale, isTrue);
      expect(client.nextCursor, isNull);
      expect(client.historyStatus.detail, 'Native storage inaccessible');
      await client.refreshDiagnostics();
      client.update(view(completion: result(id: 3, operation: 'diagnostics')), sessionA, 4, true);
      client.update(view(available: false), sessionA, 5, true);
      expect(client.report, isNotNull);
      expect(client.diagnosticsStale, isTrue);
      expect(client.canRequest, isFalse);
    });

    test('send error does not invent native failure and a late result is still accepted', () async {
      final client = clientFor([], send: (_) async { throw StateError('write failed'); })
        ..update(view(), sessionA, 1, true);
      await client.refreshDiagnostics();
      expect(client.diagnosticsStatus.state, QueryState.uncertain);
      expect(client.diagnosticsStatus.localIssue, 'querySendFailed');
      expect(client.report, isNull);
      client.update(view(completion: result(operation: 'diagnostics')), sessionA, 2, true);
      expect(client.report, isNotNull);
      expect(client.diagnosticsStatus.state, QueryState.succeeded);
    });

    test('pagination enforces limits, rejects cycling cursors and refresh replaces prior pages', () async {
      final sent = <Map<String, dynamic>>[];
      final client = clientFor(sent)..update(view(), sessionA, 1, true);
      await expectLater(client.refreshHistory(limit: 0), throwsRangeError);
      await expectLater(client.refreshHistory(limit: 51), throwsRangeError);
      expect(sent, isEmpty);
      await client.refreshHistory(limit: 1);
      client.update(view(completion: result(history: page(events: [entry(), entry()]))), sessionA, 2, true);
      expect(client.historyLoaded, isFalse);
      expect(client.historyStatus.localIssue, 'queryPaginationInvalid');
      await client.refreshHistory(limit: 50);
      client.update(view(completion: result(id: 2, history: page(cursor: 'a'))), sessionA, 3, true);
      await client.loadMore();
      expect(sent.last['request']['operation'], {'kind': 'history', 'cursor': 'a', 'limit': 30});
      client.update(view(completion: result(id: 3, history: page(cursor: 'b'))), sessionA, 4, true);
      expect(client.entries, hasLength(2));
      await client.loadMore();
      client.update(view(completion: result(id: 4, history: page(cursor: 'a'))), sessionA, 5, true);
      expect(client.entries, hasLength(2));
      expect(client.nextCursor, isNull);
      expect(client.historyStale, isTrue);
      await client.refreshHistory();
      client.update(view(completion: result(id: 5, history: page(events: []))), sessionA, 6, true);
      expect(client.entries, isEmpty);
      expect(client.historyLoaded, isTrue);
      expect(client.historyStale, isFalse);
      final count = sent.length;
      await client.loadMore();
      expect(sent, hasLength(count));
    });
  });

  group('usable pages', () {
    testWidgets('real refresh and load-more interactions use exact cursors and stop at the end', (tester) async {
      final sent = <Map<String, dynamic>>[];
      final client = clientFor(sent)..update(view(), sessionA, 1, true);
      await tester.pumpWidget(host(client));
      await tester.tap(find.byKey(const ValueKey('history.refresh')));
      await tester.pump();
      expect(sent.single['request']['operation'], {'kind': 'history', 'cursor': null, 'limit': 30});
      final first = entry(application: 'Recorded newest', observedAt: '2025-01-01T01:00:00-05:00');
      client.update(view(completion: result(history: page(events: [first], cursor: 'opaque+/=='))), sessionA, 2, true);
      await tester.pump();
      final more = find.byKey(const ValueKey('history.more'));
      await tester.ensureVisible(more);
      await tester.tap(more);
      await tester.pump();
      expect(sent.last['request']['operation'], {'kind': 'history', 'cursor': 'opaque+/==', 'limit': 30});
      expect(tester.widget<OutlinedButton>(more).onPressed, isNull);
      client.update(view(completion: result(id: 2, history: page(events: [entry(application: 'Recorded earlier')]))),
        sessionA, 3, true);
      await tester.pump();
      expect(client.entries.map((event) => event.application), ['Recorded newest', 'Recorded earlier']);
      expect(client.entries.first.observedAt, '2025-01-01T01:00:00-05:00');
      expect(find.byKey(const ValueKey('history.entry.0')), findsOneWidget);
      expect(find.byKey(const ValueKey('history.entry.1')), findsOneWidget);
      await tester.ensureVisible(more);
      expect(tester.widget<OutlinedButton>(more).onPressed, isNull);
      final count = sent.length;
      await tester.tap(more);
      expect(sent, hasLength(count));
      final refresh = find.byKey(const ValueKey('history.refresh'));
      await tester.ensureVisible(refresh);
      await tester.tap(refresh);
      await tester.pump();
      expect(sent.last['request']['operation'], {'kind': 'history', 'cursor': null, 'limit': 30});
      expect(find.byKey(const ValueKey('history.stale')), findsOneWidget);
      client.update(view(completion: result(id: 3, history: page(events: []))), sessionA, 4, true);
      await tester.pump();
      expect(find.byKey(const ValueKey('history.empty')), findsOneWidget);
      expect(find.byKey(const ValueKey('history.entry.0')), findsNothing);
    });

    testWidgets('diagnostics is explicitly on demand and cached report is marked stale', (tester) async {
      final sent = <Map<String, dynamic>>[];
      final client = clientFor(sent)..update(view(), sessionA, 1, true);
      await tester.pumpWidget(host(client, diagnostics: true));
      await tester.pump(const Duration(seconds: 1));
      expect(sent, isEmpty);
      final run = find.byKey(const ValueKey('diagnostics.run'));
      await tester.tap(run);
      await tester.pump();
      expect(sent.single['request']['operation'], {'kind': 'diagnostics'});
      client.update(view(completion: result(operation: 'diagnostics')), sessionA, 2, true);
      await tester.pump();
      expect(find.byKey(const ValueKey('diagnostics.report')), findsOneWidget);
      client.update(view(), sessionA, 2, false);
      await tester.pump();
      expect(tester.widget<OutlinedButton>(run).onPressed, isNull);
      expect(find.byKey(const ValueKey('diagnostics.stale')), findsOneWidget);
      expect(client.report!.observedAt, nativeTime);
    });

    testWidgets('keyboard activation submits one query and pending disables refresh', (tester) async {
      final sent = <Map<String, dynamic>>[];
      final client = clientFor(sent)..update(view(), sessionA, 1, true);
      await tester.pumpWidget(host(client));
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(sent, hasLength(1));
      expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('history.refresh'))).onPressed, isNull);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(sent, hasLength(1));
    });

    testWidgets('all languages fit compact light and dark pages with native partial data and disabled gates', (tester) async {
      tester.view.physicalSize = const Size(360, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final client = clientFor([])..update(view(), sessionA, 1, true);
      await client.refreshHistory();
      client.update(view(completion: result(history: page(incomplete: true, issues: ['Native issue']))), sessionA, 2, true);
      await client.refreshDiagnostics();
      client.update(view(completion: result(id: 2, operation: 'diagnostics')), sessionA, 3, true);
      client.update(view(), sessionA, 3, false);
      for (final locale in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
        for (final brightness in Brightness.values) {
          await tester.pumpWidget(host(client, locale: locale, brightness: brightness));
          expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('history.refresh'))).onPressed, isNull);
          expect(find.byKey(const ValueKey('history.stale')), findsOneWidget);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(host(client, diagnostics: true, locale: locale, brightness: brightness));
          expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('diagnostics.run'))).onPressed, isNull);
          expect(find.byKey(const ValueKey('diagnostics.stale')), findsOneWidget);
          expect(tester.takeException(), isNull);
        }
      }
    });
  });
}
