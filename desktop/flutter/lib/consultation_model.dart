import 'dart:convert';

enum ConsultationOperation {
  history('history'),
  diagnostics('diagnostics'),
  autostartStatus('autostart_status'),
  setAutostart('set_autostart');

  const ConsultationOperation(this.wireName);
  final String wireName;
}

enum ConsultationState { accepted, succeeded, failed, rejected, uncertain }

enum DiagnosticStatus { ok, warning, error, unknown }

sealed class ConsultationQuery {
  const ConsultationQuery();
  ConsultationOperation get kind;
  Map<String, dynamic> toJson();
}

class HistoryQuery extends ConsultationQuery {
  HistoryQuery({this.cursor, this.limit = 30}) {
    if (limit < 1 || limit > 50) throw RangeError.range(limit, 1, 50, 'limit');
    if (cursor != null) {
      _text(cursor, maximum: 256);
      if (cursor!.isEmpty) _invalid('Empty history cursor');
    }
  }
  final String? cursor;
  final int limit;
  @override
  ConsultationOperation get kind => ConsultationOperation.history;
  @override
  Map<String, dynamic> toJson() => {
    'kind': kind.wireName,
    'cursor': cursor,
    'limit': limit,
  };
}

class DiagnosticsQuery extends ConsultationQuery {
  const DiagnosticsQuery();
  @override
  ConsultationOperation get kind => ConsultationOperation.diagnostics;
  @override
  Map<String, dynamic> toJson() => {'kind': kind.wireName};
}

class AutostartStatusQuery extends ConsultationQuery {
  const AutostartStatusQuery();
  @override
  ConsultationOperation get kind => ConsultationOperation.autostartStatus;
  @override
  Map<String, dynamic> toJson() => {'kind': kind.wireName};
}

class SetAutostartQuery extends ConsultationQuery {
  const SetAutostartQuery({
    required this.requested,
    required this.expectedEnabled,
  });
  final bool requested;
  final bool expectedEnabled;
  @override
  ConsultationOperation get kind => ConsultationOperation.setAutostart;
  @override
  Map<String, dynamic> toJson() => {
    'kind': kind.wireName,
    'requested': requested,
    'expected_enabled': expectedEnabled,
    'confirmation': 'change_autostart_for_this_installation',
  };
}

class ConsultationRequest {
  ConsultationRequest({
    required this.session,
    required this.id,
    required this.observedSequence,
    required this.operation,
  }) {
    if (!RegExp(r'^[0-9a-f]{32}$').hasMatch(session))
      _invalid('Invalid consultation session');
    _integer(id, minimum: 1, maximum: 0xffffffff);
    _integer(observedSequence, minimum: 1, maximum: 0x7fffffffffffffff);
  }
  final String session;
  final int id;
  final int observedSequence;
  final ConsultationQuery operation;
  Map<String, dynamic> toJson() => {
    'session': session,
    'id': id,
    'observed_sequence': observedSequence,
    'operation': operation.toJson(),
  };
}

/// Native-owned consultation and installation settings, independent of microphone IDs.
class ConsultationView {
  const ConsultationView.unavailable()
    : available = false,
      reason = null,
      result = null;
  const ConsultationView._(this.available, this.reason, this.result);
  final bool available;
  final String? reason;
  final ConsultationResult? result;

  factory ConsultationView.fromJson(dynamic value) {
    final map = _object(value, ['available', 'reason', 'result']);
    final available = _boolean(map['available']);
    return ConsultationView._(
      available,
      _optionalText(map['reason']),
      map['result'] == null ? null : ConsultationResult.fromJson(map['result']),
    );
  }
}

class ConsultationResult {
  const ConsultationResult._(
    this.id,
    this.state,
    this.code,
    this.detail,
    this.operation,
    this.history,
    this.diagnostics,
    this.autostart,
  );
  final int id;
  final ConsultationState state;
  final String? code;
  final String? detail;
  final ConsultationOperation operation;
  final HistoryPage? history;
  final DetailedDiagnostics? diagnostics;
  final AutostartView? autostart;

