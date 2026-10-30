import 'dart:async';

import 'package:flutter/foundation.dart';

import 'consultation_model.dart';
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
    }
  }

  void _invalidate() {
    final ticket = _ticket;
    if (ticket != null) {
      ticket.timer?.cancel();
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

  Future<void> _request(ConsultationQuery query) async {
    final operation = query.kind;
    final cursor = query is HistoryQuery ? query.cursor : null;
    final limit = query is HistoryQuery ? query.limit : 30;
    if (!canRequest) return;
    // Superseding an uncertain request intentionally retires its late response.
    _ticket?.timer?.cancel();
    final ticket = _Query(
      _nextId++,
      operation,
      _sequence,
      cursor,
      limit,
      query is SetAutostartQuery ? query.requested : null,
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
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _ticket?.timer?.cancel();
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
    this.requested,
  );
  final int id;
  final ConsultationOperation operation;
  final int sequence;
  final String? cursor;
  final int limit;
  final bool? requested;
  Timer? timer;
}
