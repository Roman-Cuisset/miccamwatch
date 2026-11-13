import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/consultation.dart';
import 'package:miccamwatch_desktop/tools_model.dart';
import 'package:miccamwatch_desktop/tools_panels.dart';
import 'package:miccamwatch_desktop/preferences_model.dart';
import 'consultation_test.dart' as fixtures;

Map<String, dynamic> nativeState({String microphone = 'unmuted', String camera = 'unknown'}) => {
  'microphone': microphone, 'microphone_scope': 'windows_endpoint_sdk_mute',
  'camera': camera, 'camera_scope': 'windows_pnp_camera',
  'camera_requested': null, 'camera_service': null, 'camera_present': null,
  'camera_pending': null, 'camera_error': 'Fixture camera command unavailable',
  'microphone_error': null, 'can_mute': true, 'can_restore': true,
  'can_camera': false, 'can_resume': false, 'can_inspect': false,
  'lock_microphone': false, 'lock_camera': false, 'restore_on_unlock': true,
  'lock_supported': true, 'camera_lock_supported': false,
};
Map<String, dynamic> report({String outcome = 'completed',
    Map<String, dynamic>? state, Map<String, dynamic>? control,
    Map<String, dynamic>? data, String? error}) => {
  'outcome': outcome, 'state': state, 'control': control, 'data': data,
  'token': outcome == 'prepared' ? 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' : null,
  'error': error,
};
void complete(ConsultationClient client, Map<String, dynamic> report, {
    int id = 1, int sequence = 2, String session = fixtures.sessionA}) {
  client.update(fixtures.view(completion: fixtures.result(id: id, operation: 'tools', tools: report)),
    session, sequence, true, operational: true);
}

