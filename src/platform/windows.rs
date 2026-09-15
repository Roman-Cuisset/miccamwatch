use crate::{
    cli::Filter,
    model::{Access, Confidence, Device, Resource},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, ffi::OsStr, mem::size_of, path::Path};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::CloseHandle,
        Media::Audio::{
            AudioSessionStateActive, DEVICE_STATE_ACTIVE, IAudioSessionControl2,
            IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture,
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize, STGM_READ, StructuredStorage::PropVariantToStringAlloc,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
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
    _com: ComGuard,
}

impl PlatformMonitor {
    pub fn new() -> Result<Self> {
        let com = ComGuard::new()?;
        let enumerator = unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .context("failed to create the Windows audio device enumerator")?
        };
        Ok(Self {
            enumerator,
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

fn camera_accesses() -> Result<Vec<Access>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(webcam) = root.open_subkey(format!(r"{CONSENT_STORE}\webcam")) else {
        return Ok(Vec::new());
    };
    let processes = running_processes().unwrap_or_default();
    let mut accesses = Vec::new();

    for key_name in webcam.enum_keys().filter_map(|item| item.ok()) {
        if key_name.eq_ignore_ascii_case("NonPackaged") {
            let non_packaged = webcam.open_subkey(&key_name)?;
            for encoded_path in non_packaged.enum_keys().filter_map(|item| item.ok()) {
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
    })
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
