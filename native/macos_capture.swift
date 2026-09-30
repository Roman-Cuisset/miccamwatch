import AVFoundation
import CoreAudio
import Darwin
import Foundation

// A standalone, embedded helper: never requests camera or microphone permission,
// creates capture sessions, or opens a device. The camera property is only about
// *another* application and contains no client identity.
struct AudioProcess: Encodable {
    let pid: Int32?
    let startSeconds: UInt64?
    let startMicroseconds: UInt64?
    let executable: String?
}

struct CameraDevice: Encodable {
    let id: String
    let name: String
    let inUse: Bool
}

struct AudioResult: Encodable {
    var processes: [AudioProcess] = []
    var error: String?
    var available: Bool = true
}

struct VideoResult: Encodable {
    var devices: [CameraDevice] = []
    var error: String?
    var interactive: Bool = false
}

struct Output: Encodable {
    var audio: AudioResult?
    var video: VideoResult?
    var microphones: [InventoryDevice]?
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
    guard result == size, info.pbi_start_tvsec > 0 else { return nil }
    var bytes = [CChar](repeating: 0, count: Int(MAXPATHLEN))
    guard proc_pidpath(pid, &bytes, UInt32(bytes.count)) > 0 else { return nil }
    return (UInt64(info.pbi_start_tvsec), UInt64(info.pbi_start_tvusec), String(cString: bytes))
}

func audioProcesses() -> AudioResult {
    var result = AudioResult()
    do {
        for process in try AudioHardwareSystem.shared.processes {
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

func videoDevices() -> VideoResult {
    var result = VideoResult()
    // The Rust collector intentionally supplies /dev/null as standard input.
    // Thus this helper never claims an interactive validation run.
    result.interactive = isatty(STDIN_FILENO) == 1
    // AVCaptureDevice.devices(for:) is discovery only; no requestAccess(),
    // AVCaptureSession, or lockForConfiguration() is used. On some unattended
    // macOS hosts discovery can report zero devices despite installed cameras.
    let devices = AVCaptureDevice.devices(for: .video)
    for device in devices {
        result.devices.append(CameraDevice(id: device.uniqueID, name: device.localizedName,
                                           inUse: device.isInUseByAnotherApplication))
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

let mode = CommandLine.arguments.dropFirst().first
var output = Output()
switch mode {
case "audio": output.audio = audioProcesses()
case "video": output.video = videoDevices()
case "both":
    output.audio = audioProcesses()
    output.video = videoDevices()
case "devices":
    output.video = videoDevices()
    output.microphones = microphones()
default:
    fputs("expected audio, video, both or devices\n", stderr)
    exit(2)
}
do {
    let json = try JSONEncoder().encode(output)
    FileHandle.standardOutput.write(json)
} catch {
    fputs("cannot encode capture observation: \(error)\n", stderr)
    exit(1)
}
