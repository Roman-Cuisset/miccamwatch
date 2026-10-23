import AppKit
import CoreGraphics
import CoreFoundation
import Foundation
import UserNotifications
import os
import Darwin

// Desktop protocol v4: UTF-8 newline-delimited JSON, maximum frame 65536 bytes.
// State carries bounded observations, explicit protection facts and localized UI
// language/version. Only profile has children (four direct checked choices).
// show opens the existing consultation window; stop retires the desktop.
// Local consultation/About never emit a device action. Advanced controls remain
// explicit actions in the consultation window, never implicit during refresh.
private struct DesktopCodingKey: CodingKey {
    let stringValue: String
    var intValue: Int? { nil }
    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { return nil }
}
private func desktopFields(_ decoder: Decoder, allowed: [String]) throws {
    let fields = try decoder.container(keyedBy: DesktopCodingKey.self)
    guard fields.allKeys.allSatisfy({ allowed.contains($0.stringValue) }) else {
        throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "unknown desktop protocol field"))
    }
}
struct DesktopItem: Decodable {
    let action: String
    let detail: String?
    let label: String
    let enabled: Bool
    let checked: Bool?
    let children: [DesktopItem]?

    private enum CodingKeys: String, CodingKey { case action, detail, label, enabled, checked, children }
    init(from decoder: Decoder) throws {
        try desktopFields(decoder, allowed: ["action", "detail", "label", "enabled", "checked", "children"])
        guard decoder.codingPath.filter({ $0.stringValue == "children" }).count <= 1 else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "desktop menu exceeds two levels"))
        }
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        action = try fields.decode(String.self, forKey: .action)
        detail = try fields.decodeIfPresent(String.self, forKey: .detail)
        label = try fields.decode(String.self, forKey: .label)
        enabled = try fields.decode(Bool.self, forKey: .enabled)
        checked = try fields.decodeIfPresent(Bool.self, forKey: .checked)
        children = try fields.decodeIfPresent([DesktopItem].self, forKey: .children)
    }
}
struct DesktopAccess: Decodable {
    let application: String
    let resource: String
    let activity: String
    let pid: Int?
    private enum CodingKeys: String, CodingKey { case application, resource, activity, pid }
    init(from decoder: Decoder) throws {
        try desktopFields(decoder, allowed: ["application", "resource", "activity", "pid"])
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        application = try fields.decode(String.self, forKey: .application)
        resource = try fields.decode(String.self, forKey: .resource)
        activity = try fields.decode(String.self, forKey: .activity)
        guard fields.contains(.pid) else {
            throw DecodingError.keyNotFound(CodingKeys.pid, .init(codingPath: decoder.codingPath, debugDescription: "missing pid"))
        }
        pid = try fields.decodeIfPresent(Int.self, forKey: .pid)
    }
}
struct DesktopOverview: Decodable {
    let microphone: String
    let camera: String
    let observed_at: String?
    let total_accesses: Int
    let omitted_accesses: Int
    let accesses: [DesktopAccess]
    private enum CodingKeys: String, CodingKey { case microphone, camera, observed_at, total_accesses, omitted_accesses, accesses }
    init(from decoder: Decoder) throws {
        try desktopFields(decoder, allowed: ["microphone", "camera", "observed_at", "total_accesses", "omitted_accesses", "accesses"])
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        microphone = try fields.decode(String.self, forKey: .microphone)
        camera = try fields.decode(String.self, forKey: .camera)
        guard fields.contains(.observed_at) else {
            throw DecodingError.keyNotFound(CodingKeys.observed_at, .init(codingPath: decoder.codingPath, debugDescription: "missing observation time"))
        }
        observed_at = try fields.decodeIfPresent(String.self, forKey: .observed_at)
        total_accesses = try fields.decode(Int.self, forKey: .total_accesses)
        omitted_accesses = try fields.decode(Int.self, forKey: .omitted_accesses)
        accesses = try fields.decode([DesktopAccess].self, forKey: .accesses)
    }
}
struct DesktopPreferences: Decodable {
    let notifications_enabled: Bool
    let sound_enabled: Bool
    let history_enabled: Bool
    let notifications_paused: Bool
    private enum CodingKeys: String, CodingKey { case notifications_enabled, sound_enabled, history_enabled, notifications_paused }
    init(from decoder: Decoder) throws {
        try desktopFields(decoder, allowed: ["notifications_enabled", "sound_enabled", "history_enabled", "notifications_paused"])
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        notifications_enabled = try fields.decode(Bool.self, forKey: .notifications_enabled)
        sound_enabled = try fields.decode(Bool.self, forKey: .sound_enabled)
        history_enabled = try fields.decode(Bool.self, forKey: .history_enabled)
        notifications_paused = try fields.decode(Bool.self, forKey: .notifications_paused)
    }
}
struct DesktopFrame: Decodable {
    let kind: String
    let summary: String?
    let diagnostics: String?
    let limited: Bool?
    let visual: String?
    let items: [DesktopItem]?
    let language: String?
    let version: String?
    let protection: DesktopProtection?
    let overview: DesktopOverview?
    let preferences: DesktopPreferences?
    let feedback: String?
    private enum CodingKeys: String, CodingKey {
        case kind, summary, diagnostics, limited, visual, items, language, version, protection, overview, preferences, feedback
    }
    init(from decoder: Decoder) throws {
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        kind = try fields.decode(String.self, forKey: .kind)
        let keys = ["kind", "summary", "diagnostics", "limited", "visual", "items", "language", "version", "protection", "overview", "preferences", "feedback"]
        try desktopFields(decoder, allowed: kind == "state" ? keys : ["kind"])
        guard ["state", "show", "stop"].contains(kind) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "unknown desktop frame kind"))
        }
        if kind == "state" {
            guard fields.allKeys.count == keys.count else {
                throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "missing desktop state field"))
            }
        }
        summary = try fields.decodeIfPresent(String.self, forKey: .summary)
        diagnostics = try fields.decodeIfPresent(String.self, forKey: .diagnostics)
        limited = try fields.decodeIfPresent(Bool.self, forKey: .limited)
        visual = try fields.decodeIfPresent(String.self, forKey: .visual)
        items = try fields.decodeIfPresent([DesktopItem].self, forKey: .items)
        language = try fields.decodeIfPresent(String.self, forKey: .language)
        version = try fields.decodeIfPresent(String.self, forKey: .version)
        protection = try fields.decodeIfPresent(DesktopProtection.self, forKey: .protection)
        overview = try fields.decodeIfPresent(DesktopOverview.self, forKey: .overview)
        preferences = try fields.decodeIfPresent(DesktopPreferences.self, forKey: .preferences)
        feedback = try fields.decodeIfPresent(String.self, forKey: .feedback)
    }
}
struct DesktopProtection: Decodable {
    let microphone: String
    let camera: String
    let requested: [Bool?]
    let microphone_on_lock: Bool
    let camera_on_lock: Bool
    private enum CodingKeys: String, CodingKey { case microphone, camera, requested, microphone_on_lock, camera_on_lock }
    init(from decoder: Decoder) throws {
        try desktopFields(decoder, allowed: ["microphone", "camera", "requested", "microphone_on_lock", "camera_on_lock"])
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        microphone = try fields.decode(String.self, forKey: .microphone)
        camera = try fields.decode(String.self, forKey: .camera)
        requested = try fields.decode([Bool?].self, forKey: .requested)
        microphone_on_lock = try fields.decode(Bool.self, forKey: .microphone_on_lock)
        camera_on_lock = try fields.decode(Bool.self, forKey: .camera_on_lock)
    }
}
private let desktopTimestampPattern = try! NSRegularExpression(
    pattern: #"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:Z|\+00:00)$"#)
private let desktopTimestampFraction: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter
}()
private let desktopTimestampSeconds: ISO8601DateFormatter = {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime]
    return formatter
}()
// Same escaped-content projection as the Rust producer, excluding JSON quotes.
private func desktopTextCost(_ text: String?) -> Int {
    guard let text = text else { return 0 }
    return text.utf8.reduce(0) { cost, byte in
        switch byte {
        case 34, 92, 8, 12, 10, 13, 9: return cost + 2
        case 0...31: return cost + 6
        default: return cost + 1
        }
    }
}
func validDesktopState(_ frame: DesktopFrame) -> Bool {
    guard frame.kind == "state", let summary = frame.summary,
          let diagnostics = frame.diagnostics, diagnostics.utf8.count <= 32768, frame.limited != nil,
          let visual = frame.visual, ["idle", "ready", "active", "partial", "error"].contains(visual),
          let language = frame.language, ["en", "fr", "de", "es", "ja", "zh", "ru"].contains(language),
          let version = frame.version, !version.isEmpty, version.utf8.count <= 64,
          let protection = frame.protection, protection.requested.count == 2,
          ["muted", "unmuted", "mixed", "unknown"].contains(protection.microphone),
          ["owned", "absent", "system_managed", "unknown"].contains(protection.camera),
          let entries = frame.items, entries.count <= 16, summary.utf8.count <= 2048,
          entries.last?.action == "exit", entries.filter({ $0.action == "exit" }).count == 1 else { return false }
    guard let overview = frame.overview, frame.preferences != nil,
          ["active", "ready", "idle", "unknown"].contains(overview.microphone),
          ["active", "ready", "idle", "unknown"].contains(overview.camera),
          overview.accesses.count <= 8, overview.total_accesses >= overview.accesses.count,
          overview.omitted_accesses == overview.total_accesses - overview.accesses.count,
          (frame.feedback?.utf8.count ?? 0) <= 2048 else { return false }
    if let stamp = overview.observed_at {
        guard desktopTimestampPattern.rangeOfFirstMatch(in: stamp, options: [], range: NSRange(location: 0, length: stamp.utf16.count)).location != NSNotFound,
              desktopTimestampFraction.date(from: stamp) != nil || desktopTimestampSeconds.date(from: stamp) != nil else { return false }
    } else if overview.microphone != "unknown" || overview.camera != "unknown" || overview.total_accesses != 0 {
        return false
    }
    for access in overview.accesses {
        guard access.application.utf8.count <= 256,
              ["microphone", "camera"].contains(access.resource),
              ["active", "ready"].contains(access.activity) else { return false }
        let state = access.resource == "microphone" ? overview.microphone : overview.camera
        guard state == "active" || state == access.activity else { return false }
    }
    var projection = desktopTextCost(summary) + desktopTextCost(diagnostics) + desktopTextCost(frame.feedback) + desktopTextCost(overview.observed_at)
    projection += overview.accesses.reduce(0) { $0 + desktopTextCost($1.application) }
    let roots = ["status", "mic", "camera", "camera_inspect", "pause", "profile", "autostart", "exit", "header"]
    let choices = Set(["profile_balanced", "profile_private", "profile_meeting", "profile_development"])
    guard ["mic", "camera", "pause", "profile", "autostart"].allSatisfy({ action in entries.filter { $0.action == action }.count == 1 }),
          entries.filter({ $0.action == "camera_inspect" }).count <= 1 else { return false }
    var count = entries.count
    for entry in entries {
        guard roots.contains(entry.action), entry.checked == nil,
              entry.label.utf8.count <= 16384, (entry.detail?.utf8.count ?? 0) <= 16384 else { return false }
        projection += desktopTextCost(entry.label) + desktopTextCost(entry.detail)
        if entry.action == "profile" {
            guard let children = entry.children, children.count == 4,
                  Set(children.map { $0.action }) == choices,
                  children.filter({ $0.checked == true }).count <= 1 else { return false }
            count += children.count
            for child in children {
                projection += desktopTextCost(child.label) + desktopTextCost(child.detail)
                guard child.children == nil, child.checked != nil,
                      child.label.utf8.count <= 16384, (child.detail?.utf8.count ?? 0) <= 16384,
                      entry.enabled || !child.enabled else { return false }
            }
        } else if entry.children != nil { return false }
    }
    return count <= 20 && projection <= 48000
}

