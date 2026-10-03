use anyhow::{Context, Result, bail};
use std::{
    ffi::OsString,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        process::CommandExt,
    },
    path::Path,
    process::Command,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
        },
        UI::WindowsAndMessaging::{
            FindWindowW, GetWindowThreadProcessId, SMTO_ABORTIFHUNG, SMTO_BLOCK,
            SendMessageTimeoutW,
        },
    },
    core::{PCWSTR, PWSTR},
};

pub struct RunningTray {
    process: HANDLE,
    window: HWND,
    pid: u32,
}
impl Drop for RunningTray {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.process);
        }
    }
}

fn wide(path: &std::ffi::OsStr) -> Vec<u16> {
    path.encode_wide().chain(Some(0)).collect()
}

pub fn owned_running_tray(target: &Path) -> Result<Option<RunningTray>> {
    let class = wide(std::ffi::OsStr::new("MicCamWatchTrayClass"));
    let Ok(window) = (unsafe { FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) }) else {
        return Ok(None);
    };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid == 0 {
        bail!("could not identify the running tray; no process was stopped");
    }
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    }
    .context("cannot inspect running tray ownership; no process was stopped")?;
    let running = RunningTray {
        process,
        window,
        pid,
    };
    let mut image = [0u16; 32768];
    let mut length = image.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(image.as_mut_ptr()),
            &mut length,
        )
    }
    .context("cannot inspect running tray executable; no process was stopped")?;
    let image = std::path::PathBuf::from(OsString::from_wide(&image[..length as usize]));
    if !same_target(&image, target)? {
        return Ok(None);
    }
    // Recheck the HWND's PID after opening a stable process handle: HWNDs and PIDs are reusable.
    let mut current_pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut current_pid));
    }
    if current_pid != pid || unsafe { WaitForSingleObject(process, 0) } != WAIT_TIMEOUT {
        bail!("running tray changed while inspecting it; retry the update");
    }
    Ok(Some(running))
}

fn same_target(image: &Path, target: &Path) -> Result<bool> {
    let image = std::fs::canonicalize(image).context("cannot resolve running tray executable")?;
    let target =
        std::fs::canonicalize(target).context("cannot resolve installed tray executable")?;
    // Canonicalization resolves junctions/symlinks. Do not match a basename or a sibling installation.
    Ok(image == target)
}

impl RunningTray {
    pub fn stop(&self) -> Result<()> {
        if unsafe { WaitForSingleObject(self.process, 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        let mut current_pid = 0;
        unsafe {
            GetWindowThreadProcessId(self.window, Some(&mut current_pid));
        }
        if current_pid != self.pid {
            bail!("tray window ownership changed; no process was stopped");
        }
        let mut reply = 0;
        let result = unsafe {
            SendMessageTimeoutW(
                self.window,
                crate::frontends::tray::WM_UPDATE_STOP,
                WPARAM(self.pid as usize),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                5000,
                Some(&mut reply),
            )
        };
        if result.0 == 0 {
            bail!(
                "tray did not acknowledge safe shutdown; installation unchanged; finish pending camera operations and exit the tray manually, then retry"
            );
        }
        match reply {
            crate::frontends::tray::UPDATE_STOPPED => {}
            crate::frontends::tray::UPDATE_BUSY => bail!(
                "tray has pending camera operations (possibly a UAC prompt); installation unchanged; finish them and retry; nothing was cancelled"
            ),
            _ => bail!(
                "this tray does not support safe updater shutdown; installation unchanged; finish pending camera operations and exit the tray manually, then retry"
            ),
        }
        match unsafe { WaitForSingleObject(self.process, 15000) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => bail!(
                "tray acknowledged shutdown but has not terminated; installation unchanged; wait for it to exit and retry"
            ),
            _ => bail!("failed waiting on the tray process handle; installation unchanged"),
        }
    }
}

pub fn restart(target: &Path) -> Result<()> {
    Command::new(target)
        .creation_flags(0x0800_0000)
        .spawn()
        .map_err(|error| {
            super::transaction::io_error("restart previously running tray", target, error)
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ownership_requires_exact_installed_target() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let a = temp.path().join("a");
        let b = temp.path().join("b");
        std::fs::create_dir(&a)?;
        std::fs::create_dir(&b)?;
        let installed = a.join("mcw-tray.exe");
        let other = b.join("mcw-tray.exe");
        std::fs::write(&installed, b"owned")?;
        std::fs::write(&other, b"other")?;
        assert!(same_target(&installed, &installed)?);
        assert!(!same_target(&other, &installed)?);
        Ok(())
    }
}
