use crate::model::{Action, Activity, Confidence, Resource, Risk};
use colored::{ColoredString, Colorize};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Fr,
    De,
    Es,
    Ja,
    Zh,
    Ru,
}

impl Language {
    pub fn from_code(code: &str) -> Option<Self> {
        let trimmed = code.trim().to_ascii_lowercase();
        match trimmed.as_str() {
            "en" | "en-us" | "en-gb" | "english" => Some(Self::En),
            "fr" | "fr-fr" | "fr-ca" | "french" | "francais" | "français" => Some(Self::Fr),
            "de" | "de-de" | "de-at" | "german" | "deutsch" => Some(Self::De),
            "es" | "es-es" | "es-mx" | "spanish" | "espanol" | "español" => Some(Self::Es),
            "ja" | "ja-jp" | "japanese" | "nihongo" => Some(Self::Ja),
            "zh" | "zh-cn" | "zh-hans" | "chinese" => Some(Self::Zh),
            "ru" | "ru-ru" | "russian" | "russkiy" => Some(Self::Ru),
            _ => None,
        }
    }

    pub fn detect() -> Self {
        #[cfg(windows)]
        {
            let langid = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
            let primary = langid & 0x03ff;
            match primary {
                0x0c => Self::Fr,
                0x07 => Self::De,
                0x0a => Self::Es,
                0x11 => Self::Ja,
                0x04 => Self::Zh,
                0x19 => Self::Ru,
                _ => Self::En,
            }
        }
        #[cfg(not(windows))]
        {
            Self::En
        }
    }

