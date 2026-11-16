use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

pub(super) struct JournalFile {
    pub file: File,
    path: PathBuf,
    _directory: private::Directory,
}

impl JournalFile {
    pub fn verify(&self) -> Result<()> {
        private::verify(&self.file, &self.path)
    }
}

pub(super) fn create(root: &Path, at: DateTime<Utc>) -> Result<(String, String, JournalFile)> {
    create_with(root, at, super::super::new_private_token)
}

fn create_with(
    root: &Path,
    at: DateTime<Utc>,
    mut token: impl FnMut() -> Result<String>,
) -> Result<(String, String, JournalFile)> {
    let directory = root.join("sessions");
    let display = directory
        .to_str()
        .context("Session directory cannot be displayed exactly")?;
    ensure!(
        display.len() <= 3000,
        "Session directory exceeds the IPC path limit"
    );
    let directory = private::Directory::open(&directory)?;
    for _ in 0..8 {
        let id = token()?;
        let name = format!("session-{}-{id}.jsonl", at.format("%Y%m%dT%H%M%SZ"));
        match directory.create_new(&name) {
            Ok(file) => {
                let path = root.join("sessions").join(name);
                let display = path.display().to_string();
                let journal = JournalFile {
                    file,
                    path,
                    _directory: directory,
                };
                journal.verify()?;
                return Ok((id, display, journal));
            }
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::AlreadyExists) => {}
            Err(error) => return Err(error).context("Create private observation journal"),
        }
    }
    anyhow::bail!("Could not allocate a unique observation journal name")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn collision_retries_without_overwriting_the_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("MicCamWatch");
        let at = Utc::now();
        let first_id = "a".repeat(32);
        let (_, path, mut first) = create_with(&root, at, || Ok(first_id.clone())).unwrap();
        first.file.write_all(b"existing data").unwrap();
        drop(first);
        let second_id = "b".repeat(32);
        let mut ids = [first_id, second_id.clone()].into_iter();
        let (id, second_path, _) = create_with(&root, at, || Ok(ids.next().unwrap())).unwrap();
        assert_eq!(id, second_id);
        assert_ne!(path, second_path);
        assert_eq!(std::fs::read(path).unwrap(), b"existing data");
    }
}

#[cfg(unix)]
mod private {
    use super::*;
    use std::{
        ffi::{CStr, CString},
        os::unix::{
            ffi::OsStrExt,
            fs::MetadataExt,
            io::{AsRawFd, FromRawFd},
        },
        path::Component,
    };

    pub(super) struct Directory(File);