// Vector/template rendering is resolved by AppKit at the current backing scale
// and appearance. Badge shape (not color alone) distinguishes each state.
func desktopStatusImage(_ visual: String) -> NSImage {
    let brand = mcwBrandImage(size: 20, menuBar: true)
    let image = NSImage(size: NSSize(width: 26, height: 20), flipped: false) { _ in
        brand.draw(in: NSRect(x: 0, y: 0, width: 20, height: 20))
        guard visual != "idle" else { return true }
        let badge = NSRect(x: 18, y: 0, width: 8, height: 8)
        NSColor.clear.setFill()
        badge.insetBy(dx: -1, dy: -1).fill(using: .copy)
        NSColor.black.setFill()
        NSColor.black.setStroke()
        if visual == "active" {
            NSBezierPath(ovalIn: badge.insetBy(dx: 0.5, dy: 0.5)).fill()
        } else if visual == "ready" {
            let ring = NSBezierPath(ovalIn: badge.insetBy(dx: 1, dy: 1))
            ring.lineWidth = 1.5
            ring.stroke()
        } else if visual == "partial" {
            let triangle = NSBezierPath()
            triangle.move(to: NSPoint(x: badge.midX, y: badge.maxY))
            triangle.line(to: NSPoint(x: badge.minX, y: badge.minY))
            triangle.line(to: NSPoint(x: badge.maxX, y: badge.minY))
            triangle.close()
            triangle.fill()
        } else {
            let cross = NSBezierPath()
            cross.lineWidth = 2
            cross.move(to: NSPoint(x: badge.minX + 1, y: badge.minY + 1)); cross.line(to: NSPoint(x: badge.maxX - 1, y: badge.maxY - 1))
            cross.move(to: NSPoint(x: badge.minX + 1, y: badge.maxY - 1)); cross.line(to: NSPoint(x: badge.maxX - 1, y: badge.minY + 1))
            cross.stroke()
        }
        return true
    }
    image.isTemplate = true
    return image
}
private struct DesktopEvent: Encodable {
    var kind: String
    var `protocol`: Int? = nil
    var action: String? = nil
    var error: String? = nil
}
private func desktopEmit(_ event: DesktopEvent) {
    do {
        var bytes = try JSONEncoder().encode(event)
        bytes.append(10)
        try FileHandle.standardOutput.write(contentsOf: bytes)
    } catch { Darwin.exit(1) }
}
// Only documented Quartz session keys are used; console/login context is not lock evidence.
private func desktopGUIContext() -> (available: Bool, detail: String) {
    guard let session = CGSessionCopyCurrentDictionary() else {
        return (false, "quartzSession=unavailable euid=\(geteuid())")
    }
    let dictionary = session as NSDictionary
    let uid = (dictionary[kCGSessionUserIDKey] as? NSNumber)?.stringValue ?? "unknown"
    let onConsole = (dictionary[kCGSessionOnConsoleKey] as? Bool).map { String($0) } ?? "unknown"
    let loginDone = (dictionary[kCGSessionLoginDoneKey] as? Bool).map { String($0) } ?? "unknown"
    return (true, "quartzSession=available euid=\(geteuid()) sessionUID=\(uid) onConsole=\(onConsole) loginDone=\(loginDone)")
}
private func desktopPolicyName(_ policy: NSApplication.ActivationPolicy) -> String {
    switch policy {
    case .regular: return "regular"
    case .accessory: return "accessory"
    case .prohibited: return "prohibited"
    @unknown default: return "unknown(\(policy.rawValue))"
    }
}
private func desktopDiagnostic(_ context: String) {
    try? FileHandle.standardError.write(contentsOf: Data("MicCamWatch AppKit: \(context)\n".utf8))
}
struct DesktopWords {
    let open: String
    let about: String
    let observed: String
    let requested: String
    let effective: String
    let automatic: String
    let microphone: String
    let camera: String
    let yes: String
    let no: String
    let unknown: String
    let noRequest: String
    let systemManaged: String
    let muted: String
    let unmuted: String
    let mixed: String
    let restricted: String
    let unrestricted: String
    let scope: String
    let limited: String
    let description: String
    init(_ language: String) {
        let values: [String]
        switch language {
        case "fr": values = [
            "Ouvrir MicCamWatch…", "À propos de MicCamWatch…", "Activité observée et accès connus",
            "Dernière protection demandée dans cette session", "Protection effective observée", "Préférences au verrouillage de session",
            "Microphone", "Caméra", "Oui", "Non", "Inconnu",
            "Entrées modifiables coupées", "Entrées modifiables non coupées", "Entrées dans des états différents",
            "Profil de restriction MCW présent", "Profil de restriction MCW absent",
            "Le contrôle micro ne couvre que les entrées modifiables, pas une coupure physique. La caméra dépend des profils et des règles système ; une absence de profil MCW ne prouve pas une autorisation globale. Inconnu signifie que l’API ne permet pas de conclure ; des droits admin ne garantissent ni observation complète ni blocage. Les préférences ne prouvent pas leur application.",
            "MicCamWatch surveille les accès au microphone et à la caméra. Il affiche les informations rendues visibles par le système, sans enregistrer de son ni d’image.", "Aucune demande connue", "Restriction d’un autre profil de gestion (non détenue par MCW)"]
        case "de": values = [
            "MicCamWatch öffnen…", "Über MicCamWatch…", "Beobachtete Aktivität und bekannte Zugriffe",
            "Zuletzt in dieser Sitzung angeforderter Schutz", "Beobachteter wirksamer Schutz", "Einstellungen bei Sitzungssperre",
            "Mikrofon", "Kamera", "Ja", "Nein", "Unbekannt",
            "Schreibbare Eingänge stumm", "Schreibbare Eingänge nicht stumm", "Unterschiedliche Eingangszustände",
            "MCW-Sperrprofil vorhanden", "MCW-Sperrprofil nicht vorhanden",
            "Die Mikrofonsteuerung umfasst nur schreibbare Eingänge, keine physische Abschaltung. Kameras unterliegen Profilen und Systemregeln; ein fehlendes MCW-Profil belegt keine globale Freigabe. Unbekannt bedeutet, dass die API keine Aussage erlaubt; Adminrechte garantieren weder vollständige Beobachtung noch Sperrung. Einstellungen belegen keine Anwendung.",
            "MicCamWatch überwacht Mikrofon- und Kamerazugriffe. Es zeigt die vom System sichtbaren Informationen, ohne Ton oder Bilder aufzunehmen.", "Keine bekannte Anforderung", "Sperre durch anderes Verwaltungsprofil (nicht MCW-eigen)"]
        case "es": values = [
            "Abrir MicCamWatch…", "Acerca de MicCamWatch…", "Actividad observada y accesos conocidos",
            "Última protección solicitada en esta sesión", "Protección efectiva observada", "Preferencias al bloquear la sesión",
            "Micrófono", "Cámara", "Sí", "No", "Desconocido",
            "Entradas modificables silenciadas", "Entradas modificables sin silenciar", "Entradas en estados distintos",
            "Perfil de restricción MCW presente", "Perfil de restricción MCW ausente",
            "El control del micrófono solo cubre entradas modificables, no un corte físico. La cámara depende de perfiles y reglas del sistema; la ausencia de un perfil MCW no prueba un permiso global. Desconocido significa que la API no permite concluir; ser administrador no garantiza observación completa ni bloqueo. Las preferencias no prueban su aplicación.",
            "MicCamWatch supervisa los accesos al micrófono y a la cámara. Muestra la información visible para el sistema, sin grabar sonido ni imágenes.", "Sin solicitud conocida", "Restricción de otro perfil de gestión (no pertenece a MCW)"]
        case "ja": values = [
            "MicCamWatch を開く…", "MicCamWatch について…", "観測された動作と既知のアクセス",
            "このセッションで最後に要求した保護", "観測された実効保護", "セッションロック時の設定",
            "マイク", "カメラ", "はい", "いいえ", "不明",
            "変更可能な入力はミュート", "変更可能な入力は非ミュート", "入力の状態が混在",
            "MCW 制限プロファイルあり", "MCW 制限プロファイルなし",
            "マイク制御は変更可能な入力のみが対象で、物理的な遮断ではありません。カメラはプロファイルとシステム規則に依存し、MCW プロファイルがなくても全体の許可は証明されません。不明は API では判断できない状態です。管理者権限でも完全な観測や遮断は保証されません。設定は適用の証明ではありません。",
            "MicCamWatch はマイクとカメラへのアクセスを監視します。システムが公開する情報を表示し、音声や映像を記録しません。", "既知の要求なし", "他の管理プロファイルによる制限（MCW の所有ではない）"]
        case "zh": values = [
            "打开 MicCamWatch…", "关于 MicCamWatch…", "观测活动与已知访问",
            "本次会话最后请求的保护", "观测到的有效保护", "锁定会话时的偏好",
            "麦克风", "摄像头", "是", "否", "未知",
            "可写输入已静音", "可写输入未静音", "输入状态不一致",
            "存在 MCW 限制描述文件", "不存在 MCW 限制描述文件",
            "麦克风控制仅覆盖可写输入，并非物理断开。摄像头受描述文件及系统规则约束；没有 MCW 描述文件并不证明全局许可。未知表示 API 无法得出结论；管理员权限也不保证完整观测或阻止。偏好设置不证明已生效。",
            "MicCamWatch 监视麦克风和摄像头访问。它显示系统可见的信息，不录制声音或图像。", "无已知请求", "其他管理描述文件的限制（不属于 MCW）"]
        case "ru": values = [
            "Открыть MicCamWatch…", "О MicCamWatch…", "Наблюдаемая активность и известные доступы",
            "Последний запрос защиты в этом сеансе", "Наблюдаемая действующая защита", "Настройки при блокировке сеанса",
            "Микрофон", "Камера", "Да", "Нет", "Неизвестно",
            "Доступные для записи входы заглушены", "Доступные для записи входы не заглушены", "Разные состояния входов",
            "Профиль ограничения MCW присутствует", "Профиль ограничения MCW отсутствует",
            "Управление микрофоном охватывает лишь доступные для записи входы, а не физическое отключение. Камера зависит от профилей и правил системы; отсутствие профиля MCW не доказывает глобальное разрешение. Неизвестно означает, что API не позволяет сделать вывод; права администратора не гарантируют полное наблюдение или блокировку. Настройки не доказывают применение.",
            "MicCamWatch отслеживает доступ к микрофону и камере. Программа показывает доступную системе информацию, не записывая звук или изображение.", "Нет известного запроса", "Ограничение другим профилем управления (не принадлежит MCW)"]
        default: values = [
            "Open MicCamWatch…", "About MicCamWatch…", "Observed activity and known accesses",
            "Last protection requested in this session", "Observed effective protection", "Preferences on session lock",
            "Microphone", "Camera", "Yes", "No", "Unknown",
            "Writable inputs muted", "Writable inputs not muted", "Inputs have mixed states",
            "MCW restriction profile present", "MCW restriction profile absent",
            "Microphone control covers writable inputs only, not a physical cutoff. Camera protection depends on profiles and system policy; an absent MCW profile does not prove global permission. Unknown means the API cannot establish the state; administrator rights guarantee neither full observation nor blocking. Preferences do not prove application.",
            "MicCamWatch monitors microphone and camera access. It displays information made visible by the system, without recording audio or video.", "No known request", "Restriction from another management profile (not owned by MCW)"]
        }
        open = values[0]; about = values[1]; observed = values[2]; requested = values[3]
        effective = values[4]; automatic = values[5]; microphone = values[6]; camera = values[7]
        yes = values[8]; no = values[9]; unknown = values[10]; muted = values[11]
        unmuted = values[12]; mixed = values[13]; restricted = values[14]; unrestricted = values[15]
        scope = values[16]; description = values[17]
        noRequest = values[18]
        systemManaged = values[19]
        switch language {
        case "fr": limited = "Affichage limité : une partie du texte est omise pour borner cet instantané. Le nombre d’accès observés inclut les détails omis ; la surveillance continue."
        case "de": limited = "Begrenzte Anzeige: Ein Teil des Texts wurde ausgelassen. Die Zahl beobachteter Zugriffe umfasst ausgelassene Details; die Überwachung läuft weiter."
        case "es": limited = "Vista limitada: se ha omitido parte del texto para acotar esta instantánea. El recuento de accesos incluye los detalles omitidos; la supervisión continúa."
        case "ja": limited = "表示上限：スナップショットのサイズを制限するため、一部の文章を省略しています。観測アクセス数には省略した詳細も含まれます。監視は継続しています。"
        case "zh": limited = "显示有上限：为限制快照大小，已省略部分文本。观测访问总数包含被省略的详情；监视仍在继续。"
        case "ru": limited = "Ограниченный вид: часть текста опущена для ограничения размера снимка. Число доступов включает опущенные подробности; наблюдение продолжается."
        default: limited = "Bounded view: some text is omitted to limit this snapshot. The observed access count includes omitted details; monitoring continues."
        }
    }
}

