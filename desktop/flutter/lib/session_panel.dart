import 'package:flutter/material.dart';

import 'consultation.dart';
import 'labels.dart';
import 'session_model.dart';
import 'tools_model.dart';

class ObservationSessionPanel extends StatefulWidget {
  const ObservationSessionPanel({
    super.key,
    required this.client,
    required this.locale,
    required this.status,
  });

  final ConsultationClient client;
  final String locale;
  final ObservationSession status;

  @override
  State<ObservationSessionPanel> createState() =>
      _ObservationSessionPanelState();
}

class _ObservationSessionPanelState extends State<ObservationSessionPanel> {
  ObservationSession? readback;
  String? readbackConnection;
  String? issue;
  bool busy = false;

  Labels get labels => Labels(widget.locale);

  @override
  void didUpdateWidget(ObservationSessionPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.status != widget.status) readback = null;
  }

  Future<void> _request(ToolsRequest request, {String? confirmLabel}) async {
    final client = widget.client;
    if (busy || !client.canRequest) return;
    final connection = client.session;
    setState(() {
      busy = true;
      issue = null;
    });
    try {
      var report = await client.requestTools(request);
      if (!mounted || client.session != connection) return;
      if (confirmLabel != null) {
        final token = report.token;
        if (report.outcome != 'prepared' ||
            token == null ||
            report.data?['observation_confirmation'] != true) {
          throw const FormatException(
            'Invalid observation session preparation',
          );
        }
        final l = labels;
        final confirmed = await showDialog<bool>(
          context: context,
          builder: (context) => AlertDialog(
            title: Text(l[confirmLabel]),
            content: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 450),
              child: SingleChildScrollView(
                child: Text(
                  l[confirmLabel == 'sessionStart'
                      ? 'sessionConsent'
                      : 'sessionStopPrompt'],
                ),
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(context, false),
                child: Text(l['autostartCancel']),
              ),
              Semantics(
                identifier: 'session.confirm',
                child: FilledButton(
                  key: const ValueKey('session.confirm'),
                  onPressed: () => Navigator.pop(context, true),
                  child: Text(l[confirmLabel]),
                ),
              ),
            ],
          ),
        );
        if (!mounted || confirmed != true) return;
        if (client.session != connection ||
            !client.operational ||
            !client.canRequest) {
          setState(() => issue = l['nativeContextChanged']);
          return;
        }
        report = await client.requestTools(ToolsRequest.sessionConfirm(token));
      }
      if (mounted && client.session == connection) {
        setState(() {
          readback = report.observationSession;
          readbackConnection = connection;
          issue = report.error;
        });
      }
    } on Object catch (error) {
      if (mounted && client.session == connection) {
        setState(() => issue = '${labels['nativeUnconfirmed']}\n$error');
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.client,
    builder: (context, _) {
      final l = labels;
      final available = !busy && widget.client.canRequest;
      final operational = widget.client.operational;
      final status = readbackConnection == widget.client.session
          ? readback ?? widget.status
          : widget.status;
      final cadence = status.cadence;
      final selected = cadence.requestedMs ?? cadence.presentationMs;
      final options = {1000, 2000, 5000, cadence.nativeMs, selected}.toList()
        ..sort();
      final recording = status.phase == ObservationPhase.recording;
      final stopping = status.phase == ObservationPhase.stopping;
      final phase = switch (status.phase) {
        ObservationPhase.idle => 'sessionIdle',
        ObservationPhase.recording => 'sessionRecording',
        ObservationPhase.stopping => 'sessionStopping',
        ObservationPhase.failed => 'sessionFailed',
      };
      return Card(
        margin: EdgeInsets.zero,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                l['observationSession'],
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 8),
              Text(l[phase], key: const ValueKey('session.phase')),
              Text('${l['sessionNativeCadence']} · ${cadence.nativeMs} ms'),
              if (cadence.cameraInventoryMs case final inventory?)
                Text('${l['sessionCameraCadence']} · $inventory ms'),
              Text(
                '${l['sessionRequestedCadence']} · '
                '${cadence.requestedMs == null ? '—' : '${cadence.requestedMs} ms'}',
              ),
              Text(
                '${l['sessionEffectiveCadence']} · ${cadence.presentationMs} ms',
              ),
              if (operational) ...[
                const SizedBox(height: 8),
                Text(l['sessionPresentationOnly']),
                DropdownButton<int>(
                  key: const ValueKey('session.cadence'),
                  value: selected,
                  items: options
                      .map(
                        (ms) =>
                            DropdownMenuItem(value: ms, child: Text('$ms ms')),
                      )
                      .toList(),
                  onChanged: available && !stopping
                      ? (ms) {
                          if (ms != null)
                            _request(ToolsRequest.sessionCadence(ms));
                        }
                      : null,
                ),
              ],
              if (status.path case final path?) ...[
                Text(l['sessionFile']),
                SelectableText(path, key: const ValueKey('session.path')),
                Text(
                  '${l['sessionEvents']} · ${status.events}',
                  key: const ValueKey('session.events'),
                ),
                Text(
                  '${l['sessionBytes']} · ${status.bytes} / ${status.maxBytes}',
                ),
              ],
              if (status.error case final error?)
                SelectableText(
                  error,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              if (issue case final detail?)
                SelectableText(
                  detail,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              const SizedBox(height: 8),
              if (operational)
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    Semantics(
                      identifier: recording ? 'session.stop' : 'session.start',
                      child: FilledButton(
                        key: ValueKey(
                          recording ? 'session.stop' : 'session.start',
                        ),
                        onPressed: available && !stopping
                            ? () => _request(
                                recording
                                    ? const ToolsRequest.sessionStop()
                                    : ToolsRequest.sessionStart(selected),
                                confirmLabel: recording
                                    ? 'sessionStop'
                                    : 'sessionStart',
                              )
                            : null,
                        child: Text(
                          l[recording ? 'sessionStop' : 'sessionStart'],
                        ),
                      ),
                    ),
                    Semantics(
                      identifier: 'session.read',
                      child: OutlinedButton(
                        key: const ValueKey('session.read'),
                        onPressed: available
                            ? () => _request(const ToolsRequest.sessionStatus())
                            : null,
                        child: Text(l['sessionReadback']),
                      ),
                    ),
                  ],
                ),
            ],
          ),
        ),
      );
    },
  );
}
