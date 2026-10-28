import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'labels.dart';
import 'commands.dart';
import 'snapshot.dart' as wire;
import 'preferences.dart';
import 'preferences_model.dart';

final _started = Stopwatch()..start();
Future<void> _output = Future<void>.value();

void emit(Map<String, Object> event) {
  _output = _output.then((_) async {
    stdout.writeln(jsonEncode({'protocol_version': wire.protocolVersion, ...event}));
    await stdout.flush();
  }).catchError((Object _) {});
}

Future<void> sendCommand(Map<String, dynamic> event) {
  final sent = _output.then((_) async {
    stdout.writeln(jsonEncode(event));
    await stdout.flush();
  });
  _output = sent.catchError((Object _) {});
  return sent;
}
void timing(String stage) => emit({'type': 'timing', 'stage': stage, 'elapsed_ms': _started.elapsedMilliseconds});

void main(List<String> arguments) {
  timing('dart_started');
  WidgetsFlutterBinding.ensureInitialized();
  var theme = 'system';
  for (var i = 0; i < arguments.length; i++) {
    if (arguments[i].startsWith('--theme=')) {
      theme = arguments[i].substring('--theme='.length);
    } else if (arguments[i] == '--theme' && i + 1 < arguments.length) {
      theme = arguments[++i];
    }
  }
  final dark = theme == 'dark' || (theme == 'system' &&
      WidgetsBinding.instance.platformDispatcher.platformBrightness == Brightness.dark);
  runApp(MicCamWatchApp(dark: dark));
  WidgetsBinding.instance.addPostFrameCallback((_) => timing('first_frame'));
}

class Palette {
  const Palette(this.dark);
  final bool dark;
  Color get background => Color(dark ? 0xFF111A17 : 0xFFF4F6F5);
  Color get card => Color(dark ? 0xFF1C2822 : 0xFFFFFFFF);
  Color get text => Color(dark ? 0xFFEAF2ED : 0xFF172720);
  Color get secondary => Color(dark ? 0xFFADBCB3 : 0xFF53675D);
  Color get border => Color(dark ? 0xFF31433A : 0xFFDDE5E0);
  Color get accent => Color(dark ? 0xFF66D5AD : 0xFF167454);
  Color get tint => Color(dark ? 0xFF233C30 : 0xFFE7F2EC);
  Color get warning => Color(dark ? 0xFFE3C991 : 0xFF76551A);
  TextStyle type(double size, {bool muted = false, bool bold = false}) => TextStyle(
    fontSize: size, height: 1.35, color: muted ? secondary : text,
    fontWeight: bold ? FontWeight.w600 : FontWeight.w400);
}

class MicCamWatchApp extends StatefulWidget {
  const MicCamWatchApp({super.key, required this.dark});
  final bool dark;
  @override
  State<MicCamWatchApp> createState() => _MicCamWatchAppState();
}

class _MicCamWatchAppState extends State<MicCamWatchApp> {
  final session = wire.SnapshotSession();
  late final wire.SnapshotInput input;
  late final MicrophoneCommandClient microphoneCommands;
  late final PreferenceCommandClient preferenceCommands;
  late final Timer clock;
  late bool dark = widget.dark;
  int page = 0;
  bool ready = false;
  bool receivedTiming = false;
  bool closing = false;
  static const window = MethodChannel('miccamwatch/window');

  @override
  void initState() {
    super.initState();
    microphoneCommands = MicrophoneCommandClient(send: sendCommand);
    preferenceCommands = PreferenceCommandClient(send: sendCommand);
    session.addListener(_changed);
    clock = Timer.periodic(const Duration(seconds: 1), (_) => session.tick());
    input = wire.SnapshotInput(stdin, session, onSnapshot: () {
      if (!receivedTiming && (session.sequence ?? 0) > 0) {
        receivedTiming = true;
        timing('snapshot_received');
      }
    }, onDone: _close);
    WidgetsBinding.instance.addPostFrameCallback((_) => _frameTheme());
  }

  void _changed() {
    if (!mounted || closing) return;
    microphoneCommands.update(session.snapshot?.controls ?? const <String, dynamic>{},
        session.sequence ?? 0, session.transportLive);
    preferenceCommands.update(
        session.snapshot?.preferences ?? const PreferenceView.unavailable(),
        session.snapshot?.controls['session'] as String? ?? '',
        session.sequence ?? 0, session.transportLive);
    setState(() {});
  }
  Future<void> _close() async {
    if (closing) return;
    closing = true;
    clock.cancel();
    await input.close();
    await _output;
    // The disposable process must not leave an engine behind after parent EOF.
    exit(0);
  }