// Accessory helpers have no application menu bar. Keep the standard document
// keyboard actions local to this window; Command-W must never become tray Exit.
private final class DesktopConsultationWindow: NSWindow {
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.intersection([.command, .option, .control, .shift]) == .command {
            switch event.charactersIgnoringModifiers?.lowercased() {
            case "w": performClose(nil); return true
            case "c": return NSApp.sendAction(#selector(NSText.copy(_:)), to: firstResponder, from: self)
            case "a": return NSApp.sendAction(#selector(NSText.selectAll(_:)), to: firstResponder, from: self)
            default: break
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}

// UI language follows the state, not the host operating system.
private struct DesktopGUIWords {
    let values: [String]
    subscript(_ index: Int) -> String { values[index] }
    init(_ language: String) {
        switch language {
        case "fr": values = ["Vue d’ensemble", "Réglages", "Diagnostics", "Activité observée", "Contrôle effectif", "Dernière observation", "Applications observées", "Aucun accès observé dans cet instantané.", "Observation indisponible ; cela ne prouve pas une absence d’accès.", "accès omis", "Profil", "Notifications", "Son des notifications", "Historique local", "Les notifications dépendent aussi de l’autorisation macOS. Le son suit les notifications ; la pause ne suspend pas la surveillance.", "Le profil ne coupe ni le micro ni la caméra. Meeting et Balanced ont actuellement la même politique.", "Portée et limites", "Actif", "Prêt", "Inactif observé", "Retour de la dernière action", "Copier les diagnostics", "En pause", "Notifications non suspendues"]
        case "de": values = ["Übersicht", "Einstellungen", "Diagnose", "Beobachtete Aktivität", "Wirksame Steuerung", "Letzte Beobachtung", "Beobachtete Anwendungen", "In diesem Schnappschuss kein Zugriff beobachtet.", "Beobachtung nicht verfügbar; kein Nachweis fehlender Zugriffe.", "Zugriffe ausgelassen", "Profil", "Mitteilungen", "Mitteilungston", "Lokaler Verlauf", "Mitteilungen benötigen auch die macOS-Erlaubnis. Ton folgt Mitteilungen; Pause stoppt nicht die Überwachung.", "Ein Profil schaltet Mikrofon und Kamera nicht ab. Meeting und Balanced nutzen derzeit dieselbe Richtlinie.", "Umfang und Grenzen", "Aktiv", "Bereit", "Beobachtet inaktiv", "Letzte Aktionsrückmeldung", "Diagnose kopieren", "Pausiert", "Mitteilungen nicht pausiert"]
        case "es": values = ["Resumen", "Ajustes", "Diagnóstico", "Actividad observada", "Control efectivo", "Última observación", "Aplicaciones observadas", "No se observó acceso en esta instantánea.", "Observación no disponible; no demuestra ausencia de acceso.", "accesos omitidos", "Perfil", "Notificaciones", "Sonido de notificaciones", "Historial local", "Las notificaciones también requieren permiso de macOS. El sonido sigue a las notificaciones; la pausa no detiene la supervisión.", "El perfil no silencia ni bloquea dispositivos. Meeting y Balanced usan actualmente la misma política.", "Alcance y límites", "Activo", "Preparado", "Inactivo observado", "Resultado de la última acción", "Copiar diagnóstico", "En pausa", "Notificaciones sin pausar"]
        case "ja": values = ["概要", "設定", "診断", "観測された動作", "実効制御", "最終観測", "観測されたアプリ", "このスナップショットではアクセスは観測されませんでした。", "観測できません。アクセスがないことの証明ではありません。", "件のアクセスを省略", "プロファイル", "通知", "通知音", "ローカル履歴", "通知には macOS の許可も必要です。音は通知設定に従います。一時停止しても監視は続きます。", "プロファイルはマイクやカメラを遮断しません。Meeting と Balanced は現在同じポリシーです。", "対象範囲と制限", "使用中", "準備完了", "未使用を観測", "最後の操作結果", "診断をコピー", "一時停止中", "通知の一時停止なし"]
        case "zh": values = ["概览", "设置", "诊断", "观测活动", "有效控制", "最近观测", "观测到的应用", "此快照中未观测到访问。", "无法观测；这不能证明没有访问。", "条访问已省略", "配置", "通知", "通知声音", "本地历史", "通知也需要 macOS 授权。声音跟随通知设置；暂停通知不会停止监视。", "配置不会静音或阻止设备。Meeting 和 Balanced 当前使用相同策略。", "范围与限制", "使用中", "就绪", "观测为空闲", "最近操作结果", "复制诊断", "已暂停", "通知未暂停"]
        case "ru": values = ["Обзор", "Настройки", "Диагностика", "Наблюдаемая активность", "Действующее управление", "Последнее наблюдение", "Наблюдаемые приложения", "В этом снимке доступ не наблюдался.", "Наблюдение недоступно; это не доказывает отсутствие доступа.", "доступов опущено", "Профиль", "Уведомления", "Звук уведомлений", "Локальная история", "Уведомления также требуют разрешения macOS. Звук зависит от уведомлений; пауза не останавливает наблюдение.", "Профиль не заглушает микрофон и не блокирует камеру. Meeting и Balanced сейчас используют одну политику.", "Область и ограничения", "Активно", "Готово", "Наблюдается простой", "Результат последнего действия", "Копировать диагностику", "Приостановлено", "Уведомления не приостановлены"]
        default: values = ["Overview", "Settings", "Diagnostics", "Observed activity", "Effective control", "Latest observation", "Observed applications", "No access observed in this snapshot.", "Observation unavailable; this is not proof of no access.", "accesses omitted", "Profile", "Notifications", "Notification sound", "Local history", "Notifications also depend on macOS permission. Sound follows notifications; pausing does not stop observation.", "A profile does not mute or block devices. Meeting and Balanced currently use the same policy.", "Scope and limitations", "Active", "Ready", "Observed idle", "Last action feedback", "Copy diagnostics", "Paused", "Notifications not paused"]
        }
    }
}
private final class DesktopLabel: NSTextField {
    override var stringValue: String {
        get { super.stringValue }
        set { if super.stringValue != newValue { super.stringValue = newValue } }
    }
}
private func desktopLabel(_ size: CGFloat = 13, weight: NSFont.Weight = .regular) -> NSTextField {
    let label = DesktopLabel(wrappingLabelWithString: "")
    label.font = .systemFont(ofSize: size, weight: weight)
    label.isSelectable = true
    label.lineBreakMode = .byCharWrapping
    label.maximumNumberOfLines = 0
    label.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
    return label
}
private final class DesktopColumn: NSStackView {
    override var isFlipped: Bool { true }
}
private func desktopColumn(_ views: [NSView], inset: CGFloat = 0) -> NSStackView {
    let stack = DesktopColumn(views: views)
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.edgeInsets = NSEdgeInsets(top: inset, left: inset, bottom: inset, right: inset)
    for view in views {
        view.translatesAutoresizingMaskIntoConstraints = false
        view.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -2 * inset).isActive = true
    }
    return stack
}
private final class DesktopCard: NSView {
    override func draw(_ dirtyRect: NSRect) {
        let outline = NSBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), xRadius: 10, yRadius: 10)
        NSColor.controlBackgroundColor.setFill()
        outline.fill()
        NSColor.separatorColor.setStroke()
        outline.stroke()
    }
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }
}
private func desktopCard(_ views: [NSView]) -> NSView {
    let card = DesktopCard()
    let stack = desktopColumn(views, inset: 14)
    stack.translatesAutoresizingMaskIntoConstraints = false
    card.addSubview(stack)
    NSLayoutConstraint.activate([
        stack.leadingAnchor.constraint(equalTo: card.leadingAnchor),
        stack.trailingAnchor.constraint(equalTo: card.trailingAnchor),
        stack.topAnchor.constraint(equalTo: card.topAnchor),
        stack.bottomAnchor.constraint(equalTo: card.bottomAnchor)
    ])
    return card
}
private func desktopScroll(_ document: NSView) -> NSScrollView {
    let scroll = NSScrollView()
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = false
    scroll.drawsBackground = false
    document.translatesAutoresizingMaskIntoConstraints = false
    scroll.documentView = document
    NSLayoutConstraint.activate([
        document.leadingAnchor.constraint(equalTo: scroll.contentView.leadingAnchor),
        document.topAnchor.constraint(equalTo: scroll.contentView.topAnchor),
        document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor)
    ])
    return scroll
}

