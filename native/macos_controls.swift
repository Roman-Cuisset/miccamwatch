import AppKit
import CoreAudio
import Darwin
import Foundation

// Public device INPUT mute properties only. Gain and process-input mute are not
// substitutes for a hardware/virtual device's actual mute property.
struct MacControlIdentity: Codable, Hashable {
    let uid: String
    let selector: UInt32
    let scope: UInt32
    let element: UInt32
}
struct MacMuteControl: Codable {
    let identity: MacControlIdentity
    let name: String
    let muted: Bool
    let writable: Bool
}
struct MacUnsupportedInput: Codable {
    let uid: String
    let name: String
    let reason: String
}
struct MacControlChange: Codable {
    let identity: MacControlIdentity
    let muted: Bool
}
struct MacControlResult: Codable {
    let identity: MacControlIdentity
    let ok: Bool
    let muted: Bool?
    let error: String?
}
struct MacCameraProfile: Codable {
    var state: String = "unknown"
    var ownedInstalled: Bool = false
    var completeInventory: Bool = false
    var detail: String = "Camera restriction status is unknown."
}
struct MacControlReply: Codable {
    var ok: Bool = true
    var error: String?
    var controls: [MacMuteControl] = []
    var unsupported: [MacUnsupportedInput] = []
    var results: [MacControlResult] = []
    var camera: MacCameraProfile?
}
private struct MacControlRequest: Decodable {
    let operation: String
    var changes: [MacControlChange]?
    var identifier: String?
    var uuid: String?
    var path: String?
}
private enum MacControlError: Error, CustomStringConvertible {
    case failed(String)
    var description: String { switch self { case .failed(let text): return text } }
}
private func controlAddress(_ selector: UInt32, _ scope: UInt32 = kAudioObjectPropertyScopeGlobal,
                            _ element: UInt32 = kAudioObjectPropertyElementMain) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(mSelector: selector, mScope: scope, mElement: element)
}
private func controlScalar(_ device: AudioObjectID, _ address: inout AudioObjectPropertyAddress) throws -> UInt32 {
    var value: UInt32 = 0
    var size = UInt32(MemoryLayout<UInt32>.size)
    let status = AudioObjectGetPropertyData(device, &address, 0, nil, &size, &value)
    guard status == noErr, size == MemoryLayout<UInt32>.size else {
        throw MacControlError.failed("CoreAudio property read failed: \(status)")
    }
    return value
}
private func controlString(_ device: AudioObjectID, _ selector: UInt32) throws -> String {
    var address = controlAddress(selector)
    var value: CFString = "" as CFString
    var size = UInt32(MemoryLayout<CFString>.size)
    let status = AudioObjectGetPropertyData(device, &address, 0, nil, &size, &value)
    guard status == noErr else { throw MacControlError.failed("CoreAudio identity read failed: \(status)") }
    return value as String
}
private func controlDevices() throws -> [AudioObjectID] {
    var address = controlAddress(kAudioHardwarePropertyDevices)
    var size: UInt32 = 0
    let status = AudioObjectGetPropertyDataSize(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size)
    guard status == noErr, size % UInt32(MemoryLayout<AudioObjectID>.size) == 0 else {
        throw MacControlError.failed("CoreAudio device enumeration failed: \(status)")
    }
    var devices = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
    if size > 0 {
        let read = devices.withUnsafeMutableBytes {
            AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, $0.baseAddress!)
        }
        guard read == noErr else { throw MacControlError.failed("CoreAudio device enumeration failed: \(read)") }
    }
    return devices
}
private func inputChannelCount(_ device: AudioObjectID) throws -> UInt32 {
    var address = controlAddress(kAudioDevicePropertyStreamConfiguration, kAudioDevicePropertyScopeInput)
    var size: UInt32 = 0
    let status = AudioObjectGetPropertyDataSize(device, &address, 0, nil, &size)
    guard status == noErr, size >= MemoryLayout<UInt32>.size else {
        throw MacControlError.failed("CoreAudio input stream configuration unavailable: \(status)")
    }
    let memory = UnsafeMutableRawPointer.allocate(byteCount: max(Int(size), MemoryLayout<AudioBufferList>.size), alignment: MemoryLayout<AudioBufferList>.alignment)
    defer { memory.deallocate() }
    let read = AudioObjectGetPropertyData(device, &address, 0, nil, &size, memory)
    guard read == noErr else { throw MacControlError.failed("CoreAudio input stream configuration failed: \(read)") }
    let buffers = UnsafeMutableAudioBufferListPointer(memory.assumingMemoryBound(to: AudioBufferList.self))
    return buffers.reduce(0) { $0 + $1.mNumberChannels }
}
private func muteInventory() throws -> MacControlReply {
    var reply = MacControlReply()
    for device in try controlDevices() {
        let name = (try? controlString(device, kAudioObjectPropertyName)) ?? "CoreAudio device \(device)"
        let uid: String
        let channels: UInt32
        do {
            channels = try inputChannelCount(device)
            guard channels > 0 else { continue }
            uid = try controlString(device, kAudioDevicePropertyDeviceUID)
            guard !uid.isEmpty else { throw MacControlError.failed("Empty device UID") }
        } catch {
            reply.unsupported.append(MacUnsupportedInput(uid: "", name: name, reason: String(describing: error)))
            continue
        }
        var writableCount = 0
        var errors: [String] = []
        // A master mute and channel mutes may coexist. Preserve every real control
        // independently instead of treating an output/default device as an input.
        for channel in 0...channels {
            var address = controlAddress(kAudioDevicePropertyMute, kAudioDevicePropertyScopeInput, channel)
            guard AudioObjectHasProperty(device, &address) else { continue }
            var settable = DarwinBoolean(false)
            let status = AudioObjectIsPropertySettable(device, &address, &settable)
            do {
                guard status == noErr else { throw MacControlError.failed("Mute capability query failed: \(status)") }
                let muted = try controlScalar(device, &address) != 0
                let writable = settable.boolValue
                reply.controls.append(MacMuteControl(identity: MacControlIdentity(uid: uid,
                    selector: address.mSelector, scope: address.mScope, element: channel),
                    name: name, muted: muted, writable: writable))
                if writable { writableCount += 1 }
            } catch { errors.append("channel \(channel): \(error)") }
        }
        if writableCount == 0 || !errors.isEmpty {
            reply.unsupported.append(MacUnsupportedInput(uid: uid, name: name,
                reason: errors.isEmpty ? "No writable INPUT mute property (master or channel)" : errors.joined(separator: "; ")))
        }
    }
    return reply
}
private func setInputMute(_ change: MacControlChange) -> MacControlResult {
    do {
        guard change.identity.selector == kAudioDevicePropertyMute,
              change.identity.scope == kAudioDevicePropertyScopeInput else {
            throw MacControlError.failed("Only device INPUT mute properties are supported")
        }
        let matches = try controlDevices().filter { (try? controlString($0, kAudioDevicePropertyDeviceUID)) == change.identity.uid }
        guard matches.count == 1, let device = matches.first else {
            throw MacControlError.failed("Device UID is disconnected or ambiguous: \(change.identity.uid)")
        }
        let channels = try inputChannelCount(device)
        guard channels > 0, change.identity.element <= channels else {
            throw MacControlError.failed("Input mute channel no longer exists")
        }
        var address = controlAddress(change.identity.selector, change.identity.scope, change.identity.element)
        var writable = DarwinBoolean(false)
        guard AudioObjectHasProperty(device, &address),
              AudioObjectIsPropertySettable(device, &address, &writable) == noErr, writable.boolValue else {
            throw MacControlError.failed("Input mute control is no longer writable")
        }
        var value: UInt32 = change.muted ? 1 : 0
        let status = AudioObjectSetPropertyData(device, &address, 0, nil, UInt32(MemoryLayout<UInt32>.size), &value)
        guard status == noErr else { throw MacControlError.failed("CoreAudio mute write failed: \(status)") }
        let actual = try controlScalar(device, &address) != 0
        guard actual == change.muted else { throw MacControlError.failed("CoreAudio mute readback did not match requested value") }
        return MacControlResult(identity: change.identity, ok: true, muted: actual, error: nil)
    } catch {
        return MacControlResult(identity: change.identity, ok: false, muted: nil, error: String(describing: error))
    }
}