    impl Directory {
        pub(super) fn open(path: &Path) -> Result<Self> {
            ensure!(path.is_absolute(), "Session directory must be absolute");
            let mut components = path.components().peekable();
            ensure!(
                components.next() == Some(Component::RootDir),
                "Session directory has no root"
            );
            let mut directory = open_at(libc::AT_FDCWD, c"/", DIRECTORY_FLAGS, 0)?;
            while let Some(component) = components.next() {
                let Component::Normal(name) = component else {
                    anyhow::bail!("Session directory has a relative component");
                };
                let name = CString::new(name.as_bytes())?;
                let parent = directory.as_raw_fd();
                let next = match open_at(parent, &name, DIRECTORY_FLAGS, 0) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        if unsafe { libc::mkdirat(parent, name.as_ptr(), 0o700) } != 0 {
                            let error = io::Error::last_os_error();
                            if error.kind() != io::ErrorKind::AlreadyExists {
                                return Err(error.into());
                            }
                        }
                        open_at(parent, &name, DIRECTORY_FLAGS, 0)?
                    }
                    result => result?,
                };
                let meta = next.metadata()?;
                let user = unsafe { libc::geteuid() };
                let application_root_or_leaf = components.clone().count() <= 1;
                let sticky_root = meta.uid() == 0 && meta.mode() & 0o1000 != 0;
                ensure!(
                    (meta.uid() == user || (meta.uid() == 0 && !application_root_or_leaf))
                        && (meta.mode() & 0o022 == 0 || (sticky_root && !application_root_or_leaf)),
                    "Unsafe owner or permissions on a session directory component"
                );
                let leaf = components.peek().is_none();
                ensure!(
                    !leaf || meta.mode() & 0o777 == 0o700,
                    "Session directory must have mode 0700"
                );
                #[cfg(target_os = "macos")]
                crate::frontends::desktop::unix::check_directory_acl(&next, leaf)?;
                directory = next;
            }
            Ok(Self(directory))
        }

        pub(super) fn create_new(&self, name: &str) -> Result<File> {
            let name = CString::new(name)?;
            let file = open_at(
                self.0.as_raw_fd(),
                &name,
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )?;
            if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
                return Err(io::Error::last_os_error().into());
            }
            let meta = file.metadata()?;
            ensure!(
                meta.is_file() && meta.nlink() == 1 && meta.uid() == unsafe { libc::geteuid() },
                "Session journal must be an owned single-link file"
            );
            #[cfg(target_os = "macos")]
            crate::frontends::desktop::unix::check_directory_acl(&file, true)?;
            Ok(file)
        }
    }

    pub(super) fn verify(file: &File, path: &Path) -> Result<()> {
        let directory = Directory::open(path.parent().context("Missing session directory")?)?;
        let filename = CString::new(
            path.file_name()
                .context("Missing journal filename")?
                .as_bytes(),
        )?;
        let named = open_at(
            directory.0.as_raw_fd(),
            &filename,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0,
        )?;
        let actual = named.metadata()?;
        let created = file.metadata()?;
        ensure!(
            actual.dev() == created.dev()
                && actual.ino() == created.ino()
                && actual.nlink() == 1
                && actual.mode() & 0o777 == 0o600,
            "Session journal was replaced or its permissions changed"
        );
        #[cfg(target_os = "macos")]
        crate::frontends::desktop::unix::check_directory_acl(&named, true)?;
        Ok(())
    }

    const DIRECTORY_FLAGS: libc::c_int =
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

    fn open_at(
        parent: libc::c_int,
        name: &CStr,
        flags: libc::c_int,
        mode: libc::c_uint,
    ) -> io::Result<File> {
        // The variadic mode argument uses C integer promotion on Darwin.
        let fd = unsafe { libc::openat(parent, name.as_ptr(), flags, mode) };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
}

