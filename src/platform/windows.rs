use crate::{
    collector::{CaptureCollector, CaptureScope},
    config::{Policy, Profile, TrustPolicy},
    model::{
        Access, Activity, CollectorHealth, CollectorState, Confidence, Device, DiagnosticCheck,
        DiagnosticStatus, EnforcementDecision, Evidence, EvidenceKind, MicrophoneMuteState,
        ProcessAncestor, ProcessContext, Resource, Risk, SignatureInfo, Snapshot,
    },
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io,
    mem::size_of,
    path::Path,
    ptr, slice,
    sync::mpsc::Sender,
};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, NTSTATUS},
        Globalization::{CSTR_EQUAL, CompareStringOrdinal},
        Media::{
            Audio::{
                AudioSessionStateActive, DEVICE_STATE_ACTIVE, Endpoints::IAudioEndpointVolume,
                IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
                IAudioSessionNotification, IAudioSessionNotification_Impl, IMMDevice,
                IMMDeviceEnumerator, MMDeviceEnumerator, eCapture,
            },
            MediaFoundation::{
                IMFActivate, IMFAttributes, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
                MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION,
                MFCreateAttributes, MFEnumDeviceSources, MFSTARTUP_FULL, MFShutdown, MFStartup,
            },
        },
        Security::{
            Cryptography::{
                CERT_CONTEXT, CERT_NAME_SIMPLE_DISPLAY_TYPE,
                CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED, CERT_QUERY_CONTENT_TYPE,
                CERT_QUERY_ENCODING_TYPE, CERT_QUERY_FORMAT_FLAG_ALL, CERT_QUERY_FORMAT_TYPE,
                CERT_QUERY_OBJECT_FILE, CertFreeCertificateContext, CertGetNameStringW,
                CryptQueryObject,
            },
            GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, LookupAccountSidW,
            SID_NAME_USE, TOKEN_INFORMATION_CLASS, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER,
            TokenIntegrityLevel, TokenUser,
            WinTrust::{
                WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
                WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
                WTD_REVOKE_WHOLECHAIN, WTD_SAFER_FLAG, WTD_STATEACTION_IGNORE, WTD_UI_NONE,
                WinVerifyTrust,
            },
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize, STGM_READ, StructuredStorage::PropVariantToStringAlloc,
            },
            Diagnostics::{
                Debug::MessageBeep,
                ToolHelp::{
                    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW,
                    PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPMODULE,
                    TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
                },
            },
            RemoteDesktop::ProcessIdToSessionId,
            StationsAndDesktops::{
                CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS,
                GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
            },
            Threading::{
                GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
                TerminateProcess,
            },
        },
        UI::WindowsAndMessaging::MB_ICONASTERISK,
    },
    core::{Interface, PCWSTR, PWSTR},
};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};
const CONSENT_STORE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
const WINDOWS_TO_UNIX_EPOCH_100NS: i128 = 116_444_736_000_000_000;

/// DLL combinations that indicate a camera-capable capture pipeline is loaded.
/// They do not prove that frames are currently flowing.
const CAPTURE_MODULES: &[&str] = &[
    "kswdmcap.ax",
    "ksproxy.ax",
    "mfcaptureengine.dll",
    "mfsensorgroup.dll",
];

/// Known browser executable names (lowercase).
const BROWSERS: &[&str] = &[
    "msedge.exe",
    "chrome.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "chromium.exe",
    "iexplore.exe",
];

/// Known video-call / streaming applications (lowercase).
const VIDEO_APPS: &[&str] = &[
    "teams.exe",
    "zoom.exe",
    "obs64.exe",
    "obs32.exe",
    "telegram.exe",
    "discord.exe",
    "slack.exe",
    "skype.exe",
    "whatsapp.exe",
    "signal.exe",
    "webex.exe",
    "gotomeeting.exe",
    "ktalk.exe",
];

#[repr(C)]
struct UNICODE_STRING {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

windows::core::link!("ntdll.dll" "system" fn NtQueryInformationProcess(
    process_handle: HANDLE,
    process_information_class: u32,
    process_information: *mut core::ffi::c_void,
    process_information_length: u32,
    return_length: *mut u32,
) -> NTSTATUS);

pub struct PlatformMonitor {
    enumerator: IMMDeviceEnumerator,
    policy: Policy,
    _media_foundation: MediaFoundationGuard,
    _com: ComGuard,
}

fn healthy(collector: &'static str) -> CollectorHealth {
    CollectorHealth {
        collector,
        state: CollectorState::Healthy,
        detail: None,
    }
}

fn unhealthy(
    collector: &'static str,
    state: CollectorState,
    error: &anyhow::Error,
) -> CollectorHealth {
    CollectorHealth {
        collector,
        state,
        detail: Some(format!("{error:#}")),
    }
}

impl PlatformMonitor {
    pub fn new(policy: Policy) -> Result<Self> {
        let com = ComGuard::new()?;
        let media_foundation = MediaFoundationGuard::new()?;
        let enumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .context("failed to create the Windows audio device enumerator")?
        };
        Ok(Self {
            enumerator,
            policy,
            _media_foundation: media_foundation,
            _com: com,
        })
    }

    pub fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot> {
        let mut accesses = Vec::new();
        let mut collectors = Vec::new();
        if scope.microphone {
            match self.microphone_accesses() {
                Ok(found) => {
                    accesses.extend(found);
                    collectors.push(healthy("wasapi"));
                }
                Err(error) => {
                    collectors.push(unhealthy("wasapi", CollectorState::Unavailable, &error))
                }
            }
        }
        if scope.camera {
            match self.camera_accesses() {
                Ok(found) => {
                    accesses.extend(found);
                    collectors.push(healthy("privacy_store"));
                    collectors.push(healthy("module_scanner"));
                }
                Err(error) => {
                    collectors.push(unhealthy(
                        "privacy_store",
                        CollectorState::Unavailable,
                        &error,
                    ));
                    collectors.push(unhealthy(
                        "module_scanner",
                        CollectorState::Unavailable,
                        &error,
                    ));
                }
            }
        }
        if !scope.include_ready {
            accesses.retain(|access| access.activity == Activity::Active);
        }
        let incomplete_context = accesses
            .iter()
            .filter_map(|access| access.process.as_ref())
            .any(|process| process.user.is_none() || process.integrity.is_none());
        collectors.push(CollectorHealth {
            collector: "process_context",
            state: if incomplete_context {
                CollectorState::Degraded
            } else {
                CollectorState::Healthy
            },
            detail: incomplete_context.then(|| {
                "User or integrity information was inaccessible for at least one process."
                    .to_owned()
            }),
        });
        // Profile-based risk escalation
        match self.policy.profile {
            Profile::Strict => {
                for access in &mut accesses {
                    if access.risk == Risk::Unexplained {
                        access.risk = Risk::Suspicious;
                    }
                }
            }
            Profile::Conservative => {}
            Profile::Balanced => {}
        }
        accesses.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(Snapshot {
            collectors,
            accesses,
        })
    }

