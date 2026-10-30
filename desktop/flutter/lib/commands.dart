import 'dart:async';

import 'package:flutter/material.dart';

import 'snapshot.dart' show Snapshot, protocolVersion;

/// Keep one client above navigation and call [update] for every snapshot and
/// connection change. This controller never represents actual protection.
class MicrophoneCommandClient extends ChangeNotifier {
  MicrophoneCommandClient({
    required Future<void> Function(Map<String, dynamic>) send,
    Duration timeout = const Duration(seconds: 15),
  }) : _send = send, _timeout = timeout;

  static const _maxId = 0xffffffff;
  static const _maxCounter = 0x7fffffffffffffff;
  static final _sessionPattern = RegExp(r'^[0-9a-f]{32}$');
  final Future<void> Function(Map<String, dynamic>) _send;
  final Duration _timeout;
  final Set<String> _retiredSessions = {};
  String _session = '';
  bool _connected = false;
  bool _valid = false;
  bool _canEnable = false;
  bool _canRelease = false;
  bool? _currentRequested;
  int _generation = 0;
  int _observedSequence = 0;
  int _nextId = 1;
  String? _reason;
  String? _state;
  String? _code;
  String? _detail;
  String? _error;
  _PendingCommand? _pending;
  bool _disposed = false;

  String get session => _session;
  bool get connected => _connected;
  int get observedSequence => _observedSequence;
  int get generation => _generation;
  bool? get currentRequested => _currentRequested;
  String? get reason => _reason;
  String? get state => _state;
  String? get code => _code;
  String? get detail => _detail;

  /// Local failure key, localized by the panel; backend code/detail are separate.
  String? get error => _error;
  bool get pending => _pending != null;
  int? get pendingId => _pending?.id;
  bool get canEnable => _gate(true) == null;
  bool get canRelease => _gate(false) == null;
  String? get disabledReason => canEnable || canRelease ? null : _gate(true);

  String? _gate(bool requested) {
    if (_disposed) return 'unavailable';
    if (!_connected) return 'disconnected';
    if (_pending != null) return 'busy';
    if (!_valid || _session.isEmpty || _observedSequence <= 0 ||
        _currentRequested == null) return 'unavailable';
    if (_nextId > _maxId) return 'exhausted';
    if (!(requested ? _canEnable : _canRelease)) return 'capability';
    return null;
  }

  /// Snapshots are authoritative for capabilities/requested state, not send or
  /// acceptance. Old sessions and regressing sequences cannot revive controls.
  void update(Map<String, dynamic> controls, int observedSequence, bool connected) {
    if (_disposed) return;
    final incomingSession = controls['session'];
    if (incomingSession is String && _retiredSessions.contains(incomingSession)) {
      return;
    }
    if (incomingSession == _session && observedSequence < _observedSequence) {
      // A disconnect still disables commands, even with a stale last snapshot.
      if (!connected && _connected) {
        _connected = false;
        notifyListeners();
      }
      return;
    }
    _connected = connected;
    final microphone = controls['microphone'];
    if (incomingSession is! String ||
        !_sessionPattern.hasMatch(incomingSession) ||
        observedSequence <= 0 || observedSequence > _maxCounter ||
        microphone is! Map<String, dynamic> ||
        microphone['can_enable'] is! bool ||
        microphone['can_release'] is! bool ||
        (microphone['current_requested'] != null &&
            microphone['current_requested'] is! bool) ||
        microphone['generation'] is! int ||
        (microphone['generation'] as int) < 0 ||
        (microphone['generation'] as int) > _maxCounter ||
        (microphone['reason'] != null && microphone['reason'] is! String)) {
      _valid = false;
      notifyListeners();
      return;
    }
    if (incomingSession != _session) {
      if (_session.isNotEmpty) _retiredSessions.add(_session);
      _pending?.timer?.cancel();
      _pending = null;
      _session = incomingSession;
      _nextId = 1;
      _observedSequence = 0;
      _state = null;
      _code = null;
      _detail = null;
      _error = null;
    }
    _valid = true;
    // Re-delivery for a connection change must not replay an old result or
    // replace capabilities with another payload bearing the same sequence.
    if (observedSequence == _observedSequence) {
      notifyListeners();
      return;
    }
    _observedSequence = observedSequence;
    _canEnable = microphone['can_enable'] as bool;
    _canRelease = microphone['can_release'] as bool;
    _currentRequested = microphone['current_requested'] as bool?;
    _generation = microphone['generation'] as int;
    _reason = microphone['reason'] as String?;
    final result = controls['command'];
    final operation = _pending;
    if (operation != null && result is Map<String, dynamic> &&
        result['id'] == operation.id &&
        result['requested'] == operation.requested &&
        (result['code'] == null || result['code'] is String) &&
        (result['detail'] == null || result['detail'] is String)) {
      final resultState = result['state'];
      if (resultState == 'accepted') {
        // A late acceptance does not resolve a local timeout/delivery error.
        if (_state != 'uncertain') {
          _state = 'accepted';
          _code = result['code'] as String?;
          _detail = result['detail'] as String?;
        }
      } else if (const ['succeeded', 'failed', 'rejected', 'uncertain']
          .contains(resultState)) {
        operation.timer?.cancel();
        _pending = null;
        _state = resultState as String;
        _code = result['code'] as String?;
        _detail = result['detail'] as String?;
        _error = null;
      }
    }
    notifyListeners();
  }

