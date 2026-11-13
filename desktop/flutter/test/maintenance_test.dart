import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/consultation.dart';
import 'package:miccamwatch_desktop/labels.dart';
import 'package:miccamwatch_desktop/tools_model.dart';
import 'package:miccamwatch_desktop/tools_panels.dart';
import 'consultation_test.dart' as fixtures;
import 'tools_test.dart' as tools;

Map<String, dynamic> installation() => {
  'version': '0.22.0', 'executable': '/isolated-fixture/bin/mcw',
  'native_owner_guidance': null, 'owner_guidance_available': false,
  'transactions_available': false, 'reason': 'hybrid_bundle_unqualified',
  'releases_url': 'https://github.com/Roman-Cuisset/miccamwatch/releases',
};
Map<String, dynamic> release() => {
  'current_version': '0.22.0', 'latest_tag': 'v17.2.3',
  'release_url': 'https://github.com/Roman-Cuisset/miccamwatch/releases/tag/v17.2.3',
  'update_available': true, 'checked_at': '2026-11-13T07:00:00Z',
  'scope': 'release_metadata_only',
};
Widget host(ConsultationClient client, {String locale = 'en', Brightness brightness = Brightness.light}) => MaterialApp(
  theme: ThemeData(useMaterial3: true, brightness: brightness),
  home: Scaffold(body: SingleChildScrollView(child: NativeToolsPanel(client: client, locale: locale, accesses: const []))));

