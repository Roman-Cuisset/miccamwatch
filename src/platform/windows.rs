use crate::{
    cli::Filter,
    model::{
        Access, Activity, Confidence, Device, Evidence, EvidenceKind, Resource, Risk, SignatureInfo,
    },
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::{
    cell::RefCell, collections::HashMap, ffi::OsStr, fs, mem::size_of, path::Path, ptr, slice,
    time::SystemTime,
};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, NTSTATUS},
        Media::{
            Audio::{
                AudioSessionStateActive, DEVICE_STATE_ACTIVE, IAudioSessionControl2,
                IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
                eCapture,
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
            WinTrust::{
                WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
                WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
                WTD_SAFER_FLAG, WTD_STATEACTION_IGNORE, WTD_UI_NONE, WinVerifyTrust,
            },
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize, STGM_READ, StructuredStorage::PropVariantToStringAlloc,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW,
                PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPMODULE,
                TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
            },
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
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
    signature_cache: RefCell<HashMap<String, CachedSignature>>,
    _media_foundation: MediaFoundationGuard,
    _com: ComGuard,
}

impl PlatformMonitor {
    pub fn new() -> Result<Self> {
        let com = ComGuard::new()?;
        let media_foundation = MediaFoundationGuard::new()?;
        let enumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .context("failed to create the Windows audio device enumerator")?
        };
        Ok(Self {
            enumerator,
            signature_cache: RefCell::new(HashMap::new()),
            _media_foundation: media_foundation,
            _com: com,
        })
    }

    pub fn snapshot(&self, filter: &Filter) -> Result<Vec<Access>> {
        let mut accesses = Vec::new();
        if filter.includes_microphone() {
            accesses.extend(self.microphone_accesses()?);
        }
        if filter.includes_camera() {
            accesses.extend(self.camera_accesses()?);
        }
        accesses.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(accesses)
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

                let (parent_pid, parent_name) = match processes.parent(pid) {
                    Some((ppid, pname)) => (Some(ppid), Some(pname)),
                    None => (None, None),
                };

                let signature = executable
                    .as_deref()
                    .map(|path| self.cached_signature(path));
                let key = format!("microphone:{id}:{}", process_identity(pid));
                accesses.entry(key.clone()).or_insert(Access {
                    key,
                    resource: Resource::Microphone,
                    activity: Activity::Active,
                    risk: Risk::Normal,
                    confidence: Confidence::High,
                    application,
                    pid: Some(pid),
                    parent_pid,
                    parent_name,
                    executable,
                    signature,
                    device: Some(name.clone()),
                    started_at: None,
                    modules: Vec::new(),
                    evidence: vec![Evidence::new(
                        EvidenceKind::LiveApi,
                        "WASAPI",
                        "An active capture audio session is attributed to this PID.",
                    )],
                });
            }
        }

        Ok(accesses.into_values().collect())
    }

    fn cached_signature(&self, path: &str) -> SignatureInfo {
        let modified = fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok();
        let key = path.to_ascii_lowercase();
        if let Some(cached) = self.signature_cache.borrow().get(&key)
            && cached.modified == modified
        {
            return cached.info.clone();
        }
        let info = verify_signature(path);
        self.signature_cache.borrow_mut().insert(
            key,
            CachedSignature {
                modified,
                info: info.clone(),
            },
        );
        info
    }

    fn camera_accesses(&self) -> Result<Vec<Access>> {
        camera_accesses(|path| self.cached_signature(path))
    }
}

#[derive(Clone)]
struct CachedSignature {
    modified: Option<SystemTime>,
    info: SignatureInfo,
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

fn camera_accesses(mut signature_for: impl FnMut(&str) -> SignatureInfo) -> Result<Vec<Access>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(webcam) = root.open_subkey(format!(r"{CONSENT_STORE}\webcam")) else {
        return Ok(Vec::new());
    };
    let processes = ProcessTable::load();
    let mut accesses = Vec::new();
    let mut known_apps: HashMap<String, (String, CameraPermission)> = HashMap::new();

