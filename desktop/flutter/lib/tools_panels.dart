import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'consultation.dart';
import 'labels.dart';
import 'tools_model.dart';

/// Readbacks are explicitly on-demand, never a replacement for live observations.
class NativeToolsPanel extends StatefulWidget {
  const NativeToolsPanel({super.key, required this.client, required this.locale,
    required this.accesses});
  final ConsultationClient client;
  final String locale;
  final List<Map<String, dynamic>> accesses;
  @override
  State<NativeToolsPanel> createState() => _NativeToolsPanelState();
}
class _NativeToolsPanelState extends State<NativeToolsPanel> {
  NativeState? native;
  String? nativeSession;
  EnforcementStatus? enforcement;
  String? enforcementSession;
  ExitStatus? exit;
  String? exitSession;
  NativeReport? detailReport;
  Map<String, dynamic>? get data => detailReport?.data;
  String? section;
  String? detailSession;
  String? issue;
  bool dialogOpen = false;
  final policyPath = TextEditingController();
  final cameraInstance = TextEditingController();
  Labels get l => Labels(widget.locale);
  ConsultationClient get client => widget.client;
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && client.canRequest) _query(const ToolsRequest.controls());
    });
  }
  @override
  void dispose() { policyPath.dispose(); cameraInstance.dispose(); super.dispose(); }

  Future<NativeReport?> _query(ToolsRequest request) async {
    if (!client.canRequest) return null;
    final session = client.session;
    setState(() => issue = null);
    try {
      final result = await client.requestTools(request);
      if (!mounted || client.session != session) return null;
      setState(() {
        native = result.state ?? native;
        if (result.state != null) nativeSession = session;
        if (result.data?.containsKey('enforcement_runtime') == true) {
          enforcement = result.enforcement;
          enforcementSession = session;
        }
        if (result.data?.containsKey('exit') == true) {
          exit = result.exit;
          exitSession = session;
        }
        if (request.action != 'prepare' && request.action != 'commit') {
          detailReport = result; section = request.action;
          detailSession = session;
        }
        issue = result.error;
      });
      return result;
    } catch (error) {
      if (mounted && client.session == session) setState(() => issue = '${l['nativeUnconfirmed']}\n$error');
      return null;
    }
  }
  Future<void> _apply(NativeControl control) async {
    if (dialogOpen || (!client.operational && control.kind != 'quit_engine') || !client.canRequest) return;
    final session = client.session;
    final prepared = await _query(ToolsRequest.prepare(control));
    if (!mounted || prepared == null || prepared.outcome != 'prepared') return;
    setState(() => dialogOpen = true);
    final target = prepared.data?['target'];
    final confirmed = await showDialog<bool>(context: context, builder: (context) => AlertDialog(
      title: Text(l['nativeConfirm']),
      content: ConstrainedBox(constraints: const BoxConstraints(maxWidth: 470),
        child: SingleChildScrollView(child: Column(mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(l[control.kind], style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 10),
            Text(l['nativeConfirmScope']),
            const SizedBox(height: 10),
            if (control.kind.startsWith('microphone')) Text(l[prepared.state!.text('microphone_scope')]),
            if (control.kind.startsWith('camera')) Text(l[prepared.state!.text('camera_scope')]),
            if (control.kind == 'quit_engine') Text(l['quitScope']),
            if (control.kind == 'camera_restore_legacy') ...[
              Text(l['cameraLegacyWarning']),
              SelectableText(control.parameters['instance_id'] as String),
            ],
            if (control.kind == 'automatic_enforcement') ...[
              Text(l['enforcementScope']),
              Text(l[control.parameters['enabled'] == true ? 'enforcementEnable' : 'enforcementDisable']),
            ],
            if (control.kind == 'lock_policy') ...[
              Text('${l['lockMicrophone']}: ${l[control.parameters['microphone'] == true ? 'yes' : 'no']}'),
              Text('${l['lockCamera']}: ${l[control.parameters['camera'] == true ? 'yes' : 'no']}'),
              Text('${l['restoreOnUnlock']}: ${l[control.parameters['restore'] == true ? 'yes' : 'no']}'),
            ],
            if (target != null) SelectableText(const JsonEncoder.withIndent('  ').convert(target)),
            Text('${l['nativeConfirmLifetime']}: ${prepared.data?['confirmation_seconds']} s'),
          ]))),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context, false), child: Text(l['autostartCancel'])),
        Semantics(identifier: 'tools.confirm', child: FilledButton(key: const ValueKey('tools.confirm'),
          onPressed: () => Navigator.pop(context, true), child: Text(l['autostartConfirm']))),
      ],
    ));
    if (!mounted) return;
    setState(() => dialogOpen = false);
    if (confirmed != true) return;
    if (session != client.session || (!client.operational && control.kind != 'quit_engine') || !client.canRequest) {
      setState(() => issue = l['nativeContextChanged']); return;
    }
    await _query(ToolsRequest.commit(prepared));
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(animation: client,
    builder: (context, _) {
      final state = native;
      final enabled = client.canRequest && !dialogOpen;
      final controlsEnabled = enabled && client.operational && nativeSession == client.session;
      final applications = <int, String>{};
      for (final access in widget.accesses) {
        final pid = access['pid'];
        if (pid is int && pid > 1) applications.putIfAbsent(pid, () => access['application'] as String);
      }
      Widget action(String key, IconData icon, VoidCallback? invoke) => Semantics(
        identifier: 'tools.$key', enabled: invoke != null, child: OutlinedButton.icon(
          key: ValueKey('tools.$key'), onPressed: invoke, icon: Icon(icon, size: 18), label: Text(l[key])));
      Widget controlled(String kind, IconData icon, bool available) => action(kind, icon,
        controlsEnabled && available ? () => _apply(NativeControl.action(kind)) : null);
      Widget lock(String label, String field, bool supported) => Semantics(
        identifier: 'tools.$field', enabled: controlsEnabled && supported, child: SwitchListTile(
        key: ValueKey('tools.$field'), contentPadding: EdgeInsets.zero, dense: true,
        title: Text(l[label]), value: state!.flag(field),
        onChanged: controlsEnabled && supported ? (value) {
          _apply(NativeControl.lock(
            microphone: field == 'lock_microphone' ? value : state.flag('lock_microphone'),
            camera: field == 'lock_camera' ? value : state.flag('lock_camera'),
            restore: field == 'restore_on_unlock' ? value : state.flag('restore_on_unlock')));
        } : null,
      ));
      return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        _ToolCard(title: l['nativeControls'], children: [
          Text(l['nativeOnDemand']), const SizedBox(height: 10),
          Wrap(spacing: 8, runSpacing: 8, children: [
            action('queryRefresh', Icons.refresh, enabled ? () => _query(const ToolsRequest.controls()) : null),
          ]),
          if (!client.operational) Padding(padding: const EdgeInsets.only(top: 8), child: Text(l['nativeReadOnly'])),
          if (client.disabledReason case final reason?) Text(l[reason]),
          if (client.toolsStatus.busy) const Padding(padding: EdgeInsets.only(top: 8), child: LinearProgressIndicator()),
          if (issue != null) Semantics(liveRegion: true, child: Padding(
            padding: const EdgeInsets.only(top: 10), child: SelectableText(issue!))),
          if (state != null) ...[
            const Divider(height: 28),
            Text('${l['microphone']}: ${l[state.text('microphone')]}', style: Theme.of(context).textTheme.titleSmall),
            Text(l[state.text('microphone_scope')]),
            if (state.issue('microphone_error') case final error?) SelectableText(error),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 8, children: [
              controlled('microphone_mute', Icons.mic_off_outlined, state.flag('can_mute')),
              controlled('microphone_restore', Icons.restore, state.flag('can_restore')),
            ]),
            const Divider(height: 28),
            Text('${l['camera']}: ${l[state.text('camera')]}', style: Theme.of(context).textTheme.titleSmall),
            Text(l[state.text('camera_scope')]),
            if (state.issue('camera_error') case final error?) SelectableText(error),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 8, children: [
              controlled('camera_block', Icons.videocam_off_outlined, state.flag('can_camera')),
              controlled('camera_allow', Icons.restore, state.flag('can_camera')),
              if (state.values['camera_requested'] != null)
                controlled('camera_resume', Icons.restart_alt, state.flag('can_resume')),
              if (state.flag('can_inspect')) controlled('camera_inspect', Icons.fact_check_outlined, true),
            ]),
            if (state.values['camera_requested'] != null) ...[
              const SizedBox(height: 12),
              Text(l['cameraLegacyWarning']),
              const SizedBox(height: 8),
              TextField(controller: cameraInstance, maxLength: 2048,
                decoration: InputDecoration(labelText: l['cameraInstance'], border: const OutlineInputBorder(), isDense: true)),
              action('camera_restore_legacy', Icons.history,
                controlsEnabled && state.flag('can_camera') ? () {
                  try { _apply(NativeControl.restoreLegacyCamera(cameraInstance.text)); }
                  on ArgumentError { setState(() => issue = l['cameraInstance']); }
                } : null),
            ],
          ],
        ]),
        if (state != null) _ToolCard(title: l['lockSettings'], children: [
          Text(l['lockScope']),
          lock('lockMicrophone', 'lock_microphone', state.flag('lock_supported')),
          lock('lockCamera', 'lock_camera', state.flag('camera_lock_supported')),
          lock('restoreOnUnlock', 'restore_on_unlock', state.flag('lock_supported')),
        ]),
        _ToolCard(title: l['automatic_enforcement'], children: [
          Text(l['enforcementScope']),
          const SizedBox(height: 8),
          action('enforcementRefresh', Icons.refresh, enabled ? () => _query(const ToolsRequest.controls()) : null),
          if (enforcement case final status?) ...[
            Text(l[status.enabled ? 'enforcementEnabled' : 'enforcementDisabled'],
              style: Theme.of(context).textTheme.titleSmall),
            Text('${l['enforcementRequests']}: ${status.accepted} · ${l['enforcementFailures']}: ${status.failed}'),
            if (status.error case final error?) SelectableText(error),
            const SizedBox(height: 8),
            action(status.enabled ? 'enforcementDisable' : 'enforcementEnable', Icons.policy_outlined,
              controlsEnabled && enforcementSession == client.session
                ? () => _apply(NativeControl.enforcement(!status.enabled)) : null),
          ] else Text(l['unavailable']),
        ]),
        _ToolCard(title: l['devicesAndApplications'], children: [
          Wrap(spacing: 8, runSpacing: 8, children: [
            action('deviceInventory', Icons.devices_other, enabled ? () => _query(const ToolsRequest.devices()) : null),
            for (final application in applications.entries)
              Semantics(identifier: 'tools.application.${application.key}', child:
                OutlinedButton(key: ValueKey('tools.application.${application.key}'),
                  onPressed: enabled ? () => _query(ToolsRequest.application(application.key)) : null,
                  child: Text('${application.value} · PID ${application.key}'))),
          ]),
          if (section == 'devices' && data != null) ...[
            for (final device in (data!['devices'] as List? ?? const []))
              ListTile(contentPadding: EdgeInsets.zero,
                leading: Icon(device['resource'] == 'microphone' ? Icons.mic_none : Icons.videocam_outlined),
                title: Text(device['name'] as String), subtitle: SelectableText(device['id'] as String)),
            if (data!['truncated'] == true) Text(l['truncated']),
          ],
          if (const {'application', 'application_next'}.contains(section) && data != null) ...[
            const SizedBox(height: 12),
            Text('${l['applicationAccess']}: ${data!['access_index']} / ${data!['access_count']}',
              style: Theme.of(context).textTheme.titleSmall),
            Text('${l['applicationSnapshot']}: ${data!['snapshot_read_at']}'),
            Text(l['applicationSnapshotScope']),
            Text(l['applicationPolicyScope']),
            if (detailSession != client.session) Text(l['nativeContextChanged']),
            if (data!['process_identity_confirmed'] != true) Text(l['applicationIdentityUnknown']),
            if (data!['truncated'] == true) Text(l['truncated']),
            _DataView(data: {for (final entry in data!.entries)
              if (!const {'access_index','access_count','snapshot_read_at','snapshot_not_live',
                'process_identity_confirmed','next_cursor','truncated'}.contains(entry.key)) entry.key: entry.value}),
            if (data!['next_cursor'] case final String cursor)
              action('applicationNext', Icons.navigate_next,
                enabled && detailSession == client.session ? () => _query(ToolsRequest.applicationNext(cursor)) : null),
            const SizedBox(height: 8),
            action('terminate', Icons.stop_circle_outlined,
              enabled && detailSession == client.session && client.operational &&
                data!['pid'] is int && data!['process_identity_confirmed'] == true
                ? () => _apply(NativeControl.terminate(data!['pid'] as int)) : null),
            Text(l['terminateScope']),
          ],
        ]),
        _ToolCard(title: l['policyAndPaths'], children: [
          Wrap(spacing: 8, runSpacing: 8, children: [
            action('consultPolicy', Icons.policy_outlined, enabled ? () => _query(const ToolsRequest.policy()) : null),
            action('consultPaths', Icons.folder_outlined, enabled ? () => _query(const ToolsRequest.paths()) : null),
            action('installationActions', Icons.info_outline, enabled ? () => _query(const ToolsRequest.maintenance()) : null),
            action('checkUpdates', Icons.system_update_alt, enabled ? () => _query(const ToolsRequest.checkUpdates()) : null),
          ]),
          const SizedBox(height: 12),
          TextField(controller: policyPath, maxLength: 1024,
            decoration: InputDecoration(labelText: l['policyFile'], border: const OutlineInputBorder(), isDense: true)),
          action('validatePolicy', Icons.rule, enabled ? () => _query(ToolsRequest.validatePolicy(
            policyPath.text.isEmpty ? null : policyPath.text)) : null),
          Text(l['policyValidationScope']),
          if (const {'paths', 'policy', 'validate_policy'}.contains(section) && data != null)
            Padding(padding: const EdgeInsets.only(top: 12), child: _DataView(data: data!)),
          if (const {'maintenance', 'check_updates'}.contains(section) && detailReport != null &&
              (client.toolsStale || client.toolsStatus.busy || detailSession != client.session))
            Text(l['reportPrevious']),
          if (section == 'maintenance' && detailReport?.maintenance != null) ...[
            const SizedBox(height: 12),
            Text('${l['reportVersion']}: ${detailReport!.maintenance!.version}'),
            Text(l['installedExecutable']),
            SelectableText(detailReport!.maintenance!.executable),
            const SizedBox(height: 8),
            Text(l[detailReport!.maintenance!.ownerGuidance == null
              ? 'maintenanceOwnerUnknown' : 'maintenanceNativeOwner']),
            if (detailReport!.maintenance!.ownerGuidance case final guidance?) SelectableText(guidance),
            const SizedBox(height: 8),
            Text(l['hybridTransactionsUnavailable']),
            Wrap(spacing: 8, runSpacing: 8, children: [
              action('updateInstallation', Icons.system_update_alt, null),
              action('removeInstallation', Icons.delete_outline, null),
            ]),
            SelectableText(detailReport!.maintenance!.releasesUrl),
          ],
          if (section == 'check_updates' && detailReport?.releaseCheck != null) ...[
            const SizedBox(height: 12),
            Semantics(liveRegion: true, child: Text(l[detailReport!.releaseCheck!.available
              ? 'releaseAvailable' : 'releaseNotNewer'], style: Theme.of(context).textTheme.titleSmall)),
            Text('${l['reportVersion']}: ${detailReport!.releaseCheck!.currentVersion}'),
            Text('${l['latestPublicRelease']}: ${detailReport!.releaseCheck!.tag}'),
            Text('${l['releaseCheckedAt']}: ${detailReport!.releaseCheck!.checkedAt.toUtc().toIso8601String()}'),
            Text(l['releaseMetadataOnly']),
            SelectableText(detailReport!.releaseCheck!.url),
          ],
        ]),
        _ToolCard(title: l['quit_engine'], children: [
          Text(l['quitScope']),
          const SizedBox(height: 8),
          if (exit?.requested == true) Text(l['quitRequested']),
          Wrap(spacing: 8, runSpacing: 8, children: [
            action('exitRefresh', Icons.refresh, enabled ? () => _query(const ToolsRequest.exitStatus()) : null),
            action('quit_engine', Icons.power_settings_new,
              enabled && exitSession == client.session && exit?.ready == true
                ? () => _apply(NativeControl.action('quit_engine')) : null),
          ]),
        ]),
      ]);
    });
}