  /// Invoke only after human confirmation of this explicit desired value.
  /// Transport failures/timeouts retain the ticket: delivery may have happened.
  Future<void> request(bool requested) async {
    if (_disposed) return;
    final blocked = _gate(requested);
    if (blocked != null) {
      _error = blocked;
      notifyListeners();
      return;
    }
    final operation = _PendingCommand(_nextId++, requested);
    _pending = operation;
    _state = 'pending';
    _code = null;
    _detail = null;
    _error = null;
    final envelope = <String, dynamic>{
      'type': 'command',
      'protocol_version': protocolVersion,
      'request': <String, dynamic>{
        'session': _session,
        'id': operation.id,
        'action': 'set_microphone_protection',
        'requested': requested,
        'expected_requested': _currentRequested,
        'expected_generation': _generation,
        'observed_sequence': _observedSequence,
        'confirmation': requested
            ? 'sdk_mute_all_inputs' : 'release_all_active_inputs',
      },
    };
    operation.timer = Timer(_timeout, () {
      if (_disposed || !identical(_pending, operation)) return;
      _state = 'uncertain';
      _code = 'client_timeout';
      _detail = null;
      notifyListeners();
    });
    notifyListeners();
    try {
      await _send(envelope);
    } catch (_) {
      if (_disposed || !identical(_pending, operation)) return;
      operation.timer?.cancel();
      _state = 'uncertain';
      _code = 'client_transport_error';
      _detail = null;
      _error = 'sendFailed';
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _pending?.timer?.cancel();
    super.dispose();
  }
}

class _PendingCommand {
  _PendingCommand(this.id, this.requested);
  final int id;
  final bool requested;
  Timer? timer;
}

/// The caller owns [client]; removing this widget never cancels or resends work.
class MicrophoneCommandPanel extends StatefulWidget {
  const MicrophoneCommandPanel({
    super.key,
    required this.client,
    required this.locale,
    required this.snapshot,
  });
  final MicrophoneCommandClient client;
  final String locale;
  final Snapshot? snapshot;

  @override
  State<MicrophoneCommandPanel> createState() => _MicrophoneCommandPanelState();
}

class _MicrophoneCommandPanelState extends State<MicrophoneCommandPanel> {
  bool _confirming = false;
  bool _confirmationChanged = false;

  bool get _windowsEngine => widget.snapshot?.engine['platform'] == 'windows';

