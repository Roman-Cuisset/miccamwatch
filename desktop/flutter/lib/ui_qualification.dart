// Hosted/local qualification only. The normal release's constant branch is off.
// Uses the real rendered semantics callbacks and native IPC readbacks: no fake
// snapshots, direct settings writes, public socket or hardware command bypass.
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

import 'consultation.dart';
import 'preferences.dart';
import 'preferences_model.dart';

class UiQualification {
  UiQualification({required this.queries, required this.preferences,
    required this.describe, required this.live, required this.closing});

  static const scenario = String.fromEnvironment('MCW_UI_QUALIFICATION');
  static const enabled = scenario != '';
  final ConsultationClient queries;
  final PreferenceCommandClient preferences;
  final Map<String, Object?> Function() describe;
  final bool Function() live;
  final bool Function() closing;
  final List<String> cases = [];
  final List<String> errors = [];
  SemanticsHandle? _handle;
  FlutterExceptionHandler? _previousError;
  FlutterExceptionHandler? _errorHandler;
  bool _disposed = false;

  void _require(bool value, String message) {
    if (!value) throw StateError(message);
  }

  void _record(String status, [String? error]) {
    stderr.writeln('MCW_FLUTTER_UI_PROOF ${jsonEncode({
      'pid': pid, 'scenario': scenario, 'status': status, ...describe(),
      'cases': cases, 'framework_errors': errors, if (error != null) 'error': error,
    })}');
  }

  Future<void> _until(bool Function() condition, String failure) async {
    final deadline = DateTime.now().add(const Duration(seconds: 20));
    while (DateTime.now().isBefore(deadline)) {
      _require(!_disposed && !closing(), 'Window closed during qualification');
      _require(errors.isEmpty, 'Framework/layout error');
      if (condition()) return;
      await Future<void>.delayed(const Duration(milliseconds: 40));
    }
    throw StateError(failure);
  }

  SemanticsOwner? get _owner {
    final views = RendererBinding.instance.renderViews;
    return views.length == 1 ? views.single.owner?.semanticsOwner : null;
  }

  SemanticsNode? _node(String identifier) {
    SemanticsNode? result;
    bool visit(SemanticsNode node) {
      if (node.getSemanticsData().identifier == identifier) {
        _require(result == null, 'Duplicate semantic identifier: $identifier');
        result = node;
      }
      node.visitChildren(visit);
      return true;
    }
    final root = _owner?.rootSemanticsNode;
    if (root != null) visit(root);
    return result;
  }

  SemanticsNode? _action(String identifier) {
    final root = _node(identifier);
    if (root == null) return null;
    final targets = <SemanticsNode>[];
    bool visit(SemanticsNode node) {
      if (node.getSemanticsData().hasAction(SemanticsAction.tap)) {
        targets.add(node);
      } else {
        node.visitChildren(visit);
      }
      return true;
    }
    visit(root);
    _require(targets.length <= 1, 'Ambiguous action under $identifier');
    return targets.isEmpty ? null : targets.single;
  }

  Future<void> _tap(String identifier) async {
    const permitted = {'navigation.overview', 'navigation.history',
      'navigation.settings', 'navigation.diagnostics', 'navigation.about',
      'navigation.controls', 'tools.queryRefresh', 'tools.deviceInventory',
      'tools.consultPolicy', 'tools.consultPaths', 'tools.validatePolicy',
      'tools.enforcementRefresh',
      'tools.exitRefresh', 'tools.quit_engine', 'tools.confirm',
      'preferences.language', 'language.engine', 'language.en', 'language.fr', 'language.de',
      'language.es', 'language.ja', 'language.zh', 'language.ru',
      'history.clear', 'history.confirmClear',
      'history.refresh', 'diagnostics.run', 'autostart.refresh',
      'preferences.notificationsEnabled', 'autostart.change', 'autostart.confirm'};
    _require(permitted.contains(identifier), 'Not a qualification action');
    _require(scenario == 'startup' || !identifier.startsWith('autostart.c'),
      'Startup mutation is not admitted in consultation qualification');
    await _until(() => _action(identifier) != null,
      'No enabled rendered action: $identifier (identified node: ${_node(identifier) != null})');
    var node = _action(identifier)!;
    if (node.getSemanticsData().hasAction(SemanticsAction.showOnScreen)) {
      _owner!.performAction(node.id, SemanticsAction.showOnScreen);
      WidgetsBinding.instance.scheduleFrame();
      await WidgetsBinding.instance.endOfFrame;
    }
    node = _action(identifier)!;
    _require(node.getSemanticsData().hasAction(SemanticsAction.tap), 'Action became disabled');
    _owner!.performAction(node.id, SemanticsAction.tap);
    WidgetsBinding.instance.scheduleFrame();
    await WidgetsBinding.instance.endOfFrame;
  }

  Future<void> _query(QueryStatus status, String label) async {
    await _until(() => !status.busy && status.state != QueryState.idle, '$label did not finish');
    _require(status.state == QueryState.succeeded,
      '$label failed: ${status.state.name}/${status.code}/${status.localIssue}');
  }

  Future<void> run() async {
    if (!enabled) return;
    _previousError = FlutterError.onError;
    _errorHandler = (details) {
      if (errors.length < 8) errors.add(details.exceptionAsString().substring(0,
        details.exceptionAsString().length.clamp(0, 512)));
      _previousError?.call(details);
    };
    FlutterError.onError = _errorHandler;
    _handle = WidgetsBinding.instance.ensureSemantics();
    try {
      _require(const {'consultation', 'startup', 'parity', 'controls', 'enforcement', 'quit'}.contains(scenario), 'Unknown proof scenario');
      await _until(() => live() && _owner?.rootSemanticsNode != null, 'No live native snapshot/semantics');
      final mode = describe()['mode'];
      if (scenario == 'quit') {
        _require(mode == 'settings_development' && !queries.operational,
          'Quit proof requires the private owner with device actions disabled');
        await _tap('navigation.controls');
        await _until(() => !queries.toolsStatus.busy, 'Initial controls pending');
        await _tap('tools.exitRefresh');
        await _query(queries.toolsStatus, 'Private native exit readiness');
        _require(queries.toolsReport?.exit?.ready == true, 'Native exit refused or unavailable');
        await _tap('tools.quit_engine');
        await _until(() => _action('tools.confirm') != null, 'Explicit quit confirmation missing');
        cases.add('private-owner-quit-prepared-without-device-authority');
        // This is NOT success. The external harness must prove the exact native
        // owner exits normally and every owned frontend process is gone.
        _record('quit_prepared');
        final confirmation = _action('tools.confirm')!;
        _owner!.performAction(confirmation.id, SemanticsAction.tap);
        return;
      }
      if (scenario == 'startup') {
        _require(mode == 'operational' && Platform.environment['GITHUB_ACTIONS'] == 'true' &&
          Platform.environment['RUNNER_ENVIRONMENT'] == 'github-hosted' &&
          Platform.environment['MCW_UI_QUALIFICATION_STARTUP'] == 'allow',
          'Startup proof requires the separate disposable-runner preflight');
        await _tap('navigation.settings');
        await _query(queries.autostartStatus, 'Startup readback');
        final phase = Platform.environment['MCW_UI_QUALIFICATION_PHASE'];
        _require(const {'enable', 'disable', 'foreign_refusal', 'write_failure', 'user_disabled'}
          .contains(phase), 'Startup phase required');
        final before = phase == 'disable';
        _require(queries.autostart?.enabled == before && queries.canSetAutostart,
          'Unexpected startup state; do not adopt or disable an existing registration');
        cases.add(before ? 'enabled-state-survived-owner-restart' : 'fresh-disabled-installation');
        await _tap('autostart.change');
        await _tap('autostart.confirm');
        if (phase == 'enable' || phase == 'disable') {
          await _query(queries.autostartStatus, 'Startup write/readback');
          _require(queries.autostart?.enabled == !before, 'Requested startup state not confirmed');
          cases.add(before ? 'startup-disabled-confirmed' : 'startup-enabled-confirmed');
        } else {
          await _until(() => !queries.autostartStatus.busy, 'Native refusal did not finish');
          _require(const {QueryState.failed, QueryState.rejected}.contains(queries.autostartStatus.state)
            && queries.autostart?.enabled == false && queries.autostartStale
            && !queries.canSetAutostart, 'Refusal became optimistic state or remained writable');
          cases.add('$phase-confirmed-without-optimistic-state');
        }
      } else {
        _require(mode == 'settings_development' || mode == 'read_only_development',
          'Consultation proof never runs in a daily operational context');
        await _tap('navigation.history');
        await _query(queries.historyStatus, 'History');
        _require(queries.historyLoaded && !queries.historyStale, 'History did not load');
        await _tap('history.refresh');
        await _query(queries.historyStatus, 'History refresh');
        cases.add('real-native-history-query-and-refresh');
        await _tap('navigation.settings');
        await _query(queries.autostartStatus, 'Startup readback');
        _require(!queries.canSetAutostart, 'Development startup write capability leaked');
        final startup = _node('autostart.change');
        _require(startup != null && _action('autostart.change') == null,
          'Development startup action is not disabled in rendered semantics');
        cases.add('development-startup-mutation-disabled');
        if (mode == 'settings_development') {
          final original = preferences.view?.values?.notificationsEnabled;
          _require(original != null && preferences.canChange(PreferenceKey.notificationsEnabled),
            'Isolated preference capability unavailable');
          for (final expected in [!original!, original]) {
            await _tap('preferences.notificationsEnabled');
            await _until(() => preferences.state != 'pending' && preferences.state != 'accepted',
              'Preference write did not finish');
            _require(preferences.state == 'succeeded' &&
              preferences.view?.values?.notificationsEnabled == expected,
              'Native preference write/readback failed');
          }
          cases.add('notification-preference-changed-and-restored-by-Rust');
        }
        await _tap('navigation.about');
        cases.add('about-rendered');
        await _tap('navigation.diagnostics');
        await _tap('diagnostics.run');
        await _query(queries.diagnosticsStatus, 'Diagnostics');
        _require(queries.report != null && queries.report!.platform == describe()['platform'] &&
          !queries.diagnosticsStale, 'Native diagnostic identity/readback mismatch');
        cases.add('real-native-diagnostics-rendered');
        if (const {'parity', 'controls', 'enforcement'}.contains(scenario)) {
          _require(mode == 'settings_development', 'Parity writes require the isolated settings owner');
          final engineLocale = describe()['locale'];
          _require(describe()['interface_locale'] == engineLocale,
            'A new window retained a previous language override');
          await _tap('navigation.settings');
          for (final language in ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru']) {
            await _tap('preferences.language');
            await _tap('language.$language');
            await _until(() => describe()['interface_locale'] == language,
              'Explicit interface language was not applied');
            _require(describe()['locale'] == engineLocale, 'Interface choice changed the engine language');
          }
          await _tap('preferences.language');
          await _tap('language.engine');
          await _until(() => describe()['interface_locale'] == engineLocale,
            'Engine language was not restored');
          cases.add('seven-window-languages-without-native-language-or-settings-mutation');
          await _tap('navigation.history');
          await _query(queries.historyStatus, 'History before confirmed clear');
          await _tap('history.clear');
          await _tap('history.confirmClear');
          await _until(() => queries.toolsReport?.data?['cleared'] == true && !queries.toolsStatus.busy,
            'Native history clear not confirmed');
          await _query(queries.historyStatus, 'History readback after clear');
          await _until(() => queries.historyLoaded && !queries.historyStale && !queries.historyStatus.busy,
            'History was not actually reloaded after clearing');
          cases.add('isolated-history-clear-confirmed-and-reloaded');
          await _tap('navigation.controls');
          await _until(() => queries.toolsReport?.state != null && !queries.toolsStatus.busy,
            'Native controls not read');
          await _until(() => _node('tools.microphone_mute') != null && _node('tools.lock_microphone') != null,
            'Native readback was not rendered');
          _require(!queries.operational && _node('tools.microphone_mute') != null &&
            _action('tools.microphone_mute') == null && _action('tools.camera_block') == null &&
            _action('tools.lock_microphone') == null, 'Development device/lock actions became enabled');
          cases.add('native-controls-readback-with-development-device-mutations-disabled');
          for (final action in ['tools.deviceInventory', 'tools.consultPolicy',
              'tools.consultPaths', 'tools.validatePolicy']) {
            await _tap(action);
            await _query(queries.toolsStatus, action);
            _require(queries.toolsReport?.outcome == 'completed', 'Native query not completed: $action');
            cases.add('native-$action-confirmed');
          }
          await _tap('tools.queryRefresh');
          await _query(queries.toolsStatus, 'Final controls');
          if (scenario == 'enforcement') {
            await _tap('tools.enforcementRefresh');
            await _query(queries.toolsStatus, 'Native enforcement readback');
            _require(queries.toolsReport?.enforcement?.enabled == false && !queries.operational,
              'Isolated native enforcement state unavailable or unexpectedly enabled');
            cases.add('native-enforcement-disabled-readback-and-no-development-process-effects');
          }
          if (scenario == 'parity') await _tap('navigation.settings');
        }
      }
      WidgetsBinding.instance.scheduleFrame();
      await WidgetsBinding.instance.endOfFrame;
      _require(errors.isEmpty, 'Framework/layout errors');
      _record('passed');
    } catch (error) {
      _record('failed', error.toString());
    }
  }

  void dispose() {
    _disposed = true;
    _handle?.dispose();
    if (FlutterError.onError == _errorHandler) FlutterError.onError = _previousError;
  }
}
