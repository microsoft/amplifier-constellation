use anyhow::{Context, Result, ensure};
use std::fs::File;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::{fs, io::Write, path::Path};

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
pub(crate) fn account_sid(account: &str) -> Result<String> {
    ensure!(
        !account.is_empty() && account.len() <= 1024 && !account.chars().any(char::is_control),
        "Invalid Windows account identity"
    );
    if account.starts_with("S-1-") {
        return Ok(account.to_owned());
    }
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, LocalFree},
        Security::{Authorization::ConvertSidToStringSidW, LookupAccountNameW},
    };
    let account: Vec<u16> = account.encode_utf16().chain(Some(0)).collect();
    let mut sid_bytes = 0;
    let mut domain_chars = 0;
    let mut sid_type = 0;
    unsafe {
        LookupAccountNameW(
            ptr::null(),
            account.as_ptr(),
            ptr::null_mut(),
            &mut sid_bytes,
            ptr::null_mut(),
            &mut domain_chars,
            &mut sid_type,
        );
        ensure!(
            std::io::Error::last_os_error().raw_os_error()
                == Some(ERROR_INSUFFICIENT_BUFFER as i32)
                && (1..=256).contains(&sid_bytes)
                && domain_chars <= 32768,
            "Cannot resolve startup task account"
        );
        // Native SIDs contain DWORD fields and require an aligned buffer.
        let mut sid = vec![0u32; (sid_bytes as usize).div_ceil(4)];
        let mut domain = vec![0u16; domain_chars as usize];
        ensure!(
            LookupAccountNameW(
                ptr::null(),
                account.as_ptr(),
                sid.as_mut_ptr().cast(),
                &mut sid_bytes,
                domain.as_mut_ptr(),
                &mut domain_chars,
                &mut sid_type,
            ) != 0,
            "Cannot resolve startup task account"
        );
        let mut rendered = ptr::null_mut();
        ensure!(
            ConvertSidToStringSidW(sid.as_mut_ptr().cast(), &mut rendered) != 0,
            "Cannot read startup task account SID"
        );
        let mut length = 0;
        while *rendered.add(length) != 0 {
            length += 1;
        }
        let result = String::from_utf16(std::slice::from_raw_parts(rendered, length));
        LocalFree(rendered.cast());
        Ok(result?)
    }
}
#[cfg(windows)]
fn private_permissions(path: &Path, _directory: bool) -> Result<()> {
    use std::ptr;
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
    let wide = wide_path(path)?;
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
        let result = SetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        let error = (result == 0).then(std::io::Error::last_os_error);
        LocalFree(descriptor);
        if let Some(error) = error {
            return Err(error).context("Cannot restrict application storage to this Windows user");
        }
    }
    Ok(())
}

/// Atomically replace an application-owned file after durably writing a protected temp file.
pub fn private_write(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, Publication::ReplaceClosed)
}
/// Publish a protected data snapshot while existing readers retain the old file.
///
/// On Windows, readers must allow delete sharing. A reader denying that access,
/// a read-only destination, or a failed flush remains an error. A flush error
/// after publication is uncertain: callers must inspect their durable operation,
/// not assume the old file remains. This is for data journals, not executable or
/// directory replacement; use the existing installation lifecycle for those.
pub fn private_write_data(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, Publication::ReplaceData)
}
/// Atomically create a protected file; an existing destination is never overwritten.
pub fn private_create(path: &Path, bytes: &[u8]) -> Result<()> {
    write_atomic(path, bytes, Publication::CreateOnly)
}
enum Publication {
    CreateOnly,
    ReplaceClosed,
    ReplaceData,
}
fn write_atomic(path: &Path, bytes: &[u8], publication: Publication) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .context("Private file needs a parent folder")?;
    ensure!(!path.is_symlink(), "Private files cannot be symbolic links");
    ensure!(
        !path.exists() || path.is_file(),
        "Private file is not an ordinary file"
    );
    prepare_parent(parent)?;
    let temporary = parent.join(format!(".save-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = create_private_file(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        match publication {
            Publication::CreateOnly => commit(&temporary, path, true)?,
            Publication::ReplaceClosed => commit(&temporary, path, false)?,
            Publication::ReplaceData => commit_data(&temporary, path)?,
        }
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Writing a token into Documents must not change Documents' permissions.
/// Only directories created by this operation receive our private policy.
fn prepare_parent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Private file parent must be an ordinary directory"
            );
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        prepare_parent(parent)?;
    }
    match create_private_directory(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            // Another caller created it after our inspection. Do not chmod it.
            let metadata = fs::symlink_metadata(path)?;
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Private file parent changed during creation"
            );
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}
#[cfg(unix)]
fn create_private_file(path: &Path) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    Ok(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?)
}

