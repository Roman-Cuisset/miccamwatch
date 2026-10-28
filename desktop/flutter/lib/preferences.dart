import 'dart:async';

import 'package:flutter/material.dart';

import 'preferences_model.dart';
import 'snapshot.dart' show protocolVersion;

/// Own above navigation, alongside the microphone client. Both use the native
/// controls.session, but neither shares IDs, pending work, or capability state.
class PreferenceCommandClient extends ChangeNotifier {
  PreferenceCommandClient({
    required Future<void> Function(Map<String, dynamic>) send,
    Duration timeout = const Duration(seconds: 15),
  }) : _send = send, _timeout = timeout;

  final Future<void> Function(Map<String, dynamic>) _send;
  final Duration _timeout;
  final Set<String> _retiredSessions = {};
  static final _sessionPattern = RegExp(r'^[0-9a-f]{32}$');
  PreferenceView? _view;
  String _session = '';
  int _observedSequence = 0;
  int _nextId = 1;
  bool _connected = false;
  bool _valid = false;
  bool _disposed = false;
  _PendingPreference? _pending;
  String? _state;
  String? _code;
  String? _detail;
  String? _error;

  PreferenceView? get view => _view;
  String get session => _session;
  int get observedSequence => _observedSequence;
  bool get connected => _connected;
  bool get pending => _pending != null;
  int? get pendingId => _pending?.id;
  PreferenceKey? get pendingKey => _pending?.key;
  String? get state => _state;
  String? get code => _code;
  String? get detail => _detail;
  /// Local localization key; native code/detail remain separate.
  String? get error => _error;

  bool canChange(PreferenceKey key) => disabledReasonFor(key) == null;

  String? disabledReasonFor(PreferenceKey key) {
    if (_disposed) return 'unavailable';
    if (!_connected) return 'disconnected';
    if (_pending != null) return 'busy';
    final current = _view;
    if (!_valid || current == null || !current.available ||
        current.values == null) return 'unavailable';
    if (_nextId > 0xffffffff) return 'exhausted';
    if (!current.writable) return 'readOnly';
    if (key == PreferenceKey.profile && !current.profileWritable) {
      return 'profileReadOnly';
    }
    return null;
  }

  /// Only a newer snapshot can replace native values or complete a ticket.
  /// Re-delivery can update connectivity, never replay an outcome.
  void update(PreferenceView view, String session, int observedSequence,
      bool connected) {
    if (_disposed || _retiredSessions.contains(session)) return;
    if (session == _session && observedSequence < _observedSequence) {
      if (!connected && _connected) {
        _connected = false;
        notifyListeners();
      }
      return;
    }
    _connected = connected;
    if (!_sessionPattern.hasMatch(session) || observedSequence <= 0 ||
        observedSequence > 0x7fffffffffffffff) {
      _valid = false;
      // A read-only initial snapshot can still explain native availability.
      if (_session.isEmpty) _view = view;
      notifyListeners();
      return;
    }
    if (session != _session) {
      if (_session.isNotEmpty) _retiredSessions.add(_session);
      _pending?.timer?.cancel();
      _pending = null;
      _session = session;
      _observedSequence = 0;
      _nextId = 1;
      _state = null;
      _code = null;
      _detail = null;
      _error = null;
    }
    _valid = true;
    if (observedSequence == _observedSequence) {
      notifyListeners();
      return;
    }
    _observedSequence = observedSequence;
    _view = view;
    final outcome = view.command;
    final operation = _pending;
    // Never reuse a native ticket already visible before this client sent work.
    // An inherited/re-delivered outcome is not a result for a new operation.
    if (operation == null && outcome != null && outcome.id >= _nextId) {
      _nextId = outcome.id + 1;
    }
    if (outcome != null && operation != null &&
        observedSequence > operation.observedSequence &&
        outcome.id == operation.id && outcome.key == operation.key &&
        outcome.value == operation.value) {
      if (outcome.state == 'accepted') {
        // Acceptance cannot undo uncertainty about delivery/completion.
        if (_state != 'uncertain') {
          _state = outcome.state;
          _code = outcome.code;
          _detail = outcome.detail;
        }
      } else {
        operation.timer?.cancel();
        _pending = null;
        _state = outcome.state;
        _code = outcome.code;
        _detail = outcome.detail;
        _error = null;
      }
    }
    notifyListeners();
  }

