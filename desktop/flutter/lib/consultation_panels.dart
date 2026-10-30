import 'package:flutter/material.dart';

import 'consultation.dart';
import 'consultation_model.dart';
import 'labels.dart';

class HistoryPanel extends StatelessWidget {
  const HistoryPanel({super.key, required this.client, required this.locale});
  final ConsultationClient client;
  final String locale;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: client, builder: (context, _) {
      final l = Labels(locale);
      final theme = Theme.of(context);
      return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        _Card(children: [
          Text(l['historyScope'], style: theme.textTheme.bodyMedium),
          const SizedBox(height: 8),
          Text(l['historyOrdering'], style: theme.textTheme.bodySmall),
          const SizedBox(height: 12),
          Align(alignment: Alignment.centerLeft, child: OutlinedButton.icon(
            key: const ValueKey('history.refresh'),
            onPressed: client.canRequest ? () => client.refreshHistory() : null,
            icon: const Icon(Icons.refresh, size: 18), label: Text(l['queryRefresh']))),
          _Availability(client: client, labels: l),
          _Status(status: client.historyStatus, labels: l, name: 'history'),
        ]),
        if (client.historyStale) _Warning(key: const ValueKey('history.stale'),
          text: l['historyPrevious']),
        if (client.historyIncomplete) _Warning(text: l['historyPartial']),
        for (final issue in client.historyIssues) _Warning(text: issue),
        if (!client.historyLoaded && !client.historyStatus.busy)
          Padding(padding: const EdgeInsets.symmetric(vertical: 16),
            child: Text(l['historyNotLoaded'], style: theme.textTheme.bodyMedium)),
        if (client.historyLoaded) ...[
          Padding(padding: const EdgeInsets.symmetric(vertical: 12),
            child: Text('${l['historyLoaded']}: ${client.entries.length}',
              style: theme.textTheme.bodySmall)),
          if (client.entries.isEmpty) _Card(key: const ValueKey('history.empty'),
            children: [Text(l['historyEmpty'])]),
          Column(key: const ValueKey('history.records'),
            crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              for (var i = 0; i < client.entries.length; i++) Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: _HistoryRecord(key: ValueKey('history.entry.$i'),
                  entry: client.entries[i], labels: l)),
            ]),
          Wrap(spacing: 12, runSpacing: 8, crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              OutlinedButton.icon(key: const ValueKey('history.more'),
                onPressed: client.canLoadMore ? () => client.loadMore() : null,
                icon: const Icon(Icons.expand_more, size: 18), label: Text(l['historyMore'])),
              if (!client.historyStale && client.nextCursor == null &&
                  client.historyStatus.state == QueryState.succeeded)
                Text(l['historyEnd'], style: theme.textTheme.bodySmall),
            ]),
        ],
      ]);
    });
}

class _HistoryRecord extends StatelessWidget {
  const _HistoryRecord({super.key, required this.entry, required this.labels});
  final HistoryEntry entry;
  final Labels labels;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return _Card(children: [
      Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Icon(entry.resource == 'microphone' ? Icons.mic_none
            : entry.resource == 'camera' ? Icons.videocam_outlined : Icons.event_note_outlined,
          size: 20, color: theme.colorScheme.primary),
        const SizedBox(width: 9),
        Expanded(child: SelectableText('${labels['historyApplication']}: ${entry.application ?? labels['unknown']}',
          style: theme.textTheme.titleSmall)),
      ]),
      const SizedBox(height: 8),
      SelectableText('${labels['observedAt']}: ${entry.observedAt}',
        style: theme.textTheme.bodySmall),
      const SizedBox(height: 8),
      Wrap(spacing: 18, runSpacing: 5, children: [
        Text('${labels['historyAction']}: ${labels[entry.action]}'),
        Text('${labels['historyResource']}: ${labels[entry.resource]}'),
        Text('${labels['historyActivity']}: ${labels[entry.activity]}'),
        Text('${labels['confidence']}: ${labels[entry.confidence]}'),
        Text('${labels['historyPid']}: ${entry.pid ?? labels['unknown']}'),
      ]),
    ]);
  }
}