    pub fn register_audio_notifications(
        &self,
        sender: Sender<()>,
    ) -> Result<AudioNotificationGuard> {
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .context("failed to enumerate microphone devices for notifications")?
        };
        let count = unsafe { collection.GetCount()? };
        let mut registrations = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = unsafe { collection.Item(index)? };
            let manager: IAudioSessionManager2 = unsafe { device.Activate(CLSCTX_ALL, None)? };
            let callback: IAudioSessionNotification = AudioSessionNotifier {
                sender: sender.clone(),
            }
            .into();
            unsafe { manager.RegisterSessionNotification(&callback)? };
            registrations.push((manager, callback));
        }
        Ok(AudioNotificationGuard { registrations })
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .context("failed to enumerate microphone devices")?
        };
        let count = unsafe { collection.GetCount()? };
        let mut devices = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = unsafe { collection.Item(index)? };
            let id = device_id(&device)?;
            let name = device_name(&device).unwrap_or_else(|_| id.clone());
            devices.push(Device {
                resource: Resource::Microphone,
                id,
                name,
            });
        }
        devices.extend(camera_devices()?);
        Ok(devices)
    }

    pub fn microphone_mute_state(&self) -> Result<MicrophoneMuteState> {
        let endpoints = self.microphone_volumes()?;
        if endpoints.is_empty() {
            return Ok(MicrophoneMuteState::Unavailable);
        }
        let muted = endpoints
            .iter()
            .map(|volume| unsafe { volume.GetMute().map(|value| value.as_bool()) })
            .collect::<windows::core::Result<Vec<_>>>()?;
        Ok(if muted.iter().all(|value| *value) {
            MicrophoneMuteState::Muted
        } else if muted.iter().all(|value| !*value) {
            MicrophoneMuteState::Unmuted
        } else {
            MicrophoneMuteState::Mixed
        })
    }

    fn microphone_volumes(&self) -> Result<Vec<IAudioEndpointVolume>> {
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .context("failed to enumerate microphone devices")?
        };
        let count = unsafe { collection.GetCount()? };
        let mut volumes = Vec::with_capacity(count as usize);
        for index in 0..count {
            let device = unsafe { collection.Item(index)? };
            volumes.push(unsafe { device.Activate(CLSCTX_ALL, None)? });
        }
        Ok(volumes)
    }

    pub fn set_microphone_mute(&self, mute: bool) -> Result<usize> {
        let endpoints = self.microphone_volumes()?;
        if endpoints.is_empty() {
            anyhow::bail!("no active microphone capture device was found");
        }
        let original = endpoints
            .iter()
            .map(|volume| unsafe { volume.GetMute().map(|value| value.as_bool()) })
            .collect::<windows::core::Result<Vec<_>>>()?;
        for (index, volume) in endpoints.iter().enumerate() {
            if let Err(error) = unsafe { volume.SetMute(mute, ptr::null()) } {
                for (changed, was_muted) in endpoints[..index].iter().zip(&original[..index]) {
                    let _ = unsafe { changed.SetMute(*was_muted, ptr::null()) };
                }
                return Err(error).context(
                    "failed to change microphone mute state; prior devices were restored",
                );
            }
        }
        Ok(endpoints.len())
    }

    pub fn toggle_microphone_mute(&self) -> Result<bool> {
        let mute = match self.microphone_mute_state()? {
            MicrophoneMuteState::Unavailable => {
                anyhow::bail!("no active microphone capture device was found")
            }
            MicrophoneMuteState::Muted => false,
            MicrophoneMuteState::Unmuted | MicrophoneMuteState::Mixed => true,
        };
        self.set_microphone_mute(mute)?;
        Ok(mute)
    }

    pub fn doctor(&self) -> Vec<DiagnosticCheck> {
        let mut checks = Vec::new();

        // Windows version
        let version = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
            .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .and_then(|key| key.get_value::<String, _>("CurrentBuild"));
        checks.push(match version {
            Ok(build) => DiagnosticCheck {
                name: "windows_version",
                status: DiagnosticStatus::Ok,
                detail: format!("Windows build {build}"),
            },
            Err(error) => DiagnosticCheck {
                name: "windows_version",
                status: DiagnosticStatus::Warning,
                detail: format!("cannot read Windows version: {error}"),
            },
        });

        // COM
        checks.push(DiagnosticCheck {
            name: "com_runtime",
            status: DiagnosticStatus::Ok,
            detail: "COM initialized successfully".into(),
        });

        // Media Foundation
        checks.push(DiagnosticCheck {
            name: "media_foundation",
            status: DiagnosticStatus::Ok,
            detail: "Media Foundation initialized successfully".into(),
        });

        // Audio capture devices
        let mic_count = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .and_then(|collection| collection.GetCount())
        };
        checks.push(match mic_count {
            Ok(count) => DiagnosticCheck {
                name: "microphone_devices",
                status: if count > 0 {
                    DiagnosticStatus::Ok
                } else {
                    DiagnosticStatus::Warning
                },
                detail: format!("{count} capture device(s) detected"),
            },
            Err(error) => DiagnosticCheck {
                name: "microphone_devices",
                status: DiagnosticStatus::Error,
                detail: format!("failed to enumerate: {error}"),
            },
        });

        // Camera devices
        match camera_devices() {
            Ok(cams) => checks.push(DiagnosticCheck {
                name: "camera_devices",
                status: if cams.is_empty() {
                    DiagnosticStatus::Warning
                } else {
                    DiagnosticStatus::Ok
                },
                detail: format!("{} camera device(s) detected", cams.len()),
            }),
            Err(error) => checks.push(DiagnosticCheck {
                name: "camera_devices",
                status: DiagnosticStatus::Error,
                detail: format!("failed to enumerate: {error}"),
            }),
        }

        // ConsentStore registry
        let consent =
            RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!(r"{CONSENT_STORE}\webcam"));
        checks.push(match consent {
            Ok(_) => DiagnosticCheck {
                name: "consent_store",
                status: DiagnosticStatus::Ok,
                detail: "Camera ConsentStore registry is accessible".into(),
            },
            Err(error) => DiagnosticCheck {
                name: "consent_store",
                status: DiagnosticStatus::Warning,
                detail: format!("cannot open ConsentStore: {error}"),
            },
        });

        let executable = std::env::current_exe();
        checks.push(match executable {
            Ok(path) => {
                let installed = std::env::var_os("LOCALAPPDATA")
                    .map(std::path::PathBuf::from)
                    .map(|root| path.starts_with(root.join("Programs").join("MicCamWatch")))
                    .unwrap_or(false);
                DiagnosticCheck {
                    name: "installation",
                    status: if installed {
                        DiagnosticStatus::Ok
                    } else {
                        DiagnosticStatus::Warning
                    },
                    detail: if installed {
                        format!("installed executable: {}", path.display())
                    } else {
                        format!("portable executable: {}", path.display())
                    },
                }
            }
            Err(error) => DiagnosticCheck {
                name: "installation",
                status: DiagnosticStatus::Warning,
                detail: format!("cannot locate executable: {error}"),
            },
        });

        checks.push(match crate::autostart::state() {
            Ok(state) => DiagnosticCheck {
                name: "autostart",
                status: DiagnosticStatus::Ok,
                detail: format!("{state:?}").to_ascii_lowercase(),
            },
            Err(error) => DiagnosticCheck {
                name: "autostart",
                status: DiagnosticStatus::Warning,
                detail: format!("cannot inspect autostart: {error:#}"),
            },
        });

        let notification_identity = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Classes\AppUserModelId\MicCamWatch.MicCamWatch");
        checks.push(match notification_identity {
            Ok(_) => DiagnosticCheck {
                name: "notification_identity",
                status: DiagnosticStatus::Ok,
                detail: "MicCamWatch.MicCamWatch is registered".into(),
            },
            Err(error) => DiagnosticCheck {
                name: "notification_identity",
                status: DiagnosticStatus::Warning,
                detail: format!("notification identity is not registered: {error}"),
            },
        });

        // Policy
        checks.push(DiagnosticCheck {
            name: "policy",
            status: DiagnosticStatus::Ok,
            detail: format!(
                "profile={:?}, trust={:?}, {} application rule(s)",
                self.policy.profile,
                self.policy.trust_policy,
                self.policy.applications.len()
            ),
        });

        checks
    }

    fn microphone_accesses(&self) -> Result<Vec<Access>> {
        let collection = unsafe {
            self.enumerator
                .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
                .context("failed to enumerate microphone devices")?
        };
        let device_count = unsafe { collection.GetCount()? };
        let processes = ProcessTable::load();
        let mut accesses = HashMap::new();

        for device_index in 0..device_count {
            let device = unsafe { collection.Item(device_index)? };
            let id = device_id(&device)?;
            let name = device_name(&device).unwrap_or_else(|_| id.clone());
            let manager: IAudioSessionManager2 = unsafe {
                device
                    .Activate(CLSCTX_ALL, None)
                    .context("failed to inspect a microphone device")?
            };
            let sessions = unsafe { manager.GetSessionEnumerator()? };
            let session_count = unsafe { sessions.GetCount()? };

            for session_index in 0..session_count {
                let control = unsafe { sessions.GetSession(session_index)? };
                if unsafe { control.GetState()? } != AudioSessionStateActive {
                    continue;
                }
                let control: IAudioSessionControl2 = control.cast()?;
                let pid = unsafe { control.GetProcessId()? };
                if pid == 0 {
                    continue;
                }
                let executable = process_path(pid);
                let application = executable
                    .as_deref()
                    .and_then(|path| Path::new(path).file_name())
                    .and_then(OsStr::to_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("PID {pid}"));

                let context = processes.context(pid);
                let (parent_pid, parent_name) = immediate_parent(&context)
                    .map_or((None, None), |(ppid, name)| (Some(ppid), Some(name)));

                let signature = executable
                    .as_deref()
                    .map(|path| self.signature_for_path(path));
                let mut evidence = vec![Evidence::new(
                    EvidenceKind::LiveApi,
                    "WASAPI",
                    "An active capture audio session is attributed to this PID.",
                )];
                let (risk, enforcement) = assess_policy(
                    &self.policy,
                    &application,
                    executable.as_deref(),
                    signature.as_ref(),
                    &mut evidence,
                );
                let key = format!("microphone:{id}:{}", process_identity(pid));
                accesses.entry(key.clone()).or_insert(Access {
                    key,
                    resource: Resource::Microphone,
                    activity: Activity::Active,
                    risk,
                    confidence: Confidence::High,
                    enforcement,
                    application,
                    pid: Some(pid),
                    parent_pid,
                    parent_name,
                    executable,
                    signature,
                    device: Some(name.clone()),
                    started_at: None,
                    modules: Vec::new(),
                    evidence,
                    process: Some(context),
                });
            }
        }

        Ok(accesses.into_values().collect())
    }

    fn signature_for_path(&self, path: &str) -> SignatureInfo {
        signature_for_path(path, self.policy.trust_policy, verify_signature_with_policy)
    }

    fn camera_accesses(&self) -> Result<Vec<Access>> {
        camera_accesses(&self.policy, |path| self.signature_for_path(path))
    }
}

