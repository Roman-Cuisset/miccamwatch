import 'package:flutter/material.dart';
import 'labels.dart';
import 'snapshot.dart' show riskNames;

/// View state only. No commands, collector configuration or security decisions.
class ObservationView extends StatelessWidget {
  const ObservationView({super.key, required this.accesses, required this.locale,
    required this.resource, required this.minimumRisk, required this.includeReady,
    required this.onResource, required this.onMinimumRisk, required this.onIncludeReady});
  final List<Map<String, dynamic>> accesses;
  final String locale, resource;
  final int minimumRisk;
  final bool includeReady;
  final ValueChanged<String> onResource;
  final ValueChanged<int> onMinimumRisk;
  final ValueChanged<bool> onIncludeReady;

  @override
  Widget build(BuildContext context) {
    final l = Labels(locale);
    final theme = Theme.of(context);
    final rows = <Widget>[];
    for (var i = 0; i < accesses.length; i++) {
      final access = accesses[i];
      final kind = access['resource'] as String;
      final activity = access['activity'] as String;
      final risk = access['risk'] as String;
      if ((resource != 'all' && kind != resource) || (!includeReady && activity == 'ready') ||
          riskNames.indexOf(risk) < minimumRisk) continue;
      rows.add(Semantics(identifier: 'observations.row.$kind.$activity.$risk.$i',
        child: Padding(padding: const EdgeInsets.symmetric(vertical: 8),
          child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Icon(kind == 'microphone' ? Icons.mic_none : Icons.videocam_outlined,
              size: 20, color: theme.colorScheme.secondary),
            const SizedBox(width: 10),
            Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(access['application'] as String, style: theme.textTheme.titleSmall),
              Text('${l[kind]} · PID ${access['pid'] ?? '—'} · ${l[activity]}', style: theme.textTheme.bodySmall),
              Text('${l['confidence']}: ${l[access['confidence'] as String? ?? 'unknown']}', style: theme.textTheme.bodySmall),
              Text('${l['riskAssessment']}: ${l['risk_$risk']}', style: theme.textTheme.bodySmall),
              if (access['device'] != null) Text(access['device'] as String, style: theme.textTheme.bodySmall),
              if (access['started_at'] != null) Text(access['started_at'] as String, style: theme.textTheme.bodySmall),
            ])),
          ]))));
    }
    Widget choice(String id, String label, bool selected, VoidCallback select) => Semantics(
      identifier: id, child: ChoiceChip(key: ValueKey(id), label: Text(label), selected: selected,
        onSelected: (value) { if (value) select(); }));
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text(l['viewFilterScope'], style: theme.textTheme.bodySmall),
      const SizedBox(height: 8),
      Wrap(spacing: 8, runSpacing: 6, children: [
        for (final kind in const ['all','microphone','camera'])
          choice('observations.resource.$kind', l[kind == 'all' ? 'viewAllResources' : kind],
            resource == kind, () => onResource(kind)),
      ]),
      const SizedBox(height: 8),
      Text(l['minimumRiskAssessment'], style: theme.textTheme.labelLarge),
      Wrap(spacing: 8, runSpacing: 6, children: [
        for (var i = 0; i < riskNames.length; i++)
          choice('observations.risk.${riskNames[i]}', l['risk_${riskNames[i]}'], minimumRisk == i,
            () => onMinimumRisk(i)),
      ]),
      Semantics(identifier: 'observations.includeReady', child: SwitchListTile(
        key: const ValueKey('observations.includeReady'), contentPadding: EdgeInsets.zero, dense: true,
        title: Text(l['viewIncludeReady']), value: includeReady, onChanged: onIncludeReady)),
      Text(l['applicationPolicyScope'], style: theme.textTheme.bodySmall),
      const Divider(height: 24),
      if (rows.isEmpty) Text(l['viewNoMatchingRows'], style: theme.textTheme.bodySmall) else ...rows,
    ]);
  }
}