    for key_name in webcam.enum_keys().filter_map(|item| item.ok()) {
        if key_name.eq_ignore_ascii_case("NonPackaged") {
            let non_packaged = webcam.open_subkey(&key_name)?;
            for encoded_path in non_packaged.enum_keys().filter_map(|item| item.ok()) {
                let executable = encoded_path.replace('#', r"\");
                let sub_key = non_packaged.open_subkey(&encoded_path)?;
                let consent = sub_key.get_value::<String, _>("Value").ok();
                if let Some(application) = Path::new(&executable)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .map(str::to_owned)
                {
                    known_apps.insert(
                        application.to_ascii_lowercase(),
                        (executable.clone(), CameraPermission { consent }),
                    );
                }
                if is_privacy_active(&sub_key)
                    && let Some(access) = registry_access(
                        &sub_key,
                        &encoded_path,
                        true,
                        &processes,
                        &mut signature_for,
                    )
                {
                    accesses.push(access);
                }
            }
        } else {
            let sub_key = webcam.open_subkey(&key_name)?;
            let consent = sub_key.get_value::<String, _>("Value").ok();
            known_apps.insert(
                key_name.to_ascii_lowercase(),
                (key_name.clone(), CameraPermission { consent }),
            );
            if is_privacy_active(&sub_key)
                && let Some(access) =
                    registry_access(&sub_key, &key_name, false, &processes, &mut signature_for)
            {
                accesses.push(access);
            }
        }
    }

    for (application, (executable, permission)) in &known_apps {
        let Some(pids) = processes.by_name.get(application.as_str()) else {
            continue;
        };
        for &pid in pids {
            if accesses.iter().any(|access| access.pid == Some(pid)) {
                continue;
            }
            let modules = capture_modules_loaded(pid);
            if modules.is_empty() {
                continue;
            }
            let command_line = process_command_line(pid);
            let parent_info = processes.parent(pid);
            let signature = signature_for(executable);
            let assessment = classify_forensic(
                application,
                executable,
                permission,
                command_line.as_deref(),
                parent_info.as_ref(),
                &signature,
                &modules,
            );
            let (parent_pid, parent_name) = match parent_info {
                Some((ppid, name)) => (Some(ppid), Some(name)),
                None => (None, None),
            };
            accesses.push(Access {
                key: format!("camera:forensic:{}", process_identity(pid)),
                resource: Resource::Camera,
                activity: Activity::Ready,
                risk: assessment.risk,
                confidence: assessment.confidence,
                application: application.clone(),
                pid: Some(pid),
                parent_pid,
                parent_name,
                executable: Some(executable.clone()),
                signature: Some(signature),
                device: None,
                started_at: None,
                modules,
                evidence: assessment.evidence,
            });
        }
    }
    Ok(accesses)
}

fn is_privacy_active(key: &RegKey) -> bool {
    let start = key.get_value::<u64, _>("LastUsedTimeStart").unwrap_or(0);
    let stop = key.get_value::<u64, _>("LastUsedTimeStop").unwrap_or(0);
    start > 0 && stop == 0
}

fn registry_access(
    key: &RegKey,
    identity: &str,
    non_packaged: bool,
    processes: &ProcessTable,
    signature_for: &mut impl FnMut(&str) -> SignatureInfo,
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
    let pid = (matching_pids.len() == 1).then(|| matching_pids[0]);
    let (parent_pid, parent_name) = match pid.and_then(|value| processes.parent(value)) {
        Some((ppid, name)) => (Some(ppid), Some(name)),
        None => (None, None),
    };
    let signature = executable.as_deref().map(signature_for);
    let start = key.get_value::<u64, _>("LastUsedTimeStart").ok()?;
    Some(Access {
        key: format!("camera:{}", identity.to_ascii_lowercase()),
        resource: Resource::Camera,
        activity: Activity::Active,
        risk: Risk::Normal,
        confidence: Confidence::Medium,
        application,
        pid,
        parent_pid,
        parent_name,
        executable,
        signature,
        device: None,
        started_at: filetime_to_utc(start),
        modules: Vec::new(),
        evidence: vec![Evidence::new(
            EvidenceKind::PrivacyActivity,
            "CapabilityAccessManager",
            "Windows reports an open camera privacy activity interval.",
        )],
    })
}

// ---------------------------------------------------------------------------
// Forensic classification with Authenticode and Parent analysis
// ---------------------------------------------------------------------------

struct Assessment {
    risk: Risk,
    confidence: Confidence,
    evidence: Vec<Evidence>,
}

fn classify_forensic(
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
            risk = Risk::Normal;
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

    Assessment {
        risk,
        confidence: Confidence::Low,
        evidence,
    }
}

// ---------------------------------------------------------------------------
// Process inspection & Authenticode helpers
// ---------------------------------------------------------------------------

pub fn verify_signature(path: &str) -> SignatureInfo {
    let wide_path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide_path.as_ptr()),
        ..Default::default()
    };
    let mut trust_data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &mut file_info,
        },
        dwStateAction: WTD_STATEACTION_IGNORE,
        dwProvFlags: WTD_SAFER_FLAG | WTD_CACHE_ONLY_URL_RETRIEVAL,
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
                let name_lower = name.to_ascii_lowercase();
                by_name
                    .entry(name_lower)
                    .or_default()
                    .push(entry.th32ProcessID);
                by_pid.insert(
                    entry.th32ProcessID,
                    ProcessNode {
                        parent_pid: entry.th32ParentProcessID,
                        name,
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

    pub fn parent(&self, pid: u32) -> Option<(u32, String)> {
        let node = self.by_pid.get(&pid)?;
        if node.parent_pid == 0 {
            return None;
        }
        let parent_node = self.by_pid.get(&node.parent_pid);
        let parent_name = parent_node
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("PID {}", node.parent_pid));
        Some((node.parent_pid, parent_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(verified: bool) -> SignatureInfo {
        SignatureInfo {
            verified,
            signer: verified.then(|| "Expected Publisher".to_owned()),
            error: (!verified).then(|| "unsigned".to_owned()),
        }
    }

    #[test]
    fn denied_permission_is_blocked_not_unauthorized() {
        let assessment = classify_forensic(
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
    }

    #[test]
    fn trusted_browser_pipeline_is_normal_but_low_confidence() {
        let assessment = classify_forensic(
            "msedge.exe",
            r"C:\Program Files\Microsoft\Edge\msedge.exe",
            &CameraPermission { consent: None },
            Some("--type=utility --utility-sub-type=VideoCaptureService"),
            None,
            &signature(true),
            &["mfcaptureengine.dll".to_owned()],
        );
        assert_eq!(assessment.risk, Risk::Normal);
        assert_eq!(assessment.confidence, Confidence::Low);
    }

    #[test]
    fn unsigned_known_application_is_suspicious() {
        let assessment = classify_forensic(
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
}