void main() {
  test('observed application decisions are not automatic enforcement ownership', () {
    final parsed = NativeReport.fromJson(report(data: {
      'pid': 42, 'resource': 'camera', 'enforcement': 'unknown',
      'access_index': 1, 'access_count': 1, 'next_cursor': null,
      'snapshot_read_at': '2026-11-13T07:00:00Z', 'snapshot_not_live': true,
      'process_identity_confirmed': false, 'truncated': false,
    }));
    expect(parsed.data!['enforcement'], 'unknown');
    expect(parsed.enforcement, isNull);
  });

  test('camera recovery is exact, bounded and cannot carry forged native authority', () {
    final control = NativeControl.restoreLegacyCamera(r'USB\VID_TEST\EXACT');
    expect(NativeControl.fromJson(control.toJson()), control);
    expect(NativeControl.action('camera_resume').toJson(), {'kind': 'camera_resume'});
    for (final id in ['', '   ', 'device\nother', 'x' * 2049]) {
      expect(() => NativeControl.restoreLegacyCamera(id), throwsArgumentError);
    }
    expect(() => NativeControl.fromJson({...control.toJson(), 'generation': 12}), throwsFormatException);
  });

  testWidgets('camera confirmation cancellation sends no command; native refusal never becomes protection', (tester) async {
    tester.view.physicalSize = const Size(900, 1400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var sequence = 1;
    var commits = 0;
    late ConsultationClient client;
    final state = {...nativeState(), 'can_camera': true, 'camera_requested': true,
      'camera_service': false, 'camera_present': 1, 'can_resume': true};
    client = ConsultationClient(send: (message) async {
      final request = message['request'] as Map<String, dynamic>;
      final tools = (request['operation'] as Map<String, dynamic>)['request'] as Map<String, dynamic>;
      final action = tools['action'];
      if (action == 'commit') commits++;
      complete(client, action == 'prepare'
        ? report(outcome: 'prepared', state: state, control: tools['control'] as Map<String, dynamic>,
            data: {'confirmation_seconds': 30})
        : action == 'commit'
          ? report(outcome: 'not_confirmed', state: state, control: {'kind': 'camera_block'}, error: 'UAC declined; no confirmed protection')
          : report(state: state),
        id: request['id'] as int, sequence: ++sequence);
    });
    client.update(fixtures.view(), fixtures.sessionA, sequence, true, operational: true);
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [])))));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('tools.camera_block')));
    await tester.pumpAndSettle();
    expect(find.byType(AlertDialog), findsOneWidget);
    await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
    await tester.pumpAndSettle();
    expect(commits, 0);
    await tester.tap(find.byKey(const ValueKey('tools.camera_block')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('tools.confirm')));
    await tester.pumpAndSettle();
    expect(commits, 1);
    expect(find.textContaining('UAC declined'), findsOneWidget);
    expect(client.toolsReport!.state!.text('camera'), 'unknown');
    expect(client.toolsStatus.state, QueryState.uncertain);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });

  test('closed native reports reject inconsistent confirmations and unknown authority', () {
    final prepared = report(outcome: 'prepared', state: nativeState(),
      control: {'kind': 'microphone_mute'});
    NativeReport.fromJson(prepared);
    for (final mutation in <void Function(Map<String, dynamic>)>[
      (value) => value['token'] = null,
      (value) => value['state'] = null,
      (value) => value['elevate'] = true,
      (value) => value['control'] = {'kind': 'terminate', 'pid': 123, 'instance': 'forged'},
      (value) => value['state'] = {...nativeState(), 'can_camera': 'yes'},
      (value) => value['state'] = {...nativeState(), 'microphone_scope': 'universal_hardware_block'},
      (value) => value['state'] = {...nativeState(), 'camera_scope': 'unknown_scope'},
    ]) {
      final value = {...prepared}; mutation(value);
      expect(() => NativeReport.fromJson(value), throwsFormatException);
    }
    expect(() => NativeReport.fromJson(report(outcome: 'not_confirmed',
      control: {'kind': 'camera_block'})), throwsFormatException);
  });

  test('a prepared action must match the requested control before confirmation is usable', () async {
    final client = ConsultationClient(send: (_) async {});
    addTearDown(client.dispose);
    client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    final pending = client.requestTools(ToolsRequest.prepare(NativeControl.action('microphone_mute')));
    final rejected = expectLater(pending, throwsStateError);
    complete(client, report(outcome: 'prepared', state: nativeState(),
      control: {'kind': 'camera_block'}));
    await rejected;
    expect(client.toolsReport, isNull);
    expect(client.toolsStatus.state, QueryState.failed);
  });

  test('an accepted command never synthesizes a confirmed state and a refused readback stays uncertain', () async {
    final client = ConsultationClient(send: (_) async {});
    addTearDown(client.dispose);
    client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    final prepared = NativeReport.fromJson(report(outcome: 'prepared', state: nativeState(),
      control: {'kind': 'microphone_mute'}));
    final pending = client.requestTools(ToolsRequest.commit(prepared));
    client.update(fixtures.view(completion: fixtures.result(operation: 'tools', state: 'accepted')),
      fixtures.sessionA, 2, true, operational: true);
    expect(client.toolsReport, isNull);
    expect(client.toolsStatus.state, QueryState.accepted);
    complete(client, report(outcome: 'not_confirmed', state: nativeState(microphone: 'mixed'),
      control: {'kind': 'microphone_mute'}, error: 'Native readback is mixed'), sequence: 3);
    final result = await pending;
    expect(result.state!.text('microphone'), 'mixed');
    expect(result.outcome, 'not_confirmed');
    expect(client.toolsStatus.state, QueryState.uncertain);
  });

  testWidgets('timeouts never retry and a late readback can update the visible native result', (tester) async {
    var deliveries = 0;
    final client = ConsultationClient(send: (_) async { deliveries++; }, timeout: const Duration(seconds: 2));
    addTearDown(client.dispose);
    client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    final pending = client.requestTools(const ToolsRequest.controls());
    final timeout = expectLater(pending, throwsStateError);
    await tester.pump(const Duration(seconds: 3));
    await timeout;
    expect(client.toolsStatus.state, QueryState.uncertain);
    await tester.pump(const Duration(minutes: 2));
    expect(deliveries, 1);
    complete(client, report(state: nativeState(microphone: 'muted')));
    expect(client.toolsReport!.state!.text('microphone'), 'muted');
    expect(client.toolsStatus.state, QueryState.succeeded);
  });

  test('disconnection retires the old completion; a new session cannot inherit preparation', () async {
    final client = ConsultationClient(send: (_) async {});
    addTearDown(client.dispose);
    client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    final pending = client.requestTools(ToolsRequest.prepare(NativeControl.action('camera_block')));
    final disconnected = expectLater(pending, throwsStateError);
    client.update(fixtures.view(), fixtures.sessionA, 2, false);
    await disconnected;
    client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    complete(client, report(outcome: 'prepared', state: nativeState(),
      control: {'kind': 'camera_block'}), sequence: 3);
    expect(client.session, fixtures.sessionB);
    expect(client.toolsReport, isNull);
  });

  test('history is retained on a refusal and invalidated only after confirmed clearing', () async {
    final client = ConsultationClient(send: (_) async {});
    addTearDown(client.dispose);
    client.update(fixtures.view(), fixtures.sessionA, 1, true);
    await client.refreshHistory();
    client.update(fixtures.view(completion: fixtures.result()), fixtures.sessionA, 2, true);
    final original = client.entries;
    final refused = client.requestTools(const ToolsRequest.clearHistory());
    final failure = expectLater(refused, throwsStateError);
    client.update(fixtures.view(completion: fixtures.result(id: 2, operation: 'tools', state: 'failed',
      detail: 'Read-only history')), fixtures.sessionA, 3, true);
    await failure;
    expect(identical(client.entries, original), isTrue);
    final clear = client.requestTools(const ToolsRequest.clearHistory());
    expect(identical(client.entries, original), isTrue);
    complete(client, report(data: {'cleared': true}), id: 3, sequence: 4);
    await clear;
    expect(client.entries, isEmpty);
    expect(client.historyLoaded, isFalse);
    expect(client.nextCursor, isNull);
  });

  test('invalid pause durations, ambiguous value types and extra authority are refused', () {
    for (final value in [
      {'kind': 'pause', 'minutes': 0}, {'kind': 'pause', 'minutes': 10081},
      {'kind': 'pause', 'minutes': 1.5}, {'kind': 'pause', 'minutes': '30'},
      {'kind': 'resume', 'minutes': 30}, 'pause_30_minutes',
    ]) {
      expect(() => PauseDirective.fromJson(value), throwsFormatException);
    }
  });

  testWidgets('unknown camera and development mode remain disabled in seven narrow layouts', (tester) async {
    tester.view.physicalSize = const Size(360, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final locale in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
      var sequence = 1;
      late ConsultationClient client;
      client = ConsultationClient(send: (message) async {
        final id = message['request']['id'] as int;
        client.update(fixtures.view(completion: fixtures.result(id: id,
          operation: 'tools', tools: report(state: nativeState()))),
          fixtures.sessionA, ++sequence, true, operational: false);
      });
      client.update(fixtures.view(), fixtures.sessionA, sequence, true);
      await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
        child: NativeToolsPanel(client: client, locale: locale, accesses: const [])))));
      await tester.pump();
      await tester.pump();
      expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('tools.camera_block'))).onPressed, isNull);
      expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('tools.microphone_mute'))).onPressed, isNull);
      expect(tester.widget<SwitchListTile>(find.byKey(const ValueKey('tools.lock_microphone'))).onChanged, isNull);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      client.dispose();
    }
  });

  testWidgets('one process using several devices has one unambiguous details action', (tester) async {
    final client = ConsultationClient(send: (_) async {});
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [
        {'pid': 12345, 'application': 'Observed application', 'resource': 'microphone', 'device': 'input-one'},
        {'pid': 12345, 'application': 'Observed application', 'resource': 'microphone', 'device': 'input-two'},
        {'pid': 12345, 'application': 'Observed application', 'resource': 'camera', 'device': 'camera-one'},
      ])))));
    expect(find.byKey(const ValueKey('tools.application.12345')), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });

  testWidgets('a retained device readback cannot authorize controls in a replacement session', (tester) async {
    late ConsultationClient client;
    client = ConsultationClient(send: (message) async {
      complete(client, report(state: nativeState()), id: message['request']['id'] as int);
    });
    client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [])))));
    await tester.pump();
    await tester.pump();
    final mute = find.byKey(const ValueKey('tools.microphone_mute'));
    expect(tester.widget<OutlinedButton>(mute).onPressed, isNotNull);
    client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    await tester.pump();
    expect(tester.widget<OutlinedButton>(mute).onPressed, isNull);
    expect(tester.widget<SwitchListTile>(find.byKey(const ValueKey('tools.lock_microphone'))).onChanged, isNull);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });
  test('enforcement requires a boolean intent and strictly typed native status', () {
    expect(NativeControl.fromJson(NativeControl.enforcement(true).toJson()), NativeControl.enforcement(true));
    expect(() => NativeControl.fromJson({'kind': 'automatic_enforcement', 'enabled': 'true'}), throwsFormatException);
    expect(() => NativeControl.fromJson({'kind': 'automatic_enforcement', 'enabled': true, 'generation': 12}), throwsFormatException);
    expect(() => NativeReport.fromJson(report(data: {'enforcement_runtime': {
      'enabled': 'true', 'generation': 1, 'accepted_requests': 0,
      'failed_requests': 0, 'last_error': null, 'scope': 'fixture only',
    }})), throwsFormatException);
  });

  testWidgets('enforcement intent waits for Rust; refusal and session changes cannot enable it', (tester) async {
    tester.view.physicalSize = const Size(900, 2400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var sequence = 1;
    Map<String, dynamic>? pendingCommit;
    Map<String, dynamic> status(bool enabled) => {
      'enabled': enabled, 'generation': enabled ? 1 : 0, 'accepted_requests': 0,
      'failed_requests': 0, 'last_error': null, 'scope': 'fixture only',
    };
    late ConsultationClient client;
    client = ConsultationClient(send: (message) async {
      final request = message['request'] as Map<String, dynamic>;
      final tools = (request['operation'] as Map<String, dynamic>)['request'] as Map<String, dynamic>;
      if (tools['action'] == 'commit') { pendingCommit = request; return; }
      complete(client, tools['action'] == 'prepare'
        ? report(outcome: 'prepared', state: nativeState(),
            control: tools['control'] as Map<String, dynamic>, data: {'confirmation_seconds': 30})
        : report(state: nativeState(), data: {'enforcement_runtime': status(false)}),
        id: request['id'] as int, sequence: ++sequence);
    });
    client.update(fixtures.view(), fixtures.sessionA, sequence, true, operational: true);
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [])))));
    await tester.pumpAndSettle();
    final enable = find.byKey(const ValueKey('tools.enforcementEnable'));
    await tester.ensureVisible(enable);
    await tester.tap(enable);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('tools.confirm')));
    await tester.pump(const Duration(milliseconds: 500));
    expect(pendingCommit, isNotNull);
    expect(find.text('Automatic termination disabled'), findsOneWidget);
    complete(client, report(outcome: 'not_confirmed', state: nativeState(),
      control: {'kind': 'automatic_enforcement', 'enabled': true},
      data: {'enforcement_runtime': status(false)}, error: 'Native authorization refused'),
      id: pendingCommit!['id'] as int, sequence: ++sequence);
    await tester.pumpAndSettle();
    expect(find.text('Automatic termination disabled'), findsOneWidget);
    expect(find.textContaining('Native authorization refused'), findsOneWidget);
    client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    await tester.pump();
    expect(tester.widget<OutlinedButton>(enable).onPressed, isNull);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });
  test('only explicit quit preparation may omit unrelated device observations', () {
    final quit = report(outcome: 'prepared', control: {'kind': 'quit_engine'});
    expect(NativeReport.fromJson(quit).state, isNull);
    expect(() => NativeReport.fromJson({...quit, 'control': {'kind': 'camera_block'}}), throwsFormatException);
    expect(() => NativeReport.fromJson(report(data: {'exit': {
      'ready': true, 'requested': true, 'scope': 'inconsistent fixture',
    }})), throwsFormatException);
  });

  testWidgets('explicit quit can use a private owner without granting device rights and cancellation is harmless', (tester) async {
    tester.view.physicalSize = const Size(900, 2400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var sequence = 1;
    var commits = 0;
    late ConsultationClient client;
    client = ConsultationClient(send: (message) async {
      final request = message['request'] as Map<String, dynamic>;
      final tools = (request['operation'] as Map<String, dynamic>)['request'] as Map<String, dynamic>;
      if (tools['action'] == 'commit') commits++;
      final result = tools['action'] == 'prepare'
        ? report(outcome: 'prepared', control: {'kind': 'quit_engine'}, data: {'confirmation_seconds': 30})
        : report(state: nativeState(), data: {'exit': {'ready': true, 'requested': false, 'scope': 'private fixture'}});
      client.update(fixtures.view(completion: fixtures.result(id: request['id'] as int, operation: 'tools', tools: result)),
        fixtures.sessionA, ++sequence, true, operational: false);
    });
    client.update(fixtures.view(), fixtures.sessionA, sequence, true, operational: false);
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [])))));
    await tester.pumpAndSettle();
    expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('tools.microphone_mute'))).onPressed, isNull);
    final quit = find.byKey(const ValueKey('tools.quit_engine'));
    await tester.ensureVisible(quit);
    expect(tester.widget<OutlinedButton>(quit).onPressed, isNotNull);
    await tester.tap(quit);
    await tester.pumpAndSettle();
    expect(find.byType(AlertDialog), findsOneWidget);
    await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
    await tester.pumpAndSettle();
    expect(commits, 0);
    client.update(fixtures.view(), fixtures.sessionB, 1, true);
    await tester.pump();
    expect(tester.widget<OutlinedButton>(quit).onPressed, isNull);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });
  test('application pages reject missing continuations and inconsistent snapshot metadata', () {
    final data = <String, dynamic>{'access_index': 1, 'access_count': 2,
      'next_cursor': 'c' * 32, 'snapshot_not_live': true,
      'process_identity_confirmed': true, 'snapshot_read_at': '2026-11-12T09:00:00Z', 'truncated': false};
    NativeReport.fromJson(report(data: data));
    for (final change in <Map<String, dynamic>>[
      {'access_index': 3}, {'access_count': 0}, {'next_cursor': null},
      {'snapshot_not_live': false}, {'process_identity_confirmed': 'yes'},
    ]) {
      expect(() => NativeReport.fromJson(report(data: {...data, ...change})), throwsFormatException);
    }
    expect(() => ToolsRequest.applicationNext('forged'), throwsArgumentError);
  });

  testWidgets('all simultaneous accesses are reachable and a new session retires page actions', (tester) async {
    tester.view.physicalSize = const Size(900, 2400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var sequence = 1;
    late ConsultationClient client;
    client = ConsultationClient(send: (message) async {
      final request = message['request'] as Map<String, dynamic>;
      final tools = (request['operation'] as Map<String, dynamic>)['request'] as Map<String, dynamic>;
      final next = tools['action'] == 'application_next';
      complete(client, tools['action'] == 'controls' ? report(state: nativeState()) : report(data: {
        'pid': 12345, 'application': 'Fixture recorder',
        'resource': next ? 'camera' : 'microphone', 'device': next ? 'camera-two' : 'input-one',
        'access_index': next ? 2 : 1, 'access_count': 2, 'next_cursor': next ? null : 'c' * 32,
        'snapshot_not_live': true, 'process_identity_confirmed': true,
        'snapshot_read_at': '2026-11-12T09:00:00Z', 'truncated': false,
      }), id: request['id'] as int, sequence: ++sequence);
    });
    client.update(fixtures.view(), fixtures.sessionA, sequence, true, operational: true);
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [
        {'pid': 12345, 'application': 'Fixture recorder', 'resource': 'microphone', 'device': 'input-one'},
        {'pid': 12345, 'application': 'Fixture recorder', 'resource': 'camera', 'device': 'camera-two'},
      ])))));
    await tester.pumpAndSettle();
    final application = find.byKey(const ValueKey('tools.application.12345'));
    await tester.ensureVisible(application);
    await tester.tap(application);
    await tester.pumpAndSettle();
    expect(find.textContaining('input-one'), findsOneWidget);
    final next = find.byKey(const ValueKey('tools.applicationNext'));
    await tester.ensureVisible(next);
    await tester.tap(next);
    await tester.pumpAndSettle();
    expect(find.textContaining('camera-two'), findsOneWidget);
    expect(find.textContaining('input-one'), findsNothing);
    expect(find.textContaining('2 / 2'), findsOneWidget);
    expect(next, findsNothing);
    final terminate = find.byKey(const ValueKey('tools.terminate'));
    expect(tester.widget<OutlinedButton>(terminate).onPressed, isNotNull);
    client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    await tester.pump();
    expect(tester.widget<OutlinedButton>(terminate).onPressed, isNull);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });

  testWidgets('a completion delivered immediately before session replacement cannot authorize the new view', (tester) async {
    late ConsultationClient client;
    client = ConsultationClient(send: (message) async {
      complete(client, report(state: nativeState()), id: message['request']['id'] as int);
      client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    });
    client.update(fixtures.view(), fixtures.sessionA, 1, true, operational: true);
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(
      child: NativeToolsPanel(client: client, locale: 'en', accesses: const [])))));
    await tester.pumpAndSettle();
    expect(client.session, fixtures.sessionB);
    expect(find.byKey(const ValueKey('tools.microphone_mute')), findsNothing);
    await tester.pumpWidget(const SizedBox());
    client.dispose();
  });
}
