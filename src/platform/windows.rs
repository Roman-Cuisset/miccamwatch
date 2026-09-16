use crate::{
    cli::Filter,
    model::{Access, Confidence, Device, Resource},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, ffi::OsStr, mem::size_of, path::Path, ptr, slice};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::CloseHandle,
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
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
    core::{Interface, PWSTR},
};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

const CONSENT_STORE: &str =
    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore";
const WINDOWS_TO_UNIX_EPOCH_100NS: i128 = 116_444_736_000_000_000;

pub struct PlatformMonitor {
    enumerator: IMMDeviceEnumerator,
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
            accesses.extend(camera_accesses()?);
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
                let key = format!("microphone:{id}:{pid}");
                accesses.entry(key.clone()).or_insert(Access {
                    key,
                    resource: Resource::Microphone,
                    application,
                    pid: Some(pid),
                    executable,
                    device: Some(name.clone()),
                    started_at: None,
                    confidence: Confidence::Confirmed,
                    evidence: None,
                });
            }
        }

        Ok(accesses.into_values().collect())
    }
}

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

fn camera_accesses() -> Result<Vec<Access>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(webcam) = root.open_subkey(format!(r"{CONSENT_STORE}\webcam")) else {
        return Ok(Vec::new());
    };
    let processes = running_processes().unwrap_or_default();
    let mut accesses = Vec::new();
    let mut forensic_candidates = HashMap::new();

    for key_name in webcam.enum_keys().filter_map(|item| item.ok()) {
        if key_name.eq_ignore_ascii_case("NonPackaged") {
            let non_packaged = webcam.open_subkey(&key_name)?;
            for encoded_path in non_packaged.enum_keys().filter_map(|item| item.ok()) {
                let executable = encoded_path.replace('#', r"\");
                if let Some(application) = Path::new(&executable)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .map(str::to_owned)
                {
                    forensic_candidates.insert(application.to_ascii_lowercase(), executable);
                }
                let key = non_packaged.open_subkey(&encoded_path)?;
                if let Some(access) = registry_access(&key, &encoded_path, true, &processes) {
                    accesses.push(access);
                }
            }
        } else {
            let key = webcam.open_subkey(&key_name)?;
            if let Some(access) = registry_access(&key, &key_name, false, &processes) {
                accesses.push(access);
            }
        }
    }

    for (application, executable) in forensic_candidates {
        let Some(pids) = processes.get(&application) else {
            continue;
        };
        for &pid in pids {
            if accesses.iter().any(|access| access.pid == Some(pid)) {
                continue;
            }
            let evidence = camera_stack_evidence(pid);
            if evidence.is_empty() {
                continue;
            }
            accesses.push(Access {
                key: format!("camera:forensic:{pid}"),
                resource: Resource::Camera,
                application: application.clone(),
                pid: Some(pid),
                executable: Some(executable.clone()),
                device: None,
                started_at: None,
                confidence: Confidence::Forensic,
                evidence: Some(format!(
                    "capture stack loaded outside Windows privacy tracking: {}",
                    evidence.join(", ")
                )),
            });
        }
    }

    Ok(accesses)
}

fn registry_access(
    key: &RegKey,
    identity: &str,
    non_packaged: bool,
    processes: &HashMap<String, Vec<u32>>,
) -> Option<Access> {
    let start = key.get_value::<u64, _>("LastUsedTimeStart").ok()?;
    let stop = key.get_value::<u64, _>("LastUsedTimeStop").ok()?;
    if start == 0 || stop != 0 {
        return None;
    }

    let executable = non_packaged.then(|| identity.replace('#', r"\"));
    let application = executable
        .as_deref()
        .and_then(|path| Path::new(path).file_name())
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| identity.to_owned());
    let matching_pids = processes
        .get(&application.to_ascii_lowercase())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let pid = (matching_pids.len() == 1).then(|| matching_pids[0]);

    Some(Access {
        key: format!("camera:{}", identity.to_ascii_lowercase()),
        resource: Resource::Camera,
        application,
        pid,
        executable,
        device: None,
        started_at: filetime_to_utc(start),
        confidence: Confidence::Inferred,
        evidence: None,
    })
}

fn camera_stack_evidence(pid: u32) -> Vec<String> {
    let modules = process_modules(pid);
    let has = |name: &str| {
        modules
            .iter()
            .any(|module| module.eq_ignore_ascii_case(name))
    };
    let directshow_capture = has("kswdmcap.ax");
    let media_foundation_capture =
        has("MFCaptureEngine.dll") && (has("mfsensorgroup.dll") || has("ksproxy.ax"));

    if directshow_capture || media_foundation_capture {
        modules
            .into_iter()
            .filter(|module| {
                matches!(
                    module.to_ascii_lowercase().as_str(),
                    "kswdmcap.ax" | "ksproxy.ax" | "mfcaptureengine.dll" | "mfsensorgroup.dll"
                )
            })
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

fn wide_array_string(value: &[u16]) -> String {
    let length = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length])
}

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

fn running_processes() -> Result<HashMap<String, Vec<u32>>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? };
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes: HashMap<String, Vec<u32>> = HashMap::new();

    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|character| *character == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..length]).to_ascii_lowercase();
            processes.entry(name).or_default().push(entry.th32ProcessID);
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    unsafe { CloseHandle(snapshot)? };
    Ok(processes)
}
