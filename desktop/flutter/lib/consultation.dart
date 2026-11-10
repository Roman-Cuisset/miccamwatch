import 'dart:async';

import 'package:flutter/foundation.dart';

import 'consultation_model.dart';
import 'tools_model.dart';
import 'snapshot.dart' show protocolVersion;

enum QueryState {
  idle,
  pending,
  accepted,
  succeeded,
  failed,
  rejected,
  uncertain,
}

class QueryStatus {
  QueryState state = QueryState.idle;
  String? code;
  String? detail;
  String? localIssue;
  bool get busy => state == QueryState.pending || state == QueryState.accepted;
  void _reset() {
    state = QueryState.idle;
    code = detail = localIssue = null;
  }
}

/// Own above navigation. Only a newer snapshot in the current authenticated
/// session may complete the latest request. Timeouts are not native failures.
class ConsultationClient extends ChangeNotifier {
  ConsultationClient({
    required Future<void> Function(Map<String, dynamic>) send,
    Duration timeout = const Duration(seconds: 15),
  }) : _send = send,
       _timeout = timeout;

  final Future<void> Function(Map<String, dynamic>) _send;
  final Duration _timeout;
  final Set<String> _retiredSessions = {};
  final Set<String> _usedCursors = {};
  String _session = '';
  int _sequence = 0;
  int _nextId = 1;
  bool _live = false;
  bool _available = false;
  bool _operational = false;
  bool _disposed = false;
  String? reason;
  _Query? _ticket;

  final historyStatus = QueryStatus();
  final diagnosticsStatus = QueryStatus();
  final autostartStatus = QueryStatus();
  final toolsStatus = QueryStatus();
  NativeReport? toolsReport;
  bool toolsStale = false;
  bool get operational => _operational;
  AutostartView? autostart;
  bool autostartStale = false;
  String get session => _session;
  bool get canSetAutostart => autostartDisabledReason == null;
  String? get autostartDisabledReason {
    if (disabledReason != null) return disabledReason;
    if (!_operational) return 'autostartDevelopment';
    if (autostart?.enabled == null) return 'autostartUnknown';
    if (autostartStale) return 'autostartPrevious';
    if (!autostart!.writable) return 'autostartReadOnly';
    return null;
  }

  List<HistoryEntry> _entries = const [];
  List<String> _historyIssues = const [];
  List<HistoryEntry> get entries => _entries;
  List<String> get historyIssues => _historyIssues;
  bool historyLoaded = false;
  bool historyIncomplete = false;
  bool historyStale = false;
  String? _cursor;
  String? get nextCursor => _cursor;
  DetailedDiagnostics? report;
  bool diagnosticsStale = false;
  bool get busy => _ticket != null && _status(_ticket!.operation).busy;
  bool get canRequest => disabledReason == null;
  bool get canLoadMore =>
      canRequest && historyLoaded && !historyStale && _cursor != null;
  String? get disabledReason {
    if (_disposed || !_live) return 'queryDisconnected';
    if (!_available) return 'queryUnavailable';
    if (busy) return 'queryBusy';
    if (_nextId > 0xffffffff) return 'queryExhausted';
    return null;
  }

  QueryStatus _status(ConsultationOperation operation) => switch (operation) {
    ConsultationOperation.history => historyStatus,
    ConsultationOperation.diagnostics => diagnosticsStatus,
    ConsultationOperation.autostartStatus ||
    ConsultationOperation.setAutostart => autostartStatus,
    ConsultationOperation.tools => toolsStatus,
  };

  void _markPrevious(ConsultationOperation operation) {
    switch (operation) {
      case ConsultationOperation.history:
        historyStale = historyLoaded;
        _cursor = null;
      case ConsultationOperation.diagnostics:
        diagnosticsStale = report != null;
      case ConsultationOperation.autostartStatus ||
          ConsultationOperation.setAutostart:
        autostartStale = autostart != null;
      case ConsultationOperation.tools:
        toolsStale = toolsReport != null;
    }
  }

  void _invalidate() {
    final ticket = _ticket;
    if (ticket != null) {
      ticket.timer?.cancel();
      _failTools(ticket, 'queryDisconnected');
      final status = _status(ticket.operation);
      if (status.busy) {
        status.state = QueryState.uncertain;
        status.localIssue = 'queryDisconnected';
      }
    }
    _ticket = null;
    _cursor = null;
    _usedCursors.clear();
    if (historyLoaded) historyStale = true;
    if (report != null) diagnosticsStale = true;
    if (autostart != null) autostartStale = true;
    if (toolsReport != null) toolsStale = true;
  }