class DetailedDiagnosticsPanel extends StatelessWidget {
  const DetailedDiagnosticsPanel({super.key, required this.client, required this.locale});
  final ConsultationClient client;
  final String locale;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: client, builder: (context, _) {
      final l = Labels(locale);
      final theme = Theme.of(context);
      final report = client.report;
      return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        _Card(children: [
          Text(l['reportTitle'], style: theme.textTheme.titleMedium),
          const SizedBox(height: 6),
          Text(l['reportScope']),
          const SizedBox(height: 12),
          Align(alignment: Alignment.centerLeft, child: OutlinedButton.icon(
            key: const ValueKey('diagnostics.run'),
            onPressed: client.canRequest ? () => client.refreshDiagnostics() : null,
            icon: const Icon(Icons.fact_check_outlined, size: 18),
            label: Text(l[report == null ? 'reportRun' : 'queryRefresh']))),
          _Availability(client: client, labels: l),
          _Status(status: client.diagnosticsStatus, labels: l, name: 'diagnostics'),
        ]),
        if (client.diagnosticsStale) _Warning(key: const ValueKey('diagnostics.stale'),
          text: l['reportPrevious']),
        if (report != null) ...[
          const SizedBox(height: 10),
          _Card(key: const ValueKey('diagnostics.report'), children: [
            Wrap(spacing: 18, runSpacing: 5, children: [
              SelectableText('${l['reportVersion']}: ${report.engineVersion}'),
              SelectableText('${l['reportPlatform']}: ${report.platform}'),
            ]),
            const SizedBox(height: 6),
            SelectableText('${l['observedAt']}: ${report.observedAt}',
              style: theme.textTheme.bodySmall),
            if (report.incomplete) _Warning(text: l['reportIncomplete']),
            if (report.checks.isEmpty) Padding(padding: const EdgeInsets.only(top: 12),
              child: Text(l['reportNoChecks'])),
            for (final check in report.checks) Padding(
              padding: const EdgeInsets.only(top: 16),
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Icon(switch (check.status) {
                    DiagnosticStatus.ok => Icons.check_circle_outline,
                    DiagnosticStatus.warning => Icons.warning_amber_outlined,
                    DiagnosticStatus.error => Icons.error_outline,
                    DiagnosticStatus.unknown => Icons.help_outline,
                  }, size: 20, color: check.status == DiagnosticStatus.ok
                    ? theme.colorScheme.primary : theme.colorScheme.error),
                  const SizedBox(width: 8),
                  Expanded(child: SelectableText('${check.name} · ${l['check${check.status.name}']}',
                    style: theme.textTheme.titleSmall)),
                ]),
                const SizedBox(height: 5),
                SelectableText(check.detail),
                if (check.remedy != null) Padding(padding: const EdgeInsets.only(top: 6),
                  child: SelectableText('${l['reportRemedy']}: ${check.remedy}')),
              ])),
            if (report.limitations.isNotEmpty) ...[
              const SizedBox(height: 16),
              Text(l['reportLimitations'], style: theme.textTheme.titleSmall),
              for (final limitation in report.limitations) Padding(
                padding: const EdgeInsets.only(top: 5), child: SelectableText(limitation)),
            ],
          ]),
        ],
      ]);
    });
}

class AutostartPanel extends StatefulWidget {
  const AutostartPanel({super.key, required this.client, required this.locale});
  final ConsultationClient client;
  final String locale;
  @override
  State<AutostartPanel> createState() => _AutostartPanelState();
}

class _AutostartPanelState extends State<AutostartPanel> {
  bool _confirming = false;
  bool _confirmationChanged = false;