class _ToolCard extends StatelessWidget {
  const _ToolCard({required this.title, required this.children});
  final String title;
  final List<Widget> children;
  @override
  Widget build(BuildContext context) => Card.outlined(margin: const EdgeInsets.only(bottom: 12),
    child: Padding(padding: const EdgeInsets.all(16), child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [Text(title, style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 10), ...children])));
}
class _DataView extends StatelessWidget {
  const _DataView({required this.data});
  final Map<String, dynamic> data;
  @override
  Widget build(BuildContext context) => SelectableText(
    const JsonEncoder.withIndent('  ').convert(data), style: Theme.of(context).textTheme.bodySmall);
}

Future<void> clearRecordedHistory(BuildContext context, ConsultationClient client, Labels l) async {
  final session = client.session;
  final confirmed = await showDialog<bool>(context: context, builder: (context) => AlertDialog(
    title: Text(l['clearHistory']), content: Text(l['clearHistoryScope']), actions: [
      TextButton(onPressed: () => Navigator.pop(context, false), child: Text(l['autostartCancel'])),
      Semantics(identifier: 'history.confirmClear', child: FilledButton(
        key: const ValueKey('history.confirmClear'), onPressed: () => Navigator.pop(context, true),
        child: Text(l['clearHistory']))),
    ]));
  if (confirmed != true || !context.mounted) return;
  if (session != client.session || !client.canRequest) return;
  try {
    final result = await client.requestTools(const ToolsRequest.clearHistory());
    if (result.data?['cleared'] == true && client.canRequest) await client.refreshHistory();
  } catch (error) {
    if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('${l['nativeUnconfirmed']}\n$error')));
  }
}
Future<void> copyNativeDiagnostics(BuildContext context, ConsultationClient client, Labels l) async {
  final report = client.report;
  if (report == null) return;
  final text = StringBuffer('${l['reportTitle']}\n${l['observedAt']}: ${report.observedAt}\n')
    ..writeln('${l['reportVersion']}: ${report.engineVersion}')
    ..writeln('${l['reportPlatform']}: ${report.platform}')
    ..writeln(client.diagnosticsStale ? l['reportPrevious'] : l['reportScope']);
  for (final check in report.checks) {
    text.writeln('${check.name}: ${check.status.name}\n${check.detail}');
    if (check.remedy != null) text.writeln(check.remedy);
  }
  for (final limit in report.limitations) { text.writeln(limit); }
  try {
    await Clipboard.setData(ClipboardData(text: text.toString()));
    if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(l['diagnosticsCopied'])));
  } catch (error) {
    if (context.mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('${l['copyFailed']}: $error')));
  }
}
