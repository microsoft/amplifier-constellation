use anyhow::{Context, Result, ensure};
#[cfg(unix)]
use std::fs::File;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

/// Create or restrict an application-owned directory to this user.
pub fn private_dir(path: &Path) -> Result<()> {
    ensure!(
        !path.is_symlink(),
        "Private folders cannot be symbolic links"
    );
    fs::create_dir_all(path)?;
    ensure!(path.is_dir(), "Private folder must be a directory");
    private_permissions(path, true)
}
/// Restrict an existing ordinary file to this user (and Windows SYSTEM).
pub fn protect_file(path: &Path) -> Result<()> {
    ensure!(
        !path.is_symlink() && path.is_file(),
        "Private files must be ordinary files"
    );
    private_permissions(path, false)
}

#[cfg(unix)]
fn private_permissions(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )?;
    Ok(())
}

#[cfg(windows)]
pub fn current_user_sid() -> Result<String> {
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
            TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    unsafe {
        let mut token = ptr::null_mut();
        ensure!(
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) != 0,
            "Cannot identify this Windows user"
        );
        let mut needed = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
        let mut buffer = vec![0_usize; (needed as usize).div_ceil(size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        );
        CloseHandle(token);
        ensure!(ok != 0, "Cannot read this Windows user's identity");
        let user = &*(buffer.as_ptr().cast::<TOKEN_USER>());
        let mut sid = ptr::null_mut();
        ensure!(
            ConvertSidToStringSidW(user.User.Sid, &mut sid) != 0,
            "Cannot describe this Windows user"
        );
        let mut len = 0;
        while *sid.add(len) != 0 {
            len += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(sid, len));
        LocalFree(sid.cast());
        Ok(result)
    }
}
#[cfg(windows)]
fn private_permissions(path: &Path, _directory: bool) -> Result<()> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SetFileSecurityW,
        },
    };
    let sid = current_user_sid()?;
    unsafe {
        let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        ensure!(
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut()
            ) != 0,
            "Cannot create private storage policy"
        );
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = SetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        LocalFree(descriptor);
        ensure!(
            result != 0,
            "Cannot restrict application storage to this Windows user"
        );
    }
    Ok(())
}

/// Atomically replace an application-owned file after durably writing a protected temp file.
pub fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, false)
}
/// Atomically create a protected file; an existing destination is never overwritten.
pub fn private_create(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, true)
}
fn write_atomic(path: &Path, bytes: &[u8], create_only: bool) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .context("Private file needs a parent folder")?;
    ensure!(!path.is_symlink(), "Private files cannot be symbolic links");
    ensure!(
        !path.exists() || path.is_file(),
        "Private file is not an ordinary file"
    );
    private_dir(parent)?;
    let temporary = parent.join(format!(".save-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        private_permissions(&temporary, false)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        commit(&temporary, path, create_only)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
#[cfg(not(windows))]
fn commit(from: &Path, to: &Path, create_only: bool) -> Result<()> {
    if create_only {
        fs::hard_link(from, to)?;
        fs::remove_file(from)?;
    } else {
        fs::rename(from, to)?;
    }
    Ok(())
}
#[cfg(windows)]
fn commit(from: &Path, to: &Path, create_only: bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let flags = MOVEFILE_WRITE_THROUGH
        | if create_only {
            0
        } else {
            MOVEFILE_REPLACE_EXISTING
        };
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } == 0 {
        return Err(std::io::Error::last_os_error()).context("Cannot commit private state");
    }
    Ok(())
}