impl CaptureCollector for PlatformMonitor {
    fn snapshot(&self, scope: CaptureScope) -> Result<Snapshot> {
        PlatformMonitor::snapshot(self, scope)
    }

    fn devices(&self) -> Result<Vec<Device>> {
        PlatformMonitor::devices(self)
    }

    fn diagnostics(&self) -> Vec<DiagnosticCheck> {
        self.doctor()
    }
}

#[windows::core::implement(IAudioSessionNotification)]
struct AudioSessionNotifier {
    sender: Sender<()>,
}

impl IAudioSessionNotification_Impl for AudioSessionNotifier_Impl {
    fn OnSessionCreated(
        &self,
        _new_session: windows::core::Ref<IAudioSessionControl>,
    ) -> windows::core::Result<()> {
        let _ = self.sender.send(());
        Ok(())
    }
}

pub struct AudioNotificationGuard {
    registrations: Vec<(IAudioSessionManager2, IAudioSessionNotification)>,
}

impl Drop for AudioNotificationGuard {
    fn drop(&mut self) {
        for (manager, callback) in &self.registrations {
            let _ = unsafe { manager.UnregisterSessionNotification(callback) };
        }
    }
}

// ---------------------------------------------------------------------------
// COM / Media Foundation lifecycle
// ---------------------------------------------------------------------------

struct ComGuard;

impl ComGuard {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .context("failed to initialize Windows COM")?;
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct MediaFoundationGuard;

impl MediaFoundationGuard {
    fn new() -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL)
                .context("failed to initialize Windows Media Foundation")?;
        }
        Ok(Self)
    }
}

impl Drop for MediaFoundationGuard {
    fn drop(&mut self) {
        let _ = unsafe { MFShutdown() };
    }
}

// ---------------------------------------------------------------------------
// Camera device enumeration (Media Foundation)
// ---------------------------------------------------------------------------

fn camera_devices() -> Result<Vec<Device>> {
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 1)? };
    let attributes = attributes.context("Media Foundation returned no attribute store")?;
    unsafe {
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
    }

    let mut raw_activates: *mut Option<IMFActivate> = ptr::null_mut();
    let mut count = 0;
    unsafe { MFEnumDeviceSources(&attributes, &mut raw_activates, &mut count)? };
    let activates = unsafe { slice::from_raw_parts_mut(raw_activates, count as usize) };
    let mut devices = Vec::with_capacity(count as usize);
    for activate in activates.iter_mut().filter_map(Option::take) {
        let Ok(id) = mf_string(
            &activate,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
        ) else {
            continue;
        };
        let name = mf_string(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
            .unwrap_or_else(|_| id.clone());
        devices.push(Device {
            resource: Resource::Camera,
            id,
            name,
        });
    }
    unsafe { CoTaskMemFree(Some(raw_activates.cast())) };
    Ok(devices)
}

fn mf_string(attributes: &IMFAttributes, key: &windows::core::GUID) -> Result<String> {
    let mut value = PWSTR::null();
    let mut length = 0;
    unsafe { attributes.GetAllocatedString(key, &mut value, &mut length)? };
    let result =
        unsafe { String::from_utf16_lossy(slice::from_raw_parts(value.0, length as usize)) };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Ok(result)
}

// ---------------------------------------------------------------------------
// Camera access: registry privacy activity + forensic module scan
// ---------------------------------------------------------------------------

struct CameraPermission {
    consent: Option<String>,
}

struct CameraApp {
    executable: String,
    permission: CameraPermission,
    non_packaged: bool,
}

fn remember_camera_app(
    apps: &mut HashMap<String, Vec<CameraApp>>,
    application: &str,
    app: CameraApp,
) {
    apps.entry(application.to_ascii_lowercase())
        .or_default()
        .push(app);
}

// ConsentStore records a path, not just an executable name. This deliberately
// does not equate short names, junctions or other aliases: those may miss an
// attribution, but must not inherit another executable's consent or signature.
fn same_windows_path(left: &str, right: &str) -> bool {
    if left.is_ascii() && right.is_ascii() {
        return left.eq_ignore_ascii_case(right);
    }
    let left = left.encode_utf16().collect::<Vec<_>>();
    let right = right.encode_utf16().collect::<Vec<_>>();
    unsafe { CompareStringOrdinal(&left, &right, true) == CSTR_EQUAL }
}

fn camera_app_for_process<'a>(apps: &'a [CameraApp], path: &str) -> Option<&'a CameraApp> {
    apps.iter()
        .find(|app| app.non_packaged && same_windows_path(&app.executable, path))
        .or_else(|| apps.iter().find(|app| !app.non_packaged))
}

fn attributed_registry_pid(
    matching_pids: &[u32],
    executable: Option<&str>,
    mut path_for: impl FnMut(u32) -> Option<String>,
) -> Option<u32> {
    let mut matches = matching_pids.iter().copied().filter(|&pid| {
        executable.is_none_or(|expected| {
            path_for(pid).is_some_and(|actual| same_windows_path(expected, &actual))
        })
    });
    let pid = matches.next()?;
    matches.next().is_none().then_some(pid)
}

// Component boundaries prevent sibling directories (e.g. ZoomEvil) from
// matching Zoom. This is still lexical, not a reparse-point/security boundary.
fn path_matches_policy_prefix(path: &str, prefix: &str) -> bool {
    let Some(head) = path.get(..prefix.len()) else {
        return false;
    };
    same_windows_path(head, prefix)
        && (path.len() == prefix.len()
            || prefix.ends_with('\\')
            || path.as_bytes()[prefix.len()] == b'\\')
}

fn signature_for_path(
    path: &str,
    trust_policy: TrustPolicy,
    verify: impl FnOnce(&str, TrustPolicy) -> SignatureInfo,
) -> SignatureInfo {
    // Re-verify for each capture: pathname and mtime cannot identify a file
    // that has been replaced while preserving its last-write timestamp.
    verify(path, trust_policy)
}

fn webcam_consent_store(opened: io::Result<RegKey>) -> Result<RegKey> {
    match opened {
        Ok(key) => Ok(key),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(error).context("webcam consent store key is missing; camera detection unavailable")
        }
        Err(error) => {
            Err(error).context("failed to open webcam consent store; camera detection unavailable")
        }
    }
}