    #[allow(dead_code)]
    pub fn code(&self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Es => "es",
            Self::Ja => "ja",
            Self::Zh => "zh",
            Self::Ru => "ru",
        }
    }

    pub fn resource(&self, resource: Resource) -> &'static str {
        match (self, resource) {
            (Self::Ja, Resource::Microphone) => "MIC",
            (Self::Ja, Resource::Camera) => "CAM",
            (Self::Zh, Resource::Microphone) => "MIC",
            (Self::Zh, Resource::Camera) => "CAM",
            (Self::Ru, Resource::Microphone) => "МИК",
            (Self::Ru, Resource::Camera) => "КАМ",
            (Self::De, Resource::Microphone) => "MIK",
            (Self::De, Resource::Camera) => "KAM",
            (Self::Es, Resource::Camera) => "CÁM",
            _ => match resource {
                Resource::Microphone => "MIC",
                Resource::Camera => "CAM",
            },
        }
    }

    pub fn state_str(&self, action: Option<Action>, activity: Activity) -> &'static str {
        match (self, action) {
            (_, Some(Action::Start)) => match self {
                Self::Fr => "DÉBUT",
                Self::De => "START",
                Self::Es => "INICIO",
                Self::Ja => "開始",
                Self::Zh => "启动",
                Self::Ru => "СТАРТ",
                Self::En => "START",
            },
            (_, Some(Action::Update)) => match self {
                Self::Fr => "MIS À JOUR",
                Self::De => "AKTUALISIERT",
                Self::Es => "ACTUALIZADO",
                Self::Ja => "更新",
                Self::Zh => "已更新",
                Self::Ru => "ОБНОВЛЕНО",
                Self::En => "UPDATED",
            },
            (_, Some(Action::Stop)) => match self {
                Self::Fr => "ARRÊTÉ",
                Self::De => "GESTOPPT",
                Self::Es => "DETENIDO",
                Self::Ja => "停止",
                Self::Zh => "已停止",
                Self::Ru => "ОСТАНОВЛЕНО",
                Self::En => "STOPPED",
            },
            (_, None) => match (self, activity) {
                (Self::Fr, Activity::Active) => "ACTIF",
                (Self::Fr, Activity::Ready) => "PRÊT",
                (Self::De, Activity::Active) => "AKTIV",
                (Self::De, Activity::Ready) => "BEREIT",
                (Self::Es, Activity::Active) => "ACTIVO",
                (Self::Es, Activity::Ready) => "LISTO",
                (Self::Ja, Activity::Active) => "アクティブ",
                (Self::Ja, Activity::Ready) => "待機中",
                (Self::Zh, Activity::Active) => "活动",
                (Self::Zh, Activity::Ready) => "就绪",
                (Self::Ru, Activity::Active) => "АКТИВНО",
                (Self::Ru, Activity::Ready) => "ГОТОВ",
                (Self::En, Activity::Active) => "ACTIVE",
                (Self::En, Activity::Ready) => "READY",
            },
        }
    }

    pub fn risk_str(&self, risk: Risk) -> &'static str {
        match (self, risk) {
            (Self::Fr, Risk::Expected) => "ATTENDU",
            (Self::Fr, Risk::Unexplained) => "INEXPLIQUÉ",
            (Self::Fr, Risk::Suspicious) => "SUSPECT",
            (Self::Fr, Risk::Blocked) => "BLOQUÉ",

            (Self::De, Risk::Expected) => "ERWARTET",
            (Self::De, Risk::Unexplained) => "UNERKLÄRT",
            (Self::De, Risk::Suspicious) => "VERDÄCHTIG",
            (Self::De, Risk::Blocked) => "BLOCKIERT",

            (Self::Es, Risk::Expected) => "ESPERADO",
            (Self::Es, Risk::Unexplained) => "INEXPLICADO",
            (Self::Es, Risk::Suspicious) => "SOSPECHOSO",
            (Self::Es, Risk::Blocked) => "BLOQUEADO",

            (Self::Ja, Risk::Expected) => "正常",
            (Self::Ja, Risk::Unexplained) => "説明不能",
            (Self::Ja, Risk::Suspicious) => "不審",
            (Self::Ja, Risk::Blocked) => "ブロック",

            (Self::Zh, Risk::Expected) => "符合预期",
            (Self::Zh, Risk::Unexplained) => "原因不明",
            (Self::Zh, Risk::Suspicious) => "可疑",
            (Self::Zh, Risk::Blocked) => "已阻止",

            (Self::Ru, Risk::Expected) => "ОЖИДАЕМО",
            (Self::Ru, Risk::Unexplained) => "НЕОБЪЯСНИМО",
            (Self::Ru, Risk::Suspicious) => "ПОДОЗРИТЕЛЬНО",
            (Self::Ru, Risk::Blocked) => "ЗАБЛОКИРОВАНО",

            (Self::En, Risk::Expected) => "EXPECTED",
            (Self::En, Risk::Unexplained) => "UNEXPLAINED",
            (Self::En, Risk::Suspicious) => "SUSPICIOUS",
            (Self::En, Risk::Blocked) => "BLOCKED",
        }
    }

    pub fn confidence_badge(&self, confidence: Confidence) -> ColoredString {
        match (self, confidence) {
            (Self::Fr, Confidence::High) => "[confiance: haute]".green(),
            (Self::Fr, Confidence::Medium) => "[confiance: moyenne]".yellow(),
            (Self::Fr, Confidence::Low) => "[confiance: basse]".dimmed(),

            (Self::De, Confidence::High) => "[Vertrauen: hoch]".green(),
            (Self::De, Confidence::Medium) => "[Vertrauen: mittel]".yellow(),
            (Self::De, Confidence::Low) => "[Vertrauen: niedrig]".dimmed(),

            (Self::Es, Confidence::High) => "[confianza: alta]".green(),
            (Self::Es, Confidence::Medium) => "[confianza: media]".yellow(),
            (Self::Es, Confidence::Low) => "[confianza: baja]".dimmed(),

            (Self::Ja, Confidence::High) => "[信頼度: 高]".green(),
            (Self::Ja, Confidence::Medium) => "[信頼度: 中]".yellow(),
            (Self::Ja, Confidence::Low) => "[信頼度: 低]".dimmed(),

            (Self::Zh, Confidence::High) => "[可信度: 高]".green(),
            (Self::Zh, Confidence::Medium) => "[可信度: 中]".yellow(),
            (Self::Zh, Confidence::Low) => "[可信度: 低]".dimmed(),

            (Self::Ru, Confidence::High) => "[достоверность: высокая]".green(),
            (Self::Ru, Confidence::Medium) => "[достоверность: средняя]".yellow(),
            (Self::Ru, Confidence::Low) => "[достоверность: низкая]".dimmed(),

            (Self::En, Confidence::High) => "[confidence: high]".green(),
            (Self::En, Confidence::Medium) => "[confidence: medium]".yellow(),
            (Self::En, Confidence::Low) => "[confidence: low]".dimmed(),
        }
    }

    pub fn no_activity(&self) -> &'static str {
        match self {
            Self::Fr => "✔ Aucun accès micro ou caméra détecté.",
            Self::De => "✔ Kein Mikrofon- oder Kamerazugriff erkannt.",
            Self::Es => "✔ No se detectó acceso al micrófono o a la cámara.",
            Self::Ja => "✔ マイクまたはカメラへのアクセスは検出されませんでした。",
            Self::Zh => "✔ 未检测到麦克风或摄像头访问。",
            Self::Ru => "✔ Доступ к микрофону или камере не обнаружен.",
            Self::En => "✔ No microphone or camera activity detected.",
        }
    }

    pub fn no_matching_activity(&self) -> &'static str {
        match self {
            Self::Fr => "Aucun accès ne correspond au filtre demandé.",
            Self::De => "Kein Zugriff entspricht dem angeforderten Filter.",
            Self::Es => "Ningún acceso coincide con el filtro solicitado.",
            Self::Ja => "指定されたフィルターに一致するアクセスはありません。",
            Self::Zh => "没有访问符合请求的筛选条件。",
            Self::Ru => "Нет доступов, соответствующих заданному фильтру.",
            Self::En => "No access matches the requested filter.",
        }
    }

    pub fn microphone_status(&self, state: crate::model::MicrophoneMuteState) -> &'static str {
        use crate::model::MicrophoneMuteState::{Mixed, Muted, Unavailable, Unmuted};
        match (self, state) {
            (Self::Fr, Unavailable) => "Aucun périphérique microphone actif.",
            (Self::Fr, Muted) => "Le microphone est COUPÉ.",
            (Self::Fr, Unmuted) => "Le microphone est ACTIVÉ.",
            (Self::Fr, Mixed) => "Les microphones ont des états de sourdine MIXTES.",
            (Self::De, Unavailable) => "Kein aktives Mikrofon gefunden.",
            (Self::De, Muted) => "Das Mikrofon ist STUMMGESCHALTET.",
            (Self::De, Unmuted) => "Das Mikrofon ist AKTIVIERT.",
            (Self::De, Mixed) => "Die Mikrofone haben GEMISCHTE Stummschaltzustände.",
            (Self::Es, Unavailable) => "No se encontró ningún micrófono activo.",
            (Self::Es, Muted) => "El micrófono está SILENCIADO.",
            (Self::Es, Unmuted) => "El micrófono está ACTIVADO.",
            (Self::Es, Mixed) => "Los micrófonos tienen estados de silencio MIXTOS.",
            (Self::Ja, Unavailable) => "有効なマイクデバイスがありません。",
            (Self::Ja, Muted) => "マイクはミュートされています。",
            (Self::Ja, Unmuted) => "マイクは有効です。",
            (Self::Ja, Mixed) => "マイクのミュート状態が混在しています。",
            (Self::Zh, Unavailable) => "未找到活动麦克风设备。",
            (Self::Zh, Muted) => "麦克风已静音。",
            (Self::Zh, Unmuted) => "麦克风已启用。",
            (Self::Zh, Mixed) => "麦克风的静音状态不一致。",
            (Self::Ru, Unavailable) => "Активные микрофоны не найдены.",
            (Self::Ru, Muted) => "Микрофон ЗАГЛУШЕН.",
            (Self::Ru, Unmuted) => "Микрофон ВКЛЮЧЕН.",
            (Self::Ru, Mixed) => "Микрофоны имеют СМЕШАННЫЕ состояния.",
            (Self::En, Unavailable) => "No active microphone capture device found.",
            (Self::En, Muted) => "Microphone is MUTED.",
            (Self::En, Unmuted) => "Microphone is UNMUTED.",
            (Self::En, Mixed) => "Microphone devices have MIXED mute states.",
        }
    }

    pub fn device_unavailable(&self) -> &'static str {
        match self {
            Self::Fr => "périphérique non disponible",
            Self::De => "Gerät nicht verfügbar",
            Self::Es => "dispositivo no disponible",
            Self::Ja => "デバイス利用不可",
            Self::Zh => "设备不可用",
            Self::Ru => "устройство недоступно",
            Self::En => "device unavailable",
        }
    }

    pub fn no_devices_found(&self) -> &'static str {
        match self {
            Self::Fr => "Aucun périphérique micro ou caméra détecté.",
            Self::De => "Kein Mikrofon- oder Kameragerät gefunden.",
            Self::Es => "No se encontró ningún micrófono o cámara.",
            Self::Ja => "マイクまたはカメラが見つかりません。",
            Self::Zh => "未找到麦克风或摄像头设备。",
            Self::Ru => "Устройства микрофона или камеры не найдены.",
            Self::En => "No microphone or camera device found.",
        }
    }

    pub fn parent_label(&self) -> &'static str {
        match self {
            Self::Fr => "parent:",
            Self::De => "Übergeordnet:",
            Self::Es => "padre:",
            Self::Ja => "親プロセス:",
            Self::Zh => "父进程:",
            Self::Ru => "родитель:",
            Self::En => "parent:",
        }
    }

    pub fn session_label(&self) -> &'static str {
        match self {
            Self::Fr => "session:",
            Self::De => "Sitzung:",
            Self::Es => "sesión:",
            Self::Ja => "セッション:",
            Self::Zh => "会话:",
            Self::Ru => "сессия:",
            Self::En => "session:",
        }
    }

    pub fn ancestry_label(&self) -> &'static str {
        match self {
            Self::Fr => "ascendance:",
            Self::De => "Abstammung:",
            Self::Es => "ascendencia:",
            Self::Ja => "プロセスツリー:",
            Self::Zh => "进程链:",
            Self::Ru => "предки:",
            Self::En => "ancestry:",
        }
    }

    pub fn signer_label(&self) -> &'static str {
        match self {
            Self::Fr => "signataire:",
            Self::De => "Unterzeichner:",
            Self::Es => "firmante:",
            Self::Ja => "署名者:",
            Self::Zh => "签名者:",
            Self::Ru => "издатель:",
            Self::En => "signer:",
        }
    }

    pub fn signature_label(&self) -> &'static str {
        match self {
            Self::Fr => "signature:",
            Self::De => "Signatur:",
            Self::Es => "firma:",
            Self::Ja => "署名:",
            Self::Zh => "签名:",
            Self::Ru => "подпись:",
            Self::En => "signature:",
        }
    }

    pub fn verified_badge(&self) -> &'static str {
        match self {
            Self::Fr => "[vérifié]",
            Self::De => "[verifiziert]",
            Self::Es => "[verificado]",
            Self::Ja => "[検証済]",
            Self::Zh => "[已验证]",
            Self::Ru => "[проверено]",
            Self::En => "[verified]",
        }
    }

    pub fn unverified_badge(&self) -> &'static str {
        match self {
            Self::Fr => "[non vérifié]",
            Self::De => "[nicht verifiziert]",
            Self::Es => "[no verificado]",
            Self::Ja => "[未検証]",
            Self::Zh => "[未验证]",
            Self::Ru => "[не проверено]",
            Self::En => "[unverified]",
        }
    }

    pub fn signer_unavailable(&self) -> &'static str {
        match self {
            Self::Fr => "certificat approuvé; signataire indisponible",
            Self::De => "vertrauenswürdiges Zertifikat; Unterzeichner nicht verfügbar",
            Self::Es => "certificado de confianza; firmante no disponible",
            Self::Ja => "信頼された証明書; 署名者情報なし",
            Self::Zh => "受信任的证书; 签名者不可用",
            Self::Ru => "доверенный сертификат; издатель недоступен",
            Self::En => "trusted certificate; signer unavailable",
        }
    }

    pub fn modules_label(&self) -> &'static str {
        match self {
            Self::Fr => "modules:",
            Self::De => "Module:",
            Self::Es => "módulos:",
            Self::Ja => "モジュール:",
            Self::Zh => "模块:",
            Self::Ru => "модули:",
            Self::En => "modules:",
        }
    }

    pub fn evidence_label(&self) -> &'static str {
        match self {
            Self::Fr => "preuve:",
            Self::De => "Beweis:",
            Self::Es => "prueba:",
            Self::Ja => "根拠:",
            Self::Zh => "证据:",
            Self::Ru => "свидетельство:",
            Self::En => "evidence:",
        }
    }

    pub fn via_label(&self) -> &'static str {
        match self {
            Self::Fr => "via",
            Self::De => "via",
            Self::Es => "vía",
            Self::Ja => "経由",
            Self::Zh => "来源",
            Self::Ru => "через",
            Self::En => "via",
        }
    }

    pub fn recommended_warning(&self) -> &'static str {
        match self {
            Self::Fr => "⚠ recommandé : inspectez le processus et l'exécutable avant toute action.",
            Self::De => "⚠ Empfohlen: Untersuchen Sie Prozess und Datei vor weiteren Aktionen.",
            Self::Es => "⚠ recomendado: inspeccione el proceso y ejecutable antes de actuar.",
            Self::Ja => "⚠ 推奨: 処置を行う前に対象プロセスと実行ファイルを確認してください。",
            Self::Zh => "⚠ 建议: 在采取行动前先检查该进程和可执行文件。",
            Self::Ru => "⚠ Рекомендуется проверить процесс и исполняемый файл перед действием.",
            Self::En => "⚠ recommended: inspect the process and executable before taking action.",
        }
    }

    pub fn doctor_status(&self, status: crate::model::DiagnosticStatus) -> ColoredString {
        match status {
            crate::model::DiagnosticStatus::Ok => "[  OK]".bold().green(),
            crate::model::DiagnosticStatus::Warning => match self {
                Self::Fr => "[ATTN]".bold().yellow(),
                Self::De => "[WARN]".bold().yellow(),
                Self::Es => "[AVIS]".bold().yellow(),
                Self::Ja => "[警告]".bold().yellow(),
                Self::Zh => "[警告]".bold().yellow(),
                Self::Ru => "[ВНИМ]".bold().yellow(),
                Self::En => "[WARN]".bold().yellow(),
            },
            crate::model::DiagnosticStatus::Error => match self {
                Self::Fr => "[ERR ]".bold().red(),
                Self::De => "[FEHL]".bold().red(),
                Self::Es => "[ERR ]".bold().red(),
                Self::Ja => "[異常]".bold().red(),
                Self::Zh => "[错误]".bold().red(),
                Self::Ru => "[ОШИБ]".bold().red(),
                Self::En => "[ ERR]".bold().red(),
            },
        }
    }

    pub fn toast_title(&self, action: Action, resource: Resource) -> String {
        let res = match (self, resource) {
            (Self::Fr, Resource::Microphone) => "Microphone",
            (Self::Fr, Resource::Camera) => "Caméra",
            (Self::De, Resource::Microphone) => "Mikrofon",
            (Self::De, Resource::Camera) => "Kamera",
            (Self::Es, Resource::Microphone) => "Micrófono",
            (Self::Es, Resource::Camera) => "Cámara",
            (Self::Ja, Resource::Microphone) => "マイク",
            (Self::Ja, Resource::Camera) => "カメラ",
            (Self::Zh, Resource::Microphone) => "麦克风",
            (Self::Zh, Resource::Camera) => "摄像头",
            (Self::Ru, Resource::Microphone) => "Микрофон",
            (Self::Ru, Resource::Camera) => "Камера",
            (Self::En, Resource::Microphone) => "Microphone",
            (Self::En, Resource::Camera) => "Camera",
        };
        let act = match (self, action) {
            (Self::Fr, Action::Start) => "Accès démarré",
            (Self::Fr, Action::Update) => "Accès mis à jour",
            (Self::Fr, Action::Stop) => "Accès terminé",
            (Self::De, Action::Start) => "Zugriff gestartet",
            (Self::De, Action::Update) => "Zugriff aktualisiert",
            (Self::De, Action::Stop) => "Zugriff beendet",
            (Self::Es, Action::Start) => "Acceso iniciado",
            (Self::Es, Action::Update) => "Acceso actualizado",
            (Self::Es, Action::Stop) => "Acceso finalizado",
            (Self::Ja, Action::Start) => "アクセス開始",
            (Self::Ja, Action::Update) => "アクセス更新",
            (Self::Ja, Action::Stop) => "アクセス停止",
            (Self::Zh, Action::Start) => "访问已开始",
            (Self::Zh, Action::Update) => "访问已更新",
            (Self::Zh, Action::Stop) => "访问已停止",
            (Self::Ru, Action::Start) => "Доступ начат",
            (Self::Ru, Action::Update) => "Доступ обновлен",
            (Self::Ru, Action::Stop) => "Доступ завершен",
            (Self::En, Action::Start) => "Access started",
            (Self::En, Action::Update) => "Access updated",
            (Self::En, Action::Stop) => "Access stopped",
        };
        format!("{act} — {res}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_all_seven_languages() {
        let codes = [
            ("en", Language::En),
            ("fr", Language::Fr),
            ("de", Language::De),
            ("es", Language::Es),
            ("ja", Language::Ja),
            ("zh", Language::Zh),
            ("ru", Language::Ru),
            ("FR-FR", Language::Fr),
            ("DE-DE", Language::De),
            ("zh-cn", Language::Zh),
            ("ru-ru", Language::Ru),
        ];
        for (code, expected) in codes {
            assert_eq!(Language::from_code(code), Some(expected));
        }
        assert_eq!(Language::from_code("unsupported"), None);
    }

    #[test]
    fn translations_are_non_empty_and_valid() {
        let langs = [
            Language::En,
            Language::Fr,
            Language::De,
            Language::Es,
            Language::Ja,
            Language::Zh,
            Language::Ru,
        ];
        for lang in langs {
            assert!(!lang.no_activity().is_empty());
            assert!(!lang.device_unavailable().is_empty());
            assert!(!lang.no_devices_found().is_empty());
            assert!(!lang.parent_label().is_empty());
            assert!(!lang.recommended_warning().is_empty());
            assert!(!lang.risk_str(Risk::Expected).is_empty());
            assert!(!lang.risk_str(Risk::Suspicious).is_empty());
            assert!(
                !lang
                    .state_str(Some(Action::Start), Activity::Active)
                    .is_empty()
            );
            assert!(!lang.toast_title(Action::Start, Resource::Camera).is_empty());
        }
    }

    #[test]
    fn detect_returns_valid_language() {
        let _lang = Language::detect();
    }
}
