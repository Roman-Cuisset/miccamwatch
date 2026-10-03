use anyhow::{Context, Result, bail};
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

// Callers pass internal tool names, never a user-supplied path. Binding to the
// OS directory avoids PATH/current-directory lookup at an elevation boundary.
pub(crate) fn system_executable(name: &str) -> Result<PathBuf> {
    let mut short = [0u16; 260];
    let length = unsafe { GetSystemDirectoryW(Some(&mut short)) } as usize;
    if length == 0 {
        return Err(std::io::Error::last_os_error())
            .context("cannot locate Windows system directory");
    }
    let directory = if length < short.len() {
        OsString::from_wide(&short[..length])
    } else {
        let mut long = vec![0u16; length + 1];
        let length = unsafe { GetSystemDirectoryW(Some(&mut long)) } as usize;
        if length == 0 {
            return Err(std::io::Error::last_os_error())
                .context("cannot locate Windows system directory");
        }
        if length >= long.len() {
            bail!("Windows system directory changed during lookup");
        }
        OsString::from_wide(&long[..length])
    };
    Ok(PathBuf::from(directory).join(name))
}