fn camera_accesses(
    policy: &Policy,
    mut signature_for: impl FnMut(&str) -> SignatureInfo,
) -> Result<Vec<Access>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let webcam = webcam_consent_store(root.open_subkey(format!(r"{CONSENT_STORE}\webcam")))?;
    let processes = ProcessTable::load();
    let mut accesses = Vec::new();
    let mut known_apps: HashMap<String, Vec<CameraApp>> = HashMap::new();
    let global_consent = webcam.get_value::<String, _>("Value").ok();

    for key_name in webcam.enum_keys().filter_map(|item| item.ok()) {
        if key_name.eq_ignore_ascii_case("NonPackaged") {
            let non_packaged = webcam.open_subkey(&key_name)?;
            let desktop_consent = non_packaged
                .get_value::<String, _>("Value")
                .ok()
                .or_else(|| global_consent.clone());
            for encoded_path in non_packaged.enum_keys().filter_map(|item| item.ok()) {
                let executable = encoded_path.replace('#', r"\");
                let sub_key = non_packaged.open_subkey(&encoded_path)?;
                let consent = sub_key
                    .get_value::<String, _>("Value")
                    .ok()
                    .or_else(|| desktop_consent.clone());
                if let Some(application) = Path::new(&executable)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .map(str::to_owned)
                {
                    remember_camera_app(
                        &mut known_apps,
                        &application,
                        CameraApp {
                            executable,
                            permission: CameraPermission { consent },
                            non_packaged: true,
                        },
                    );
                }
                if is_privacy_active(&sub_key)
                    && let Some(access) = registry_access(
                        &sub_key,
                        &encoded_path,
                        true,
                        &processes,
                        &mut signature_for,
                        policy,
                    )
                {
                    accesses.push(access);
                }
            }
        } else {
            let sub_key = webcam.open_subkey(&key_name)?;
            let consent = sub_key
                .get_value::<String, _>("Value")
                .ok()
                .or_else(|| global_consent.clone());
            remember_camera_app(
                &mut known_apps,
                &key_name,
                CameraApp {
                    executable: key_name.clone(),
                    permission: CameraPermission { consent },
                    non_packaged: false,
                },
            );
            if is_privacy_active(&sub_key)
                && let Some(access) = registry_access(
                    &sub_key,
                    &key_name,
                    false,
                    &processes,
                    &mut signature_for,
                    policy,
                )
            {
                accesses.push(access);
            }
        }
    }

    // Packaged camera applications run under their executable name, not their
    // package-family consent key. The registry interval can remain stale while
    // a packaged application continues capturing frames.
    let camera_package = "microsoft.windowscamera_";
    let packaged_camera = webcam
        .enum_keys()
        .filter_map(Result::ok)
        .find(|key| key.to_ascii_lowercase().starts_with(camera_package));
    let packaged_camera_pids = packaged_camera
        .as_ref()
        .and_then(|_| processes.by_name.get("windowscamera.exe"))
        .into_iter()
        .flatten()
        .filter_map(|&pid| {
            let executable = process_path(pid)?;
            executable
                .to_ascii_lowercase()
                .contains(camera_package)
                .then_some((pid, executable))
        })
        .collect::<Vec<_>>();

    let mut candidate_runtime = HashMap::new();
    for (application, apps) in &known_apps {
        let Some(pids) = processes.by_name.get(application.as_str()) else {
            continue;
        };
        for &pid in pids {
            if accesses.iter().any(|access| access.pid == Some(pid)) {
                continue;
            }
            let Some(executable) = process_path(pid) else {
                continue;
            };
            if camera_app_for_process(apps, &executable).is_none() {
                continue;
            }
            let modules = capture_modules_loaded(pid);
            if modules.is_empty() {
                continue;
            }
            candidate_runtime.insert(pid, (executable, modules, process_command_line(pid)));
        }
    }
    let mut packaged_modules = packaged_camera_pids
        .iter()
        .filter_map(|(pid, _)| {
            let modules = capture_modules_loaded(*pid);
            (!modules.is_empty()).then_some((*pid, modules))
        })
        .collect::<HashMap<_, _>>();
    let busy_capture_services = busy_capture_services(
        candidate_runtime
            .iter()
            .filter_map(|(&pid, (_, _, command))| {
                command
                    .as_deref()
                    .is_some_and(is_video_capture_service)
                    .then_some(pid)
            })
            .chain(packaged_modules.keys().copied()),
    );
    // Opening Windows Camera wakes idle browser capture services too. CPU time
    // cannot attribute those wakeups to the browser while Camera is capturing.
    let packaged_camera_active = packaged_modules
        .keys()
        .any(|pid| busy_capture_services.contains(pid));

    for (application, apps) in &known_apps {
        let Some(pids) = processes.by_name.get(application.as_str()) else {
            continue;
        };
        for &pid in pids {
            if accesses.iter().any(|access| access.pid == Some(pid)) {
                continue;
            }
            let Some((executable, modules, command_line)) = candidate_runtime.get(&pid) else {
                continue;
            };
            let Some(app) = camera_app_for_process(apps, executable) else {
                continue;
            };
            let live_capture = !packaged_camera_active && busy_capture_services.contains(&pid);
            let context = processes.context(pid);
            let parent_info = immediate_parent(&context);
            let signature = signature_for(executable);
            let mut assessment = classify_forensic(
                policy,
                application,
                executable,
                &app.permission,
                command_line.as_deref(),
                parent_info.as_ref(),
                &signature,
                modules,
            );
            if live_capture {
                assessment.evidence.push(Evidence::new(
                    EvidenceKind::ApplicationProfile,
                    "ProcessTimes",
                    "Sustained CPU activity in the browser video-capture service; frame flow is inferred, not directly observed.",
                ));
            }
            let (parent_pid, parent_name) = parent_info
                .clone()
                .map_or((None, None), |(ppid, name)| (Some(ppid), Some(name)));
            accesses.push(Access {
                key: format!("camera:forensic:{}", process_identity(pid)),
                resource: Resource::Camera,
                activity: if live_capture {
                    Activity::Active
                } else {
                    Activity::Ready
                },
                risk: assessment.risk,
                confidence: if live_capture {
                    Confidence::Medium
                } else {
                    assessment.confidence
                },
                enforcement: assessment.enforcement,
                application: application.clone(),
                pid: Some(pid),
                parent_pid,
                parent_name,
                executable: Some(executable.clone()),
                signature: Some(signature),
                device: None,
                started_at: None,
                modules: modules.clone(),
                evidence: assessment.evidence,
                process: Some(context),
            });
        }
    }
    for (pid, executable) in packaged_camera_pids {
        if !busy_capture_services.contains(&pid) {
            continue;
        }
        let context = processes.context(pid);
        let signature = signature_for(&executable);
        let mut evidence = vec![Evidence::new(
            EvidenceKind::ApplicationProfile,
            "ProcessTimes",
            "Sustained CPU activity in the Windows Camera capture process; frame flow is inferred, not directly observed.",
        )];
        let application = "WindowsCamera.exe";
        let (risk, enforcement) = assess_policy(
            policy,
            application,
            Some(&executable),
            Some(&signature),
            &mut evidence,
        );
        let (parent_pid, parent_name) =
            immediate_parent(&context).map_or((None, None), |(pid, name)| (Some(pid), Some(name)));
        accesses.push(Access {
            key: format!("camera:forensic:{}", process_identity(pid)),
            resource: Resource::Camera,
            activity: Activity::Active,
            risk,
            confidence: Confidence::Medium,
            enforcement,
            application: application.to_owned(),
            pid: Some(pid),
            parent_pid,
            parent_name,
            executable: Some(executable),
            signature: Some(signature),
            device: None,
            started_at: None,
            modules: packaged_modules.remove(&pid).unwrap_or_default(),
            evidence,
            process: Some(context),
        });
    }
    Ok(accesses)
}

fn is_privacy_active(key: &RegKey) -> bool {
    let start = key.get_value::<u64, _>("LastUsedTimeStart").unwrap_or(0);
    let stop = key.get_value::<u64, _>("LastUsedTimeStop").unwrap_or(0);
    privacy_interval_active(start, stop)
}

fn privacy_interval_active(start: u64, stop: u64) -> bool {
    start > 0 && start > stop
}

