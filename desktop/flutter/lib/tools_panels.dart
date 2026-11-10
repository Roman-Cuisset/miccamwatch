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
  Map<String, dynamic>? data;
  String? section;
  String? issue;
  bool dialogOpen = false;
  final policyPath = TextEditingController();
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
  void dispose() { policyPath.dispose(); super.dispose(); }

  Future<NativeReport?> _query(ToolsRequest request) async {
    if (!client.canRequest) return null;
    setState(() => issue = null);
    try {
      final result = await client.requestTools(request);
      if (mounted) setState(() {
        native = result.state ?? native;
        if (request.action != 'prepare' && request.action != 'commit') {
          data = result.data; section = request.action;
        }
        issue = result.error;
      });
      return result;
    } catch (error) {
      if (mounted) setState(() => issue = '${l['nativeUnconfirmed']}\n$error');
      return null;
    }
  }
  Future<void> _apply(NativeControl control) async {
    if (dialogOpen || !client.operational || !client.canRequest) return;
    final session = client.session;
    final prepared = await _query(ToolsRequest.prepare(control));
    if (!mounted || prepared == null || prepared.outcome != 'prepared') return;
    dialogOpen = true;
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
            if (control.kind.startsWith('microphone')) Text(prepared.state!.text('microphone_scope')),
            if (control.kind.startsWith('camera')) Text(prepared.state!.text('camera_scope')),
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
        FilledButton(key: const ValueKey('tools.confirm'),
          onPressed: () => Navigator.pop(context, true), child: Text(l['autostartConfirm'])),
      ],
    ));
    dialogOpen = false;
    if (!mounted || confirmed != true) return;
    if (session != client.session || !client.operational || !client.canRequest) {
      setState(() => issue = l['nativeContextChanged']); return;
    }
    await _query(ToolsRequest.commit(prepared));
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(animation: client,
    builder: (context, _) {
      final state = native;
      final enabled = client.canRequest && !dialogOpen;
      Widget action(String key, IconData icon, VoidCallback? invoke) => OutlinedButton.icon(
        key: ValueKey('tools.$key'), onPressed: invoke, icon: Icon(icon, size: 18), label: Text(l[key]));
      Widget controlled(String kind, IconData icon, bool available) => action(kind, icon,
        enabled && client.operational && available ? () => _apply(NativeControl.action(kind)) : null);
      Widget lock(String label, String field, bool supported) => SwitchListTile(
        key: ValueKey('tools.$field'), contentPadding: EdgeInsets.zero, dense: true,
        title: Text(l[label]), value: state!.flag(field),
        onChanged: enabled && client.operational && supported ? (value) {
          _apply(NativeControl.lock(
            microphone: field == 'lock_microphone' ? value : state.flag('lock_microphone'),
            camera: field == 'lock_camera' ? value : state.flag('lock_camera'),
            restore: field == 'restore_on_unlock' ? value : state.flag('restore_on_unlock')));
        } : null,
      );
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
            Text(state.text('microphone_scope')),
            if (state.issue('microphone_error') case final error?) SelectableText(error),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 8, children: [
              controlled('microphone_mute', Icons.mic_off_outlined, state.flag('can_mute')),
              controlled('microphone_restore', Icons.restore, state.flag('can_restore')),
            ]),
            const Divider(height: 28),
            Text('${l['camera']}: ${l[state.text('camera')]}', style: Theme.of(context).textTheme.titleSmall),
            Text(state.text('camera_scope')),
            if (state.issue('camera_error') case final error?) SelectableText(error),
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 8, children: [
              controlled('camera_block', Icons.videocam_off_outlined, state.flag('can_camera')),
              controlled('camera_allow', Icons.restore, state.flag('can_camera')),
              if (state.flag('can_inspect')) controlled('camera_inspect', Icons.fact_check_outlined, true),
            ]),
          ],
        ]),
        if (state != null) _ToolCard(title: l['lockSettings'], children: [
          Text(l['lockScope']),
          lock('lockMicrophone', 'lock_microphone', state.flag('lock_supported')),
          lock('lockCamera', 'lock_camera', state.flag('camera_lock_supported')),
          lock('restoreOnUnlock', 'restore_on_unlock', state.flag('lock_supported')),
        ]),
        _ToolCard(title: l['devicesAndApplications'], children: [
          Wrap(spacing: 8, runSpacing: 8, children: [
            action('deviceInventory', Icons.devices_other, enabled ? () => _query(const ToolsRequest.devices()) : null),
            for (final access in widget.accesses.take(16))
              if (access['pid'] is int && (access['pid'] as int) > 1)
                OutlinedButton(key: ValueKey('tools.application.${access['pid']}.${access['resource']}'),
                  onPressed: enabled ? () => _query(ToolsRequest.application(access['pid'] as int)) : null,
                  child: Text('${access['application']} · PID ${access['pid']}')),
          ]),
          if (section == 'devices' && data != null) ...[
            for (final device in (data!['devices'] as List? ?? const []))
              ListTile(contentPadding: EdgeInsets.zero,
                leading: Icon(device['resource'] == 'microphone' ? Icons.mic_none : Icons.videocam_outlined),
                title: Text(device['name'] as String), subtitle: SelectableText(device['id'] as String)),
            if (data!['truncated'] == true) Text(l['truncated']),
          ],
          if (section == 'application' && data != null) ...[
            const SizedBox(height: 12), _DataView(data: data!),
            const SizedBox(height: 8),
            action('terminate', Icons.stop_circle_outlined,
              enabled && client.operational && data!['pid'] is int
                ? () => _apply(NativeControl.terminate(data!['pid'] as int)) : null),
            Text(l['terminateScope']),
          ],
        ]),
        _ToolCard(title: l['policyAndPaths'], children: [
          Wrap(spacing: 8, runSpacing: 8, children: [
            action('consultPolicy', Icons.policy_outlined, enabled ? () => _query(const ToolsRequest.policy()) : null),
            action('consultPaths', Icons.folder_outlined, enabled ? () => _query(const ToolsRequest.paths()) : null),
            action('installationActions', Icons.info_outline, enabled ? () => _query(const ToolsRequest.maintenance()) : null),
          ]),
          const SizedBox(height: 12),
          TextField(controller: policyPath, maxLength: 1024,
            decoration: InputDecoration(labelText: l['policyFile'], border: const OutlineInputBorder(), isDense: true)),
          action('validatePolicy', Icons.rule, enabled ? () => _query(ToolsRequest.validatePolicy(
            policyPath.text.isEmpty ? null : policyPath.text)) : null),
          Text(l['policyValidationScope']),
          if (const {'paths', 'policy', 'validate_policy', 'maintenance'}.contains(section) && data != null)
            Padding(padding: const EdgeInsets.only(top: 12), child: _DataView(data: data!)),
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
      FilledButton(key: const ValueKey('history.confirmClear'), onPressed: () => Navigator.pop(context, true),
        child: Text(l['clearHistory'])),
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