  @override
  void dispose() {
    clock.cancel();
    input.close();
    session.removeListener(_changed);
    session.dispose();
    microphoneCommands.dispose();
    preferenceCommands.dispose();
    super.dispose();
  }

  Future<void> _theme() async {
    setState(() => dark = !dark);
    await _frameTheme();
  }

  Future<void> _frameTheme() async {
    if (Platform.isWindows) {
      try { await window.invokeMethod<void>('setTheme', dark); }
      on PlatformException { /* Unsupported frame styling is non-fatal. */ }
      on MissingPluginException { /* Other runners keep their native frame. */ }
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = Palette(dark);
    final snapshot = session.snapshot;
    final l = Labels(snapshot?.locale ?? 'en');
    final sequence = snapshot?.sequence;
    if (!ready && session.isLive && sequence != null && sequence > 0) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!ready && mounted && session.isLive && session.sequence == sequence) {
          ready = true;
          emit({'type': 'ready', 'sequence': sequence});
        }
      });
    }
    final titles = [l['overview'], l['settings'], l['diagnostics'], l['about']];
    return MaterialApp(
      title: 'MicCamWatch', debugShowCheckedModeBanner: false,
      themeAnimationDuration: Duration.zero,
      theme: ThemeData(
        brightness: dark ? Brightness.dark : Brightness.light,
        fontFamily: Platform.isWindows ? 'Segoe UI' : null,
        scaffoldBackgroundColor: c.background,
        colorScheme: ColorScheme.fromSeed(seedColor: c.accent,
          brightness: dark ? Brightness.dark : Brightness.light),
        splashFactory: NoSplash.splashFactory,
        useMaterial3: true,
      ),
      home: Scaffold(body: Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        Container(width: 180, padding: const EdgeInsets.fromLTRB(12, 22, 12, 16),
          decoration: BoxDecoration(color: c.card, border: Border(right: BorderSide(color: c.border))),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Padding(padding: const EdgeInsets.only(left: 8), child: Row(children: [
              Image.asset('assets/miccamwatch.png', width: 28, height: 28, excludeFromSemantics: true),
              const SizedBox(width: 7),
              Text('MicCamWatch', style: c.type(15, bold: true)),
            ])),
            const SizedBox(height: 26),
            for (var i = 0; i < titles.length; i++) Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: TextButton(
                style: TextButton.styleFrom(alignment: Alignment.centerLeft,
                  padding: const EdgeInsets.all(12),
                  foregroundColor: page == i ? c.accent : c.secondary,
                  backgroundColor: page == i ? c.tint : Colors.transparent,
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8))),
                onPressed: () => setState(() => page = i),
                child: Row(children: [
                  Icon([Icons.dashboard_outlined, Icons.tune_outlined, Icons.monitor_heart_outlined, Icons.info_outline][i], size: 18),
                  const SizedBox(width: 10), Expanded(child: Text(titles[i])),
                ]))),
            const Spacer(),
            Text('Protocol ${wire.protocolVersion}', style: c.type(11, muted: true)),
          ])),
        Expanded(child: SingleChildScrollView(padding: const EdgeInsets.all(22),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Row(children: [Expanded(child: Text(titles[page], style: c.type(27, bold: true))),
              TextButton.icon(onPressed: _theme,
                icon: Icon(dark ? Icons.light_mode_outlined : Icons.dark_mode_outlined, size: 18),
                label: Text(l[dark ? 'light' : 'dark'])),
            ]),
            const SizedBox(height: 10),
            _panel(c, Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(l[snapshot?.engine['mode'] == 'read_only_development' ? 'readOnly'
                  : snapshot?.engine['mode'] == 'controlled_development' ? 'controlled'
                  : snapshot?.engine['mode'] == 'settings_development' ? 'settingsDevelopment' : 'hybrid'],
                style: c.type(14, bold: true)),
              const SizedBox(height: 4),
              Text(l[sequence == 0 ? 'connecting' : session.state.name],
                style: c.type(13).copyWith(color: session.isLive && sequence != 0 ? c.accent : c.warning)),
              if (session.error != null) Text(session.error!, style: c.type(12, muted: true)),
              if (snapshot != null) Text('Engine ${snapshot.engine['version']} · ${snapshot.engine['platform']} · ${l[snapshot.engine['state'] as String]}', style: c.type(12, muted: true)),
              if (snapshot?.engine['detail'] != null) Text(snapshot!.engine['detail'] as String, style: c.type(12, muted: true)),
            ])),
            const SizedBox(height: 14),
            if (page == 0) ..._overview(c, l, snapshot),
            if (page == 1) PreferencesPanel(client: preferenceCommands, locale: snapshot?.locale ?? 'en'),
            if (page == 2) ..._diagnostics(c, l, snapshot),
            if (page == 3) ..._about(c, l, snapshot),
            const SizedBox(height: 14),
            Text(l['limits'], style: c.type(12, muted: true)),
            const SizedBox(height: 12),
            Wrap(spacing: 18, runSpacing: 5, children: [
              Text('${l['clock']}: ${_time(DateTime.now())}', style: c.type(11, muted: true)),
              Text('${l['lastUpdate']}: ${session.receivedAt == null ? '—' : _time(session.receivedAt!)}', style: c.type(11, muted: true)),
              if (snapshot != null) Text('${l['observedAt']}: ${snapshot.data['observed_at']} · #${snapshot.sequence}', style: c.type(11, muted: true)),
            ]),
          ]))),
      ])),
    );
  }

  Widget _panel(Palette c, Widget child) => Container(
    width: double.infinity, padding: const EdgeInsets.all(16),
    decoration: BoxDecoration(color: c.card, borderRadius: BorderRadius.circular(10),
      border: Border.all(color: c.border)), child: child);

  List<Widget> _overview(Palette c, Labels l, wire.Snapshot? snapshot) => [
    if (!session.isLive && snapshot != null) Padding(padding: const EdgeInsets.only(bottom: 10),
      child: Text(l['historical'], style: c.type(13).copyWith(color: c.warning))),
    LayoutBuilder(builder: (context, constraints) {
      final cards = <Widget>[
        for (var i = 0; i < 2; i++) _resource(c, l, wire.resourceNames[i], snapshot, i),
      ];
      if (constraints.maxWidth < 580) return Column(children: [cards[0], const SizedBox(height: 12), cards[1]]);
      return Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Expanded(child: cards[0]), const SizedBox(width: 12), Expanded(child: cards[1]),
      ]);
    }),
    const SizedBox(height: 12),
    MicrophoneCommandPanel(client: microphoneCommands, locale: snapshot?.locale ?? 'en'),
    const SizedBox(height: 18),
    Text(l['applications'], style: c.type(17, bold: true)),
    const SizedBox(height: 9),
    _panel(c, snapshot == null || snapshot.accesses.isEmpty
      ? Text(l['noRows'], style: c.type(13, muted: true))
      : Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          for (final access in snapshot.accesses) Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Icon(access['resource'] == 'microphone' ? Icons.mic_none : Icons.videocam_outlined,
                size: 20, color: c.secondary), const SizedBox(width: 10),
              Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(access['application'] as String, style: c.type(14, bold: true)),
                Text('${l[access['resource'] as String]} · PID ${access['pid'] ?? '—'} · ${l[access['activity'] as String]}', style: c.type(12, muted: true)),
                Text('${l['confidence']}: ${l[access['confidence'] as String? ?? 'unknown']}', style: c.type(12, muted: true)),
                if (access['device'] != null) Text(access['device'] as String, style: c.type(12, muted: true)),
                if (access['started_at'] != null) Text(access['started_at'] as String, style: c.type(12, muted: true)),
              ])),
            ])),
        ])),
    if (snapshot?.truncated ?? false) Padding(padding: const EdgeInsets.only(top: 10),
      child: Text(l['truncated'], style: c.type(12).copyWith(color: c.warning))),
  ];

  Widget _resource(Palette c, Labels l, String name, wire.Snapshot? snapshot, int index) {
    final resource = snapshot?.resources[index];
    final activity = session.isLive && session.sequence != 0 && resource != null
        ? snapshot!.activity(resource) : 'unknown';
    final protection = session.isLive ? snapshot?.protection(name) : null;
    return _panel(c, Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Row(children: [Icon(name == 'microphone' ? Icons.mic_none : Icons.videocam_outlined,
        color: c.secondary, size: 22), const SizedBox(width: 8),
        Expanded(child: Text(l[name], style: c.type(16, bold: true)))]),
      const SizedBox(height: 12),
      Text(l[activity], style: c.type(21, bold: true).copyWith(color: activity == 'active' ? c.accent : c.text)),
      const SizedBox(height: 7),
      Text('${l['coverage']}: ${l[resource?['coverage'] as String? ?? 'unavailable']}', style: c.type(12, muted: true)),
      Text('${l['confidence']}: ${l[resource?['confidence'] as String? ?? 'unknown']}', style: c.type(12, muted: true)),
      if (resource != null) Text('${l['active']}: ${resource['active_count']} · ${l['ready']}: ${resource['ready_count']}', style: c.type(12, muted: true)),
      const SizedBox(height: 10),
      Divider(color: c.border, height: 1),
      const SizedBox(height: 10),
      Text(l[protection?['state'] as String? ?? 'unknown'],
        style: c.type(12, bold: true)),
      const SizedBox(height: 4),
      Text('${l['requested']}: ${_truth(l, protection?['requested'])}',
        style: c.type(12, muted: true)),
      Text('${l['guard']}: ${_truth(l, protection?['service_active'])}',
        style: c.type(12, muted: true)),
      if (protection?['requested'] == true && protection?['service_active'] != true)
        Padding(padding: const EdgeInsets.only(top: 5),
          child: Text(l['guardMissing'],
            style: c.type(12).copyWith(color: c.warning))),
    ]));
  }

  List<Widget> _diagnostics(Palette c, Labels l, wire.Snapshot? snapshot) => [
    if (!session.isLive && snapshot != null) Text(l['historical'], style: c.type(13).copyWith(color: c.warning)),
    Text(l['collectors'], style: c.type(17, bold: true)),
    const SizedBox(height: 9),
    _panel(c, Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      if (snapshot == null || snapshot.collectors.isEmpty) Text(l['noEvidence'], style: c.type(13, muted: true)),
      if (snapshot != null) for (final collector in snapshot.collectors) Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('${collector['collector']} · ${l[collector['state'] as String]}', style: c.type(13, bold: true)),
          if (collector['detail'] != null) Text(collector['detail'] as String, style: c.type(12, muted: true)),
        ])),
    ])),
    const SizedBox(height: 16),
    Text('${l['gaps']}: ${snapshot == null || snapshot.gaps.isEmpty ? '—' : snapshot.gaps.join(', ')}', style: c.type(13)),
    const SizedBox(height: 16),
    Text(l['protection'], style: c.type(17, bold: true)),
    const SizedBox(height: 9),
    _panel(c, Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      for (final name in wire.resourceNames) Padding(padding: const EdgeInsets.symmetric(vertical: 5),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text('${l[name]} · ${l[snapshot?.protection(name)['state'] as String? ?? 'unknown']}', style: c.type(13, bold: true)),
          Text('${l['requested']}: ${_truth(l, snapshot?.protection(name)['requested'])} · ${l['guard']}: ${_truth(l, snapshot?.protection(name)['service_active'])}', style: c.type(12, muted: true)),
          if (snapshot?.protection(name)['detail'] != null) Text(snapshot!.protection(name)['detail'] as String, style: c.type(12, muted: true)),
        ])),
    ])),
    if (snapshot != null) for (final detail in snapshot.diagnostics) Padding(
      padding: const EdgeInsets.only(top: 9), child: Text(detail, style: c.type(13))),
    if (snapshot?.truncated ?? false) Text(l['truncated'], style: c.type(12).copyWith(color: c.warning)),
  ];

  List<Widget> _about(Palette c, Labels l, wire.Snapshot? snapshot) => [
    _panel(c, Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text('MicCamWatch · 0.21.1 development', style: c.type(18, bold: true)),
      const SizedBox(height: 12),
      Text(l['hybrid'], style: c.type(14, bold: true)),
      const SizedBox(height: 8),
      Text('Rust owns collection, protection, permissions, preferences, history and the tray. '
        'This disposable Flutter window displays snapshots and submits typed requests over private inherited pipes. '
        'Only Rust applies microphone commands and merges permitted preference changes. '
        'Closing the window does not release protection. Other controls remain available in the native interfaces.',
        style: c.type(14, muted: true)),
      const SizedBox(height: 12),
      Text('Protocol ${wire.protocolVersion} · Typed Rust commands · No direct hardware access', style: c.type(12, muted: true)),
      if (snapshot != null) Text('Engine ${snapshot.engine['version']} · mode: ${snapshot.engine['mode']}', style: c.type(12, muted: true)),
    ])),
  ];
}

String _time(DateTime date) => '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}:${date.second.toString().padLeft(2, '0')}';
String _truth(Labels l, Object? value) => l[value == true ? 'yes' : value == false ? 'no' : 'unknown'];