#[cfg(windows)]
mod private {
    use super::*;
    use std::{
        fs::OpenOptions,
        os::windows::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
            io::{AsRawHandle, FromRawHandle},
        },
        path::Component,
    };
    use windows::{
        Win32::{
            Foundation::{
                ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, GENERIC_READ, GENERIC_WRITE, HANDLE,
                HLOCAL, LocalFree,
            },
            Security::{
                Authorization::{
                    ConvertSecurityDescriptorToStringSecurityDescriptorW,
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
                    SE_FILE_OBJECT,
                },
                DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
                SECURITY_ATTRIBUTES,
            },
            Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, CreateFileW,
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
                GetFileInformationByHandle,
            },
        },
        core::{PCWSTR, PWSTR},
    };

    struct Local(HLOCAL);
    impl Drop for Local {
        fn drop(&mut self) {
            unsafe {
                let _ = LocalFree(Some(self.0));
            }
        }
    }

    pub(super) fn verify(file: &File, path: &Path) -> Result<()> {
        let named = OpenOptions::new()
            .access_mode(0x20080)
            .share_mode(3)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(path)?;
        let identity = |file: &File| -> Result<(u32, u32, u32)> {
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            unsafe {
                GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info)?;
            }
            ensure!(
                info.nNumberOfLinks == 1 && info.dwFileAttributes & 0x400 == 0,
                "Session journal must be a single-link non-reparse file"
            );
            Ok((
                info.dwVolumeSerialNumber,
                info.nFileIndexHigh,
                info.nFileIndexLow,
            ))
        };
        ensure!(
            identity(file)? == identity(&named)?,
            "Session journal was replaced"
        );
        verify_descriptor(&named, &crate::windows_control::current_identity()?.user)
    }

    pub(super) struct Directory {
        path: PathBuf,
        _pins: Vec<File>,
        sid: String,
    }

    impl Directory {
        pub(super) fn open(path: &Path) -> Result<Self> {
            ensure!(
                path.is_absolute()
                    && !path
                        .components()
                        .any(|part| matches!(part, Component::ParentDir | Component::CurDir)),
                "Invalid absolute session directory"
            );
            let sid = crate::windows_control::current_identity()?.user;
            let mut pins = Vec::new();
            let ancestors = path.ancestors().collect::<Vec<_>>();
            for (index, ancestor) in ancestors.into_iter().enumerate().rev() {
                let leaf = index == 0;
                if index <= 1 {
                    let sddl = descriptor(&sid)?;
                    let attrs = attributes(&sddl);
                    if let Err(error) =
                        unsafe { CreateDirectoryW(PCWSTR(wide(ancestor)?.as_ptr()), Some(&attrs)) }
                        && error.code() != ERROR_ALREADY_EXISTS.to_hresult()
                    {
                        return Err(error.into());
                    }
                }
                let directory = OpenOptions::new()
                    .access_mode(if leaf { 0x20080 } else { 0x80 }) // READ_CONTROL, FILE_READ_ATTRIBUTES
                    .share_mode(3) // Reserve the component against rename/delete.
                    .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
                    .open(ancestor)?;
                let meta = directory.metadata()?;
                ensure!(
                    meta.is_dir() && meta.file_attributes() & 0x400 == 0,
                    "Session directory contains a reparse point or non-directory"
                );
                if leaf {
                    verify_descriptor(&directory, &sid)?;
                }
                pins.push(directory);
            }
            Ok(Self {
                path: path.to_owned(),
                _pins: pins,
                sid,
            })
        }

        pub(super) fn create_new(&self, name: &str) -> Result<File> {
            let sddl = descriptor(&self.sid)?;
            let attrs = attributes(&sddl);
            let wide = wide(&self.path.join(name))?;
            let handle = match unsafe {
                CreateFileW(
                    PCWSTR(wide.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_READ,
                    Some(&attrs),
                    CREATE_NEW,
                    FILE_FLAG_OPEN_REPARSE_POINT,
                    None,
                )
            } {
                Err(error)
                    if error.code() == ERROR_FILE_EXISTS.to_hresult()
                        || error.code() == ERROR_ALREADY_EXISTS.to_hresult() =>
                {
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists, error).into());
                }
                result => result?,
            };
            let file = unsafe { File::from_raw_handle(handle.0) };
            let mut info = BY_HANDLE_FILE_INFORMATION::default();
            unsafe {
                GetFileInformationByHandle(handle, &mut info)?;
            }
            ensure!(
                file.metadata()?.is_file()
                    && info.dwFileAttributes & 0x400 == 0
                    && info.nNumberOfLinks == 1,
                "Session journal must be a single-link non-reparse file"
            );
            verify_descriptor(&file, &self.sid)?;
            Ok(file)
        }
    }

    fn wide(path: &Path) -> Result<Vec<u16>> {
        let mut value = path.as_os_str().encode_wide().collect::<Vec<_>>();
        ensure!(!value.contains(&0), "Session path contains a null");
        value.push(0);
        Ok(value)
    }

    fn descriptor(sid: &str) -> Result<Local> {
        let text = format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FA;;;SY)");
        let wide = text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(wide.as_ptr()),
                1,
                &mut sd,
                None,
            )?;
        }
        Ok(Local(HLOCAL(sd.0)))
    }

    fn attributes(sd: &Local) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0.0,
            bInheritHandle: false.into(),
        }
    }

    fn verify_descriptor(file: &File, sid: &str) -> Result<()> {
        let flags = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let mut actual = PSECURITY_DESCRIPTOR::default();
        unsafe {
            GetSecurityInfo(
                HANDLE(file.as_raw_handle()),
                SE_FILE_OBJECT,
                flags,
                None,
                None,
                None,
                None,
                Some(&mut actual),
            )
            .ok()?;
        }
        let actual = Local(HLOCAL(actual.0));
        let expected = descriptor(sid)?;
        let sddl = |value: &Local| -> Result<String> {
            let mut text = PWSTR::null();
            unsafe {
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    PSECURITY_DESCRIPTOR(value.0.0),
                    1,
                    flags,
                    &mut text,
                    None,
                )?;
            }
            let _allocation = Local(HLOCAL(text.0.cast()));
            Ok(unsafe { text.to_string()? })
        };
        ensure!(
            sddl(&actual)? == sddl(&expected)?,
            "Unexpected owner or ACL on the session journal path"
        );
        Ok(())
    }
}