  factory ConsultationResult.fromJson(dynamic value) {
    final map = _object(value, [
      'id',
      'state',
      'code',
      'detail',
      'operation',
      'history',
      'diagnostics',
      'autostart',
    ]);
    final id = _integer(map['id'], minimum: 1, maximum: 0xffffffff);
    final state = _enumeration(map['state'], ConsultationState.values);
    final operation = ConsultationOperation.values.firstWhere(
      (item) => item.wireName == map['operation'],
      orElse: () => _invalid('Unknown consultation operation'),
    );
    final succeeded = state == ConsultationState.succeeded;
    if (succeeded) {
      final payload = switch (operation) {
        ConsultationOperation.history => 'history',
        ConsultationOperation.diagnostics => 'diagnostics',
        ConsultationOperation.autostartStatus ||
        ConsultationOperation.setAutostart => 'autostart',
      };
      if (map[payload] == null ||
          [
            'history',
            'diagnostics',
            'autostart',
          ].any((key) => key != payload && map[key] != null)) {
        _invalid('Successful consultation lacks its exclusive payload');
      }
    } else if (map['history'] != null ||
        map['diagnostics'] != null ||
        map['autostart'] != null) {
      _invalid('Non-successful consultation carries data');
    }
    return ConsultationResult._(
      id,
      state,
      _optionalText(map['code'], maximum: 64),
      _optionalText(map['detail']),
      operation,
      map['history'] == null ? null : HistoryPage.fromJson(map['history']),
      map['diagnostics'] == null
          ? null
          : DetailedDiagnostics.fromJson(map['diagnostics']),
      map['autostart'] == null
          ? null
          : AutostartView.fromJson(
              map['autostart'],
              requireKnown: operation == ConsultationOperation.setAutostart,
            ),
    );
  }
}

class AutostartView {
  const AutostartView._(this.enabled, this.writable, this.reason);
  final bool? enabled;
  final bool writable;
  final String? reason;
  factory AutostartView.fromJson(dynamic value, {bool requireKnown = false}) {
    final map = _object(value, ['enabled', 'writable', 'reason']);
    if (map['enabled'] != null && map['enabled'] is! bool)
      _invalid('Invalid autostart readback');
    if (requireKnown && map['enabled'] == null)
      _invalid('Autostart change lacks native readback');
    return AutostartView._(
      map['enabled'] as bool?,
      _boolean(map['writable']),
      _optionalText(map['reason']),
    );
  }
}

class HistoryEntry {
  const HistoryEntry._(
    this.observedAt,
    this.action,
    this.resource,
    this.activity,
    this.confidence,
    this.application,
    this.pid,
  );

  /// Kept verbatim: native event time and its original timezone, not receipt time.
  final String observedAt;
  final String action;
  final String resource;
  final String activity;
  final String confidence;
  final String? application;
  final int? pid;

  factory HistoryEntry.fromJson(dynamic value) {
    final map = _object(value, [
      'observed_at',
      'action',
      'resource',
      'activity',
      'confidence',
      'application',
      'pid',
    ]);
    return HistoryEntry._(
      _timestamp(map['observed_at']),
      _text(map['action'], maximum: 512),
      _text(map['resource'], maximum: 512),
      _text(map['activity'], maximum: 512),
      _text(map['confidence'], maximum: 512),
      _optionalText(map['application'], maximum: 512),
      map['pid'] == null ? null : _integer(map['pid'], maximum: 0xffffffff),
    );
  }
}

class HistoryPage {
  const HistoryPage._(
    this.events,
    this.nextCursor,
    this.incomplete,
    this.issues,
  );
  final List<HistoryEntry> events;
  final String? nextCursor;
  final bool incomplete;
  final List<String> issues;

  factory HistoryPage.fromJson(dynamic value) {
    final map = _object(value, [
      'events',
      'next_cursor',
      'incomplete',
      'issues',
      'ordering',
    ]);
    if (map['ordering'] != 'recorded_newest_first')
      _invalid('Unknown history ordering');
    final cursor = _optionalText(map['next_cursor'], maximum: 256);
    if (cursor != null && cursor.isEmpty) _invalid('Empty history cursor');
    return HistoryPage._(
      List.unmodifiable(_list(map['events'], 50).map(HistoryEntry.fromJson)),
      cursor,
      _boolean(map['incomplete']),
      List.unmodifiable(
        _list(map['issues'], 16).map((v) => _text(v, maximum: 256)),
      ),
    );
  }
}

