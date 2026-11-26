import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:miccamwatch_desktop/labels.dart';
import 'package:miccamwatch_desktop/main.dart';

import 'snapshot_test.dart' as wire;

Map<String, dynamic> liveFrame({
  int sequence = 1,
  String locale = 'en',
  String platform = 'windows',
  String mode = 'operational',
}) {
  final value = wire.frame(sequence: sequence);
  value['observed_at'] = DateTime.now().toUtc().toIso8601String();
  value['locale'] = locale;
  value['engine'] = {
    'version': '0.22.0',
    'platform': platform,
    'mode': mode,
    'state': 'healthy',
    'detail': null,
  };
  value['controls'] = {
    'session': '0123456789abcdef0123456789abcdef',
    'microphone': {
      'can_enable': platform == 'windows' && mode == 'operational',
      'can_release': false,
      'current_requested': false,
      'generation': 1,
      'reason': null,
    },
    'command': null,
  };
  value['preferences'] = {
    'available': true,
    'writable': true,
    'profile_writable': true,
    'values': {
      'notifications_enabled': true,
      'sound_enabled': false,
      'history_enabled': true,
      'profile': 'balanced',
      'pause_notifications_until': null,
      'notifications_paused': false,
    },
    'applied_profile': 'balanced',
    'effects': {
      'capture_notifications_supported': true,
      'capture_sounds_supported': true,
      'history_recording_supported': true,
      'history_recording_active': true,
    },
    'reason': null,
    'command': null,
  };
  value['consultation'] = {
    'available': true,
    'reason': null,
    'result': null,
  };
  value['accesses'] = [
    {
      'resource': 'microphone',
      'activity': 'active',
      'confidence': 'high',
      'risk': 'unexplained',
      'application': 'VeryLongApplicationExecutableNameForNarrowLayoutVerification.exe',
      'pid': 4242,
      'device': 'USB Conference Audio Input Device (Channel 1-2)',
      'started_at': DateTime.now().toUtc().toIso8601String(),
    },
  ];
  return value;
}

List<int> wireLine(Map<String, dynamic> value) => [...wire.encode(value), 10];