fn registry_access(
    key: &RegKey,
    identity: &str,
    non_packaged: bool,
    processes: &ProcessTable,
    signature_for: &mut impl FnMut(&str) -> SignatureInfo,
    policy: &Policy,
) -> Option<Access> {
    let executable = non_packaged.then(|| identity.replace('#', r"\"));
    let application = executable
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| identity.to_owned());
    let matching_pids = processes
        .by_name
        .get(&application.to_ascii_lowercase())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let pid = attributed_registry_pid(matching_pids, executable.as_deref(), process_path);
    let context = pid.map(|value| processes.context(value));
    let (parent_pid, parent_name) = context
        .as_ref()
        .and_then(immediate_parent)
        .map_or((None, None), |(ppid, name)| (Some(ppid), Some(name)));
    let signature = executable.as_deref().map(signature_for);
    let mut evidence = vec![Evidence::new(
        EvidenceKind::PrivacyActivity,
        "CapabilityAccessManager",
        "Windows reports an open camera privacy activity interval.",
    )];
    let (risk, enforcement) = assess_policy(
        policy,
        &application,
        executable.as_deref(),
        signature.as_ref(),
        &mut evidence,
    );
    let start = key.get_value::<u64, _>("LastUsedTimeStart").ok()?;
    Some(Access {
        key: format!("camera:{}", identity.to_ascii_lowercase()),
        resource: Resource::Camera,
        activity: Activity::Active,
        risk,
        confidence: Confidence::Medium,
        enforcement,
        application,
        pid,
        parent_pid,
        parent_name,
        executable,
        signature,
        device: None,
        started_at: filetime_to_utc(start),
        modules: Vec::new(),
        evidence,
        process: context,
    })
}

// ---------------------------------------------------------------------------
// Forensic classification with Authenticode and Parent analysis
// ---------------------------------------------------------------------------

fn assess_policy(
    policy: &Policy,
    application: &str,
    executable: Option<&str>,
    signature: Option<&SignatureInfo>,
    evidence: &mut Vec<Evidence>,
) -> (Risk, EnforcementDecision) {
    let Some(rule) = policy.application(application) else {
        return (Risk::Expected, EnforcementDecision::Alert);
    };
    let publisher = signature.and_then(|value| value.signer.as_deref());
    let publisher_ok = rule.publishers.is_empty()
        || signature.is_some_and(|value| value.verified)
            && publisher.is_some_and(|signer| {
                rule.publishers
                    .iter()
                    .any(|expected| same_windows_path(signer.trim(), expected.trim()))
            });
    let path_ok = rule.paths.is_empty()
        || executable.is_some_and(|path| {
            rule.paths
                .iter()
                .any(|expected| path_matches_policy_prefix(path, expected))
        });
    let evidence_complete = (rule.publishers.is_empty()
        || signature.is_some_and(|value| !value.verified || value.signer.is_some()))
        && (rule.paths.is_empty() || executable.is_some());

    if !evidence_complete {
        evidence.push(Evidence::new(
            EvidenceKind::ApplicationProfile,
            "policy",
            "Policy rule matched the executable name, but required identity evidence is unavailable.",
        ));
        return (Risk::Unexplained, EnforcementDecision::Unknown);
    }
    if publisher_ok && path_ok {
        evidence.push(Evidence::new(
            EvidenceKind::ApplicationProfile,
            "policy",
            "Application matches its configured policy rule.",
        ));
        return (Risk::Expected, EnforcementDecision::Allow);
    }
    if !publisher_ok {
        evidence.push(Evidence::new(
            EvidenceKind::Signature,
            "policy",
            format!(
                "Publisher mismatch: expected one of {:?}, got {:?}.",
                rule.publishers, publisher
            ),
        ));
    }
    if !path_ok {
        evidence.push(Evidence::new(
            EvidenceKind::FileLocation,
            "policy",
            format!(
                "Path mismatch: expected a prefix in {:?}, executable at {:?}.",
                rule.paths, executable
            ),
        ));
    }
    (Risk::Suspicious, EnforcementDecision::Deny)
}

struct Assessment {
    risk: Risk,
    confidence: Confidence,
    evidence: Vec<Evidence>,
    enforcement: EnforcementDecision,
}

#[allow(clippy::too_many_arguments)]
fn classify_forensic(
    policy: &Policy,
    application: &str,
    executable: &str,
    permission: &CameraPermission,
    command_line: Option<&str>,
    parent_info: Option<&(u32, String)>,
    signature: &SignatureInfo,
    modules: &[String],
) -> Assessment {
    let mut evidence = vec![Evidence::new(
        EvidenceKind::CaptureModule,
        "Toolhelp32",
        format!(
            "Camera-capable modules are loaded: {}. This does not prove frame flow.",
            modules.join(", ")
        ),
    )];
    let app_lower = application.to_ascii_lowercase();
    let is_browser = BROWSERS.contains(&app_lower.as_str());
    let is_video_app = VIDEO_APPS.contains(&app_lower.as_str());
    let mut risk = Risk::Unexplained;

    if let Some(consent) = &permission.consent {
        evidence.push(Evidence::new(
            EvidenceKind::Permission,
            "CapabilityAccessManager",
            format!("Windows camera permission is {consent} for this application."),
        ));
        if consent.eq_ignore_ascii_case("Deny") {
            return Assessment {
                risk: Risk::Blocked,
                confidence: Confidence::Low,
                evidence,
                enforcement: EnforcementDecision::Alert,
            };
        }
    }

    if signature.verified {
        evidence.push(Evidence::new(
            EvidenceKind::Signature,
            "WinVerifyTrust",
            signature
                .signer
                .as_ref()
                .map(|signer| format!("Trusted Authenticode signature: {signer}."))
                .unwrap_or_else(|| {
                    "Trusted Authenticode signature; signer identity unavailable.".to_owned()
                }),
        ));
    } else {
        risk = Risk::Suspicious;
        evidence.push(Evidence::new(
            EvidenceKind::Signature,
            "WinVerifyTrust",
            signature
                .error
                .clone()
                .unwrap_or_else(|| "Signature verification failed.".to_owned()),
        ));
    }

    if let Some((parent_pid, parent_name)) = parent_info {
        evidence.push(Evidence::new(
            EvidenceKind::ProcessLineage,
            "Toolhelp32",
            format!("Parent is {parent_name} (PID {parent_pid})."),
        ));
        if matches!(
            parent_name.to_ascii_lowercase().as_str(),
            "cmd.exe"
                | "powershell.exe"
                | "pwsh.exe"
                | "cscript.exe"
                | "wscript.exe"
                | "mshta.exe"
                | "rundll32.exe"
        ) {
            risk = Risk::Suspicious;
        }
    }

    if let Some(command_line) = command_line
        && (command_line.contains("video_capture") || command_line.contains("VideoCaptureService"))
    {
        evidence.push(Evidence::new(
            EvidenceKind::CommandLine,
            "NtQueryInformationProcess",
            "A browser video-capture service is present, but current frame flow is unproven.",
        ));
    }

    if is_browser || is_video_app {
        evidence.push(Evidence::new(
            EvidenceKind::ApplicationProfile,
            "mcw profile",
            if is_browser {
                "Known browser; a loaded capture pipeline can be idle or residual."
            } else {
                "Known video application; a loaded capture pipeline is expected."
            },
        ));
        if signature.verified && risk != Risk::Suspicious {
            risk = Risk::Expected;
        }
    } else if executable.to_ascii_lowercase().contains(r"\temp\")
        || executable.to_ascii_lowercase().contains(r"\tmp\")
        || executable.to_ascii_lowercase().contains(r"\downloads\")
    {
        risk = Risk::Suspicious;
        evidence.push(Evidence::new(
            EvidenceKind::FileLocation,
            "filesystem path",
            format!("Executable is in a temporary or downloads directory: {executable}"),
        ));
    }

    let (policy_risk, enforcement) = assess_policy(
        policy,
        application,
        Some(executable),
        Some(signature),
        &mut evidence,
    );
    match enforcement {
        EnforcementDecision::Allow if risk != Risk::Suspicious => risk = Risk::Expected,
        EnforcementDecision::Deny => risk = Risk::Suspicious,
        EnforcementDecision::Unknown if risk == Risk::Expected => risk = policy_risk,
        EnforcementDecision::Allow | EnforcementDecision::Alert | EnforcementDecision::Unknown => {}
    }

    Assessment {
        risk,
        confidence: Confidence::Low,
        evidence,
        enforcement,
    }
}

// ---------------------------------------------------------------------------
// Process inspection & Authenticode helpers
// ---------------------------------------------------------------------------

fn verify_signature_with_policy(path: &str, trust_policy: TrustPolicy) -> SignatureInfo {
    let wide_path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide_path.as_ptr()),
        ..Default::default()
    };
    let (revocation_checks, prov_flags) = match trust_policy {
        TrustPolicy::Offline => (
            WTD_REVOKE_NONE,
            WTD_SAFER_FLAG | WTD_CACHE_ONLY_URL_RETRIEVAL,
        ),
        TrustPolicy::Online => (WTD_REVOKE_WHOLECHAIN, WTD_SAFER_FLAG),
    };
    let mut trust_data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: revocation_checks,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &mut file_info,
        },
        dwStateAction: WTD_STATEACTION_IGNORE,
        dwProvFlags: prov_flags,
        ..Default::default()
    };

    let status = unsafe {
        WinVerifyTrust(
            HWND::default(),
            &mut WINTRUST_ACTION_GENERIC_VERIFY_V2.clone(),
            &mut trust_data as *mut _ as _,
        )
    };

    let signer = get_signer_name(&wide_path);
    if status == 0 {
        SignatureInfo {
            verified: true,
            signer,
            error: None,
        }
    } else {
        let err_code = status as u32;
        let error = match err_code {
            0x800B_0100 => "unsigned (TRUST_E_NOSIGNATURE)".to_owned(),
            0x800B_0109 => "untrusted root (CERT_E_UNTRUSTEDROOT)".to_owned(),
            0x800B_010E => "revoked certificate (CERT_E_REVOKED)".to_owned(),
            0x8009_6010 => "trust provider not recognized (TRUST_E_PROVIDER_UNKNOWN)".to_owned(),
            _ => format!("untrusted signature (0x{err_code:08X})"),
        };
        SignatureInfo {
            verified: false,
            signer,
            error: Some(error),
        }
    }
}

