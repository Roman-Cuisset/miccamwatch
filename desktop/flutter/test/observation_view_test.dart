import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/observation_view.dart';

Map<String, dynamic> row(String name, String resource, String risk, {String activity = 'active', String confidence = 'high'}) => {
  'application': name, 'resource': resource, 'risk': risk, 'activity': activity,
  'confidence': confidence, 'pid': null, 'device': null, 'started_at': null,
};
List<Map<String, dynamic>> records() => [
  row('Expected high-confidence microphone', 'microphone', 'expected'),
  row('Suspicious camera', 'camera', 'suspicious'),
  row('Ready blocked classification', 'camera', 'blocked', activity: 'ready', confidence: 'low'),
  row('Blocked classification microphone', 'microphone', 'blocked'),
  row('Unexplained camera', 'camera', 'unexplained'),
];

void main() {
  testWidgets('resource, native severity and Ready filters affect rows, not native facts; fresh observations are reconsidered', (tester) async {
    tester.view.physicalSize = const Size(900, 1800);tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);addTearDown(tester.view.resetDevicePixelRatio);
    var resource = 'all';var minimum = 0;var ready = true;var source = records();
    late StateSetter refresh;
    await tester.pumpWidget(MaterialApp(home: Scaffold(body: SingleChildScrollView(child:
      StatefulBuilder(builder: (context, setState) {
        refresh = setState;
        return ObservationView(accesses: source, locale: 'en', resource: resource,
          minimumRisk: minimum, includeReady: ready,
          onResource: (value) => setState(() => resource = value),
          onMinimumRisk: (value) => setState(() => minimum = value),
          onIncludeReady: (value) => setState(() => ready = value));
      })))));
    await tester.pumpAndSettle();
    expect(find.text('Expected high-confidence microphone'), findsOneWidget);
    final camera = find.byKey(const ValueKey('observations.resource.camera'));
    await tester.tap(camera);await tester.pumpAndSettle();
    expect(find.text('Expected high-confidence microphone'), findsNothing);
    expect(find.text('Unexplained camera'), findsOneWidget);
    final suspicious = find.byKey(const ValueKey('observations.risk.suspicious'));
    await tester.tap(suspicious);await tester.pumpAndSettle();
    expect(find.text('Unexplained camera'), findsNothing);
    expect(find.text('Ready blocked classification'), findsOneWidget);
    final includeReady = find.byKey(const ValueKey('observations.includeReady'));
    await tester.tap(includeReady);await tester.pumpAndSettle();
    expect(find.text('Ready blocked classification'), findsNothing);
    expect(find.text('Suspicious camera'), findsOneWidget);
    final blocked = find.byKey(const ValueKey('observations.risk.blocked'));
    await tester.tap(blocked);await tester.pumpAndSettle();
    expect(find.text('Suspicious camera'), findsNothing);
    expect(find.textContaining('does not prove there was no device access'), findsOneWidget);
    refresh(() => source = [row('Fresh camera observation', 'camera', 'blocked')]);
    await tester.pumpAndSettle();
    expect(find.text('Fresh camera observation'), findsOneWidget);
    expect(find.textContaining('does not prove there was no device access'), findsNothing);
    await tester.tap(find.byKey(const ValueKey('observations.resource.microphone')));await tester.pumpAndSettle();
    refresh(() => source = records());await tester.pumpAndSettle();
    expect(find.text('Blocked classification microphone'), findsOneWidget);
    expect(find.text('Expected high-confidence microphone'), findsNothing);
    expect(source.first['confidence'], 'high');
    expect(source.first['risk'], 'expected');
    expect(source[2]['activity'], 'ready');
  });

  testWidgets('same filters fit seven languages, both themes and 100/125/150 percent text layouts', (tester) async {
    tester.view.physicalSize = const Size(650, 900);tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);addTearDown(tester.view.resetDevicePixelRatio);
    for (final locale in const ['en','fr','de','es','ja','zh','ru']) {
      for (final brightness in Brightness.values) {
        for (final scale in const [1.0,1.25,1.5]) {
          await tester.pumpWidget(MaterialApp(theme: ThemeData(useMaterial3: true, brightness: brightness),
            home: MediaQuery(data: MediaQueryData(size: const Size(650, 900), textScaler: TextScaler.linear(scale)),
              child: Scaffold(body: SingleChildScrollView(child: ObservationView(
                accesses: records(), locale: locale, resource: 'all', minimumRisk: 0, includeReady: true,
                onResource: (_) {}, onMinimumRisk: (_) {}, onIncludeReady: (_) {}))))));
          await tester.pumpAndSettle();
          await tester.ensureVisible(find.text('Unexplained camera'));await tester.pumpAndSettle();
          expect(tester.takeException(), isNull, reason: '$locale/${brightness.name}/$scale');
          await tester.pumpWidget(const SizedBox());
        }
      }
    }
  });
}