void main() {
  testWidgets('shell fits minimum, boundary and zoomed layouts in seven languages and both themes', (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    tester.view.devicePixelRatio = 1;

    final sizes = [const Size(640, 440), const Size(680, 440), const Size(1040, 700)];
    const locales = ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru'];

    // Exercise minimum/boundary sizes and 200% text scaling in both themes across all pages.
    for (final dark in [false, true]) {
      for (final size in sizes) {
        tester.view.physicalSize = size;
        final controller = StreamController<List<int>>.broadcast();
        addTearDown(controller.close);
        final sent = <Map<String, dynamic>>[];
        await tester.pumpWidget(MediaQuery(
          data: MediaQueryData(
            size: size,
            textScaler: const TextScaler.linear(2.0),
            disableAnimations: true,
          ),
          child: MicCamWatchApp(
            dark: dark,
            inputStream: controller.stream,
            sendCommandOverride: (msg) async => sent.add(msg),
            onTransportDone: () {},
          ),
        ));
        controller.add(wireLine(liveFrame(locale: 'de', platform: 'linux', mode: 'settings_development')));
        await tester.pump();
        await tester.pump();
        expect(sent.any((m) => m['type'] == 'ready' && m['sequence'] == 1), isTrue);

        for (final page in AppPage.values) {
          final nav = find.byKey(ValueKey('navigation.${page.name}'));
          await tester.ensureVisible(nav);
          await tester.tap(nav);
          await tester.pump();
          expect(tester.takeException(), isNull, reason: 'dark=$dark size=$size page=${page.name}');
        }
        await tester.pumpWidget(const SizedBox.shrink());
      }
    }

    // Exercise all 7 languages at the 680x440 expanded sidebar threshold with 1.5x text scale.
    tester.view.physicalSize = const Size(680, 440);
    final controller = StreamController<List<int>>.broadcast();
    addTearDown(controller.close);
    await tester.pumpWidget(MediaQuery(
      data: const MediaQueryData(
        size: Size(680, 440),
        textScaler: TextScaler.linear(1.5),
      ),
      child: MicCamWatchApp(
        dark: false,
        inputStream: controller.stream,
        sendCommandOverride: (_) async {},
        onTransportDone: () {},
      ),
    ));
    var seq = 0;
    for (final locale in locales) {
      controller.add(wireLine(liveFrame(sequence: ++seq, locale: locale)));
      await tester.pump();
      expect(tester.takeException(), isNull, reason: 'locale=$locale');
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('keyboard activation navigates pages, toggles theme and operational About omits development', (tester) async {
    tester.view.physicalSize = const Size(1040, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final controller = StreamController<List<int>>.broadcast();
    var closed = false;
    await tester.pumpWidget(MicCamWatchApp(
      dark: false,
      inputStream: controller.stream,
      sendCommandOverride: (_) async {},
      onTransportDone: () => closed = true,
    ));
    controller.add(wireLine(liveFrame(sequence: 1, locale: 'en', mode: 'operational')));
    await tester.pump();
    await tester.pump();

    // Traverse via Tab key to reach the About navigation button and activate with Enter.
    bool aboutFocused() {
      var focused = false;
      FocusManager.instance.primaryFocus?.context?.visitAncestorElements((element) {
        if (element.widget.key == const ValueKey('navigation.about')) {
          focused = true;
        }
        return !focused;
      });
      return focused;
    }
    for (var i = 0; i < 12 && !aboutFocused(); i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(aboutFocused(), isTrue, reason: 'Tab navigation should reach About button');
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(find.text('MicCamWatch · 0.22.0'), findsOneWidget);
    expect(find.text('MicCamWatch · 0.22.0 · development'), findsNothing);

    // Toggle theme via button key and confirm dark mode updates label.
    expect(find.text('Dark'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('theme.toggle')));
    await tester.pump();
    expect(find.text('Light'), findsOneWidget);

    // Closing the transport switches cleanly to disconnected without throwing.
    await controller.close();
    await tester.pump();
    expect(closed, isTrue);
    expect(find.text('Disconnected'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
  });

  testWidgets('all seven languages fit all six pages at 640x440 and 680x440 with 2x text scale in operational mode', (tester) async {
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    tester.view.devicePixelRatio = 1;
    const locales = ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru'];

    for (final size in const [Size(640, 440), Size(680, 440)]) {
      tester.view.physicalSize = size;
      final controller = StreamController<List<int>>.broadcast();
      addTearDown(controller.close);
      await tester.pumpWidget(MediaQuery(
        data: MediaQueryData(
          size: size,
          textScaler: const TextScaler.linear(2.0),
          disableAnimations: true,
        ),
        child: MicCamWatchApp(
          dark: size.width == 680,
          inputStream: controller.stream,
          sendCommandOverride: (_) async {},
          onTransportDone: () {},
        ),
      ));
      var seq = 0;
      for (final locale in locales) {
        controller.add(wireLine(liveFrame(
          sequence: ++seq,
          locale: locale,
          platform: 'windows',
          mode: 'operational',
        )));
        await tester.pump();
        for (final page in AppPage.values) {
          final nav = find.byKey(ValueKey('navigation.${page.name}'));
          await tester.ensureVisible(nav);
          await tester.tap(nav);
          await tester.pump();
          expect(
            tester.takeException(),
            isNull,
            reason: 'size=$size locale=$locale page=${page.name}',
          );
        }
      }
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });

  testWidgets('critical state transitions update UI reliably without missed refreshes', (tester) async {
    tester.view.physicalSize = const Size(1040, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final controller = StreamController<List<int>>.broadcast();
    final sent = <Map<String, dynamic>>[];
    await tester.pumpWidget(MicCamWatchApp(
      dark: false,
      inputStream: controller.stream,
      sendCommandOverride: (msg) async => sent.add(msg),
      onTransportDone: () {},
    ));
    var seq = 0;
    Future<void> push(Map<String, dynamic> frame) async {
      controller.add(wireLine(frame));
      await tester.pump();
      await tester.pump();
    }

    // 1. Microphone active + Camera active
    final activeBoth = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    activeBoth['resources'] = [
      {'resource': 'microphone', 'activity': 'active', 'coverage': 'complete', 'confidence': 'high', 'active_count': 1, 'ready_count': 0},
      {'resource': 'camera', 'activity': 'active', 'coverage': 'complete', 'confidence': 'medium', 'active_count': 1, 'ready_count': 0},
    ];
    activeBoth['accesses'] = [
      {
        'resource': 'microphone', 'activity': 'active', 'confidence': 'high', 'risk': 'expected',
        'application': 'VoiceCall.exe', 'pid': 1001, 'device': 'USB Mic', 'started_at': DateTime.now().toUtc().toIso8601String(),
      },
      {
        'resource': 'camera', 'activity': 'active', 'confidence': 'medium', 'risk': 'unexplained',
        'application': 'WebcamApp.exe', 'pid': 2002, 'device': 'Integrated Camera', 'started_at': DateTime.now().toUtc().toIso8601String(),
      },
    ];
    await push(activeBoth);
    final en = Labels('en');
    expect(find.text(en['active']), findsNWidgets(2));
    expect(find.text('VoiceCall.exe'), findsOneWidget);
    expect(find.text('WebcamApp.exe'), findsOneWidget);

    // 2. Monitoring degraded (camera gap, active microphone preserved)
    final degraded = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    degraded['engine'] = {'version': '0.22.0', 'platform': 'windows', 'mode': 'operational', 'state': 'degraded', 'detail': 'Camera collector degraded'};
    degraded['resources'] = [
      {'resource': 'microphone', 'activity': 'active', 'coverage': 'complete', 'confidence': 'high', 'active_count': 1, 'ready_count': 0},
      {'resource': 'camera', 'activity': 'unknown', 'coverage': 'incomplete', 'confidence': null, 'active_count': 0, 'ready_count': 0},
    ];
    degraded['collectors'] = [
      {'collector': 'wasapi', 'state': 'healthy', 'detail': null},
      {'collector': 'privacy_store', 'state': 'degraded', 'detail': 'Partial read'},
    ];
    degraded['observation_gaps'] = ['camera'];
    await push(degraded);
    expect(find.text(en['active']), findsOneWidget);
    expect(find.text(en['unknown']), findsWidgets);
    expect(find.text('Camera collector degraded'), findsOneWidget);

    // 3. Monitoring unavailable (all collectors unavailable, both gaps)
    final unavailable = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    unavailable['engine'] = {'version': '0.22.0', 'platform': 'windows', 'mode': 'operational', 'state': 'unavailable', 'detail': 'Collectors offline'};
    unavailable['resources'] = [
      {'resource': 'microphone', 'activity': 'unknown', 'coverage': 'unavailable', 'confidence': null, 'active_count': 0, 'ready_count': 0},
      {'resource': 'camera', 'activity': 'unknown', 'coverage': 'unavailable', 'confidence': null, 'active_count': 0, 'ready_count': 0},
    ];
    unavailable['collectors'] = [
      {'collector': 'wasapi', 'state': 'unavailable', 'detail': 'No audio service'},
    ];
    unavailable['observation_gaps'] = ['microphone', 'camera'];
    unavailable['accesses'] = <Map<String, dynamic>>[];
    await push(unavailable);
    expect(find.text(en['active']), findsNothing);
    expect(find.text('Collectors offline'), findsOneWidget);

    // 4. Settings: profile modified (saved != applied, then explicit config applied == null),
    //    preferences confirmed (succeeded) and refused (rejected).
    await tester.tap(find.byKey(const ValueKey('navigation.settings')));
    await tester.pump();
    // Complete the automatic autostart readback triggered on first Settings visit.
    final autostartReq = sent.lastWhere((m) => m['type'] == 'consultation_request')['request'] as Map<String, dynamic>;
    final autostartReply = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    autostartReply['consultation'] = {
      'available': true,
      'reason': null,
      'result': {
        'id': autostartReq['id'],
        'state': 'succeeded',
        'code': null,
        'detail': null,
        'operation': 'autostart_status',
        'history': null,
        'diagnostics': null,
        'autostart': {'enabled': false, 'writable': true, 'reason': null},
        'tools': null,
      },
    };
    await push(autostartReply);
    final prefConfirmed = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    (prefConfirmed['preferences'] as Map<String, dynamic>)['values']['profile'] = 'private';
    (prefConfirmed['preferences'] as Map<String, dynamic>)['applied_profile'] = 'balanced';
    await push(prefConfirmed);
    expect(
      tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.profileEffect')),
        matching: find.byType(Text),
      )).data,
      'Uses the standard capture assessment.',
    );

    // Explicit config (applied_profile == null, profile_writable == false)
    final prefExplicit = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    (prefExplicit['preferences'] as Map<String, dynamic>)['profile_writable'] = false;
    (prefExplicit['preferences'] as Map<String, dynamic>)['values']['profile'] = 'private';
    (prefExplicit['preferences'] as Map<String, dynamic>)['applied_profile'] = null;
    await push(prefExplicit);
    expect(
      tester.widget<Text>(find.descendant(
        of: find.byKey(const ValueKey('preferences.profileEffect')),
        matching: find.byType(Text),
      )).data,
      contains('not confirmed'),
    );

    // Trigger a preference write and confirm both succeeded and rejected native outcomes.
    final soundSwitch = find.descendant(
      of: find.byType(SwitchListTile).at(1),
      matching: find.byType(Switch),
    );
    await tester.ensureVisible(soundSwitch);
    await tester.tap(soundSwitch);
    await tester.pump();
    final prefReq1 = sent.lastWhere((m) => m['type'] == 'preferences_command')['request'] as Map<String, dynamic>;
    final prefOk = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    (prefOk['preferences'] as Map<String, dynamic>)['values']['sound_enabled'] = true;
    (prefOk['preferences'] as Map<String, dynamic>)['command'] = {
      'id': prefReq1['id'], 'key': 'sound_enabled', 'value': true,
      'state': 'succeeded', 'code': null, 'detail': null,
    };
    await push(prefOk);
    expect(find.byKey(const ValueKey('preferences.commandStatus')), findsOneWidget);
    expect(find.textContaining('Saved by native'), findsOneWidget);

    // Refuse next preference write and verify authoritative value stays true.
    await tester.tap(soundSwitch);
    await tester.pump();
    final prefReq2 = sent.lastWhere((m) => m['type'] == 'preferences_command')['request'] as Map<String, dynamic>;
    final prefRefused = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    (prefRefused['preferences'] as Map<String, dynamic>)['values']['sound_enabled'] = true;
    (prefRefused['preferences'] as Map<String, dynamic>)['command'] = {
      'id': prefReq2['id'], 'key': 'sound_enabled', 'value': false,
      'state': 'rejected', 'code': 'cas_mismatch', 'detail': 'Changed concurrently',
    };
    await push(prefRefused);
    expect(find.textContaining('Change rejected'), findsOneWidget);
    expect(find.text('Changed concurrently'), findsOneWidget);

    // 5. History refreshed and updated via consultation response.
    await tester.tap(find.byKey(const ValueKey('navigation.history')));
    await tester.pump();
    final histReq = sent.lastWhere((m) => m['type'] == 'consultation_request')['request'] as Map<String, dynamic>;
    final histReply = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    histReply['consultation'] = {
      'available': true,
      'reason': null,
      'result': {
        'id': histReq['id'],
        'state': 'succeeded',
        'code': null,
        'detail': null,
        'operation': 'history',
        'history': {
          'events': [
            {
              'observed_at': '2026-11-25T10:00:00Z',
              'action': 'observed',
              'resource': 'microphone',
              'activity': 'active',
              'confidence': 'high',
              'application': 'RecordedMeeting.exe',
              'pid': 777,
            },
          ],
          'next_cursor': null,
          'incomplete': false,
          'issues': <String>[],
          'ordering': 'recorded_newest_first',
        },
        'diagnostics': null,
        'autostart': null,
        'tools': null,
      },
    };
    await push(histReply);
    expect(find.textContaining('RecordedMeeting.exe'), findsOneWidget);

    // 6. Diagnostics: macOS notification authorized & denied states localized cleanly.
    await tester.tap(find.byKey(const ValueKey('navigation.diagnostics')));
    await tester.pump();
    for (final state in const [
      ('macos_authorization=authorized; banner_delivery=unverified', 'macosNotificationsAuthorized'),
      ('macos_authorization=denied; banner_delivery=unverified', 'macosNotificationsDenied'),
      ('macos_authorization=authorized_without_alerts; banner_delivery=unverified', 'macosNotificationsNoAlerts'),
    ]) {
      final runBtn = find.byKey(const ValueKey('diagnostics.run'));
      await tester.ensureVisible(runBtn);
      await tester.tap(runBtn);
      await tester.pump();
      final diagReq = sent.lastWhere((m) => m['type'] == 'consultation_request')['request'] as Map<String, dynamic>;
      final diagReply = liveFrame(sequence: ++seq, locale: 'fr', platform: 'macos', mode: 'operational');
      diagReply['consultation'] = {
        'available': true,
        'reason': null,
        'result': {
          'id': diagReq['id'],
          'state': 'succeeded',
          'code': null,
          'detail': null,
          'operation': 'diagnostics',
          'history': null,
          'diagnostics': {
            'checks': [
              {
                'name': 'notification_authorization',
                'status': state.$1.contains('denied') ? 'warning' : 'ok',
                'detail': state.$1,
                'remedy': null,
              },
            ],
            'limitations': <String>[],
            'engine_version': '0.22.0',
            'platform': 'macos',
            'observed_at': '2026-11-25T10:01:00Z',
            'incomplete': false,
          },
          'autostart': null,
          'tools': null,
        },
      };
      await push(diagReply);
      final fr = Labels('fr');
      expect(find.text(fr[state.$2]), findsOneWidget);
      expect(find.text(fr['macosBannerUnverified']), findsOneWidget);
    }

    // 7. IPC stale observation -> reconnect with fresh snapshot -> clean EOF disconnect.
    await tester.tap(find.byKey(const ValueKey('navigation.overview')));
    await tester.pump();
    await push(liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational'));
    expect(find.text(en['live']), findsOneWidget);

    // A stale observation timestamp transitions cleanly to Stale and shows historical warning.
    final staleFrame = liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational');
    staleFrame['observed_at'] = DateTime.now().toUtc().subtract(const Duration(seconds: 15)).toIso8601String();
    await push(staleFrame);
    expect(find.text(en['stale']), findsOneWidget);
    expect(find.text(en['historical']), findsOneWidget);

    // Reconnect with a newer fresh snapshot -> must immediately return to Live.
    await push(liveFrame(sequence: ++seq, locale: 'en', platform: 'windows', mode: 'operational'));
    expect(find.text(en['live']), findsOneWidget);
    expect(find.text(en['historical']), findsNothing);

    await controller.close();
    await tester.pump();
    await tester.pump();
    expect(find.text(en['disconnected']), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
  });

  testWidgets('accessibility: navigation semantics stay intact, tooltips do not duplicate, Material panels host SwitchListTile focus, and local clock ticks independently', (tester) async {
    final semantics = tester.ensureSemantics();
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    tester.view.devicePixelRatio = 1;

    try {
      for (final size in const [Size(640, 440), Size(1040, 700)]) {
        tester.view.physicalSize = size;
        final controller = StreamController<List<int>>.broadcast();
        addTearDown(controller.close);
        await tester.pumpWidget(MicCamWatchApp(
          dark: false,
          inputStream: controller.stream,
          sendCommandOverride: (_) async {},
          onTransportDone: () {},
        ));
        controller.add(wireLine(liveFrame(sequence: 1, locale: 'en', platform: 'windows', mode: 'operational')));
        await tester.pump();
        await tester.pump();

        final root = tester.binding.renderViews.single.owner!.semanticsOwner!.rootSemanticsNode!;
        SemanticsNode? findById(SemanticsNode node, String id) {
          if (node.getSemanticsData().identifier == id) return node;
          SemanticsNode? hit;
          node.visitChildren((child) {
            hit ??= findById(child, id);
            return hit == null;
          });
          return hit;
        }
        bool hasTapAction(SemanticsNode node) {
          if (node.getSemanticsData().hasAction(SemanticsAction.tap)) return true;
          var found = false;
          node.visitChildren((child) {
            if (hasTapAction(child)) found = true;
            return !found;
          });
          return found;
        }

        for (final page in AppPage.values) {
          final node = findById(root, 'navigation.${page.name}');
          expect(node, isNotNull, reason: 'Missing semantics node for navigation.${page.name} at $size');
          expect(hasTapAction(node!), isTrue, reason: 'Missing tap action for navigation.${page.name} at $size');
          // Tooltip must not duplicate the button label in semantics.
          expect(node.getSemanticsData().tooltip, isEmpty);
        }
        expect(findById(root, 'theme.toggle'), isNotNull);

        // SwitchListTile in ObservationView must have a Material ancestor matching Palette.card.
        final includeReadyTile = find.byKey(const ValueKey('observations.includeReady'));
        expect(includeReadyTile, findsOneWidget);
        final panelMaterial = tester.widget<Material>(
          find.ancestor(of: includeReadyTile, matching: find.byType(Material)).first,
        );
        expect(panelMaterial.color, const Palette(false).card);

        // Clock widget remains rendered and ticks cleanly on 1s timer without throwing.
        final clockPrefix = '${Labels('en')['clock']}: ';
        expect(find.textContaining(clockPrefix), findsOneWidget);
        await tester.pump(const Duration(seconds: 2));
        expect(find.textContaining(clockPrefix), findsOneWidget);
        expect(tester.takeException(), isNull);

        await tester.pumpWidget(const SizedBox.shrink());
      }
    } finally {
      semantics.dispose();
    }
  });
}