  /// Compare against the latest native field, never an optimistic UI selection.
  /// Delivery failure/timeout retains the ticket; there is no automatic retry.
  Future<void> request(PreferenceKey key, Object value) async {
    if (_disposed) return;
    final blocked = disabledReasonFor(key);
    if (blocked != null || !isPreferenceValue(key, value)) {
      _error = blocked ?? 'invalidValue';
      notifyListeners();
      return;
    }
    final expected = _view!.values!.valueFor(key);
    if (expected == value) return;
    final operation = _PendingPreference(
        _nextId++, key, value, _observedSequence);
    _pending = operation;
    _state = 'pending';
    _code = null;
    _detail = null;
    _error = null;
    final envelope = <String, dynamic>{
      'type': 'preferences_command',
      'protocol_version': protocolVersion,
      'request': <String, dynamic>{
        'session': _session,
        'id': operation.id,
        'action': 'set_preference',
        'observed_sequence': _observedSequence,
        'key': key.wireName,
        'value': value,
        'expected_value': expected,
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

class _PendingPreference {
  _PendingPreference(this.id, this.key, this.value, this.observedSequence);
  final int id;
  final PreferenceKey key;
  final Object value;
  final int observedSequence;
  Timer? timer;
}

/// The caller owns the client; leaving this panel never cancels or resends work.
class PreferencesPanel extends StatelessWidget {
  const PreferencesPanel({super.key, required this.client, required this.locale});
  final PreferenceCommandClient client;
  final String locale;

  @override
  Widget build(BuildContext context) {
    final labels = _PreferenceLabels(locale);
    final theme = Theme.of(context);
    return AnimatedBuilder(animation: client, builder: (context, _) {
      final view = client.view;
      final values = view?.values;
      final gate = client.disabledReasonFor(PreferenceKey.historyEnabled);
      Widget note(String text, {Key? key}) => Padding(
        key: key,
        padding: const EdgeInsets.only(top: 6),
        child: Text(text, style: theme.textTheme.bodySmall),
      );
      Widget group(String title, List<Widget> children) => Card.outlined(
        margin: const EdgeInsets.only(bottom: 10),
        child: Padding(padding: const EdgeInsets.all(12), child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [Text(title, style: theme.textTheme.titleSmall), ...children],
        )),
      );
      Widget toggle(PreferenceKey key, String title, bool value) => SwitchListTile(
        contentPadding: EdgeInsets.zero,
        dense: true,
        title: Text(labels[title]),
        value: value,
        onChanged: client.canChange(key)
            ? (value) => client.request(key, value) : null,
      );
      return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (gate != null) note(labels[gate]),
        if (view?.reason case final String reason when reason.isNotEmpty)
          note(reason),
        if (view != null && values != null) ...[
          group(labels['capture'], [
            toggle(PreferenceKey.notificationsEnabled, 'notifications',
                values.notificationsEnabled),
            if (!view.effects.captureNotificationsSupported)
              note(labels['notificationsUnsupported'],
                  key: const ValueKey('preferences.notificationsUnsupported')),
            toggle(PreferenceKey.soundEnabled, 'sound', values.soundEnabled),
            if (!view.effects.captureSoundsSupported)
              note(labels['soundsUnsupported'],
                  key: const ValueKey('preferences.soundsUnsupported')),
            note(labels['toastsSeparate']),
          ]),
          group(labels['history'], [
            toggle(PreferenceKey.historyEnabled, 'recordHistory',
                values.historyEnabled),
            note(labels['historyScope']),
            note(labels[!view.effects.historyRecordingSupported
                ? 'historyUnsupported'
                : view.effects.historyRecordingActive
                    ? 'historyActive' : 'historyInactive'],
                key: const ValueKey('preferences.historyStatus')),
          ]),
          group(labels['assessment'], [
            note(labels['assessmentScope']),
            const SizedBox(height: 10),
            InputDecorator(
              decoration: InputDecoration(
                labelText: labels['savedProfile'],
                border: const OutlineInputBorder(),
                isDense: true,
                enabled: client.canChange(PreferenceKey.profile),
              ),
              child: DropdownButtonHideUnderline(child: DropdownButton<String>(
                value: values.profile,
                isExpanded: true,
                isDense: true,
                items: [for (final profile in assessmentProfiles)
                  DropdownMenuItem(value: profile, child: Text(labels[profile]))],
                onChanged: client.canChange(PreferenceKey.profile)
                    ? (value) {
                        if (value != null) {
                          client.request(PreferenceKey.profile, value);
                        }
                      } : null,
              )),
            ),
            note('${labels['appliedProfile']}: '
                '${view.appliedProfile == null ? labels['unknown'] : labels[view.appliedProfile!]}'),
            if (client.disabledReasonFor(PreferenceKey.profile) == 'profileReadOnly')
              note(labels['profileReadOnly']),
          ]),
        ],
        if (client.state != null) Padding(
          padding: const EdgeInsets.symmetric(vertical: 6),
          child: Semantics(liveRegion: true, child: Text(
            labels[client.state!],
            style: theme.textTheme.bodySmall?.copyWith(color:
              const ['failed', 'rejected', 'uncertain'].contains(client.state)
                  ? theme.colorScheme.error : theme.colorScheme.primary),
          )),
        ),
        if (client.detail case final String detail when detail.isNotEmpty)
          note(detail),
        if (client.error != null) note(labels[client.error!]),
      ]);
    });
  }
}

class _PreferenceLabels {
  _PreferenceLabels(String locale)
      : _index = const ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru'].indexOf(locale);
  final int _index;
  String operator [](String key) => _labels[key]![_index < 0 ? 0 : _index];
}

const _labels = <String, List<String>>{
  'capture': ['Capture alerts', 'Alertes de capture', 'Aufnahmehinweise', 'Avisos de captura', 'キャプチャ通知', '采集提醒', 'Оповещения о захвате'],
  'notifications': ['Capture notifications', 'Notifications de capture', 'Aufnahmebenachrichtigungen', 'Notificaciones de captura', 'キャプチャの通知', '采集通知', 'Уведомления о захвате'],
  'sound': ['Capture sounds', 'Sons de capture', 'Aufnahmeklänge', 'Sonidos de captura', 'キャプチャの通知音', '采集提示音', 'Звуки захвата'],
  'notificationsUnsupported': [
    'This preference can be saved, but Windows tray capture notifications are not wired in this runtime.',
    'Ce choix peut être enregistré, mais les notifications de capture de la zone Windows ne sont pas raccordées dans ce mode.',
    'Diese Einstellung kann gespeichert werden; Windows-Tray-Aufnahmebenachrichtigungen sind in dieser Laufzeit nicht angebunden.',
    'Esta preferencia se puede guardar, pero las notificaciones de captura de la bandeja de Windows no están conectadas en este entorno.',
    '設定は保存できますが、この実行環境では Windows トレイのキャプチャ通知は未接続です。',
    '可以保存此偏好，但当前运行环境尚未接入 Windows 托盘采集通知。',
    'Настройку можно сохранить, но уведомления о захвате в трее Windows в этой среде не подключены.',
  ],
  'soundsUnsupported': [
    'This preference can be saved, but Windows tray capture sound delivery is not wired in this runtime.',
    'Ce choix peut être enregistré, mais les sons de capture de la zone Windows ne sont pas raccordés dans ce mode.',
    'Diese Einstellung kann gespeichert werden; Windows-Tray-Aufnahmeklänge sind in dieser Laufzeit nicht angebunden.',
    'Esta preferencia se puede guardar, pero los sonidos de captura de la bandeja de Windows no están conectados en este entorno.',
    '設定は保存できますが、この実行環境では Windows トレイのキャプチャ通知音は未接続です。',
    '可以保存此偏好，但当前运行环境尚未接入 Windows 托盘采集提示音。',
    'Настройку можно сохранить, но звуки захвата в трее Windows в этой среде не подключены.',
  ],
  'toastsSeparate': [
    'Operation and error messages are separate from capture alerts.',
    'Les messages d’opération et d’erreur sont distincts des alertes de capture.',
    'Betriebs- und Fehlermeldungen sind von Aufnahmehinweisen unabhängig.',
    'Los mensajes de operación y error son independientes de los avisos de captura.',
    '操作やエラーのメッセージはキャプチャ通知とは別です。',
    '操作及错误提示与采集提醒相互独立。',
    'Сообщения об операциях и ошибках не зависят от оповещений о захвате.',
  ],
  'history': ['History', 'Historique', 'Verlauf', 'Historial', '履歴', '历史记录', 'История'],
  'recordHistory': ['Record future activity', 'Enregistrer l’activité future', 'Künftige Aktivitäten aufzeichnen', 'Registrar actividad futura', '今後のアクティビティを記録', '记录今后的活动', 'Записывать будущую активность'],
  'historyScope': [
    'Controls future history recording only. Does not clear, delete, or change retention of existing history.',
    'Concerne seulement les futurs enregistrements. Ne supprime pas l’historique existant et ne modifie pas sa conservation.',
    'Gilt nur für künftige Aufzeichnungen. Vorhandener Verlauf wird weder gelöscht noch seine Aufbewahrung geändert.',
    'Solo controla los registros futuros. No borra el historial existente ni cambia su retención.',
    '今後の履歴記録のみを設定します。既存の履歴の消去・削除・保存期間変更は行いません。',
    '仅控制今后的历史记录，不会清空、删除现有历史或更改其保留期限。',
    'Управляет только будущими записями. Не очищает и не удаляет историю и не меняет срок её хранения.',
  ],
  'historyUnsupported': ['History recording is not available in this runtime.', 'L’enregistrement de l’historique est indisponible dans ce mode.', 'Verlaufsaufzeichnung ist in dieser Laufzeit nicht verfügbar.', 'El registro del historial no está disponible en este entorno.', 'この実行環境では履歴を記録できません。', '当前运行环境不支持历史记录。', 'Запись истории в этой среде недоступна.'],
  'historyActive': ['Native history recording is active.', 'L’enregistrement natif est actif.', 'Native Verlaufsaufzeichnung ist aktiv.', 'El registro nativo del historial está activo.', 'ネイティブの履歴記録は動作中です。', '原生历史记录正在运行。', 'Нативная запись истории активна.'],
  'historyInactive': ['Native history recording is inactive.', 'L’enregistrement natif est inactif.', 'Native Verlaufsaufzeichnung ist inaktiv.', 'El registro nativo del historial está inactivo.', 'ネイティブの履歴記録は停止中です。', '原生历史记录未运行。', 'Нативная запись истории неактивна.'],
  'assessment': ['Assessment', 'Évaluation', 'Bewertung', 'Evaluación', '評価', '评估', 'Оценка'],
  'assessmentScope': [
    'Profiles guide assessment, not permissions or blocking. Saved selection and the policy actually applied by the collector are separate.',
    'Les profils guident l’évaluation, pas les autorisations ni le blocage. Le choix enregistré est distinct de la politique appliquée par le collecteur.',
    'Profile steuern die Bewertung, nicht Berechtigungen oder Sperren. Gespeicherte Auswahl und tatsächlich angewandte Erfassungsrichtlinie sind getrennt.',
    'Los perfiles orientan la evaluación, no los permisos ni el bloqueo. La selección guardada es independiente de la política aplicada por el recopilador.',
    'プロファイルは評価の指針であり、権限やブロックの設定ではありません。保存した選択とコレクターが実際に適用したポリシーは別です。',
    '配置用于指导评估，而非权限控制或阻止采集。保存的选择与采集器实际应用的策略相互独立。',
    'Профили задают оценку, а не разрешения или блокировку. Сохранённый выбор и фактически применённая сборщиком политика различаются.',
  ],
  'savedProfile': ['Saved profile', 'Profil enregistré', 'Gespeichertes Profil', 'Perfil guardado', '保存済みプロファイル', '已保存的配置', 'Сохранённый профиль'],
  'appliedProfile': ['Applied profile', 'Profil appliqué', 'Angewandtes Profil', 'Perfil aplicado', '適用中のプロファイル', '已应用的配置', 'Применённый профиль'],
  'unknown': ['Unknown', 'Inconnu', 'Unbekannt', 'Desconocido', '不明', '未知', 'Неизвестно'],
  'private': ['Private', 'Privé', 'Privat', 'Privado', 'プライベート', '私密', 'Приватный'],
  'meeting': ['Meeting', 'Réunion', 'Besprechung', 'Reunión', '会議', '会议', 'Встреча'],
  'development': ['Development', 'Développement', 'Entwicklung', 'Desarrollo', '開発', '开发', 'Разработка'],
  'balanced': ['Balanced', 'Équilibré', 'Ausgewogen', 'Equilibrado', 'バランス', '均衡', 'Сбалансированный'],
  'pending': ['Sending preference…', 'Envoi du choix…', 'Einstellung wird gesendet…', 'Enviando preferencia…', '設定を送信中…', '正在发送偏好…', 'Отправка настройки…'],
  'accepted': ['Accepted by native; saving is not yet confirmed.', 'Accepté par le moteur ; l’enregistrement n’est pas encore confirmé.', 'Nativ angenommen; Speicherung noch nicht bestätigt.', 'Aceptado por el motor; el guardado aún no está confirmado.', 'ネイティブ側で受理されました。保存はまだ確認されていません。', '原生端已接受，但尚未确认保存。', 'Принято нативным модулем; сохранение ещё не подтверждено.'],
  'succeeded': ['Saved by native. Applied behavior is reported separately.', 'Enregistré par le moteur. L’application effective est indiquée séparément.', 'Nativ gespeichert. Tatsächliche Anwendung wird separat angezeigt.', 'Guardado por el motor. El comportamiento aplicado se indica por separado.', 'ネイティブ側で保存されました。実際の適用状況は別途表示されます。', '原生端已保存。实际生效情况另行显示。', 'Сохранено нативным модулем. Применение отображается отдельно.'],
  'failed': ['Saving failed. Native values remain authoritative.', 'Échec de l’enregistrement. Les valeurs natives font foi.', 'Speichern fehlgeschlagen. Native Werte bleiben maßgeblich.', 'Error al guardar. Los valores nativos siguen siendo la referencia.', '保存に失敗しました。ネイティブ側の値が正となります。', '保存失败，以原生值为准。', 'Не удалось сохранить. Нативные значения остаются источником истины.'],
  'rejected': ['Change rejected. Review the latest native values.', 'Modification refusée. Vérifiez les dernières valeurs natives.', 'Änderung abgelehnt. Aktuelle native Werte prüfen.', 'Cambio rechazado. Revise los últimos valores nativos.', '変更は拒否されました。最新のネイティブ値を確認してください。', '更改被拒绝，请查看最新原生值。', 'Изменение отклонено. Проверьте последние нативные значения.'],
  'uncertain': ['Completion is uncertain. No automatic retry; wait for native confirmation or a new session.', 'Résultat incertain. Aucun nouvel essai automatique ; attendez la confirmation native ou une nouvelle session.', 'Abschluss ungewiss. Kein automatischer Wiederholungsversuch; native Bestätigung oder neue Sitzung abwarten.', 'Finalización incierta. Sin reintento automático; espere confirmación nativa o una nueva sesión.', '完了状態は不明です。自動再試行は行いません。ネイティブ側の確認または新しいセッションを待ってください。', '完成情况不确定。不会自动重试，请等待原生确认或新会话。', 'Завершение не подтверждено. Автоповтора нет; дождитесь нативного подтверждения или новой сессии.'],
  'unavailable': ['Preferences are not available yet.', 'Les préférences ne sont pas encore disponibles.', 'Einstellungen sind noch nicht verfügbar.', 'Las preferencias aún no están disponibles.', '設定はまだ利用できません。', '偏好设置尚不可用。', 'Настройки пока недоступны.'],
  'disconnected': ['Native connection is offline. Changes are disabled.', 'Connexion native hors ligne. Modifications désactivées.', 'Native Verbindung getrennt. Änderungen deaktiviert.', 'Conexión nativa desconectada. Cambios deshabilitados.', 'ネイティブ接続がありません。変更できません。', '原生连接已断开，无法更改。', 'Нативное соединение отсутствует. Изменения отключены.'],
  'busy': ['A preference change is pending.', 'Une modification est en cours.', 'Eine Einstellungsänderung steht aus.', 'Hay un cambio de preferencia pendiente.', '設定の変更を処理中です。', '偏好更改正在等待处理。', 'Изменение настройки ожидает завершения.'],
  'readOnly': ['This native session is read-only.', 'Cette session native est en lecture seule.', 'Diese native Sitzung ist schreibgeschützt.', 'Esta sesión nativa es de solo lectura.', 'このネイティブセッションは読み取り専用です。', '此原生会话为只读。', 'Эта нативная сессия доступна только для чтения.'],
  'profileReadOnly': ['The native configuration fixes the assessment profile; it cannot be changed here.', 'La configuration native fixe le profil d’évaluation ; il ne peut pas être modifié ici.', 'Die native Konfiguration legt das Bewertungsprofil fest; hier nicht änderbar.', 'La configuración nativa fija el perfil de evaluación; no se puede cambiar aquí.', '評価プロファイルはネイティブ構成で固定されているため、ここでは変更できません。', '原生配置已固定评估配置，无法在此更改。', 'Профиль оценки задан нативной конфигурацией; здесь его изменить нельзя.'],
  'exhausted': ['Command IDs exhausted; a new native session is required.', 'Identifiants épuisés ; une nouvelle session native est requise.', 'Befehls-IDs ausgeschöpft; neue native Sitzung erforderlich.', 'Identificadores agotados; se requiere una nueva sesión nativa.', 'コマンド ID を使い切りました。新しいネイティブセッションが必要です。', '命令编号已耗尽，需要新原生会话。', 'Идентификаторы исчерпаны; нужна новая нативная сессия.'],
  'invalidValue': ['Invalid preference value. Nothing was sent.', 'Valeur invalide. Rien n’a été envoyé.', 'Ungültiger Einstellungswert. Nichts gesendet.', 'Valor de preferencia no válido. No se envió nada.', '設定値が無効です。何も送信されませんでした。', '偏好值无效，未发送任何内容。', 'Недопустимое значение. Ничего не отправлено.'],
  'sendFailed': ['Delivery is uncertain. The change will not be retried automatically.', 'Livraison incertaine. La modification ne sera pas renvoyée automatiquement.', 'Zustellung ungewiss. Die Änderung wird nicht automatisch erneut gesendet.', 'Entrega incierta. El cambio no se reintentará automáticamente.', '配信状態は不明です。変更を自動再送しません。', '传送情况不确定，不会自动重试。', 'Доставка не подтверждена. Автоматической повторной отправки не будет.'],
};