// Width-dependent height keeps long paths readable rather than clipping them
// into a fixed-height pushbutton. Measurements are reused until width/title change.
private final class DesktopActionButton: NSButton {
    private var measuredWidth: CGFloat = -1
    private var measuredTitle = ""
    private var wrappingHeight: NSLayoutConstraint?
    override func layout() {
        super.layout()
        guard bounds.width > 0, measuredWidth != bounds.width || measuredTitle != title else { return }
        measuredWidth = bounds.width
        measuredTitle = title
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byCharWrapping
        let rectangle = (title as NSString).boundingRect(
            with: NSSize(width: max(1, bounds.width - 28), height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin, .usesFontLeading],
            attributes: [.font: font ?? NSFont.systemFont(ofSize: 13), .paragraphStyle: paragraph])
        let height = max(32, ceil(rectangle.height) + 16)
        if let constraint = wrappingHeight { constraint.constant = height }
        else {
            let constraint = heightAnchor.constraint(equalToConstant: height)
            constraint.isActive = true
            wrappingHeight = constraint
        }
    }
}

// Retain controls and tabs across snapshots. Refresh never steals focus, opens
// the window or emits actions. Closing only orders out this modeless window.
final class DesktopConsultation: NSObject {
    let window: NSWindow
    let text: NSTextView
    let tabs = NSTabView()
    let profile = NSPopUpButton(frame: .zero, pullsDown: false)
    let notifications = NSButton(checkboxWithTitle: "", target: nil, action: nil)
    let sound = NSButton(checkboxWithTitle: "", target: nil, action: nil)
    let history = NSButton(checkboxWithTitle: "", target: nil, action: nil)
    private let brand = desktopLabel(22, weight: .semibold)
    private let subtitle = desktopLabel(12)
    private let observedTitle = desktopLabel(15, weight: .semibold)
    private let micObservation = desktopLabel(17, weight: .medium)
    private let cameraObservation = desktopLabel(17, weight: .medium)
    private let effectiveTitle = desktopLabel(15, weight: .semibold)
    private let micEffective = desktopLabel()
    private let cameraEffective = desktopLabel()
    private let timestamp = desktopLabel(12)
    private let accessesTitle = desktopLabel(15, weight: .semibold)
    private let empty = desktopLabel()
    private let accessRows = (0..<8).map { _ in desktopLabel() }
    private let omitted = desktopLabel(12)
    private let feedback = desktopLabel()
    private let limitations = desktopLabel(12)
    private let scopeTitle = desktopLabel(15, weight: .semibold)
    private let scope = desktopLabel(12)
    private let profileTitle = desktopLabel(15, weight: .semibold)
    private let profileNote = desktopLabel(12)
    private let notificationNote = desktopLabel(12)
    private let paused = desktopLabel()
    private let headerFeedback = desktopLabel(12)
    private let settingsFeedback = desktopLabel()
    private let copyButton = NSButton()
    private var buttons: [String: NSButton] = [:]
    private var paneScrolls: [NSScrollView] = []
    private var frame: DesktopFrame?
    private var dirty = false
    override init() {
        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 800, height: 600)
        let size = NSSize(width: min(645, screen.width - 40), height: min(548, screen.height - 60))
        window = DesktopConsultationWindow(contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "MicCamWatch"
        text = NSTextView(frame: NSRect(x: 0, y: 0, width: size.width - 48, height: 300))
        super.init()
        window.minSize = NSSize(width: min(440, size.width), height: min(360, size.height))
        for (identifier, label) in [
            ("observation_microphone", micObservation), ("observation_camera", cameraObservation),
            ("effective_microphone", micEffective), ("effective_camera", cameraEffective),
            ("observed_at", timestamp), ("access_empty", empty), ("omitted_accesses", omitted),
            ("last_action_feedback", feedback)
        ] {
            label.identifier = NSUserInterfaceItemIdentifier(identifier)
        }
        for (index, row) in accessRows.enumerated() {
            row.identifier = NSUserInterfaceItemIdentifier("access_\(index)")
        }
        let icon = NSImageView()
        icon.image = mcwBrandImage(size: 44)
        icon.setAccessibilityLabel("MicCamWatch")
        icon.widthAnchor.constraint(equalToConstant: 44).isActive = true
        icon.heightAnchor.constraint(equalToConstant: 44).isActive = true
        let heading = NSStackView(views: [icon, desktopColumn([brand, subtitle])])
        heading.spacing = 12
        heading.alignment = .centerY
        let overview = desktopColumn([
            desktopCard([observedTitle, micObservation, cameraObservation, timestamp]),
            feedback, limitations,
            desktopCard([accessesTitle, empty] + accessRows + [omitted]),
            desktopCard([effectiveTitle, micEffective, cameraEffective, actionButton("mic"), actionButton("camera")]),
            desktopCard([scopeTitle, scope])
        ], inset: 12)
        let settings = desktopColumn([
            settingsFeedback,
            desktopCard([profileTitle, profile, profileNote]),
            desktopCard([notifications, sound, history, notificationNote, paused, actionButton("pause")]),
            desktopCard([actionButton("autostart")])
        ], inset: 12)
        for (key, button) in [("notifications", notifications), ("sound", sound), ("history", history)] {
            button.identifier = NSUserInterfaceItemIdentifier(key)
            button.target = self
            button.action = #selector(preferenceSelected(_:))
            button.lineBreakMode = .byWordWrapping
            button.cell?.wraps = true
        }
        profile.target = self
        profile.action = #selector(profileSelected(_:))
        profile.identifier = NSUserInterfaceItemIdentifier("profile")
        profile.cell?.lineBreakMode = .byTruncatingTail
        profile.menu?.autoenablesItems = false
        text.isEditable = false
        text.isSelectable = true
        text.isRichText = false
        text.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        text.textContainerInset = NSSize(width: 10, height: 10)
        text.minSize = .zero
        text.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        text.isVerticallyResizable = true
        text.isHorizontallyResizable = false
        text.autoresizingMask = [.width]
        text.textContainer?.widthTracksTextView = true
        text.textContainer?.lineBreakMode = .byCharWrapping
        text.textContainer?.containerSize = NSSize(width: text.frame.width, height: .greatestFiniteMagnitude)
        let diagnosticScroll = NSScrollView()
        diagnosticScroll.hasVerticalScroller = true
        diagnosticScroll.documentView = text
        copyButton.target = self
        copyButton.action = #selector(copyDiagnostics(_:))
        copyButton.bezelStyle = .rounded
        copyButton.identifier = NSUserInterfaceItemIdentifier("copy_diagnostics")
        let diagnostic = desktopColumn([diagnosticScroll, actionButton("camera_inspect"), copyButton], inset: 12)
        diagnosticScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 40).isActive = true
        let overviewScroll = desktopScroll(overview), settingsScroll = desktopScroll(settings)
        paneScrolls = [overviewScroll, settingsScroll, diagnosticScroll]
        let sections: [(String, NSView)] = [("overview", overviewScroll), ("settings", settingsScroll), ("diagnostics", diagnostic)]
        for (identifier, view) in sections {
            let tab = NSTabViewItem(identifier: identifier)
            tab.view = view
            tabs.addTabViewItem(tab)
        }
        tabs.identifier = NSUserInterfaceItemIdentifier("desktop_sections")
        headerFeedback.identifier = NSUserInterfaceItemIdentifier("header_feedback")
        headerFeedback.maximumNumberOfLines = 2
        headerFeedback.lineBreakMode = .byTruncatingTail
        let root = desktopColumn([heading, headerFeedback, tabs], inset: 16)
        root.translatesAutoresizingMaskIntoConstraints = false
        window.contentView!.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor),
            root.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor),
            root.topAnchor.constraint(equalTo: window.contentView!.topAnchor),
            root.bottomAnchor.constraint(equalTo: window.contentView!.bottomAnchor)
        ])
        tabs.setContentHuggingPriority(.defaultLow, for: .vertical)
        diagnosticScroll.setContentHuggingPriority(.defaultLow, for: .vertical)
        subtitle.textColor = .secondaryLabelColor
        timestamp.textColor = .secondaryLabelColor
        notificationNote.textColor = .secondaryLabelColor
        profileNote.textColor = .secondaryLabelColor
        scope.textColor = .secondaryLabelColor
        window.center()
    }
    private func actionButton(_ action: String) -> NSButton {
        let button = DesktopActionButton(title: "", target: self, action: #selector(selected(_:)))
        button.identifier = NSUserInterfaceItemIdentifier(action)
        button.bezelStyle = .regularSquare
        button.lineBreakMode = .byCharWrapping
        button.cell?.wraps = true
        button.cell?.isScrollable = false
        button.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        button.heightAnchor.constraint(greaterThanOrEqualToConstant: 32).isActive = true
        buttons[action] = button
        return button
    }
    func update(_ frame: DesktopFrame, diagnostic: String) {
        self.frame = frame
        dirty = true
        if text.string != diagnostic {
            let selection = text.selectedRanges
            let visible = text.enclosingScrollView?.contentView.bounds.origin
            text.string = diagnostic
            text.selectedRanges = selection.map {
                let range = $0.rangeValue
                let start = min(range.location, (diagnostic as NSString).length)
                return NSValue(range: NSRange(location: start, length: min(range.length, (diagnostic as NSString).length - start)))
            }
            if let scroll = text.enclosingScrollView, let visible = visible {
                scroll.contentView.scroll(to: visible)
                scroll.reflectScrolledClipView(scroll.contentView)
            }
        }
        if window.isVisible { render() }
    }
    private func render() {
        guard dirty, let frame = frame else { return }
        dirty = false
        let positions = paneScrolls.map { $0.contentView.bounds.origin }
        let words = DesktopWords(frame.language!), gui = DesktopGUIWords(frame.language!)
        let overview = frame.overview!, protection = frame.protection!, preferences = frame.preferences!
        for index in 0..<3 where tabs.tabViewItems[index].label != gui[index] {
            tabs.tabViewItems[index].label = gui[index]
        }
        brand.stringValue = "MicCamWatch"
        subtitle.stringValue = frame.version!
        observedTitle.stringValue = gui[3]
        effectiveTitle.stringValue = gui[4]
        func activity(_ state: String) -> String {
            switch state { case "active": return gui[17]; case "ready": return gui[18]; case "idle": return gui[19]; default: return words.unknown }
        }
        micObservation.stringValue = "\(words.microphone): \(activity(overview.microphone))"
        cameraObservation.stringValue = "\(words.camera): \(activity(overview.camera))"
        micObservation.textColor = overview.microphone == "active" ? .systemOrange : .labelColor
        cameraObservation.textColor = overview.camera == "active" ? .systemOrange : .labelColor
        let mic: String
        switch protection.microphone { case "muted": mic = words.muted; case "unmuted": mic = words.unmuted; case "mixed": mic = words.mixed; default: mic = words.unknown }
        let camera: String
        switch protection.camera { case "owned": camera = words.restricted; case "absent": camera = words.unrestricted; case "system_managed": camera = words.systemManaged; default: camera = words.unknown }
        micEffective.stringValue = "\(words.microphone): \(mic)"
        cameraEffective.stringValue = "\(words.camera): \(camera)"
        timestamp.stringValue = "\(gui[5]): \(overview.observed_at ?? words.unknown)"
        accessesTitle.stringValue = "\(gui[6]) · \(overview.total_accesses)"
        empty.isHidden = !overview.accesses.isEmpty
        empty.stringValue = overview.microphone == "unknown" || overview.camera == "unknown" ? gui[8] : gui[7]
        for (index, row) in accessRows.enumerated() {
            row.isHidden = index >= overview.accesses.count
            guard index < overview.accesses.count else { continue }
            let access = overview.accesses[index]
            let resource = access.resource == "microphone" ? words.microphone : words.camera
            row.stringValue = "\(access.application)\n\(resource) · \(activity(access.activity))" + (access.pid.map { " · PID \($0)" } ?? "")
            row.setAccessibilityLabel(row.stringValue)
        }
        omitted.isHidden = overview.omitted_accesses == 0
        omitted.stringValue = "\(overview.omitted_accesses) \(gui[9])"
        let feedbackText = frame.feedback.map { "\(gui[20])\n\($0)" } ?? ""
        headerFeedback.stringValue = feedbackText
        headerFeedback.isHidden = frame.feedback == nil
        headerFeedback.toolTip = frame.feedback
        feedback.stringValue = feedbackText
        feedback.isHidden = frame.feedback == nil
        settingsFeedback.stringValue = feedbackText
        settingsFeedback.isHidden = frame.feedback == nil
        limitations.stringValue = words.limited
        limitations.isHidden = frame.limited != true
        scopeTitle.stringValue = gui[16]
        scope.stringValue = words.scope
        profileTitle.stringValue = gui[10]
        profileNote.stringValue = gui[15]
        notifications.title = gui[11]; notifications.state = preferences.notifications_enabled ? .on : .off
        sound.title = gui[12]; sound.state = preferences.sound_enabled ? .on : .off
        history.title = gui[13]; history.state = preferences.history_enabled ? .on : .off
        notifications.setAccessibilityLabel(gui[11])
        sound.setAccessibilityLabel(gui[12])
        history.setAccessibilityLabel(gui[13])
        notificationNote.stringValue = gui[14]
        paused.stringValue = gui[preferences.notifications_paused ? 22 : 23]
        copyButton.title = gui[21]
        copyButton.setAccessibilityLabel(gui[21])
        text.setAccessibilityLabel(gui[2])
        for (action, button) in buttons {
            let entry = frame.items!.first { $0.action == action }
            button.isHidden = entry == nil
            button.isEnabled = entry?.enabled == true
            button.title = entry?.label ?? ""
            button.toolTip = entry?.detail ?? entry?.label
            button.setAccessibilityLabel(entry?.label)
        }
        let entry = frame.items!.first { $0.action == "profile" }!
        let children = entry.children!
        if profile.itemTitles != children.map({ $0.label }) {
            profile.removeAllItems()
            profile.addItems(withTitles: children.map { $0.label })
        }
        for (index, child) in children.enumerated() {
            profile.item(at: index)?.representedObject = child.action
            profile.item(at: index)?.isEnabled = entry.enabled && child.enabled
        }
        let selectedProfile = children.firstIndex { $0.checked == true } ?? -1
        if profile.indexOfSelectedItem != selectedProfile { profile.selectItem(at: selectedProfile) }
        profile.isEnabled = entry.enabled
        profile.toolTip = entry.detail ?? entry.label
        profile.setAccessibilityLabel(entry.label)
        window.contentView!.layoutSubtreeIfNeeded()
        for (scroll, position) in zip(paneScrolls, positions) {
            scroll.contentView.scroll(to: position)
            scroll.reflectScrolledClipView(scroll.contentView)
        }
    }
    func show() {
        render()
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }
    @objc private func selected(_ sender: NSButton) {
        guard let action = sender.identifier?.rawValue,
              frame?.items?.contains(where: { $0.action == action && $0.enabled }) == true else { return }
        desktopEmit(DesktopEvent(kind: "action", action: action))
    }
    @objc private func preferenceSelected(_ sender: NSButton) {
        guard frame != nil, let key = sender.identifier?.rawValue,
              ["notifications", "sound", "history"].contains(key), sender.isEnabled else { return }
        desktopEmit(DesktopEvent(kind: "action", action: key + (sender.state == .on ? "_enable" : "_disable")))
    }
    @objc private func profileSelected(_ sender: NSPopUpButton) {
        guard let entry = frame?.items?.first(where: { $0.action == "profile" }), entry.enabled,
              let action = sender.selectedItem?.representedObject as? String,
              entry.children?.contains(where: { $0.action == action && $0.enabled }) == true else { return }
        desktopEmit(DesktopEvent(kind: "action", action: action))
    }
    @objc private func copyDiagnostics(_ sender: Any?) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text.string, forType: .string)
    }
}