// profiles(1) documents stdout-xml and ProfileIdentifier/ProfileUUID metadata.
// A non-root query cannot prove absence of device/other-user restrictions.
private func installedCameraProfile(_ identifier: String?, _ uuid: String?) throws -> MacCameraProfile {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/profiles")
    process.arguments = ["show", "-type", "configuration", "-output", "stdout-xml"] + (geteuid() == 0 ? ["-all"] : [])
    process.standardInput = FileHandle.nullDevice
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = FileHandle.nullDevice
    try process.run()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0, data.count <= 1024 * 1024 else {
        throw MacControlError.failed("Installed profile metadata could not be read (profiles exit \(process.terminationStatus))")
    }
    let plist = try PropertyListSerialization.propertyList(from: data, options: [], format: nil)
    guard let scopes = plist as? [String: Any], scopes.values.allSatisfy({ $0 is [Any] }) else {
        throw MacControlError.failed("Unrecognized installed profile metadata schema")
    }
    var result = MacCameraProfile()
    result.completeInventory = geteuid() == 0
    var restricted = false
    for list in scopes.values {
        for entry in list as! [Any] {
            guard let profile = entry as? [String: Any],
                  let profileID = profile["ProfileIdentifier"] as? String,
                  let profileUUID = profile["ProfileUUID"] as? String,
                  let items = profile["ProfileItems"] as? [[String: Any]] else {
                throw MacControlError.failed("Installed profile payload metadata is incomplete")
            }
            let owned = identifier == profileID && uuid?.caseInsensitiveCompare(profileUUID) == .orderedSame
            if owned { result.ownedInstalled = true }
            for item in items {
                // ProfileItems contains the native PayloadContent dictionary.
                guard let content = item["PayloadContent"] as? [String: Any] else {
                    if item["PayloadType"] as? String == "com.apple.applicationaccess" {
                        throw MacControlError.failed("Restrictions payload contents are not visible")
                    }
                    continue
                }
                let type = (item["PayloadType"] as? String) ?? (content["PayloadType"] as? String)
                guard type == "com.apple.applicationaccess" else { continue }
                if let camera = content["allowCamera"] as? NSNumber, CFGetTypeID(camera) == CFBooleanGetTypeID(), !camera.boolValue {
                    restricted = true
                    if owned { result.state = "blocked" }
                }
            }
        }
    }
    if restricted {
        if result.state != "blocked" { result.state = "system_managed" }
        result.detail = "Installed Restrictions payload sets allowCamera=false; this is profile restriction evidence, not physical capture proof."
    } else if result.completeInventory {
        result.state = "allowed"
        result.detail = "Complete installed-profile inventory contains no allowCamera=false restriction; per-app TCC permission and physical camera availability are not asserted."
    } else {
        result.detail = "Current-user profiles contain no visible camera restriction; device/other-user scope is not readable, so effective status is unknown."
    }
    return result
}
private func approveCameraUI(_ detail: String) throws {
    // No documented public SDK predicate proves an unlocked console. A live
    // user response is required before opening any profile/action UI: a locked
    // session cannot silently approve this modal sheet. Parent bounds its life.
    _ = NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    NSApp.activate(ignoringOtherApps: true)
    let alert = NSAlert()
    alert.messageText = "MicCamWatch Camera Restriction"
    alert.informativeText = detail
    alert.addButton(withTitle: "Open System Settings")
    alert.addButton(withTitle: "Cancel")
    guard alert.runModal() == .alertFirstButtonReturn else {
        throw MacControlError.failed("Camera approval workflow canceled; camera status was not changed")
    }
}
private func cameraControl(_ request: MacControlRequest) throws -> MacControlReply {
    var reply = MacControlReply()
    switch request.operation {
    case "camera_status": reply.camera = try installedCameraProfile(request.identifier, request.uuid)
    case "camera_open":
        guard let path = request.path, path.hasSuffix(".mobileconfig"), FileManager.default.fileExists(atPath: path) else {
            throw MacControlError.failed("Camera restriction profile file is unavailable")
        }
        try approveCameraUI("Open the prepared restriction profile for manual installation. Camera access is not blocked until System Settings approves installation and installed metadata confirms allowCamera=false.")
        guard NSWorkspace.shared.open(URL(fileURLWithPath: path)) else {
            throw MacControlError.failed("System Settings could not open the camera restriction profile")
        }
        reply.camera = MacCameraProfile(detail: "Profile opened for manual installation approval; camera restriction is pending, not blocked.")
    case "camera_remove":
        // System Settings owns approval/authentication. Never remove all profiles
        // or use a silent privileged helper. The user removes only the named UUID.
        guard let identifier = request.identifier, let uuid = request.uuid,
              identifier.hasPrefix("com.roman-cuisset.miccamwatch.camera."), UUID(uuidString: uuid) != nil else {
            throw MacControlError.failed("Owned camera profile identity is unavailable")
        }
        let before = try installedCameraProfile(identifier, uuid)
        guard before.ownedInstalled else { reply.camera = before; return reply }
        try approveCameraUI("Remove only MicCamWatch Camera Restriction (\(identifier), UUID \(uuid)) in Device Management. No other profiles are changed. Removal still requires your System Settings approval.")
        guard NSWorkspace.shared.open(URL(fileURLWithPath: "/System/Applications/System Settings.app")) else {
            throw MacControlError.failed("System Settings Device Management could not be opened")
        }
        reply.camera = before
        reply.camera?.detail = "Removal approval pending: in System Settings > General > Device Management, remove only MicCamWatch Camera Restriction (\(identifier), UUID \(uuid)). Other profiles are preserved."
    default: throw MacControlError.failed("Unsupported camera operation")
    }
    return reply
}
func microphoneControl(_ requestJSON: String) -> MacControlReply {
    do {
        let request = try JSONDecoder().decode(MacControlRequest.self, from: Data(requestJSON.utf8))
        if request.operation.hasPrefix("camera_") { return try cameraControl(request) }
        switch request.operation {
        case "inventory": return try muteInventory()
        case "set":
            guard let changes = request.changes, !changes.isEmpty else { throw MacControlError.failed("No input mute controls requested") }
            var reply = MacControlReply()
            reply.results = changes.map(setInputMute)
            reply.ok = reply.results.allSatisfy { $0.ok }
            if !reply.ok { reply.error = "One or more device INPUT mute controls failed; successful writes are retained." }
            return reply
        default: throw MacControlError.failed("Unsupported control operation")
        }
    } catch { return MacControlReply(ok: false, error: String(describing: error)) }
}
