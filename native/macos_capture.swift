import AVFoundation
import CoreAudio
import CoreMediaIO
import Darwin
import Foundation

// A standalone, embedded helper: never requests camera or microphone permission,
// creates capture sessions, or opens a device. CoreMediaIO running-state queries
// report device-level activity without a client identity or media frame data.
struct AudioProcess: Encodable {
    let pid: Int32?
    let startSeconds: UInt64?
    let startMicroseconds: UInt64?
    let executable: String?
}

struct CameraDevice: Encodable {
    let id: String
    let name: String
    let runningSomewhere: Bool?
}

struct AudioResult: Encodable {
    var processes: [AudioProcess] = []
    var error: String?
    var available: Bool = true
}

struct VideoResult: Encodable {
    var devices: [CameraDevice] = []
    var error: String?
}
struct NativeEffectReply: Encodable {
    let ok: Bool
    let error: String?
    let state: String?
}


struct Output: Encodable {
    var audio: AudioResult?
    var video: VideoResult?
    var microphones: [InventoryDevice]?
    var control: MacControlReply?
    var signature: MacSignatureReply?
    var effect: NativeEffectReply?
}

// libproc's process birth time, read on both sides of the CoreAudio property,
// prevents a recycled PID from inheriting another process's executable/identity.
// A failed lookup deliberately yields no PID or executable in the Rust collector.
func processIdentity(_ pid: Int32) -> (UInt64, UInt64, String)? {
    guard pid > 0 else { return nil }
    var info = proc_bsdinfo()
    let size = Int32(MemoryLayout<proc_bsdinfo>.size)
    let result = withUnsafeMutablePointer(to: &info) { pointer in
        proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, pointer, size)
    }
    guard result == size, info.pbi_pid == UInt32(pid), info.pbi_start_tvsec > 0,
          info.pbi_start_tvusec < 1_000_000 else { return nil }
    var bytes = [CChar](repeating: 0, count: Int(MAXPATHLEN))
    guard proc_pidpath(pid, &bytes, UInt32(bytes.count)) > 0 else { return nil }
    guard let path = bytes.withUnsafeBufferPointer({ buffer in
        buffer.baseAddress.flatMap { String(validatingUTF8: $0) }
    }), path.hasPrefix("/"), !path.isEmpty else { return nil }
    return (UInt64(info.pbi_start_tvsec), UInt64(info.pbi_start_tvusec), path)
}

func audioProcesses() -> AudioResult {
    var result = AudioResult()
    do {
        for process in try AudioHardwareSystem.shared.processes {
            // AudioHardwareProcess.devices describes OUTPUT devices only. It
            // cannot identify an input microphone, even when isRunningInput is true.
            do {
                let pid = try process.pid
                let before = processIdentity(pid)
                guard try process.isRunningInput else { continue }
                let after = processIdentity(pid)
                if let before, let after,
                   before.0 == after.0, before.1 == after.1, before.2 == after.2 {
                    result.processes.append(AudioProcess(pid: pid, startSeconds: before.0,
                                                         startMicroseconds: before.1,
                                                         executable: before.2))
                } else {
                    // The audio activity is real; only its owner is unknown.
                    result.processes.append(AudioProcess(pid: nil, startSeconds: nil,
                                                         startMicroseconds: nil, executable: nil))
                }
            } catch {
                // A failed per-process property could conceal active input.
                result.error = "CoreAudio process property unavailable: \(error)"
            }
        }
    } catch {
        result.available = false
        result.error = "CoreAudio process enumeration failed: \(error)"
    }
    return result
}

private enum MacCaptureError: Error, CustomStringConvertible {
    case failed(String)
    var description: String { switch self { case .failed(let text): return text } }
}