  void update(
    ConsultationView view,
    String session,
    int sequence,
    bool live, {
    bool operational = false,
  }) {
    if (_disposed) return;
    final valid =
        RegExp(r'^[0-9a-f]{32}$').hasMatch(session) &&
        sequence > 0 &&
        sequence <= 0x7fffffffffffffff;
    if (!live || !valid) {
      _live = false;
      _invalidate();
      notifyListeners();
      return;
    }
    if (_retiredSessions.contains(session) ||
        (session == _session && sequence <= _sequence))
      return;
    if (session != _session) {
      if (_session.isNotEmpty) _retiredSessions.add(_session);
      _invalidate();
      _session = session;
      _sequence = 0;
      _nextId = 1;
      historyStatus._reset();
      diagnosticsStatus._reset();
      autostartStatus._reset();
      toolsStatus._reset();
    }
    _live = true;
    _sequence = sequence;
    _operational = operational;
    _available = view.available;
    reason = view.reason;
    if (!_available) {
      _invalidate();
      notifyListeners();
      return;
    }
    final result = view.result;
    final ticket = _ticket;
    if (result != null &&
        ticket != null &&
        result.id == ticket.id &&
        result.operation == ticket.operation &&
        sequence > ticket.sequence) {
      final status = _status(ticket.operation);
      if (result.state == ConsultationState.accepted) {
        if (status.state != QueryState.uncertain) {
          status.state = QueryState.accepted;
          status.code = result.code;
          status.detail = result.detail;
        }
      } else {
        ticket.timer?.cancel();
        status.state = QueryState.values.byName(result.state.name);
        status.code = result.code;
        status.detail = result.detail;
        status.localIssue = null;
        if (result.state == ConsultationState.succeeded) {
          if (ticket.operation == ConsultationOperation.history) {
            final page = result.history!;
            // A server cursor must advance; never append the same page twice.
            if (page.events.length > ticket.limit ||
                (page.nextCursor != null &&
                    (page.nextCursor == ticket.cursor ||
                        _usedCursors.contains(page.nextCursor)))) {
              status.state = QueryState.failed;
              status.localIssue = 'queryPaginationInvalid';
              historyStale = historyLoaded;
              _cursor = null;
            } else {
              if (ticket.cursor == null) {
                _entries = page.events;
                _historyIssues = page.issues;
                historyIncomplete = page.incomplete;
              } else {
                _usedCursors.add(ticket.cursor!);
                _entries = List.unmodifiable([..._entries, ...page.events]);
                _historyIssues = List.unmodifiable({
                  ..._historyIssues,
                  ...page.issues,
                });
                historyIncomplete = historyIncomplete || page.incomplete;
              }
              historyLoaded = true;
              historyStale = false;
              _cursor = page.nextCursor;
            }
          } else if (ticket.operation == ConsultationOperation.diagnostics) {
            report = result.diagnostics!;
            diagnosticsStale = false;
          } else if (ticket.operation == ConsultationOperation.tools) {
            final native = result.tools!;
            if ((ticket.tools?.action == 'prepare' || ticket.tools?.action == 'commit')
                && native.control != ticket.tools?.control) {
              status.state = QueryState.failed;
              status.localIssue = 'queryNativeMismatch';
              _failTools(ticket, 'queryNativeMismatch');
            } else {
              toolsReport = native;
              toolsStale = false;
              if (native.outcome == 'not_confirmed') {
                status.state = QueryState.uncertain;
                status.detail = native.error;
              }
              if (ticket.tools?.action == 'clear_history' && native.data?['cleared'] == true) {
                _entries = const [];
                _historyIssues = const [];
                _cursor = null;
                _usedCursors.clear();
                historyLoaded = false;
                historyStale = true;
              }
              if (!(ticket.completion?.isCompleted ?? true)) ticket.completion!.complete(native);
            }
          } else if (ticket.operation == ConsultationOperation.setAutostart &&
              result.autostart!.enabled != ticket.requested) {
            status.state = QueryState.failed;
            status.localIssue = 'autostartReadbackMismatch';
            _markPrevious(ticket.operation);
          } else {
            autostart = result.autostart!;
            autostartStale = false;
          }
        } else {
          _markPrevious(ticket.operation);
          _failTools(ticket, result.detail ?? result.code ?? result.state.name);
        }
        // An uncertain result can still be resolved by a later native completion.
        if (result.state != ConsultationState.uncertain) _ticket = null;
      }
    }
    notifyListeners();
  }

