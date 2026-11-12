//! Lifetime locks shared with an installation's existing local processes.
//!
//! The caller owns the parent directory. A lock file is never removed, replaced,
//! or truncated: on Unix this preserves interoperability with existing `flock`
//! users holding the same inode. Closing the handle releases the lock, including
//! when its process exits. Handles do not survive an executed child process.

use std::{fs::File, io, path::Path};

#[derive(Debug)]
pub struct InstallationLock {
    _file: File,
}

impl InstallationLock {
    /// Acquire without waiting. `None` means an existing lock is contended;
    /// inaccessible, invalid, or unsupported paths remain errors.
    pub fn try_acquire(path: &Path) -> io::Result<Option<Self>> {
        let file = open_lock_file(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Installation lock must be a regular file",
            ));
        }
        if acquire(&file)? {
            Ok(Some(Self { _file: file }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(unix)]
fn open_lock_file(path: &Path) -> io::Result<File> {
    use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        // NONBLOCK prevents a malicious FIFO from hanging before the type check.
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

#[cfg(unix)]
fn acquire(file: &File) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    loop {
        // SAFETY: file owns a valid descriptor for the entire call.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            return Ok(false);
        }
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(windows)]
fn open_lock_file(path: &Path) -> io::Result<File> {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            FILE_SHARE_WRITE, FILE_TYPE_DISK, GetFileInformationByHandle, GetFileType, OPEN_ALWAYS,
        },
    };
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in lock path",
        ));
    }
    name.push(0);
    // SAFETY: the path is terminated. Null security attributes create a
    // non-inheritable handle; OPEN_ALWAYS leaves existing bytes unchanged.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_ALWAYS,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ownership of the successful CreateFileW handle is transferred once.
    let file = unsafe { File::from_raw_handle(handle) };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: the owned file handle and output buffer are valid.
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // Reject the opened object itself, without following a reparse target.
    if info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || unsafe { GetFileType(handle) } != FILE_TYPE_DISK
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Installation lock must be a regular non-reparse file",
        ));
    }
    Ok(file)
}

#[cfg(windows)]
fn acquire(file: &File) -> io::Result<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::ERROR_LOCK_VIOLATION,
        Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx},
        System::IO::OVERLAPPED,
    };
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    // SAFETY: the file is synchronous; the zeroed OVERLAPPED selects offset zero
    // and remains valid throughout this immediate, non-overlapped call.
    if unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    } != 0
    {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
        Ok(false)
    } else {
        Err(error)
    }
}

#[cfg(not(any(unix, windows)))]
fn open_lock_file(_: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Unsupported platform",
    ))
}

#[cfg(not(any(unix, windows)))]
fn acquire(_: &File) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Unsupported platform",
    ))
}