void main() {
  test('metadata cannot claim installation authority, a foreign release or a verified package', () {
    final valid = NativeReport.fromJson(tools.report(data: {'release_check': release()}));
    expect(valid.releaseCheck!.tag, 'v17.2.3');
    expect(valid.releaseCheck!.available, isTrue);
    for (final patch in <Map<String, dynamic>>[
      {'release_url': 'https://example.invalid/package'},
      {'latest_tag': 'v17.2.4'}, {'update_available': 'true'},
      {'scope': 'package_signature_verified'}, {'checked_at': 'unknown'}, {'installed': true},
    ]) {
      expect(() => NativeReport.fromJson(tools.report(data: {'release_check': {...release(), ...patch}})), throwsFormatException);
    }
    for (final patch in <Map<String, dynamic>>[
      {'transactions_available': true}, {'owner_guidance_available': true},
      {'owner_guidance_available': true, 'native_owner_guidance': ''},
      {'reason': 'fully_qualified'}, {'command': 'uninstall'},
    ]) {
      expect(() => NativeReport.fromJson(tools.report(data: {'maintenance': {...installation(), ...patch}})), throwsFormatException);
    }
  });

  testWidgets('only a native reply completes a release check; error and disconnect leave an old report stale', (tester) async {
    tester.view.physicalSize = const Size(900, 1800);tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);addTearDown(tester.view.resetDevicePixelRatio);
    var sequence = 1;
    Map<String, dynamic>? pending;
    final actions = <String>[];
    late ConsultationClient client;
    client = ConsultationClient(send: (message) async {
      final request = message['request'] as Map<String, dynamic>;
      final query = (request['operation'] as Map<String, dynamic>)['request'] as Map<String, dynamic>;
      actions.add(query['action'] as String);
      if (query['action'] == 'controls') {
        tools.complete(client, tools.report(state: tools.nativeState()), id: request['id'] as int,
          sequence: ++sequence, session: request['session'] as String);
      } else if (query['action'] == 'maintenance') {
        tools.complete(client, tools.report(data: {'maintenance': installation()}), id: request['id'] as int,
          sequence: ++sequence, session: request['session'] as String);
      } else if (query['action'] == 'check_updates') {
        pending = request;
      } else {
        throw StateError('No install/device/process action is allowed in this test');
      }
    });
    client.update(fixtures.view(), fixtures.sessionA, sequence, true, operational: true);
    await tester.pumpWidget(host(client));await tester.pumpAndSettle();
    final check = find.byKey(const ValueKey('tools.checkUpdates'));
    await tester.ensureVisible(check);await tester.tap(check);await tester.pump();
    expect(client.toolsStatus.busy, isTrue);
    expect(find.textContaining('v17.2.3'), findsNothing);
    tools.complete(client, tools.report(data: {'release_check': release()}), id: pending!['id'] as int, sequence: ++sequence);
    await tester.pumpAndSettle();
    expect(client.toolsStatus.state, QueryState.succeeded);
    expect(find.byWidgetPredicate((w) => w is Text && (w.data?.contains('v17.2.3') ?? false)), findsOneWidget);
    await tester.ensureVisible(check);await tester.tap(check);await tester.pump();
    expect(find.text(Labels('en')['reportPrevious']), findsOneWidget);
    client.update(fixtures.view(completion: fixtures.result(id: pending!['id'] as int,
      operation: 'tools', state: 'failed', code: 'native_error', detail: 'Fixture network permission denied')),
      fixtures.sessionA, ++sequence, true, operational: true);
    await tester.pumpAndSettle();
    expect(client.toolsStatus.state, QueryState.failed);
    expect(find.text(Labels('en')['reportPrevious']), findsOneWidget);
    await tester.ensureVisible(check);await tester.tap(check);await tester.pump();
    client.update(fixtures.view(), fixtures.sessionA, ++sequence, false);
    await tester.pumpAndSettle();
    expect(client.toolsStatus.state, QueryState.uncertain);
    expect(tester.widget<OutlinedButton>(check).onPressed, isNull);
    client.update(fixtures.view(), fixtures.sessionB, 1, true, operational: true);
    await tester.pumpAndSettle();
    expect(find.text(Labels('en')['reportPrevious']), findsOneWidget);
    final info = find.byKey(const ValueKey('tools.installationActions'));
    await tester.ensureVisible(info);await tester.tap(info);await tester.pumpAndSettle();
    expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('tools.updateInstallation'))).onPressed, isNull);
    expect(tester.widget<OutlinedButton>(find.byKey(const ValueKey('tools.removeInstallation'))).onPressed, isNull);
    expect(actions.every((a) => const {'controls','maintenance','check_updates'}.contains(a)), isTrue);
    await tester.pumpWidget(const SizedBox());client.dispose();
  });

  testWidgets('read-only owner guidance fits both themes and all seven narrow translations', (tester) async {
    tester.view.physicalSize = const Size(650, 1200);tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);addTearDown(tester.view.resetDevicePixelRatio);
    for (final locale in const ['en','fr','de','es','ja','zh','ru']) {
      for (final brightness in Brightness.values) {
        var sequence = 1;
        late ConsultationClient client;
        client = ConsultationClient(send: (message) async {
          final request = message['request'] as Map<String, dynamic>;
          final query = (request['operation'] as Map<String, dynamic>)['request'] as Map<String, dynamic>;
          tools.complete(client, query['action'] == 'maintenance'
            ? tools.report(data: {'maintenance': installation()}) : tools.report(state: tools.nativeState()),
            id: request['id'] as int, sequence: ++sequence, session: request['session'] as String);
        });
        client.update(fixtures.view(), fixtures.sessionA, sequence, true, operational: true);
        await tester.pumpWidget(host(client, locale: locale, brightness: brightness));await tester.pumpAndSettle();
        final info = find.byKey(const ValueKey('tools.installationActions'));
        await tester.ensureVisible(info);await tester.tap(info);await tester.pumpAndSettle();
        final disabled = find.byKey(const ValueKey('tools.updateInstallation'));
        await tester.ensureVisible(disabled);await tester.pumpAndSettle();
        expect(tester.widget<OutlinedButton>(disabled).onPressed, isNull);
        expect(tester.takeException(), isNull, reason: '$locale/${brightness.name}');
        await tester.pumpWidget(const SizedBox());client.dispose();
      }
    }
  });
}