fn get_signer_name(wide_path: &[u16]) -> Option<String> {
    let mut cert_context: *mut CERT_CONTEXT = ptr::null_mut();
    let mut msg_and_cert_type = CERT_QUERY_ENCODING_TYPE(0);
    let mut content_type = CERT_QUERY_CONTENT_TYPE(0);
    let mut format_type = CERT_QUERY_FORMAT_TYPE(0);

    let ok = unsafe {
        CryptQueryObject(
            CERT_QUERY_OBJECT_FILE,
            wide_path.as_ptr() as *const _,
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
            CERT_QUERY_FORMAT_FLAG_ALL,
            0,
            Some(&mut msg_and_cert_type),
            Some(&mut content_type),
            Some(&mut format_type),
            None,
            None,
            Some(&mut cert_context as *mut *mut _ as *mut *mut core::ffi::c_void),
        )
    };

    if ok.is_err() || cert_context.is_null() {
        return None;
    }

    let mut buffer = [0u16; 256];
    let len = unsafe {
        CertGetNameStringW(
            cert_context,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(&mut buffer),
        )
    };
    let _ = unsafe { CertFreeCertificateContext(Some(cert_context)) };

    if len <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..len as usize - 1]))
}

fn capture_modules_loaded(pid: u32) -> Vec<String> {
    let modules = process_modules(pid);
    let has = |name: &str| {
        modules
            .iter()
            .any(|module| module.eq_ignore_ascii_case(name))
    };
    let directshow = has("kswdmcap.ax");
    let media_foundation =
        has("MFCaptureEngine.dll") && (has("mfsensorgroup.dll") || has("ksproxy.ax"));

    if directshow || media_foundation {
        modules
            .into_iter()
            .filter(|module| CAPTURE_MODULES.contains(&module.to_ascii_lowercase().as_str()))
            .collect()
    } else {
        Vec::new()
    }
}

fn process_modules(pid: u32) -> Vec<String> {
    let Ok(snapshot) =
        (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) })
    else {
        return Vec::new();
    };
    let mut entry = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    let mut modules = Vec::new();

    if unsafe { Module32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            modules.push(wide_array_string(&entry.szModule));
            if unsafe { Module32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    modules
}

fn process_command_line(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let mut return_length = 0u32;
    let _ = unsafe {
        NtQueryInformationProcess(
            process,
            60, // ProcessCommandLineInformation
            ptr::null_mut(),
            0,
            &mut return_length,
        )
    };
    if return_length == 0 {
        let _ = unsafe { CloseHandle(process) };
        return None;
    }

    let mut buffer = vec![0u8; return_length as usize];
    let status = unsafe {
        NtQueryInformationProcess(
            process,
            60,
            buffer.as_mut_ptr().cast(),
            return_length,
            &mut return_length,
        )
    };
    let _ = unsafe { CloseHandle(process) };

    if status.0 != 0 || buffer.len() < size_of::<UNICODE_STRING>() {
        return None;
    }

    let unicode_str = unsafe { &*(buffer.as_ptr() as *const UNICODE_STRING) };
    let char_count = (unicode_str.length / 2) as usize;
    if unicode_str.buffer.is_null() || char_count == 0 {
        return None;
    }

    let slice = unsafe { slice::from_raw_parts(unicode_str.buffer, char_count) };
    let cmd = String::from_utf16_lossy(slice);
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn wide_array_string(value: &[u16]) -> String {
    let length = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length])
}

// ---------------------------------------------------------------------------
// Utility & Process table
// ---------------------------------------------------------------------------

fn filetime_to_utc(value: u64) -> Option<DateTime<Utc>> {
    let unix_100ns = i128::from(value) - WINDOWS_TO_UNIX_EPOCH_100NS;
    if unix_100ns < 0 {
        return None;
    }
    let seconds = unix_100ns / 10_000_000;
    let nanos = (unix_100ns % 10_000_000) * 100;
    DateTime::from_timestamp(seconds.try_into().ok()?, nanos.try_into().ok()?)
}

fn device_id(device: &IMMDevice) -> Result<String> {
    let value = unsafe { device.GetId()? };
    let result = unsafe { value.to_string() }.context("invalid microphone device identifier");
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    result
}

fn device_name(device: &IMMDevice) -> Result<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ)? };
    let property = unsafe { store.GetValue(&PKEY_Device_FriendlyName)? };
    let value = unsafe { PropVariantToStringAlloc(&property)? };
    let result = unsafe { value.to_string() }.context("invalid microphone device name");
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    result
}

fn process_identity(pid: u32) -> String {
    let Some(created) = process_creation_time(pid) else {
        return pid.to_string();
    };
    format!("{pid}:{created}")
}

fn busy_capture_services(pids: impl Iterator<Item = u32>) -> HashSet<u32> {
    let before = pids
        .filter_map(|pid| process_cpu_time(pid).map(|time| (pid, time)))
        .collect::<HashMap<_, _>>();
    if before.is_empty() {
        return HashSet::new();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    let midpoint = before
        .into_iter()
        .filter_map(|(pid, prior)| {
            let current = process_cpu_time(pid)?;
            (current.saturating_sub(prior) >= 150_000).then_some((pid, current))
        })
        .collect::<HashMap<_, _>>();
    if midpoint.is_empty() {
        return HashSet::new();
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    midpoint
        .into_iter()
        .filter_map(|(pid, prior)| {
            process_cpu_time(pid)
                .is_some_and(|current| current.saturating_sub(prior) >= 150_000)
                .then_some(pid)
        })
        .collect()
}
fn is_video_capture_service(command_line: &str) -> bool {
    let command = command_line.to_ascii_lowercase();
    command.contains("video_capture") || command.contains("videocaptureservice")
}

fn process_cpu_time(pid: u32) -> Option<u64> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let result =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    Some(filetime_value(kernel) + filetime_value(user))
}

fn filetime_value(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn process_creation_time(pid: u32) -> Option<u64> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let result =
        unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    Some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

fn process_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let mut buffer = vec![0u16; 32_768];
    let mut size = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .ok()
        .map(|_| String::from_utf16_lossy(&buffer[..size as usize]))
    };
    let _ = unsafe { CloseHandle(process) };
    result
}

#[derive(Clone, Debug)]
pub struct ProcessNode {
    pub parent_pid: u32,
    pub name: String,
    pub created_at_filetime: Option<u64>,
}

pub struct ProcessTable {
    pub by_name: HashMap<String, Vec<u32>>,
    pub by_pid: HashMap<u32, ProcessNode>,
}

impl ProcessTable {
    pub fn load() -> Self {
        let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return Self {
                by_name: HashMap::new(),
                by_pid: HashMap::new(),
            };
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut by_name: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_pid: HashMap<u32, ProcessNode> = HashMap::new();

        if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
            loop {
                let name = wide_array_string(&entry.szExeFile);
                by_name
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push(entry.th32ProcessID);
                by_pid.insert(
                    entry.th32ProcessID,
                    ProcessNode {
                        parent_pid: entry.th32ParentProcessID,
                        name,
                        created_at_filetime: process_creation_time(entry.th32ProcessID),
                    },
                );
                if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                    break;
                }
            }
        }
        let _ = unsafe { CloseHandle(snapshot) };
        Self { by_name, by_pid }
    }

    pub fn context(&self, pid: u32) -> ProcessContext {
        let mut ancestry = Vec::new();
        let mut current_pid = pid;
        let mut child_created = self
            .by_pid
            .get(&pid)
            .and_then(|node| node.created_at_filetime);
        let mut visited = std::collections::HashSet::new();

        for _ in 0..16 {
            let Some(node) = self.by_pid.get(&current_pid) else {
                break;
            };
            let parent_pid = node.parent_pid;
            if parent_pid == 0 || !visited.insert(parent_pid) {
                break;
            }
            let Some(parent) = self.by_pid.get(&parent_pid) else {
                break;
            };
            if let (Some(parent_created), Some(child_created)) =
                (parent.created_at_filetime, child_created)
                && parent_created > child_created
            {
                break;
            }
            ancestry.push(ProcessAncestor {
                pid: parent_pid,
                name: parent.name.clone(),
                created_at_filetime: parent.created_at_filetime,
            });
            current_pid = parent_pid;
            child_created = parent.created_at_filetime;
        }

        let mut session_id = 0;
        let session_id = unsafe { ProcessIdToSessionId(pid, &mut session_id) }
            .ok()
            .map(|_| session_id);
        let (user, integrity) = process_security_context(pid);
        ProcessContext {
            instance_id: process_identity(pid),
            ancestry,
            user,
            session_id,
            integrity,
        }
    }
}