  Future<void> _confirm() async {
    final client = widget.client;
    if (_confirming || !client.canSetAutostart) return;
    final readback = client.autostart!;
    final session = client.session;
    final requested = !readback.enabled!;
    final l = Labels(widget.locale);
    setState(() {
      _confirming = true;
      _confirmationChanged = false;
    });
    final confirmed = await showDialog<bool>(context: context, builder: (context) => AlertDialog(
      constraints: const BoxConstraints(maxWidth: 540), scrollable: true,
      title: Text(l[requested ? 'autostartConfirmEnable' : 'autostartConfirmDisable']),
      content: Text(l['autostartConfirmScope']),
      actions: [
        TextButton(key: const ValueKey('autostart.cancel'),
          onPressed: () => Navigator.pop(context, false), child: Text(l['autostartCancel'])),
        FilledButton(key: const ValueKey('autostart.confirm'),
          onPressed: () => Navigator.pop(context, true), child: Text(l['autostartConfirm'])),
      ],
    ));
    if (!mounted) return;
    setState(() => _confirming = false);
    if (confirmed != true) return;
    if (!identical(client, widget.client) || client.session != session ||
        !client.canSetAutostart || !identical(client.autostart, readback)) {
      setState(() => _confirmationChanged = true);
      return;
    }
    await client.setAutostart(requested, expectedEnabled: readback.enabled!);
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.client, builder: (context, _) {
      final client = widget.client;
      final l = Labels(widget.locale);
      final native = client.autostart;
      final enabled = native?.enabled;
      final theme = Theme.of(context);
      return _Card(children: [
        Text(l['autostartTitle'], style: theme.textTheme.titleMedium),
        const SizedBox(height: 6),
        Text(l['autostartScope']),
        const SizedBox(height: 10),
        Semantics(key: const ValueKey('autostart.actual'), liveRegion: true,
          child: Text('${l['autostartActual']}: ${l[enabled == null
            ? 'unknown' : enabled ? 'autostartEnabled' : 'autostartDisabled']}',
            style: theme.textTheme.titleSmall)),
        if (client.autostartStale) _Warning(key: const ValueKey('autostart.stale'),
          text: l['autostartPrevious']),
        if (native?.reason?.isNotEmpty ?? false) Padding(
          padding: const EdgeInsets.only(top: 6), child: SelectableText(native!.reason!)),
        const SizedBox(height: 12),
        Wrap(spacing: 8, runSpacing: 8, children: [
          OutlinedButton.icon(key: const ValueKey('autostart.refresh'),
            onPressed: client.canRequest && !_confirming ? () => client.refreshAutostart() : null,
            icon: const Icon(Icons.refresh, size: 18), label: Text(l['queryRefresh'])),
          OutlinedButton.icon(key: const ValueKey('autostart.change'),
            onPressed: client.canSetAutostart && !_confirming ? _confirm : null,
            icon: const Icon(Icons.power_settings_new, size: 18),
            label: Text(l[enabled == null ? 'autostartChange'
              : enabled ? 'autostartDisable' : 'autostartEnable'])),
        ]),
        _Availability(client: client, labels: l),
        if (client.autostartDisabledReason != null &&
            client.autostartDisabledReason != client.disabledReason)
          Padding(padding: const EdgeInsets.only(top: 8),
            child: Text(l[client.autostartDisabledReason!], style: theme.textTheme.bodySmall)),
        if (_confirmationChanged) _Warning(text: l['autostartConfirmationChanged']),
        _Status(status: client.autostartStatus, labels: l, name: 'autostart'),
      ]);
    });
}

class _Availability extends StatelessWidget {
  const _Availability({required this.client, required this.labels});
  final ConsultationClient client;
  final Labels labels;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start, children: [
      if (client.disabledReason != null) Padding(
        padding: const EdgeInsets.only(top: 8), child: Text(labels[client.disabledReason!],
          style: Theme.of(context).textTheme.bodySmall)),
      if (client.reason?.isNotEmpty ?? false) Padding(
        padding: const EdgeInsets.only(top: 5), child: SelectableText(client.reason!,
          style: Theme.of(context).textTheme.bodySmall)),
    ]);
}

class _Status extends StatelessWidget {
  const _Status({required this.status, required this.labels, required this.name});
  final QueryStatus status;
  final Labels labels;
  final String name;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      if (status.busy) const Padding(padding: EdgeInsets.only(top: 10),
        child: LinearProgressIndicator()),
      if (status.state != QueryState.idle) Padding(padding: const EdgeInsets.only(top: 8),
        child: Semantics(key: ValueKey('$name.status'), liveRegion: true,
          child: Text(labels['query${status.state.name}']))),
      if (status.localIssue != null) _Warning(text: labels[status.localIssue!]),
      if (status.code != null) Padding(padding: const EdgeInsets.only(top: 5),
        child: SelectableText('${labels['queryCode']}: ${status.code}')),
      if (status.detail != null) Padding(padding: const EdgeInsets.only(top: 5),
        child: SelectableText(status.detail!)),
    ]);
}

class _Warning extends StatelessWidget {
  const _Warning({super.key, required this.text});
  final String text;
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(vertical: 8), child: Row(
      crossAxisAlignment: CrossAxisAlignment.start, children: [
        Icon(Icons.info_outline, size: 18, color: Theme.of(context).colorScheme.error),
        const SizedBox(width: 7), Expanded(child: SelectableText(text)),
      ]));
}

class _Card extends StatelessWidget {
  const _Card({super.key, required this.children});
  final List<Widget> children;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(16),
    decoration: BoxDecoration(color: Theme.of(context).cardColor,
      borderRadius: BorderRadius.circular(10),
      border: Border.all(color: Theme.of(context).dividerColor)),
    child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: children));
}