private func cameraRunningSomewhere(_ uid: String) throws -> Bool {
    var property = CMIOObjectPropertyAddress(
        mSelector: CMIOObjectPropertySelector(kCMIOHardwarePropertyDeviceForUID),
        mScope: CMIOObjectPropertyScope(kCMIOObjectPropertyScopeGlobal),
        mElement: CMIOObjectPropertyElement(kCMIOObjectPropertyElementMain))
    var uidString = uid as CFString
    var device = CMIOObjectID(kCMIOObjectUnknown)
    let translationSize = UInt32(MemoryLayout<AudioValueTranslation>.size)
    let deviceSize = UInt32(MemoryLayout<CMIOObjectID>.size)
    var lookupUsed: UInt32 = 0
    var outputSize: UInt32 = 0
    let lookup = withUnsafeMutablePointer(to: &uidString) { input in
        withUnsafeMutablePointer(to: &device) { output in
            var translation = AudioValueTranslation(
                mInputData: UnsafeMutableRawPointer(input),
                mInputDataSize: UInt32(MemoryLayout<CFString>.size),
                mOutputData: UnsafeMutableRawPointer(output),
                mOutputDataSize: deviceSize)
            let status = CMIOObjectGetPropertyData(
                CMIOObjectID(kCMIOObjectSystemObject), &property, 0, nil,
                translationSize, &lookupUsed, &translation)
            outputSize = translation.mOutputDataSize
            return status
        }
    }
    guard lookup == noErr, lookupUsed == translationSize, outputSize == deviceSize,
          device != CMIOObjectID(kCMIOObjectUnknown) else {
        throw MacCaptureError.failed(
            "CoreMediaIO camera UID lookup failed: status=\(lookup), bytes=\(lookupUsed), deviceBytes=\(outputSize), device=\(device)")
    }
    property.mSelector = CMIOObjectPropertySelector(kCMIODevicePropertyDeviceIsRunningSomewhere)
    var running: UInt32 = 0
    var used: UInt32 = 0
    let runningSize = UInt32(MemoryLayout<UInt32>.size)
    let status = CMIOObjectGetPropertyData(device, &property, 0, nil,
                                          runningSize, &used, &running)
    guard status == noErr, used == runningSize else {
        throw MacCaptureError.failed(
            "CoreMediaIO camera running-state read failed: status=\(status), bytes=\(used)")
    }
    return running != 0
}

func videoDevices(observeActivity: Bool) -> VideoResult {
    var result = VideoResult()
    // AVCaptureDevice.devices(for:) is discovery only; no requestAccess(),
    // AVCaptureSession, or lockForConfiguration() is used. On some unattended
    // macOS hosts discovery can report zero devices despite installed cameras.
    let devices = AVCaptureDevice.devices(for: .video)
    for device in devices {
        var running: Bool?
        if observeActivity {
            do {
                running = try cameraRunningSomewhere(device.uniqueID)
            } catch {
                result.error = "\(device.localizedName): \(error)"
            }
        }
        result.devices.append(CameraDevice(id: device.uniqueID, name: device.localizedName,
                                           runningSomewhere: running))
    }
    return result
}

struct InventoryDevice: Encodable {
    let id: String
    let name: String
}

func microphones() -> [InventoryDevice] {
    AVCaptureDevice.devices(for: .audio).map {
        InventoryDevice(id: $0.uniqueID, name: $0.localizedName)
    }
}

func invalidArguments() -> Never {
    fputs("invalid helper mode or arguments; expected capture, control, signature, process effect, notification, sound, log or desktop mode\n", stderr)
    exit(2)
}

let arguments = Array(CommandLine.arguments.dropFirst())
guard let mode = arguments.first,
      arguments.reduce(0, { $0 + $1.utf8.count }) <= 65_536 else { invalidArguments() }
let args = Array(arguments.dropFirst())
// The persistent AppKit protocol is not a one-shot Output envelope.
if mode == "desktop" { desktopMain(args: args) }

var output = Output()
switch mode {
case "audio", "video", "both", "devices":
    guard args.isEmpty else { invalidArguments() }
    if mode == "audio" || mode == "both" { output.audio = audioProcesses() }
    if mode == "video" || mode == "both" || mode == "devices" {
        output.video = videoDevices(observeActivity: mode != "devices")
    }
    if mode == "devices" { output.microphones = microphones() }
case "locale":
    guard args.isEmpty else { invalidArguments() }
    let languageLocale = Locale.preferredLanguages.first.map { Locale(identifier: $0) } ?? Locale.current
    output.effect = NativeEffectReply(ok: true, error: nil, state: languageLocale.language.languageCode?.identifier)
case "control":
    guard args.count == 1 else { invalidArguments() }
    output.control = microphoneControl(args[0])
case "signature":
    guard args.count == 2, args[1] == "offline" || args[1] == "online" else { invalidArguments() }
    output.signature = nativeSignature(path: args[0], online: args[1] == "online")
case "session-lock", "terminate":
    output.effect = processEffect(mode: mode, args: args)
case "notification-identity", "notify", "sound", "log":
    output.effect = desktopEffect(mode: mode, args: args)
default:
    invalidArguments()
}
do {
    let json = try JSONEncoder().encode(output)
    FileHandle.standardOutput.write(json)
} catch {
    fputs("cannot encode native helper response: \(error)\n", stderr)
    exit(1)
}