fn process_security_context(pid: u32) -> (Option<String>, Option<String>) {
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return (None, None);
    };
    let mut token = HANDLE::default();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.is_err() {
        let _ = unsafe { CloseHandle(process) };
        return (None, None);
    }
    let user = token_information(token, TokenUser).and_then(|buffer| {
        let token_user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
        let sid = token_user.User.Sid;
        let mut name_len = 0;
        let mut domain_len = 0;
        let mut sid_type = SID_NAME_USE::default();
        let _ = unsafe {
            LookupAccountSidW(
                PCWSTR::null(),
                sid,
                None,
                &mut name_len,
                None,
                &mut domain_len,
                &mut sid_type,
            )
        };
        if name_len == 0 {
            return None;
        }
        let mut name = vec![0u16; name_len as usize];
        let mut domain = vec![0u16; domain_len as usize];
        unsafe {
            LookupAccountSidW(
                PCWSTR::null(),
                sid,
                Some(PWSTR(name.as_mut_ptr())),
                &mut name_len,
                Some(PWSTR(domain.as_mut_ptr())),
                &mut domain_len,
                &mut sid_type,
            )
            .ok()?;
        }
        let name = String::from_utf16_lossy(&name[..name_len as usize]);
        let domain = String::from_utf16_lossy(&domain[..domain_len as usize]);
        Some(if domain.is_empty() {
            name
        } else {
            format!("{domain}\\{name}")
        })
    });
    let integrity = token_information(token, TokenIntegrityLevel).and_then(|buffer| {
        let label = unsafe { &*(buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()) };
        let count = unsafe { *GetSidSubAuthorityCount(label.Label.Sid) };
        if count == 0 {
            return None;
        }
        let rid = unsafe { *GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)) };
        Some(
            match rid {
                0x0000..=0x0fff => "untrusted",
                0x1000..=0x1fff => "low",
                0x2000..=0x2fff => "medium",
                0x3000..=0x3fff => "high",
                0x4000..=0x4fff => "system",
                _ => "protected",
            }
            .to_owned(),
        )
    });
    let _ = unsafe { CloseHandle(token) };
    let _ = unsafe { CloseHandle(process) };
    (user, integrity)
}

fn token_information(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> Option<Vec<usize>> {
    let mut length = 0;
    let _ = unsafe { GetTokenInformation(token, class, None, 0, &mut length) };
    if length == 0 {
        return None;
    }
    let words = (length as usize).div_ceil(size_of::<usize>());
    let mut buffer = vec![0usize; words];
    unsafe {
        GetTokenInformation(
            token,
            class,
            Some(buffer.as_mut_ptr().cast()),
            length,
            &mut length,
        )
        .ok()?;
    }
    Some(buffer)
}

fn immediate_parent(context: &ProcessContext) -> Option<(u32, String)> {
    context
        .ancestry
        .first()
        .map(|parent| (parent.pid, parent.name.clone()))
}

fn process_instance_matches(pid: u32, created: FILETIME, expected: &str) -> bool {
    expected == format!("{pid}:{}", filetime_value(created))
}

pub fn terminate_process_by_pid(pid: u32, expected_instance: &str) -> Result<()> {
    if pid <= 4 || pid == std::process::id() {
        anyhow::bail!("refusing to terminate a protected process identifier");
    }
    let handle = unsafe {
        OpenProcess(
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        )
        .context("failed to open process for termination")?
    };
    let result: Result<()> = (|| {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }
            .context("failed to verify process instance before termination")?;
        if !process_instance_matches(pid, created, expected_instance) {
            anyhow::bail!("process instance changed or its creation time was unavailable");
        }
        unsafe { TerminateProcess(handle, 1) }.context("failed to terminate process")
    })();
    let _ = unsafe { CloseHandle(handle) };
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionLockState {
    Locked,
    Unlocked,
    Unknown,
}

pub fn session_lock_state() -> SessionLockState {
    let desk = unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) };
    let Ok(desk) = desk else {
        return SessionLockState::Unknown;
    };
    let mut name = [0u16; 128];
    let mut needed = 0;
    let result = unsafe {
        GetUserObjectInformationW(
            HANDLE(desk.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            (name.len() * size_of::<u16>()) as u32,
            Some(&mut needed),
        )
    };

    let _ = unsafe { CloseDesktop(desk) };
    if result.is_err() || needed < 2 || needed as usize > name.len() * size_of::<u16>() {
        return SessionLockState::Unknown;
    }
    let length = needed as usize / size_of::<u16>();
    let desktop = String::from_utf16_lossy(&name[..length.saturating_sub(1)]);
    if desktop.eq_ignore_ascii_case("Winlogon") {
        SessionLockState::Locked
    } else {
        SessionLockState::Unlocked
    }
}

