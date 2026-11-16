import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/consultation.dart';
import 'package:miccamwatch_desktop/session_model.dart';
import 'package:miccamwatch_desktop/session_panel.dart';

import 'consultation_test.dart' as transport;
import 'snapshot_test.dart' as snapshot;
import 'tools_test.dart' as tools;

Map<String, dynamic> state({bool active = false}) => {
  ...snapshot.frame()['observation_session'] as Map<String, dynamic>,
  if (active) ...{
    'phase': 'recording',
    'session_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    'started_at': '2026-11-16T12:00:00Z',
    'path': '/private/MicCamWatch/sessions/session-test.jsonl',
    'bytes_written': 1234,
    'events_written': 4,
    'cadence': snapshot.cadence(requested: 1000),
  },
};

void main() {
  testWidgets(
    'cancellation writes nothing and confirmation displays native readback',
    (tester) async {
      var sequence = 1;
      var confirms = 0;
      late ConsultationClient client;
      client = ConsultationClient(
        send: (message) async {
          final request = message['request'] as Map<String, dynamic>;
          final operation = request['operation'] as Map<String, dynamic>;
          final wire = operation['request'] as Map<String, dynamic>;
          final preparing = wire['action'] == 'session_start';
          if (preparing) {
            expect(wire.keys, unorderedEquals(['action', 'requested_ms']));
          } else {
            expect(wire['action'], 'session_confirm');
            expect(wire['consent'], 'confirm_observation_session');
            confirms += 1;
          }
          tools.complete(
            client,
            preparing
                ? tools.report(
                    outcome: 'prepared',
                    data: {
                      'observation_confirmation': true,
                      'confirmation_seconds': 30,
                    },
                  )
                : tools.report(
                    data: {'observation_session': state(active: true)},
                  ),
            id: request['id'] as int,
            sequence: ++sequence,
          );
        },
      );
      addTearDown(client.dispose);
      client.update(
        transport.view(),
        transport.sessionA,
        sequence,
        true,
        operational: true,
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(
              child: ObservationSessionPanel(
                client: client,
                locale: 'fr',
                status: ObservationSession.fromJson(state()),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.byKey(const ValueKey('session.start')));
      await tester.pumpAndSettle();
      expect(find.textContaining('JSONL privé'), findsOneWidget);
      await tester.tap(find.widgetWithText(TextButton, 'Annuler'));
      await tester.pumpAndSettle();
      expect(confirms, 0);
      await tester.tap(find.byKey(const ValueKey('session.start')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('session.confirm')));
      await tester.pumpAndSettle();
      expect(confirms, 1);
      expect(find.byKey(const ValueKey('session.path')), findsOneWidget);
      expect(
        find.text('Intervalle d’affichage demandé · 1000 ms'),
        findsOneWidget,
      );
      expect(
        find.text('Intervalle d’affichage effectif · 2000 ms'),
        findsOneWidget,
      );
      expect(find.text('Événements écrits · 4'), findsOneWidget);
      expect(find.byKey(const ValueKey('session.stop')), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'session stays readable in seven languages, two themes and three scales',
    (tester) async {
      tester.view.physicalSize = const Size(340, 720);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final client = ConsultationClient(send: (_) async {});
      addTearDown(client.dispose);
      client.update(
        transport.view(),
        transport.sessionA,
        1,
        true,
        operational: true,
      );
      for (final locale in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
        for (final theme in [ThemeData.light(), ThemeData.dark()]) {
          for (final scale in [1.0, 1.5, 2.0]) {
            await tester.pumpWidget(
              MaterialApp(
                theme: theme,
                home: Scaffold(
                  body: Builder(
                    builder: (context) => MediaQuery(
                      data: MediaQuery.of(context)
                          .copyWith(textScaler: TextScaler.linear(scale)),
                      child: SingleChildScrollView(
                        child: ObservationSessionPanel(
                          client: client,
                          locale: locale,
                          status: ObservationSession.fromJson(
                            state(active: true),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            );
            expect(find.byKey(const ValueKey('session.path')), findsOneWidget);
            expect(
              tester.takeException(),
              isNull,
              reason: '$locale ${theme.brightness} $scale',
            );
          }
        }
      }
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}
