import AppKit
import CoreGraphics
import CoreFoundation
import Foundation
import UserNotifications
import os
import Darwin

// Desktop protocol v3: UTF-8 newline-delimited JSON, maximum frame 65536 bytes.
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
    private enum CodingKeys: String, CodingKey { case kind, summary, diagnostics, limited, visual, items, language, version, protection }
    init(from decoder: Decoder) throws {
        try desktopFields(decoder, allowed: ["kind", "summary", "diagnostics", "limited", "visual", "items", "language", "version", "protection"])
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        kind = try fields.decode(String.self, forKey: .kind)
        summary = try fields.decodeIfPresent(String.self, forKey: .summary)
        diagnostics = try fields.decodeIfPresent(String.self, forKey: .diagnostics)
        limited = try fields.decodeIfPresent(Bool.self, forKey: .limited)
        visual = try fields.decodeIfPresent(String.self, forKey: .visual)
        items = try fields.decodeIfPresent([DesktopItem].self, forKey: .items)
        language = try fields.decodeIfPresent(String.self, forKey: .language)
        version = try fields.decodeIfPresent(String.self, forKey: .version)
        protection = try fields.decodeIfPresent(DesktopProtection.self, forKey: .protection)
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
    let roots = ["status", "mic", "camera", "camera_inspect", "pause", "profile", "autostart", "exit", "header"]
    let choices = Set(["profile_balanced", "profile_private", "profile_meeting", "profile_development"])
    guard ["mic", "camera", "pause", "profile", "autostart"].allSatisfy({ action in entries.filter { $0.action == action }.count == 1 }),
          entries.filter({ $0.action == "camera_inspect" }).count <= 1 else { return false }
    var count = entries.count
    for entry in entries {
        guard roots.contains(entry.action), entry.checked == nil,
              entry.label.utf8.count <= 16384, (entry.detail?.utf8.count ?? 0) <= 16384 else { return false }
        if entry.action == "profile" {
            guard let children = entry.children, children.count == 4,
                  Set(children.map { $0.action }) == choices,
                  children.filter({ $0.checked == true }).count <= 1 else { return false }
            count += children.count
            for child in children {
                guard child.children == nil, child.checked != nil,
                      child.label.utf8.count <= 16384, (child.detail?.utf8.count ?? 0) <= 16384,
                      entry.enabled || !child.enabled else { return false }
            }
        } else if entry.children != nil { return false }
    }
    return count <= 20
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

// One retained, modeless AppKit window. Refresh never opens it and closing it
// only orders out the window: the status item and Rust control owner stay alive.
final class DesktopConsultation: NSObject {
    let window: NSWindow
    let text: NSTextView
    private let actions = NSStackView()
    private var frame: DesktopFrame?
    override init() {
        let screen = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 800, height: 600)
        let size = NSSize(width: min(720, screen.width - 40), height: min(600, screen.height - 60))
        let window = DesktopConsultationWindow(contentRect: NSRect(origin: .zero, size: size),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.title = "MicCamWatch"
        let scroll = NSScrollView(frame: NSRect(origin: .zero, size: size))
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = false
        scroll.borderType = .bezelBorder
        let text = NSTextView(frame: NSRect(origin: .zero, size: scroll.contentSize))
        text.isEditable = false
        text.isSelectable = true
        text.isRichText = false
        text.font = NSFont.systemFont(ofSize: NSFont.systemFontSize)
        // NSTextView otherwise keeps its initial frame as its minimum size.
        // The inset scroll viewport is narrower than the window, and shrinks
        // again on resize; let its document follow that width before wrapping.
        text.minSize = .zero
        text.maxSize = NSSize(width: .greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        text.isVerticallyResizable = true
        text.isHorizontallyResizable = false
        text.autoresizingMask = [.width]
        text.textContainer?.widthTracksTextView = true
        text.textContainer?.lineBreakMode = .byCharWrapping
        text.textContainer?.containerSize = NSSize(width: scroll.contentSize.width, height: .greatestFiniteMagnitude)
        scroll.documentView = text
        self.window = window
        self.text = text
        super.init()
        actions.orientation = .vertical
        actions.alignment = .leading
        actions.spacing = 6
        let root = NSStackView(views: [scroll, actions])
        root.orientation = .vertical
        root.alignment = .leading
        root.spacing = 10
        root.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        root.translatesAutoresizingMaskIntoConstraints = false
        window.contentView!.addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: window.contentView!.leadingAnchor),
            root.trailingAnchor.constraint(equalTo: window.contentView!.trailingAnchor),
            root.topAnchor.constraint(equalTo: window.contentView!.topAnchor),
            root.bottomAnchor.constraint(equalTo: window.contentView!.bottomAnchor),
            scroll.widthAnchor.constraint(equalTo: root.widthAnchor, constant: -24),
            actions.widthAnchor.constraint(equalTo: root.widthAnchor, constant: -24),
            scroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 180)
        ])
        window.minSize = NSSize(width: min(480, size.width), height: min(360, size.height))
        window.center()
    }
    func update(_ frame: DesktopFrame, diagnostic: String) {
        self.frame = frame
        if text.string != diagnostic {
            let selection = text.selectedRanges
            let visible = text.enclosingScrollView?.contentView.bounds.origin
            text.string = diagnostic
            if selection.allSatisfy({ NSMaxRange($0.rangeValue) <= (diagnostic as NSString).length }) {
                text.selectedRanges = selection
            }
            if let scroll = text.enclosingScrollView, let visible = visible {
                scroll.contentView.scroll(to: visible)
                scroll.reflectScrolledClipView(scroll.contentView)
            }
        }
        for view in actions.arrangedSubviews {
            if !frame.items!.contains(where: { $0.action == view.identifier?.rawValue }) {
                actions.removeArrangedSubview(view)
                view.removeFromSuperview()
            }
        }
        for entry in frame.items! where ["camera_inspect", "autostart"].contains(entry.action) {
            let existing = actions.arrangedSubviews.first { $0.identifier?.rawValue == entry.action } as? NSButton
            let button: NSButton
            if let existing = existing { button = existing }
            else {
                button = NSButton(title: entry.label, target: self, action: #selector(selected(_:)))
                button.identifier = NSUserInterfaceItemIdentifier(entry.action)
                button.lineBreakMode = .byWordWrapping
                button.cell?.wraps = true
                button.cell?.isScrollable = false
                button.heightAnchor.constraint(greaterThanOrEqualToConstant: 36).isActive = true
                button.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
                actions.addArrangedSubview(button)
                button.widthAnchor.constraint(lessThanOrEqualTo: actions.widthAnchor).isActive = true
            }
            button.title = entry.label
            button.isEnabled = entry.enabled
            button.toolTip = entry.detail ?? entry.label
        }
    }
    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(text)
    }
    @objc private func selected(_ sender: NSButton) {
        guard let action = sender.identifier?.rawValue,
              frame?.items?.contains(where: { $0.action == action && $0.enabled }) == true else { return }
        desktopEmit(DesktopEvent(kind: "action", action: action))
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
        if let action = sender.representedObject as? String {
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
                            guard decoded.summary == nil, decoded.items == nil, decoded.protection == nil,
                                  decoded.language == nil, decoded.version == nil, decoded.visual == nil,
                                  decoded.diagnostics == nil, decoded.limited == nil else {
                                throw NSError(domain: "MicCamWatch", code: 3, userInfo: [NSLocalizedDescriptionKey: "invalid stop frame"])
                            }
                            scheduleStop()
                            return
                        } else {
                            // At most one decoded frame waits on AppKit, including while
                            // the native menu is tracking. No unbounded dispatch backlog.
                            let applied = DispatchSemaphore(value: 0)
                            RunLoop.main.perform(inModes: [.common]) { [weak self] in
                                defer { applied.signal() }
                                if decoded.kind == "show", decoded.summary == nil, decoded.items == nil,
                                   decoded.protection == nil, decoded.language == nil,
                                   decoded.version == nil, decoded.visual == nil,
                                   decoded.diagnostics == nil, decoded.limited == nil {
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
            desktopEmit(DesktopEvent(kind: "ready", protocol: 3))
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