  Future<void> refreshHistory({int limit = 30}) async =>
      _request(HistoryQuery(limit: limit));
  Future<void> loadMore() async {
    if (!canLoadMore) return;
    await _request(HistoryQuery(cursor: _cursor));
  }

  Future<void> refreshDiagnostics() => _request(const DiagnosticsQuery());
  Future<NativeReport> requestTools(ToolsRequest request) {
    if (!canRequest) return Future.error(StateError(disabledReason ?? 'queryUnavailable'));
    final completion = Completer<NativeReport>();
    unawaited(_request(ToolsQuery(request), completion: completion));
    return completion.future;
  }

  void _failTools(_Query ticket, String reason) {
    if (!(ticket.completion?.isCompleted ?? true)) {
      ticket.completion!.completeError(StateError(reason));
    }
  }
  Future<void> refreshAutostart() => _request(const AutostartStatusQuery());
  Future<void> setAutostart(
    bool requested, {
    required bool expectedEnabled,
  }) async {
    if (!canSetAutostart ||
        autostart!.enabled != expectedEnabled ||
        requested == expectedEnabled)
      return;
    await _request(
      SetAutostartQuery(requested: requested, expectedEnabled: expectedEnabled),
    );
  }

  Future<void> _request(ConsultationQuery query, {Completer<NativeReport>? completion}) async {
    final operation = query.kind;
    final cursor = query is HistoryQuery ? query.cursor : null;
    final limit = query is HistoryQuery ? query.limit : 30;
    if (!canRequest) {
      completion?.completeError(StateError(disabledReason ?? 'queryUnavailable'));
      return;
    }
    // Superseding an uncertain request intentionally retires its late response.
    _ticket?.timer?.cancel();
    if (_ticket case final previous?) _failTools(previous, 'querySuperseded');
    final ticket = _Query(
      _nextId++,
      operation,
      _sequence,
      cursor,
      limit,
      query is SetAutostartQuery ? query.requested : null,
      tools: query is ToolsQuery ? query.request : null, completion: completion,
    );
    _ticket = ticket;
    final status = _status(operation);
    status._reset();
    status.state = QueryState.pending;
    if (operation == ConsultationOperation.history) {
      if (cursor == null) {
        _cursor = null;
        _usedCursors.clear();
        historyStale = historyLoaded;
      }
    } else {
      _markPrevious(operation);
    }
    final envelope = <String, dynamic>{
      'type': 'consultation_request',
      'protocol_version': protocolVersion,
      'request': ConsultationRequest(
        session: _session,
        id: ticket.id,
        observedSequence: _sequence,
        operation: query,
      ).toJson(),
    };
    ticket.timer = Timer(_timeout, () {
      if (_disposed || !identical(_ticket, ticket)) return;
      status.state = QueryState.uncertain;
      status.localIssue = 'queryTimeout';
      _markPrevious(operation);
      _failTools(ticket, 'queryTimeout');
      notifyListeners();
    });
    notifyListeners();
    try {
      await _send(envelope);
    } catch (_) {
      if (_disposed || !identical(_ticket, ticket)) return;
      ticket.timer?.cancel();
      status.state = QueryState.uncertain;
      status.localIssue = 'querySendFailed';
      _markPrevious(operation);
      _failTools(ticket, 'querySendFailed');
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _ticket?.timer?.cancel();
    if (_ticket case final ticket?) _failTools(ticket, 'queryDisconnected');
    super.dispose();
  }
}

class _Query {
  _Query(
    this.id,
    this.operation,
    this.sequence,
    this.cursor,
    this.limit,
    this.requested, {
    this.tools,
    this.completion,
  });
  final int id;
  final ConsultationOperation operation;
  final int sequence;
  final String? cursor;
  final int limit;
  final bool? requested;
  final ToolsRequest? tools;
  final Completer<NativeReport>? completion;
  Timer? timer;
}