pub fn play_chime() {
    unsafe {
        let _ = MessageBeep(MB_ICONASTERISK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ApplicationRule;

    fn signature(verified: bool) -> SignatureInfo {
        SignatureInfo {
            verified,
            signer: verified.then(|| "Expected Publisher".to_owned()),
            error: (!verified).then(|| "unsigned".to_owned()),
        }
    }

    #[test]
    fn missing_or_unreadable_webcam_store_is_unavailable() {
        for (kind, detail) in [
            (io::ErrorKind::NotFound, "key is missing"),
            (io::ErrorKind::PermissionDenied, "failed to open"),
            (io::ErrorKind::InvalidData, "failed to open"),
        ] {
            let error = webcam_consent_store(Err(io::Error::from(kind)))
                .expect_err("a missing or unreadable store cannot be healthy");
            assert!(error.to_string().contains(detail));
            assert_eq!(
                error
                    .root_cause()
                    .downcast_ref::<io::Error>()
                    .map(io::Error::kind),
                Some(kind)
            );
        }
    }

    #[test]
    fn replacing_executable_with_same_mtime_cannot_inherit_signer_allowance() {
        let path = std::env::temp_dir().join(format!(
            "miccamwatch-signer-{}-{}.exe",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"signed").unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let path_str = path.to_str().unwrap();
        let policy = Policy {
            applications: vec![ApplicationRule {
                executable: path.file_name().unwrap().to_str().unwrap().to_owned(),
                publishers: vec!["Expected Publisher".to_owned()],
                paths: vec![path.parent().unwrap().to_str().unwrap().to_owned()],
            }],
            ..Policy::default()
        };
        let simulated_verifier =
            |path: &str, _: TrustPolicy| signature(std::fs::read(path).unwrap() == b"signed");
        let mut evidence = Vec::new();
        assert_eq!(
            assess_policy(
                &policy,
                &policy.applications[0].executable,
                Some(path_str),
                Some(&signature_for_path(
                    path_str,
                    policy.trust_policy,
                    simulated_verifier
                )),
                &mut evidence,
            ),
            (Risk::Expected, EnforcementDecision::Allow)
        );

        std::fs::write(&path, b"forged").unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            modified
        );
        let replacement = signature_for_path(path_str, policy.trust_policy, simulated_verifier);
        std::fs::remove_file(&path).unwrap();
        let mut evidence = Vec::new();
        assert_eq!(
            assess_policy(
                &policy,
                &policy.applications[0].executable,
                Some(path_str),
                Some(&replacement),
                &mut evidence,
            ),
            (Risk::Suspicious, EnforcementDecision::Deny)
        );
    }

    #[test]
    fn camera_interval_is_active_when_new_start_supersedes_old_stop() {
        assert!(!privacy_interval_active(0, 0));
        assert!(privacy_interval_active(200, 0));
        assert!(privacy_interval_active(300, 200));
        assert!(!privacy_interval_active(200, 300));
        assert!(!privacy_interval_active(300, 300));
    }
    #[test]
    fn recognizes_browser_video_capture_service_commands() {
        assert!(is_video_capture_service(
            "--utility-sub-type=video_capture.mojom.VideoCaptureService"
        ));
        assert!(!is_video_capture_service("--type=renderer"));
    }

    #[test]
    fn denied_permission_is_blocked_not_unauthorized() {
        let assessment = classify_forensic(
            &Policy::default(),
            "camera.exe",
            r"C:\camera.exe",
            &CameraPermission {
                consent: Some("Deny".to_owned()),
            },
            None,
            None,
            &signature(true),
            &["mfcaptureengine.dll".to_owned()],
        );
        assert_eq!(assessment.risk, Risk::Blocked);
        assert_eq!(assessment.confidence, Confidence::Low);
        assert_eq!(assessment.enforcement, EnforcementDecision::Alert);
    }

    #[test]
    fn trusted_browser_pipeline_is_expected_but_low_confidence() {
        let assessment = classify_forensic(
            &Policy::default(),
            "msedge.exe",
            r"C:\Program Files\Microsoft\Edge\msedge.exe",
            &CameraPermission { consent: None },
            Some("--type=utility --utility-sub-type=VideoCaptureService"),
            None,
            &signature(true),
            &["mfcaptureengine.dll".to_owned()],
        );
        assert_eq!(assessment.risk, Risk::Expected);
        assert_eq!(assessment.confidence, Confidence::Low);
    }

    #[test]
    fn unsigned_known_application_is_suspicious() {
        let assessment = classify_forensic(
            &Policy::default(),
            "zoom.exe",
            r"C:\Apps\zoom.exe",
            &CameraPermission { consent: None },
            None,
            None,
            &signature(false),
            &["kswdmcap.ax".to_owned()],
        );
        assert_eq!(assessment.risk, Risk::Suspicious);
    }

    #[test]
    fn temporary_unknown_capture_binary_is_suspicious() {
        let assessment = classify_forensic(
            &Policy::default(),
            "capture.exe",
            r"C:\Users\person\AppData\Local\Temp\capture.exe",
            &CameraPermission { consent: None },
            None,
            Some(&(10, "explorer.exe".to_owned())),
            &signature(true),
            &["kswdmcap.ax".to_owned()],
        );
        assert_eq!(assessment.risk, Risk::Suspicious);
    }

    #[test]
    fn same_name_consent_records_select_only_the_matching_process_path() {
        let mut known_apps = HashMap::new();
        remember_camera_app(
            &mut known_apps,
            "capture.exe",
            CameraApp {
                executable: r"C:\Program Files\Capture\capture.exe".to_owned(),
                permission: CameraPermission {
                    consent: Some("Allow".to_owned()),
                },
                non_packaged: true,
            },
        );
        remember_camera_app(
            &mut known_apps,
            "CAPTURE.EXE",
            CameraApp {
                executable: r"C:\Users\person\Downloads\capture.exe".to_owned(),
                permission: CameraPermission {
                    consent: Some("Deny".to_owned()),
                },
                non_packaged: true,
            },
        );
        let apps = &known_apps["capture.exe"];
        let trusted = camera_app_for_process(apps, r"c:\program files\CAPTURE\capture.exe")
            .expect("trusted path has consent");
        assert_eq!(trusted.permission.consent.as_deref(), Some("Allow"));
        let untrusted = camera_app_for_process(apps, r"C:\Users\person\Downloads\capture.exe")
            .expect("same-name untrusted path has distinct consent");
        assert_eq!(untrusted.permission.consent.as_deref(), Some("Deny"));
        assert!(camera_app_for_process(apps, r"C:\Users\person\capture.exe").is_none());
    }

    #[test]
    fn active_registry_interval_requires_matching_process_path_for_pid() {
        let trusted = r"C:\Program Files\Capture\capture.exe";
        let paths = HashMap::from([
            (
                10,
                Some(r"C:\Users\person\Downloads\capture.exe".to_owned()),
            ),
            (20, Some(r"c:\program files\capture\CAPTURE.exe".to_owned())),
            (30, None),
        ]);
        let lookup = |pid| paths.get(&pid).cloned().flatten();
        assert_eq!(attributed_registry_pid(&[10], Some(trusted), lookup), None);
        assert_eq!(attributed_registry_pid(&[30], Some(trusted), lookup), None);
        assert_eq!(
            attributed_registry_pid(&[10, 20, 30], Some(trusted), lookup),
            Some(20)
        );
        assert_eq!(
            attributed_registry_pid(&[20, 20], Some(trusted), lookup),
            None
        );
    }

    fn explicit_policy() -> Policy {
        Policy {
            applications: vec![ApplicationRule {
                executable: "capture.exe".to_owned(),
                publishers: vec!["Expected Publisher".to_owned()],
                paths: vec![r"C:\Program Files\Capture".to_owned()],
            }],
            ..Policy::default()
        }
    }

    #[test]
    fn explicit_policy_match_allows_and_mismatch_denies() {
        let policy = explicit_policy();
        let mut evidence = Vec::new();
        let matched = assess_policy(
            &policy,
            "capture.exe",
            Some(r"C:\Program Files\Capture\capture.exe"),
            Some(&signature(true)),
            &mut evidence,
        );
        assert_eq!(matched, (Risk::Expected, EnforcementDecision::Allow));

        let mut evidence = Vec::new();
        let denied = assess_policy(
            &policy,
            "capture.exe",
            Some(r"C:\Users\person\capture.exe"),
            Some(&signature(true)),
            &mut evidence,
        );
        assert_eq!(denied, (Risk::Suspicious, EnforcementDecision::Deny));
    }

    #[test]
    fn policy_rejects_unverified_matching_signer_and_sibling_path_prefix() {
        let policy = explicit_policy();
        let untrusted_signature = SignatureInfo {
            verified: false,
            signer: Some("Expected Publisher".to_owned()),
            error: Some("untrusted root".to_owned()),
        };
        let mut evidence = Vec::new();
        let unverified = assess_policy(
            &policy,
            "capture.exe",
            Some(r"C:\Program Files\Capture\capture.exe"),
            Some(&untrusted_signature),
            &mut evidence,
        );
        assert_eq!(unverified, (Risk::Suspicious, EnforcementDecision::Deny));

        let mut evidence = Vec::new();
        let sibling = assess_policy(
            &policy,
            "capture.exe",
            Some(r"C:\Program Files\CaptureEvil\capture.exe"),
            Some(&signature(true)),
            &mut evidence,
        );
        assert_eq!(sibling, (Risk::Suspicious, EnforcementDecision::Deny));
    }

    #[test]
    fn policy_publisher_requires_exact_verified_identity() {
        let policy = explicit_policy();
        for signer in ["Evil Expected Publisher", "Expected Publisher LLC"] {
            let signature = SignatureInfo {
                verified: true,
                signer: Some(signer.to_owned()),
                error: None,
            };
            let mut evidence = Vec::new();
            assert_eq!(
                assess_policy(
                    &policy,
                    "capture.exe",
                    Some(r"C:\Program Files\Capture\capture.exe"),
                    Some(&signature),
                    &mut evidence,
                ),
                (Risk::Suspicious, EnforcementDecision::Deny),
            );
        }
        let signature = SignatureInfo {
            verified: true,
            signer: Some("  expected publisher  ".to_owned()),
            error: None,
        };
        let mut evidence = Vec::new();
        assert_eq!(
            assess_policy(
                &policy,
                "capture.exe",
                Some(r"C:\Program Files\Capture\capture.exe"),
                Some(&signature),
                &mut evidence,
            ),
            (Risk::Expected, EnforcementDecision::Allow),
        );
    }

    #[test]
    fn policy_prefix_does_not_include_sibling_directory() {
        assert!(path_matches_policy_prefix(
            r"C:\Program Files\Zoom\zoom.exe",
            r"c:\program files\zoom",
        ));
        assert!(!path_matches_policy_prefix(
            r"C:\Program Files\ZoomEvil\zoom.exe",
            r"C:\Program Files\Zoom",
        ));
    }

    #[test]
    fn termination_refuses_reused_or_unknown_process_instance() {
        let created = FILETIME {
            dwLowDateTime: 42,
            dwHighDateTime: 1,
        };
        assert!(process_instance_matches(120, created, "120:4294967338"));
        assert!(!process_instance_matches(120, created, "120:4294967339"));
        assert!(!process_instance_matches(120, created, "120"));
        assert!(!process_instance_matches(121, created, "120:4294967338"));
    }

    #[test]
    fn missing_policy_identity_is_unknown_not_denied() {
        let mut evidence = Vec::new();
        let result = assess_policy(&explicit_policy(), "capture.exe", None, None, &mut evidence);
        assert_eq!(result, (Risk::Unexplained, EnforcementDecision::Unknown));
    }
}