  Future<void> _confirm(bool requested) async {
    if (_confirming || !_windowsEngine) return;
    final client = widget.client;
    final session = client.session;
    final generation = client.generation;
    final currentRequested = client.currentRequested;
    final labels = _CommandLabels(widget.locale);
    setState(() {
      _confirming = true;
      _confirmationChanged = false;
    });
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        constraints: const BoxConstraints(maxWidth: 560),
        scrollable: true,
        title: Text(labels[requested ? 'confirmMuteTitle' : 'confirmReleaseTitle']),
        content: Text(labels[requested ? 'confirmMuteBody' : 'confirmReleaseBody']),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false),
            child: Text(labels['cancel'])),
          FilledButton(onPressed: () => Navigator.pop(context, true),
            child: Text(labels[requested ? 'mute' : 'release'])),
        ],
      ),
    );
    if (!mounted) return;
    final changed = !identical(client, widget.client) ||
        client.session != session || client.generation != generation ||
        client.currentRequested != currentRequested || !_windowsEngine;
    setState(() {
      _confirming = false;
      _confirmationChanged = confirmed == true && changed;
    });
    if (confirmed == true && !changed) await client.request(requested);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final labels = _CommandLabels(widget.locale);
    return AnimatedBuilder(
      animation: widget.client,
      builder: (context, _) {
        final client = widget.client;
        final snapshot = widget.snapshot;
        final platform = snapshot?.engine['platform'];
        final microphone = snapshot?.controls['microphone'] as Map<String, dynamic>?;
        // Unix deliberately has no qualified Flutter hardware command lane.
        // Only a matching native snapshot can distinguish that from missing
        // command state; connection, pending and real failure feedback stay intact.
        final unixUnavailable = (platform == 'linux' || platform == 'macos') &&
            snapshot?.sequence == client.observedSequence &&
            snapshot?.controls['session'] == client.session &&
            microphone?['can_enable'] == false &&
            microphone?['can_release'] == false &&
            microphone?['current_requested'] == null &&
            client.disabledReason == 'unavailable';
        final hasReason = client.reason != null && client.reason!.isNotEmpty;
        final disabledReason = unixUnavailable ? 'capability'
            : client.disabledReason ?? (_windowsEngine ? null : 'capability');
        final state = client.state;
        final feedback = state == null ? null
            : labels[state == 'uncertain' && client.code == 'client_timeout'
                ? 'timeout' : state];
        final warning = const ['failed', 'rejected', 'uncertain'].contains(state);
        return Container(
          width: double.infinity,
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            border: Border.all(color: colors.outlineVariant),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(labels['title'], style: theme.textTheme.titleSmall),
            if (platform == 'windows') ...[
              const SizedBox(height: 6),
              Text(labels['limits'], style: theme.textTheme.bodySmall),
            ],
            const SizedBox(height: 8),
            Wrap(spacing: 8, runSpacing: 6, children: [
              FilledButton.tonal(
                onPressed: _windowsEngine && !_confirming && client.canEnable ? () => _confirm(true) : null,
                child: Text(labels['mute']),
              ),
              OutlinedButton(
                onPressed: _windowsEngine && !_confirming && client.canRelease ? () => _confirm(false) : null,
                child: Text(labels['release']),
              ),
            ]),
            if (disabledReason != null) ...[
              const SizedBox(height: 6),
              Text(labels[disabledReason], style: theme.textTheme.bodySmall),
            ],
            if (hasReason) ...[
              const SizedBox(height: 6),
              Text(client.reason!, style: theme.textTheme.bodySmall),
            ],
            if (feedback != null) ...[
              const SizedBox(height: 8),
              Semantics(liveRegion: true, child: Text(feedback,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: warning ? colors.error : colors.primary))),
            ],
            if (client.detail != null && client.detail!.isNotEmpty) ...[
              const SizedBox(height: 4),
              Text(client.detail!, style: theme.textTheme.bodySmall),
            ],
            if (client.error != null) ...[
              const SizedBox(height: 6),
              Text(labels[client.error!], style: theme.textTheme.bodySmall?.copyWith(color: colors.error)),
            ],
            if (_confirmationChanged) ...[
              const SizedBox(height: 6),
              Text(labels['changed'], style: theme.textTheme.bodySmall?.copyWith(color: colors.error)),
            ],
          ]),
        );
      },
    );
  }
}

class _CommandLabels {
  _CommandLabels(String locale)
      : _index = const ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru'].indexOf(locale);
  final int _index;
  String operator [](String key) => _commandLabels[key]![_index < 0 ? 0 : _index];
}