class DiagnosticCheck {
  const DiagnosticCheck._(this.name, this.status, this.detail, this.remedy);
  final String name;
  final DiagnosticStatus status;
  final String detail;
  final String? remedy;

  factory DiagnosticCheck.fromJson(dynamic value) {
    final map = _object(value, ['name', 'status', 'detail', 'remedy']);
    return DiagnosticCheck._(
      _text(map['name']),
      _enumeration(map['status'], DiagnosticStatus.values),
      _text(map['detail']),
      _optionalText(map['remedy']),
    );
  }
}

class DetailedDiagnostics {
  const DetailedDiagnostics._(
    this.checks,
    this.limitations,
    this.engineVersion,
    this.platform,
    this.observedAt,
    this.incomplete,
  );
  final List<DiagnosticCheck> checks;
  final List<String> limitations;
  final String engineVersion;
  final String platform;
  final String observedAt;
  final bool incomplete;

  factory DetailedDiagnostics.fromJson(dynamic value) {
    final map = _object(value, [
      'checks',
      'limitations',
      'engine_version',
      'platform',
      'observed_at',
      'incomplete',
    ]);
    if (!['windows', 'linux', 'macos'].contains(map['platform']))
      _invalid('Unknown platform');
    return DetailedDiagnostics._(
      List.unmodifiable(_list(map['checks'], 64).map(DiagnosticCheck.fromJson)),
      List.unmodifiable(_list(map['limitations'], 64).map((v) => _text(v))),
      _text(map['engine_version']),
      map['platform'] as String,
      _timestamp(map['observed_at']),
      _boolean(map['incomplete']),
    );
  }
}

Never _invalid(String message) => throw FormatException(message);
Map<String, dynamic> _object(dynamic value, List<String> keys) {
  if (value is! Map<String, dynamic> ||
      value.length != keys.length ||
      keys.any((key) => !value.containsKey(key)))
    _invalid('Invalid consultation object');
  return value;
}

List<dynamic> _list(dynamic value, int maximum) {
  if (value is! List || value.length > maximum)
    _invalid('Invalid consultation list');
  return value;
}

String _text(dynamic value, {int maximum = 1024}) {
  if (value is! String || utf8.encode(value).length > maximum)
    _invalid('Invalid consultation text');
  return value;
}

String? _optionalText(dynamic value, {int maximum = 1024}) =>
    value == null ? null : _text(value, maximum: maximum);
bool _boolean(dynamic value) {
  if (value is! bool) _invalid('Invalid consultation boolean');
  return value;
}

int _integer(dynamic value, {int minimum = 0, required int maximum}) {
  if (value is! int || value < minimum || value > maximum)
    _invalid('Invalid consultation integer');
  return value;
}

T _enumeration<T extends Enum>(dynamic value, List<T> values) {
  for (final item in values) {
    if (item.name == value) return item;
  }
  _invalid('Unknown consultation enum');
}

String _timestamp(dynamic value) {
  final text = _text(value, maximum: 512);
  final match = RegExp(
    r'^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(\.\d+)?(Z|[+-](\d{2}):(\d{2}))$',
  ).firstMatch(text);
  if (match == null || DateTime.tryParse(text) == null)
    _invalid('Invalid consultation timestamp');
  final year = int.parse(match.group(1)!);
  final month = int.parse(match.group(2)!);
  final day = int.parse(match.group(3)!);
  if (month < 1 ||
      month > 12 ||
      day < 1 ||
      day > DateTime.utc(year, month + 1, 0).day ||
      int.parse(match.group(4)!) > 23 ||
      int.parse(match.group(5)!) > 59 ||
      int.parse(match.group(6)!) > 59 ||
      (match.group(9) != null && int.parse(match.group(9)!) > 23) ||
      (match.group(10) != null && int.parse(match.group(10)!) > 59)) {
    _invalid('Invalid consultation calendar timestamp');
  }
  return text;
}
