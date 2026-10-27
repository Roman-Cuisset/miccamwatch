// Engine-selected language; collector/application/detail text remains verbatim.
class Labels {
  Labels(String locale) : index = const ['en', 'fr', 'de', 'es', 'ja', 'zh', 'ru'].indexOf(locale);
  final int index;
  String operator [](String key) {
    final values = _labels[key];
    return values == null ? key : values[index < 0 ? 0 : index];
  }
}

const _labels = <String, List<String>>{
  'overview': ['Overview', 'Vue d’ensemble', 'Übersicht', 'Resumen', '概要', '概览', 'Обзор'],
  'diagnostics': ['Diagnostics', 'Diagnostic', 'Diagnose', 'Diagnóstico', '診断', '诊断', 'Диагностика'],
  'about': ['About', 'À propos', 'Über', 'Acerca de', '情報', '关于', 'О программе'],
  'microphone': ['Microphone', 'Microphone', 'Mikrofon', 'Micrófono', 'マイク', '麦克风', 'Микрофон'],
  'camera': ['Camera', 'Caméra', 'Kamera', 'Cámara', 'カメラ', '摄像头', 'Камера'],
  'applications': ['Observed applications', 'Applications observées', 'Beobachtete Anwendungen', 'Aplicaciones observadas', '観測されたアプリ', '观测到的应用', 'Наблюдаемые приложения'],
  'readOnly': ['Read-only development session', 'Session de développement en lecture seule', 'Schreibgeschützte Entwicklungssitzung', 'Sesión de desarrollo de solo lectura', '読み取り専用の開発セッション', '只读开发会话', 'Сеанс разработки: только чтение'],
  'readOnlyNormal': ['Read-only presentation', 'Affichage en lecture seule', 'Schreibgeschützte Anzeige', 'Presentación de solo lectura', '読み取り専用表示', '只读显示', 'Представление: только чтение'],
  'connecting': ['Waiting for engine snapshot', 'En attente du moteur', 'Warten auf Engine-Daten', 'Esperando datos del motor', 'エンジンのデータを待機中', '等待引擎快照', 'Ожидание данных движка'],
  'live': ['Live observation', 'Observation en direct', 'Live-Beobachtung', 'Observación en vivo', '最新の観測', '实时观测', 'Текущее наблюдение'],
  'stale': ['Stale — connection not current', 'Périmé — connexion non actualisée', 'Veraltet — keine aktuellen Daten', 'Obsoleto — conexión no actualizada', '古いデータ — 接続を確認できません', '数据已过期 — 连接未更新', 'Данные устарели — нет обновлений'],
  'error': ['Snapshot error — observation unknown', 'Erreur de données — observation inconnue', 'Datenfehler — Beobachtung unbekannt', 'Error de datos — observación desconocida', 'データエラー — 観測不明', '快照错误 — 观测未知', 'Ошибка данных — наблюдение неизвестно'],
  'disconnected': ['Disconnected', 'Déconnecté', 'Getrennt', 'Desconectado', '切断', '已断开', 'Отключено'],
  'active': ['Active observed', 'Activité observée', 'Aktivität beobachtet', 'Actividad observada', '使用中を観測', '观测到活动', 'Активность обнаружена'],
  'ready': ['Ready observed', 'Disponibilité observée', 'Bereitschaft beobachtet', 'Preparación observada', '準備状態を観測', '观测到就绪', 'Готовность обнаружена'],
  'none_observed': ['None observed', 'Aucune activité observée', 'Nichts beobachtet', 'No se observó actividad', '観測なし', '未观测到活动', 'Активность не обнаружена'],
  'unknown': ['Unknown', 'Inconnu', 'Unbekannt', 'Desconocido', '不明', '未知', 'Неизвестно'],
  'coverage': ['Coverage', 'Couverture', 'Abdeckung', 'Cobertura', '観測範囲', '覆盖范围', 'Охват'],
  'complete': ['Complete', 'Complète', 'Vollständig', 'Completa', '完全', '完整', 'Полный'],
  'incomplete': ['Incomplete', 'Incomplète', 'Unvollständig', 'Incompleta', '不完全', '不完整', 'Неполный'],
  'unavailable': ['Unavailable', 'Indisponible', 'Nicht verfügbar', 'No disponible', '利用不可', '不可用', 'Недоступно'],
  'confidence': ['Confidence', 'Confiance', 'Konfidenz', 'Confianza', '確度', '置信度', 'Достоверность'],
  'high': ['High', 'Élevée', 'Hoch', 'Alta', '高', '高', 'Высокая'],
  'medium': ['Medium', 'Moyenne', 'Mittel', 'Media', '中', '中', 'Средняя'],
  'low': ['Low', 'Faible', 'Niedrig', 'Baja', '低', '低', 'Низкая'],
  'healthy': ['Healthy', 'Opérationnel', 'Funktionsfähig', 'Correcto', '正常', '正常', 'Исправен'],
  'degraded': ['Degraded', 'Dégradé', 'Beeinträchtigt', 'Degradado', '機能低下', '降级', 'Ограничен'],
  'lastUpdate': ['Last received', 'Dernière réception', 'Zuletzt empfangen', 'Última recepción', '最終受信', '最后接收', 'Последний приём'],
  'observedAt': ['Observed at', 'Observé à', 'Beobachtet um', 'Observado a las', '観測時刻', '观测时间', 'Время наблюдения'],
  'clock': ['Local time', 'Heure locale', 'Ortszeit', 'Hora local', '現在時刻', '本地时间', 'Местное время'],
  'noRows': ['No application observations in this snapshot. This does not prove access is denied.', 'Aucune application observée. Cela ne prouve pas un refus d’accès.', 'Keine Anwendung beobachtet. Dies belegt keine Zugriffssperre.', 'Sin aplicaciones observadas. Esto no demuestra un bloqueo de acceso.', 'アプリの観測なし。アクセス拒否を意味しません。', '此快照中没有应用观测记录，不代表访问被阻止。', 'Нет наблюдений приложений. Это не доказывает запрет доступа.'],
  'historical': ['Last snapshot only — current activity is unknown', 'Dernier instantané — activité actuelle inconnue', 'Nur letzter Stand — aktuelle Aktivität unbekannt', 'Última instantánea — actividad actual desconocida', '前回のデータ — 現在の使用状況は不明', '仅为上次快照 — 当前活动未知', 'Последние данные — текущая активность неизвестна'],
  'limits': ['Observation is not protection. Incomplete coverage may miss activity. Microphone mute does not deny capture access.', 'Observer ne protège pas. Une couverture incomplète peut manquer des activités. Couper le micro ne refuse pas l’accès.', 'Beobachtung ist kein Schutz. Unvollständige Abdeckung kann Aktivität übersehen. Stummschaltung sperrt keinen Aufnahmezugriff.', 'Observar no es proteger. La cobertura incompleta puede omitir actividad. Silenciar no bloquea el acceso de captura.', '観測は保護ではありません。不完全な範囲では見逃しがあります。ミュートは録音アクセスを拒否しません。', '观测不等于保护。覆盖不完整可能遗漏活动。麦克风静音不等于拒绝采集访问。', 'Наблюдение не является защитой. Неполный охват может пропустить активность. Отключение звука не запрещает захват.'],
  'truncated': ['Snapshot clipped to protocol limits', 'Données limitées par le protocole', 'Daten auf Protokollgrenzen gekürzt', 'Datos recortados a los límites del protocolo', 'プロトコルの上限でデータを省略', '快照已按协议上限截断', 'Данные обрезаны до лимитов протокола'],
  'collectors': ['Collectors', 'Collecteurs', 'Erfassungsmodule', 'Recolectores', '収集元', '采集器', 'Сборщики'],
  'gaps': ['Observation gaps', 'Lacunes d’observation', 'Beobachtungslücken', 'Lagunas de observación', '観測の欠落', '观测缺口', 'Пробелы наблюдения'],
  'protection': ['Passive protection readings', 'État passif de protection', 'Passive Schutzabfrage', 'Lecturas pasivas de protección', '保護状態の受動読み取り', '被动保护状态', 'Пассивные данные защиты'],
  'requested': ['Protection requested', 'Protection demandée', 'Schutz angefordert', 'Protección solicitada', '保護要求', '已请求保护', 'Защита запрошена'],
  'guard': ['Protection guard', 'Garde de protection', 'Schutzdienst', 'Servicio de protección', '保護サービス', '保护服务', 'Служба защиты'],
  'yes': ['Yes', 'Oui', 'Ja', 'Sí', 'はい', '是', 'Да'],
  'no': ['No', 'Non', 'Nein', 'No', 'いいえ', '否', 'Нет'],
  'guardMissing': ['Requested guard is not active', 'Protection demandée, garde inactive', 'Angeforderter Schutzdienst ist nicht aktiv', 'Protección solicitada, servicio inactivo', '要求された保護サービスは停止中', '已请求保护，但服务未运行', 'Защита запрошена, служба не активна'],
  'muted': ['SDK mute observed', 'Sourdine SDK observée', 'SDK-Stummschaltung beobachtet', 'Silencio SDK observado', 'SDKミュートを観測', '观测到SDK静音', 'Обнаружено отключение звука SDK'],
  'unmuted': ['No SDK mute observed', 'Pas de sourdine SDK observée', 'Keine SDK-Stummschaltung beobachtet', 'Sin silencio SDK observado', 'SDKミュートなしを観測', '未观测到SDK静音', 'Отключение звука SDK не обнаружено'],
  'mixed': ['Mixed SDK mute state', 'Sourdine SDK mixte', 'Gemischte SDK-Stummschaltung', 'Estado de silencio SDK mixto', 'SDKミュート状態が混在', 'SDK静音状态混合', 'Смешанное состояние звука SDK'],
  'allowed': ['No device block observed', 'Pas de blocage périphérique observé', 'Keine Gerätesperre beobachtet', 'Sin bloqueo de dispositivo observado', 'デバイスのブロックなしを観測', '未观测到设备阻止', 'Блокировка устройства не обнаружена'],
  'blocked': ['Device block observed', 'Blocage périphérique observé', 'Gerätesperre beobachtet', 'Bloqueo de dispositivo observado', 'デバイスのブロックを観測', '观测到设备阻止', 'Обнаружена блокировка устройства'],
  'system_managed': ['Mixed / system-managed device state', 'État mixte / géré par le système', 'Gemischter / systemverwalteter Zustand', 'Estado mixto / gestionado por el sistema', '混在 / システム管理状態', '混合或系统管理的设备状态', 'Смешанное / системное состояние'],
  'noEvidence': ['No evidence supplied', 'Aucune donnée fournie', 'Keine Nachweise geliefert', 'No se aportaron datos', '根拠データなし', '未提供证据', 'Нет данных'],
  'light': ['Light', 'Clair', 'Hell', 'Claro', 'ライト', '浅色', 'Светлая'],
  'dark': ['Dark', 'Sombre', 'Dunkel', 'Oscuro', 'ダーク', '深色', 'Тёмная'],
};