const _commandLabels = <String, List<String>>{
  'title': ['Microphone commands', 'Commandes du microphone', 'Mikrofonsteuerung', 'Control del micrófono', 'マイク操作', '麦克风控制', 'Управление микрофоном'],
  'mute': ['Mute inputs (SDK)', 'Couper les entrées (SDK)', 'Eingänge stummschalten (SDK)', 'Silenciar entradas (SDK)', '入力をミュート (SDK)', '静音输入（SDK）', 'Отключить звук входов (SDK)'],
  'release': ['Release all active inputs', 'Libérer toutes les entrées actives', 'Alle aktiven Eingänge freigeben', 'Liberar todas las entradas activas', 'すべての有効な入力を解除', '释放所有活动输入', 'Освободить все активные входы'],
  'limits': [
    'SDK mute does not deny microphone permission. Release unmutes all active inputs; it does not restore their original mute state.',
    'La coupure SDK ne refuse pas l’autorisation du micro. Libérer réactive toutes les entrées actives, sans restaurer leur état initial.',
    'SDK-Stummschaltung entzieht keine Mikrofonberechtigung. Freigeben hebt die Stummschaltung aller aktiven Eingänge auf, ohne den ursprünglichen Zustand wiederherzustellen.',
    'Silenciar por SDK no deniega el permiso del micrófono. Liberar reactiva todas las entradas activas; no restaura su estado original.',
    'SDK のミュートはマイクの権限を拒否しません。解除はすべての有効な入力のミュートを解除し、元の状態には戻しません。',
    'SDK 静音不会拒绝麦克风权限。释放会取消所有活动输入的静音，并非恢复原先的静音状态。',
    'Отключение звука через SDK не запрещает доступ к микрофону. Освобождение включает звук всех активных входов, а не восстанавливает исходное состояние.',
  ],
  'confirmMuteTitle': ['Mute all active inputs?', 'Couper toutes les entrées actives ?', 'Alle aktiven Eingänge stummschalten?', '¿Silenciar todas las entradas activas?', 'すべての有効な入力をミュートしますか？', '静音所有活动输入？', 'Отключить звук всех активных входов?'],
  'confirmMuteBody': [
    'Apply SDK mute to all active microphone inputs. This does not revoke microphone permission or guarantee capture is blocked. Actual protection remains separate and requires native readback.',
    'Appliquer la coupure SDK à toutes les entrées micro actives. Cela ne révoque pas l’autorisation et ne garantit pas le blocage de la capture. La protection réelle reste distincte et doit être relue par le moteur natif.',
    'SDK-Stummschaltung auf alle aktiven Mikrofoneingänge anwenden. Dies entzieht keine Berechtigung und garantiert keine Aufnahmesperre. Der tatsächliche Schutz ist separat und erfordert natives Rücklesen.',
    'Aplicar silencio por SDK a todas las entradas de micrófono activas. No revoca permisos ni garantiza el bloqueo de captura. La protección real es independiente y requiere comprobación nativa.',
    'すべての有効なマイク入力に SDK ミュートを適用します。権限の取り消しや録音の遮断を保証するものではありません。実際の保護状態は別途ネイティブ側で確認します。',
    '对所有活动麦克风输入应用 SDK 静音。这不会撤销麦克风权限，也不保证阻止采集。实际保护状态须由原生端另行回读确认。',
    'Отключить звук всех активных микрофонных входов через SDK. Это не отменяет разрешение и не гарантирует блокировку записи. Фактическая защита проверяется отдельно нативным чтением состояния.',
  ],
  'confirmReleaseTitle': ['Release all active inputs?', 'Libérer toutes les entrées actives ?', 'Alle aktiven Eingänge freigeben?', '¿Liberar todas las entradas activas?', 'すべての有効な入力を解除しますか？', '释放所有活动输入？', 'Освободить все активные входы?'],
  'confirmReleaseBody': [
    'Unmute all active microphone inputs, including inputs muted before this app. This is an explicit release, not restoration of their original state. Actual protection requires native readback.',
    'Réactiver toutes les entrées micro actives, même celles déjà coupées avant cette application. C’est une libération explicite, pas une restauration de leur état initial. L’état réel doit être relu par le moteur natif.',
    'Stummschaltung aller aktiven Mikrofoneingänge aufheben, auch der vor dieser App stummgeschalteten. Dies ist eine ausdrückliche Freigabe, keine Wiederherstellung des ursprünglichen Zustands. Der tatsächliche Zustand erfordert natives Rücklesen.',
    'Reactivar todas las entradas de micrófono activas, incluso las silenciadas antes de esta aplicación. Es una liberación explícita, no una restauración del estado original. El estado real requiere comprobación nativa.',
    'このアプリ以前にミュートされていた入力を含め、すべての有効なマイク入力のミュートを解除します。これは明示的な解除であり、元の状態への復元ではありません。実際の状態はネイティブ側で確認します。',
    '取消所有活动麦克风输入的静音，包括在本应用运行前就已静音的输入。这是明确释放，不是恢复原先状态。实际状态须由原生端回读确认。',
    'Включить звук всех активных микрофонных входов, в том числе отключённых до запуска приложения. Это явное освобождение, а не восстановление исходного состояния. Фактическое состояние требует нативной проверки.',
  ],
  'cancel': ['Cancel', 'Annuler', 'Abbrechen', 'Cancelar', 'キャンセル', '取消', 'Отмена'],
  'pending': [
    'Command sent; waiting for a native result. Protection is not yet confirmed.',
    'Commande envoyée ; résultat natif en attente. La protection n’est pas encore confirmée.',
    'Befehl gesendet; natives Ergebnis steht aus. Schutz ist noch nicht bestätigt.',
    'Comando enviado; esperando el resultado nativo. La protección aún no está confirmada.',
    'コマンドを送信しました。ネイティブの結果を待機中で、保護は未確認です。',
    '命令已发送，正在等待原生端结果。保护尚未确认。',
    'Команда отправлена; ожидается нативный результат. Защита ещё не подтверждена.',
  ],
  'accepted': [
    'Native command accepted. Acceptance is not effective protection; waiting for readback.',
    'Commande acceptée par le moteur natif. Cela ne confirme pas une protection effective ; relecture en attente.',
    'Nativer Befehl angenommen. Dies bedeutet keinen wirksamen Schutz; Rücklesen steht aus.',
    'Comando nativo aceptado. La aceptación no es protección efectiva; esperando comprobación.',
    'ネイティブ側が受理しました。受理は有効な保護を意味しません。状態確認を待機中です。',
    '原生端已接受命令。接受不等于保护已生效；正在等待回读。',
    'Нативная команда принята. Это ещё не действующая защита; ожидается проверка состояния.',
  ],
  'succeeded': [
    'Command completed with native readback. See the separate actual protection status.',
    'Commande terminée après relecture native. Consultez l’état distinct de la protection réelle.',
    'Befehl mit nativem Rücklesen abgeschlossen. Den tatsächlichen Schutz zeigt die separate Statusanzeige.',
    'Comando completado con comprobación nativa. Consulte el estado independiente de protección real.',
    'ネイティブ側の状態確認が完了しました。実際の保護状態は別の表示を確認してください。',
    '命令已完成并经原生端回读。请查看单独显示的实际保护状态。',
    'Команда завершена с нативной проверкой. Смотрите отдельный статус фактической защиты.',
  ],
  'failed': [
    'Command failed. Some inputs may have changed; their actual state may be unknown.',
    'Échec de la commande. Certaines entrées ont pu changer ; leur état réel peut être inconnu.',
    'Befehl fehlgeschlagen. Einige Eingänge können verändert worden sein; ihr tatsächlicher Zustand kann unbekannt sein.',
    'El comando falló. Algunas entradas pueden haber cambiado; su estado real puede ser desconocido.',
    'コマンドは失敗しました。一部の入力は変更され、実際の状態が不明な可能性があります。',
    '命令失败。部分输入可能已改变；实际状态可能未知。',
    'Команда не выполнена. Некоторые входы могли измениться; их фактическое состояние может быть неизвестно.',
  ],
  'rejected': [
    'Command rejected. No successful change is confirmed.',
    'Commande refusée. Aucune modification réussie n’est confirmée.',
    'Befehl abgelehnt. Keine erfolgreiche Änderung bestätigt.',
    'Comando rechazado. No se confirma ningún cambio correcto.',
    'コマンドは拒否されました。変更の成功は確認されていません。',
    '命令被拒绝。未确认任何成功的更改。',
    'Команда отклонена. Успешное изменение не подтверждено.',
  ],
  'uncertain': [
    'Result uncertain. Native work may still be running; no automatic retry was sent.',
    'Résultat incertain. Le travail natif peut se poursuivre ; aucune nouvelle tentative automatique.',
    'Ergebnis ungewiss. Native Arbeit läuft möglicherweise weiter; kein automatischer Neuversuch.',
    'Resultado incierto. La operación nativa puede seguir en curso; no se reintentó automáticamente.',
    '結果は不確定です。ネイティブ処理が続いている可能性があり、自動再試行は行いません。',
    '结果不确定。原生操作可能仍在进行；未自动重试。',
    'Результат неопределён. Нативная операция может продолжаться; автоматического повтора не было.',
  ],
  'timeout': [
    'Timed out: result uncertain. Native work may still be running; it was not cancelled or retried.',
    'Délai dépassé : résultat incertain. Le travail natif peut se poursuivre ; aucune annulation ni nouvelle tentative.',
    'Zeitüberschreitung: Ergebnis ungewiss. Native Arbeit läuft möglicherweise weiter; weder abgebrochen noch wiederholt.',
    'Tiempo de espera agotado: resultado incierto. La operación nativa puede continuar; no se canceló ni reintentó.',
    'タイムアウト：結果は不確定です。ネイティブ処理は続いている可能性があり、取消も再試行もしていません。',
    '等待超时：结果不确定。原生操作可能仍在进行；未取消或重试。',
    'Время ожидания истекло: результат неопределён. Нативная работа может продолжаться; отмены и повтора не было.',
  ],
  'disconnected': [
    'No live connection. This does not cancel pending native work.',
    'Aucune connexion active. Le travail natif en attente n’est pas annulé.',
    'Keine aktive Verbindung. Ausstehende native Arbeit wird dadurch nicht abgebrochen.',
    'Sin conexión activa. Esto no cancela la operación nativa pendiente.',
    '接続がありません。実行中のネイティブ処理は取り消されません。',
    '无有效连接。这不会取消待完成的原生操作。',
    'Нет активного соединения. Ожидающая нативная операция не отменяется.',
  ],
  'busy': ['A native command is still pending; wait for its result.', 'Une commande native est en attente ; attendez son résultat.', 'Ein nativer Befehl steht noch aus; bitte auf das Ergebnis warten.', 'Hay un comando nativo pendiente; espere su resultado.', 'ネイティブコマンドが処理中です。結果をお待ちください。', '原生命令仍待完成；请等待结果。', 'Нативная команда ещё выполняется; дождитесь результата.'],
  'unavailable': ['No valid current command snapshot is available.', 'Aucun instantané de commande actuel valide.', 'Kein gültiger aktueller Steuerungsstand verfügbar.', 'No hay una instantánea de control actual válida.', '有効な最新の操作スナップショットがありません。', '没有有效的当前控制快照。', 'Нет допустимого текущего снимка управления.'],
  'capability': ['Microphone control is unavailable in the current snapshot.', 'La commande du micro est indisponible dans cet instantané.', 'Mikrofonsteuerung ist im aktuellen Stand nicht verfügbar.', 'El control del micrófono no está disponible en esta instantánea.', '現在のスナップショットではマイク操作を利用できません。', '当前快照不允许控制麦克风。', 'Управление микрофоном недоступно в текущем снимке.'],
  'exhausted': ['Command IDs exhausted; a new native session is required.', 'Identifiants épuisés ; une nouvelle session native est nécessaire.', 'Befehls-IDs aufgebraucht; eine neue native Sitzung ist erforderlich.', 'Se agotaron los identificadores; se requiere una sesión nativa nueva.', 'コマンド ID を使い切りました。新しいネイティブセッションが必要です。', '命令编号已耗尽；需要新的原生会话。', 'Идентификаторы команд исчерпаны; нужен новый нативный сеанс.'],
  'sendFailed': [
    'Delivery could not be confirmed. Native work may still run; no automatic retry.',
    'Impossible de confirmer l’envoi. Le travail natif peut se poursuivre ; aucune nouvelle tentative automatique.',
    'Zustellung nicht bestätigt. Native Arbeit kann noch laufen; kein automatischer Neuversuch.',
    'No se pudo confirmar la entrega. La operación nativa puede ejecutarse; sin reintento automático.',
    '送信を確認できません。ネイティブ処理が実行される可能性があります。自動再試行はしません。',
    '无法确认送达。原生操作仍可能执行；不会自动重试。',
    'Доставка не подтверждена. Нативная работа всё ещё возможна; автоматического повтора нет.',
  ],
  'changed': [
    'The session or requested state changed during confirmation. Review the current controls and confirm again.',
    'La session ou l’état demandé a changé pendant la confirmation. Vérifiez les commandes actuelles et confirmez à nouveau.',
    'Sitzung oder angeforderter Zustand wurde während der Bestätigung geändert. Aktuelle Steuerung prüfen und erneut bestätigen.',
    'La sesión o el estado solicitado cambió durante la confirmación. Revise los controles actuales y confirme de nuevo.',
    '確認中にセッションまたは要求状態が変わりました。現在の操作状態を確認し、改めて確定してください。',
    '确认期间会话或请求状态已改变。请检查当前控制状态后重新确认。',
    'Во время подтверждения изменился сеанс или запрошенное состояние. Проверьте текущее управление и подтвердите снова.',
  ],
};