#[cfg(windows)]
struct PrivateSecurity(windows_sys::Win32::Security::PSECURITY_DESCRIPTOR);
#[cfg(windows)]
impl PrivateSecurity {
    fn new(directory: bool) -> Result<Self> {
        use std::ptr;
        use windows_sys::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
        };
        let sid = current_user_sid()?;
        let inherit = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> = format!("D:P(A;{inherit};FA;;;SY)(A;{inherit};FA;;;{sid})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        ensure!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    ptr::null_mut(),
                )
            } != 0,
            "Cannot create private storage policy"
        );
        Ok(Self(descriptor))
    }
    fn attributes(&self) -> windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
        windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }
}
#[cfg(windows)]
impl Drop for PrivateSecurity {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::LocalFree(self.0.cast());
        }
    }
}
#[cfg(windows)]
fn wide_path(path: &Path) -> Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    // std::fs accepts long paths, but direct Win32 calls still need their
    // extended-length form. Resolve relative/dot components without following
    // links or requiring a not-yet-created destination to exist.
    let absolute = std::path::absolute(path)?;
    let raw: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    ensure!(!raw.contains(&0), "Private path contains a null character");
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    let device: Vec<u16> = r"\\.\".encode_utf16().collect();
    let mut native = if raw.starts_with(&verbatim) || raw.starts_with(&device) {
        raw
    } else if raw.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        r"\\?\UNC\"
            .encode_utf16()
            .chain(raw.into_iter().skip(2))
            .collect()
    } else {
        verbatim.into_iter().chain(raw).collect()
    };
    native.push(0);
    Ok(native)
}
#[cfg(windows)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
    let security = PrivateSecurity::new(true).map_err(std::io::Error::other)?;
    let path = wide_path(path).map_err(std::io::Error::other)?;
    if unsafe { CreateDirectoryW(path.as_ptr(), &security.attributes()) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(windows)]
fn create_private_file(path: &Path) -> Result<File> {
    use std::{os::windows::io::FromRawHandle, ptr};
    use windows_sys::Win32::{
        Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL},
    };
    let security = PrivateSecurity::new(false)?;
    let path = wide_path(path)?;
    // Set the protected user/SYSTEM DACL at CREATE_NEW, before any file exists
    // or bytes are written. The parent ACL is neither inherited nor modified.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            0,
            &security.attributes(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error())
            .context("Cannot create private temporary file");
    }
    Ok(unsafe { File::from_raw_handle(handle) })
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

#[cfg(not(windows))]
fn commit_data(from: &Path, to: &Path) -> Result<()> {
    commit(from, to, false)
}

#[cfg(windows)]
fn commit_data(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE},
        Storage::FileSystem::{
            DELETE, FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            FileRenameInfoEx, SetFileInformationByHandle,
        },
    };
    let destination = wide_path(to)?;
    let size =
        std::mem::offset_of!(FILE_RENAME_INFO, FileName) + destination.len() * size_of::<u16>();
    // The flexible filename must be pointer-aligned and include its UTF-16 NUL;
    // FileNameLength excludes that NUL. No structure-padding bytes are a filename.
    let mut buffer = vec![0_usize; size.div_ceil(size_of::<usize>())];
    let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        // FILE_RENAME_FLAG_REPLACE_IF_EXISTS | FILE_RENAME_FLAG_POSIX_SEMANTICS.
        // Do not add IGNORE_READONLY_ATTRIBUTE or any permission override.
        (*information).Anonymous.Flags = 0x1 | 0x2;
        (*information).FileNameLength = u32::try_from((destination.len() - 1) * 2)?;
        std::ptr::copy_nonoverlapping(
            destination.as_ptr(),
            std::ptr::addr_of_mut!((*information).FileName).cast::<u16>(),
            destination.len(),
        );
    }
    let source = std::fs::OpenOptions::new()
        .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(from)?;
    if unsafe {
        SetFileInformationByHandle(
            source.as_raw_handle(),
            FileRenameInfoEx,
            information.cast(),
            u32::try_from(size)?,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("Cannot publish private data");
    }
    source
        .sync_all()
        .context("Cannot flush published private data")
}
#[cfg(windows)]
fn commit(from: &Path, to: &Path, create_only: bool) -> Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from = wide_path(from)?;
    let to = wide_path(to)?;
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

