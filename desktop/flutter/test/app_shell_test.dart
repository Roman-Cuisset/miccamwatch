import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
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
}
