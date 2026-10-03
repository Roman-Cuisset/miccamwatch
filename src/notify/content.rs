use crate::{
    i18n::Language,
    model::{Access, Action, Risk},
};

/// Common localized content; platform delivery adapters do not interpret risk.
pub(super) fn access_content(access: &Access, action: Action, lang: Language) -> (String, String) {
    let title = lang.toast_title(action, access.resource);
    let pid = access
        .pid
        .map(|value| format!("PID {value}"))
        .unwrap_or_else(|| "PID ?".to_owned());
    let mut body = vec![
        format!("{} ({pid})", access.application),
        format!(
            "{}: {} | {}: {}",
            match lang {
                Language::Fr => "risque",
                Language::De => "Risiko",
                Language::Es => "riesgo",
                Language::Ja => "リスク",
                Language::Zh => "风险",
                Language::Ru => "риск",
                Language::En => "risk",
            },
            lang.risk_str(access.risk),
            match lang {
                Language::Fr => "confiance",
                Language::De => "Vertrauen",
                Language::Es => "confianza",
                Language::Ja => "信頼度",
                Language::Zh => "可信度",
                Language::Ru => "достоверность",
                Language::En => "confidence",
            },
            match (lang, access.confidence) {
                (Language::Fr, crate::model::Confidence::High) => "haute",
                (Language::Fr, crate::model::Confidence::Medium) => "moyenne",
                (Language::Fr, crate::model::Confidence::Low) => "basse",
                (Language::De, crate::model::Confidence::High) => "hoch",
                (Language::De, crate::model::Confidence::Medium) => "mittel",
                (Language::De, crate::model::Confidence::Low) => "niedrig",
                (Language::Es, crate::model::Confidence::High) => "alta",
                (Language::Es, crate::model::Confidence::Medium) => "media",
                (Language::Es, crate::model::Confidence::Low) => "baja",
                (Language::Ja, crate::model::Confidence::High) => "高",
                (Language::Ja, crate::model::Confidence::Medium) => "中",
                (Language::Ja, crate::model::Confidence::Low) => "低",
                (Language::Zh, crate::model::Confidence::High) => "高",
                (Language::Zh, crate::model::Confidence::Medium) => "中",
                (Language::Zh, crate::model::Confidence::Low) => "低",
                (Language::Ru, crate::model::Confidence::High) => "высокая",
                (Language::Ru, crate::model::Confidence::Medium) => "средняя",
                (Language::Ru, crate::model::Confidence::Low) => "низкая",
                (Language::En, crate::model::Confidence::High) => "high",
                (Language::En, crate::model::Confidence::Medium) => "medium",
                (Language::En, crate::model::Confidence::Low) => "low",
            }
        ),
    ];
    if let (Some(parent_pid), Some(parent_name)) = (access.parent_pid, &access.parent_name) {
        body.push(format!(
            "{} {parent_name} ({parent_pid})",
            lang.parent_label()
        ));
    }
    if access.risk == Risk::Blocked {
        #[cfg(windows)]
        body.push(match lang {
            Language::Fr => "Permission refusée par Windows.".to_owned(),
            Language::De => "Windows-Berechtigung verweigert.".to_owned(),
            Language::Es => "Permiso denegado por Windows.".to_owned(),
            Language::Ja => "Windowsにより権限が拒否されました。".to_owned(),
            Language::Zh => "Windows权限已被拒绝。".to_owned(),
            Language::Ru => "Разрешение Windows отклонено.".to_owned(),
            Language::En => "Windows permission is denied.".to_owned(),
        });
        #[cfg(unix)]
        body.push(match lang {
            Language::Fr => "Accès évalué comme bloqué.".to_owned(),
            Language::De => "Zugriff als blockiert eingestuft.".to_owned(),
            Language::Es => "Acceso evaluado como bloqueado.".to_owned(),
            Language::Ja => "アクセスはブロック対象と評価されました。".to_owned(),
            Language::Zh => "访问被评估为已阻止。".to_owned(),
            Language::Ru => "Доступ оценён как заблокированный.".to_owned(),
            Language::En => "Access assessed as blocked.".to_owned(),
        });
    } else if access.risk == Risk::Suspicious {
        body.push(match lang {
            Language::Fr => "Plusieurs signaux suspects détectés.".to_owned(),
            Language::De => "Mehrere verdächtige Signale erkannt.".to_owned(),
            Language::Es => "Se detectaron múltiples señales sospechosas.".to_owned(),
            Language::Ja => "複数の不審なシグナルが検出されました。".to_owned(),
            Language::Zh => "检测到多个可疑信号。".to_owned(),
            Language::Ru => "Обнаружено несколько подозрительных сигналов.".to_owned(),
            Language::En => "Multiple suspicious signals detected.".to_owned(),
        });
    }
    #[cfg(unix)]
    if let Some(error) = access
        .signature
        .as_ref()
        .and_then(|signature| signature.error.as_deref())
    {
        body.push(format!("{} signature: {error}", lang.evidence_label()));
    }
    (title, body.join(" | "))
}