/// Publish a completed installation directory without replacing any destination,
/// including a concurrently created empty directory. No copying fallback is used.
pub fn publish_directory_if_absent(from: &Path, to: &Path) -> Result<()> {
    ensure!(
        !from.is_symlink() && from.is_dir(),
        "Publish an ordinary staged directory"
    );
    ensure!(!to.is_symlink(), "An existing destination was preserved");
    publish_directory_native(from, to)?;
    #[cfg(unix)]
    {
        if let Some(parent) = from.parent() {
            File::open(parent)?.sync_all()?;
        }
        if to.parent() != from.parent()
            && let Some(parent) = to.parent()
        {
            File::open(parent)?.sync_all()?;
        }
    }
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn publish_directory_native(from: &Path, to: &Path) -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    if result != 0 {
        return Err(std::io::Error::last_os_error()).context(
            "Cannot publish installation; the destination and staged files were preserved",
        );
    }
    Ok(())
}
#[cfg(windows)]
fn publish_directory_native(from: &Path, to: &Path) -> Result<()> {
    commit(from, to, true)
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn publish_directory_native(_from: &Path, _to: &Path) -> Result<()> {
    anyhow::bail!("Atomic installation publication is not supported on this platform")
}

#[cfg(all(test, windows))]
mod creation_tests {
    use super::*;
    #[test]
    fn temporary_file_is_protected_at_creation_before_any_write() {
        use std::{collections::BTreeSet, os::windows::io::AsRawHandle, ptr};
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::{
                ACCESS_ALLOWED_ACE,
                Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_FILE_OBJECT},
                DACL_SECURITY_INFORMATION, GetAce, GetSecurityDescriptorControl, INHERITED_ACE,
                SE_DACL_PROTECTED,
            },
            Storage::FileSystem::FILE_ALL_ACCESS,
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("unwritten private file");
        let file = create_private_file(&path).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 0);
        unsafe {
            let mut descriptor = ptr::null_mut();
            let mut acl = ptr::null_mut();
            assert_eq!(
                GetSecurityInfo(
                    file.as_raw_handle(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut acl,
                    ptr::null_mut(),
                    &mut descriptor
                ),
                0
            );
            let mut control = 0;
            let mut revision = 0;
            assert_ne!(
                GetSecurityDescriptorControl(descriptor, &mut control, &mut revision),
                0
            );
            assert_ne!(control & SE_DACL_PROTECTED, 0);
            assert!(!acl.is_null());
            assert_eq!((*acl).AceCount, 2);
            let mut principals = BTreeSet::new();
            for index in 0..(*acl).AceCount {
                let mut raw = ptr::null_mut();
                assert_ne!(GetAce(acl, u32::from(index), &mut raw), 0);
                let ace = &*raw.cast::<ACCESS_ALLOWED_ACE>();
                assert_eq!(ace.Header.AceType, 0);
                assert_eq!(u32::from(ace.Header.AceFlags) & INHERITED_ACE, 0);
                assert_eq!(ace.Mask, FILE_ALL_ACCESS);
                let mut text = ptr::null_mut();
                assert_ne!(
                    ConvertSidToStringSidW(
                        ptr::addr_of!(ace.SidStart).cast_mut().cast(),
                        &mut text
                    ),
                    0
                );
                let mut len = 0;
                while *text.add(len) != 0 {
                    len += 1;
                }
                principals.insert(String::from_utf16_lossy(std::slice::from_raw_parts(
                    text, len,
                )));
                LocalFree(text.cast());
            }
            LocalFree(descriptor);
            assert_eq!(
                principals,
                BTreeSet::from([current_user_sid().unwrap(), "S-1-5-18".to_owned()])
            );
        }
    }
}
