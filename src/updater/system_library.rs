use anyhow::{Context, Result, ensure};
use std::{ffi::CStr, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::{
            LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
            SystemInformation::GetSystemDirectoryW,
        },
    },
    core::{PCSTR, PCWSTR},
};

pub(super) fn system_directory() -> Result<PathBuf> {
    let mut buffer = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(Some(&mut buffer)) } as usize;
    ensure!(
        length > 0 && length < buffer.len(),
        "cannot locate Windows System32 directory"
    );
    Ok(PathBuf::from(String::from_utf16(&buffer[..length])?))
}

pub(super) struct SystemLibrary(HMODULE);

impl SystemLibrary {
    pub(super) fn load(name: &str) -> Result<Self> {
        ensure!(
            matches!(name, "msi.dll" | "version.dll"),
            "unexpected maintenance library"
        );
        let path = system_directory()?.join(name);
        let wide = super::msi::wide(path.as_os_str());
        let library =
            unsafe { LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
                .with_context(|| format!("cannot load {name} from System32"))?;
        Ok(Self(library))
    }

    /// # Safety
    /// The caller must use the exact Windows ABI for the named export and
    /// keep this module loaded while invoking the returned function.
    pub(super) unsafe fn export<T: Copy>(&self, name: &CStr) -> Result<T> {
        let entry = unsafe { GetProcAddress(self.0, PCSTR(name.as_ptr().cast())) }
            .with_context(|| format!("missing System32 export {}", name.to_string_lossy()))?;
        ensure!(
            std::mem::size_of::<T>() == std::mem::size_of_val(&entry),
            "invalid Win32 export representation"
        );
        Ok(unsafe { std::mem::transmute_copy(&entry) })
    }
}

impl Drop for SystemLibrary {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}