// Standard AppKit menu chrome needs about 60 points in addition to the text.
// Measure the actual menu font, not bytes or character counts (localized labels
// and wide graphemes differ substantially). Never split a composed character.
final class DesktopMenu: NSObject {
    let menu = NSMenu()
    let diagnostic: String
    let consultation: DesktopConsultation
    private let words: DesktopWords
    private let version: String
    private let textWidth: CGFloat

    init(frame: DesktopFrame, consultation: DesktopConsultation) {
        self.consultation = consultation
        let words = DesktopWords(frame.language!)
        self.words = words
        version = frame.version!
        let summary = frame.summary!, entries = frame.items!, protection = frame.protection!
        textWidth = min(340, max(80, (NSScreen.main?.visibleFrame.width ?? 400) - 60))
        func flag(_ value: Bool?) -> String { value.map { $0 ? words.yes : words.no } ?? words.noRequest }
        let mic: String
        switch protection.microphone {
        case "muted": mic = words.muted
        case "unmuted": mic = words.unmuted
        case "mixed": mic = words.mixed
        default: mic = words.unknown
        }
        let camera: String
        switch protection.camera {
        case "owned": camera = words.restricted
        case "absent": camera = words.unrestricted
        case "system_managed": camera = words.systemManaged
        default: camera = words.unknown
        }
        var paragraphs = [
            "\(words.observed)\n\(summary)",
            frame.diagnostics!,
            "\(words.requested)\n\(words.microphone): \(flag(protection.requested[0]))\n\(words.camera): \(flag(protection.requested[1]))",
            "\(words.effective)\n\(words.microphone): \(mic)\n\(words.camera): \(camera)",
            "\(words.automatic)\n\(words.microphone): \(flag(protection.microphone_on_lock))\n\(words.camera): \(flag(protection.camera_on_lock))",
            words.scope
        ]
        let overview = frame.overview!, preferences = frame.preferences!
        paragraphs.append("overview\nmicrophone: \(overview.microphone)\ncamera: \(overview.camera)\nobserved_at: \(overview.observed_at ?? "null")\ntotal_accesses: \(overview.total_accesses)\nomitted_accesses: \(overview.omitted_accesses)")
        for access in overview.accesses {
            paragraphs.append("\(access.application)\nresource: \(access.resource); activity: \(access.activity); pid: \(access.pid.map(String.init) ?? "null")")
        }
        paragraphs.append("preferences\nnotifications_enabled: \(preferences.notifications_enabled)\nsound_enabled: \(preferences.sound_enabled)\nhistory_enabled: \(preferences.history_enabled)\nnotifications_paused: \(preferences.notifications_paused)")
        if let feedback = frame.feedback { paragraphs.append(feedback) }
        if frame.limited == true { paragraphs.insert(words.limited, at: 0) }
        func appendDetails(_ entries: [DesktopItem]) {
            for entry in entries {
                let full = entry.detail ?? entry.label
                if full != summary && !paragraphs.contains(full) { paragraphs.append(full) }
                if let children = entry.children { appendDetails(children) }
            }
        }
        appendDetails(entries)
        diagnostic = paragraphs.joined(separator: "\n\n")
        super.init()
        menu.font = NSFont.menuFont(ofSize: 0)
        menu.autoenablesItems = false
        let open = NSMenuItem(title: boundedTitle(words.open), action: #selector(showConsultation(_:)), keyEquivalent: "")
        open.target = self
        menu.addItem(open)
        let status = NSMenuItem(title: boundedTitle(summary.components(separatedBy: .newlines).first ?? summary), action: nil, keyEquivalent: "")
        status.isEnabled = false
        menu.addItem(status)
        menu.addItem(.separator())
        for entry in entries where !["header", "status", "camera_inspect", "autostart"].contains(entry.action) {
            if entry.action == "exit" {
                menu.addItem(.separator())
                let about = NSMenuItem(title: boundedTitle(words.about), action: #selector(showAbout(_:)), keyEquivalent: "")
                about.target = self
                menu.addItem(about)
            }
            menu.addItem(makeRow(entry))
        }
        consultation.update(frame, diagnostic: diagnostic)
    }

    private func makeRow(_ entry: DesktopItem) -> NSMenuItem {
        let row = NSMenuItem(title: boundedTitle(entry.label), action: nil, keyEquivalent: "")
        row.isEnabled = entry.enabled
        row.state = entry.checked == true ? .on : .off
        if let children = entry.children {
            let submenu = NSMenu(title: row.title)
            submenu.font = menu.font
            submenu.autoenablesItems = false
            for child in children { submenu.addItem(makeRow(child)) }
            row.submenu = submenu
        } else {
            row.action = #selector(selected(_:))
            row.target = self
            row.representedObject = entry.action
        }
        return row
    }

    private func boundedTitle(_ original: String) -> String {
        var title = original
        if original.contains(where: { $0.isNewline || $0 == "\t" }) {
            title = ""
            title.reserveCapacity(original.utf8.count)
            for character in original {
                if character.isNewline || character == "\t" {
                    title.append(" ")
                } else {
                    title.append(character)
                }
            }
        }
        let font = menu.font ?? NSFont.menuFont(ofSize: 0)
        func fits(_ value: String) -> Bool {
            (value as NSString).size(withAttributes: [.font: font]).width <= textWidth
        }
        if fits(title) { return title }
        let source = title as NSString
        var low = 0, high = source.length
        var result = "…"
        while low <= high {
            let middle = low + (high - low) / 2
            let range = source.rangeOfComposedCharacterSequences(for: NSRange(location: 0, length: middle))
            let candidate = source.substring(with: range) + "…"
            if fits(candidate) {
                result = candidate
                low = middle + 1
            } else {
                high = middle - 1
            }
        }
        return result
    }

    func aboutAlert() -> NSAlert {
        let alert = NSAlert()
        alert.messageText = "MicCamWatch \(version)"
        alert.informativeText = words.description
        alert.icon = mcwBrandImage(size: 64)
        return alert
    }

    @objc private func showConsultation(_ sender: Any?) { consultation.show() }
    @objc private func showAbout(_ sender: Any?) {
        NSApp.activate(ignoringOtherApps: true)
        aboutAlert().runModal()
    }
    @objc private func selected(_ sender: NSMenuItem) {
        if sender.isEnabled, let action = sender.representedObject as? String {
            desktopEmit(DesktopEvent(kind: "action", action: action))
        }
    }
}
private final class DesktopDelegate: NSObject, NSApplicationDelegate {
    private var item: NSStatusItem?
    private var menuContent: DesktopMenu?
    private let consultation = DesktopConsultation()
    private weak var trackingMenu: NSMenu?
    private var stopping = false
    private var initialized = false
    private var pendingClick: DispatchWorkItem?
    func applicationDidFinishLaunching(_ notification: Notification) {
        desktopDiagnostic("didFinishLaunching policy=\(desktopPolicyName(NSApp.activationPolicy()))")
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        guard let item = item, let button = item.button, item.isVisible else {
            let context = desktopGUIContext().detail
            desktopEmit(DesktopEvent(kind: "error", error: "AppKit could not register a visible status item; policy=\(desktopPolicyName(NSApp.activationPolicy())) \(context)"))
            NSApp.terminate(nil)
            return
        }
        button.target = self
        button.action = #selector(clicked(_:))
        button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        button.toolTip = "MicCamWatch"
        button.image = desktopStatusImage("error")
        desktopDiagnostic("statusItem registered visible=\(item.isVisible) buttonWindow=\(button.window != nil)")
        DispatchQueue.global(qos: .utility).async { [weak self] in self?.readFrames() }
    }
    private func readFrames() {
        var frame = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        do {
            while true {
                let count = buffer.withUnsafeMutableBytes { bytes in
                    Darwin.read(STDIN_FILENO, bytes.baseAddress!, bytes.count)
                }
                if count == 0 { break }
                if count < 0 {
                    if errno == EINTR { continue }
                    throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
                }
                for byte in buffer.prefix(count) {
                    if byte == 10 {
                        let decoded = try JSONDecoder().decode(DesktopFrame.self, from: frame)
                        frame.removeAll(keepingCapacity: true)
                        if decoded.kind == "stop" {
                            scheduleStop()
                            return
                        } else {
                            // At most one decoded frame waits on AppKit, including while
                            // the native menu is tracking. No unbounded dispatch backlog.
                            let applied = DispatchSemaphore(value: 0)
                            RunLoop.main.perform(inModes: [.common]) { [weak self] in
                                defer { applied.signal() }
                                if decoded.kind == "show" {
                                    self?.showConsultation()
                                } else { self?.apply(decoded) }
                            }
                            CFRunLoopWakeUp(CFRunLoopGetMain())
                            applied.wait()
                        }
                    } else {
                        guard frame.count < 65536 else { throw NSError(domain: "MicCamWatch", code: 1, userInfo: [NSLocalizedDescriptionKey: "desktop frame exceeds limit"]) }
                        frame.append(byte)
                    }
                }
            }
            if !frame.isEmpty { throw NSError(domain: "MicCamWatch", code: 2, userInfo: [NSLocalizedDescriptionKey: "truncated desktop frame"]) }
            scheduleStop()
        } catch {
            desktopEmit(DesktopEvent(kind: "error", error: error.localizedDescription))
            scheduleStop()
        }
    }
    private func scheduleStop() {
        // NSMenu tracking does not service the main dispatch queue.
        RunLoop.main.perform(inModes: [.common]) { [weak self] in self?.stop() }
        CFRunLoopWakeUp(CFRunLoopGetMain())
    }
    private func apply(_ frame: DesktopFrame) {
        guard !stopping, validDesktopState(frame),
              let summary = frame.summary, let visual = frame.visual else {
            desktopEmit(DesktopEvent(kind: "error", error: "invalid desktop state frame")); stop(); return
        }
        item?.button?.toolTip = summary.components(separatedBy: .newlines).first ?? summary
        item?.button?.image = desktopStatusImage(visual)
        let content = DesktopMenu(frame: frame, consultation: consultation)
        menuContent = content
        if !initialized {
            initialized = true
            desktopEmit(DesktopEvent(kind: "ready", protocol: 4))
        }
    }
    private func showConsultation() {
        guard initialized, !stopping else { return }
        trackingMenu?.cancelTracking()
        consultation.show()
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showConsultation()
        return false
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    @objc private func clicked(_ sender: Any?) {
        guard initialized, !stopping else { return }
        pendingClick?.cancel()
        pendingClick = nil
        if NSApp.currentEvent?.clickCount == 2 {
            showConsultation()
        } else if NSApp.currentEvent?.type == .rightMouseUp {
            showMenu()
        } else {
            // Do not enter menu tracking on click one before AppKit can deliver click two.
            let work = DispatchWorkItem { [weak self] in self?.showMenu() }
            pendingClick = work
            DispatchQueue.main.asyncAfter(deadline: .now() + NSEvent.doubleClickInterval, execute: work)
        }
    }
    private func showMenu() {
        guard !stopping, trackingMenu == nil, let button = item?.button, let content = menuContent else { return }
        // A state frame can replace menuContent during native menu tracking.
        // NSMenuItem targets are weak; retain this snapshot until selection ends.
        trackingMenu = content.menu
        defer { trackingMenu = nil }
        withExtendedLifetime(content) {
            _ = content.menu.popUp(positioning: nil, at: NSPoint(x: 0, y: button.bounds.height), in: button)
        }
    }
    private func stop() {
        guard !stopping else { return }
        stopping = true
        pendingClick?.cancel()
        trackingMenu?.cancelTracking()
        NSApp.abortModal()
        consultation.window.orderOut(nil)
        if let item = item { NSStatusBar.system.removeStatusItem(item) }
        item = nil
        desktopEmit(DesktopEvent(kind: "stopped"))
        NSApp.terminate(nil)
    }
    func applicationWillTerminate(_ notification: Notification) {
        if let item = item { NSStatusBar.system.removeStatusItem(item) }
    }
}
func desktopMain(args: [String]) -> Never {
    guard args.isEmpty, Thread.isMainThread else {
        desktopEmit(DesktopEvent(kind: "error", error: "desktop requires the main thread and no arguments")); Darwin.exit(1)
    }
    let gui = desktopGUIContext()
    guard gui.available else {
        desktopEmit(DesktopEvent(kind: "error", error: "AppKit menu bar requires a Quartz GUI/WindowServer session for this process; \(gui.detail)")); Darwin.exit(1)
    }
    let application = NSApplication.shared
    let initialPolicy = application.activationPolicy()
    let agentBundle = Bundle.main.bundleURL.pathExtension == "app"
    let uiElement = (Bundle.main.object(forInfoDictionaryKey: "LSUIElement") as? NSNumber)?.boolValue
    let context = "\(gui.detail) appBundle=\(agentBundle) LSUIElement=\(uiElement.map { String($0) } ?? "unknown") initialPolicy=\(desktopPolicyName(initialPolicy))"
    // LSUIElement=true already requests accessory policy. The setter reports a
    // policy switch, not status-item registration; do not require a redundant switch.
    if initialPolicy != .accessory {
        let switched = application.setActivationPolicy(.accessory)
        let resultingPolicy = application.activationPolicy()
        desktopDiagnostic("startup \(context) switchAccepted=\(switched) resultingPolicy=\(desktopPolicyName(resultingPolicy))")
        guard switched, resultingPolicy == .accessory else {
            desktopEmit(DesktopEvent(kind: "error", error: "AppKit rejected the accessory activation policy transition; \(context) switchAccepted=\(switched) resultingPolicy=\(desktopPolicyName(resultingPolicy))")); Darwin.exit(1)
        }
    } else {
        desktopDiagnostic("startup \(context) switch=unnecessary")
    }
    let delegate = DesktopDelegate()
    application.delegate = delegate
    withExtendedLifetime(delegate) { application.run() }
    Darwin.exit(0)
}

private func desktopFailure(_ error: String) -> NativeEffectReply {
    NativeEffectReply(ok: false, error: error, state: nil)
}
// Native effect requests are explicit side effects. Passive observation never reaches here.
func desktopEffect(mode: String, args: [String]) -> NativeEffectReply {
    if mode == "log" {
        guard args.count == 1 else { return desktopFailure("log requires a message") }
        let logger = Logger(subsystem: "com.roman-cuisset.miccamwatch.helper", category: "capture")
        logger.notice("\(args[0], privacy: .public)")
        return NativeEffectReply(ok: true, error: nil, state: "accepted")
    }
    if mode == "sound" {
        guard args.isEmpty else { return desktopFailure("sound takes no arguments") }
        guard let sound = NSSound(named: NSSound.Name("Glass")), sound.play() else {
            return desktopFailure("system sound could not be played")
        }
        // Keep this one-shot helper alive while AppKit supplies the audio.
        let deadline = Date().addingTimeInterval(10)
        while sound.isPlaying && Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
        if sound.isPlaying { sound.stop(); return desktopFailure("system sound playback timed out") }
        return NativeEffectReply(ok: true, error: nil, state: "played")
    }
    guard mode == "notify" || mode == "notification-identity" else { return desktopFailure("unknown desktop effect") }
    guard Bundle.main.bundleIdentifier == "com.roman-cuisset.miccamwatch.helper",
          Bundle.main.bundleURL.pathExtension == "app" else {
        return desktopFailure("notification helper must run with its installed stable application identity")
    }
    guard (mode == "notify" && args.count == 2) || (mode == "notification-identity" && args.isEmpty) else {
        return desktopFailure("invalid notification arguments")
    }
    let center = UNUserNotificationCenter.current()
    // Callbacks mutate their result only on the main queue; the main run loop waits boundedly.
    var authorization: Bool? = nil
    var failure: String? = nil
    center.requestAuthorization(options: [.alert, .sound]) { accepted, error in
        DispatchQueue.main.async { authorization = accepted; failure = error?.localizedDescription }
    }
    let deadline = Date().addingTimeInterval(45)
    while authorization == nil && Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
    guard authorization == true else { return desktopFailure(failure ?? (authorization == nil ? "notification authorization timed out" : "notification authorization denied")) }
    if mode == "notification-identity" { return NativeEffectReply(ok: true, error: nil, state: "authorized") }
    let content = UNMutableNotificationContent()
    content.title = args[0]; content.body = args[1]
    let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
    var completed = false
    center.add(request) { error in
        DispatchQueue.main.async { failure = error?.localizedDescription; completed = true }
    }
    let deliveryDeadline = Date().addingTimeInterval(10)
    while !completed && Date() < deliveryDeadline { RunLoop.current.run(until: Date().addingTimeInterval(0.02)) }
    guard completed else { return desktopFailure("notification submission timed out") }
    if let error = failure { return desktopFailure(error) }
    return NativeEffectReply(ok: true, error: nil, state: "submitted")
}
