use anyhow::{Context, Result, ensure};
use std::fs::File;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::{fs, io::Write, path::Path};

/// Inspect one existing private directory without creating or repairing it.
/// Require current-user ownership, a real non-reparse directory, no Unix
/// group/other access, and only current-user/SYSTEM access on Windows. This is
/// a read-only observation, not a lock or authorization for later mutations.
pub fn validate_private_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).context("Inspect private directory")?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Private storage must be a real directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o077 == 0,
            "Directory is not private current-user storage"
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                == 0,
            "Private storage must not be a reparse directory"
        );
        validate_windows_private_access(path)?;
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn validate_windows_private_access(path: &Path) -> Result<()> {
    // Same current-user/SYSTEM-only inspection as the driver's private storage
    // policy (config::windows_access); observation never changes an ACL/owner.
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            ACCESS_ALLOWED_ACE,
            Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT},
            DACL_SECURITY_INFORMATION, GetAce, OWNER_SECURITY_INFORMATION,
        },
    };
    unsafe fn sid_text(sid: *mut core::ffi::c_void) -> Result<String> {
        unsafe {
            let mut rendered = ptr::null_mut();
            ensure!(
                ConvertSidToStringSidW(sid, &mut rendered) != 0,
                "Cannot read private storage SID"
            );
            let mut size = 0;
            while *rendered.add(size) != 0 {
                size += 1;
            }
            let text = String::from_utf16(std::slice::from_raw_parts(rendered, size));
            LocalFree(rendered.cast());
            Ok(text?)
        }
    }
    let path = crate::windows_native_path(path)?;
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    let code = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    ensure!(
        code == 0,
        "Cannot inspect private storage owner/policy ({code})"
    );
    let result = (|| -> Result<()> {
        ensure!(
            !owner.is_null() && !acl.is_null(),
            "Private storage owner/policy missing"
        );
        let user = crate::current_user_sid()?;
        ensure!(
            unsafe { sid_text(owner)? } == user,
            "Private storage belongs to another principal"
        );
        let count = unsafe { (*acl).AceCount };
        ensure!(
            (1..=1024).contains(&count),
            "Unexpected private storage ACL"
        );
        for index in 0..count {
            let mut entry = ptr::null_mut();
            ensure!(
                unsafe { GetAce(acl, u32::from(index), &mut entry) } != 0,
                "Cannot inspect private storage ACL entry"
            );
            let entry = entry.cast::<ACCESS_ALLOWED_ACE>();
            ensure!(
                unsafe { (*entry).Header.AceType } == 0,
                "Unsupported private storage ACL entry"
            );
            let principal = unsafe { sid_text(ptr::addr_of_mut!((*entry).SidStart).cast())? };
            ensure!(
                principal == user || principal == "S-1-5-18",
                "Private storage allows another principal"
            );
        }
        Ok(())
    })();
    unsafe {
        LocalFree(descriptor.cast());
    }
    result
}

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

/// A bootstrap executable is bounded independently of the JSON setup frame.
#[cfg(feature = "installation")]
pub const MAX_PRIVATE_EXECUTABLE_BYTES: u64 = 128 * 1024 * 1024;

/// Read-only evidence about one private executable, not an immutable lease.
#[cfg(feature = "installation")]
#[derive(Debug, PartialEq, Eq)]
pub struct PrivateExecutableSnapshot {
    pub sha256: [u8; 32],
    pub size: u64,
    pub identity: serde_json::Value,
}

/// Observe an existing executable without repairing its owner or permissions.
/// Callers also validate the complete installation path before using this helper.
#[cfg(feature = "installation")]
pub fn private_executable_snapshot(path: &Path) -> Result<PrivateExecutableSnapshot> {
    use crate::installation_exchange::native_identity;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    validate_private_directory(
        path.parent()
            .context("Executable needs an existing parent")?,
    )?;
    let before = executable_metadata(path)?;
    let identity = native_identity(path, &before)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::{
            Foundation::{GENERIC_EXECUTE, GENERIC_READ},
            Storage::FileSystem::{
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ,
            },
        };
        // Verify that this user can read and execute the existing private image.
        options
            .access_mode(GENERIC_READ | GENERIC_EXECUTE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    ensure!(
        opened_file_matches_path(&file, path)?,
        "Executable changed before opening"
    );
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        ensure!(
            size <= MAX_PRIVATE_EXECUTABLE_BYTES,
            "Executable exceeds size limit"
        );
        hasher.update(&buffer[..count]);
    }
    let after = executable_metadata(path)?;
    ensure!(
        size == before.len()
            && size == after.len()
            && before.modified()? == after.modified()?
            && native_identity(path, &after)? == identity
            && opened_file_matches_path(&file, path)?,
        "Executable changed during observation"
    );
    Ok(PrivateExecutableSnapshot {
        sha256: hasher.finalize().into(),
        size,
        identity,
    })
}

#[cfg(all(feature = "installation", unix))]
fn opened_file_matches_path(file: &File, path: &Path) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let opened = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    Ok(opened.dev() == current.dev() && opened.ino() == current.ino())
}

#[cfg(all(feature = "installation", windows))]
fn opened_file_matches_path(file: &File, path: &Path) -> Result<bool> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
    };
    fn identity(file: &File) -> Result<(u32, u32, u32)> {
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } != 0,
            "Cannot inspect executable handle identity: {}",
            std::io::Error::last_os_error()
        );
        Ok((
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
        ))
    }
    // Access 0 permits identity observation even while our CREATE_NEW writer
    // retains its no-sharing handle. This grants no read/write/delete authority.
    let current = fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    Ok(identity(file)? == identity(&current)?)
}

#[cfg(feature = "installation")]
fn executable_metadata(path: &Path) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Private executable must be an ordinary file"
    );
    ensure!(
        metadata.len() > 0 && metadata.len() <= MAX_PRIVATE_EXECUTABLE_BYTES,
        "Executable size is invalid"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o7077 == 0
                && metadata.mode() & 0o500 == 0o500,
            "Executable must be private current-user readable/executable storage"
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Executable is a reparse point"
        );
        validate_windows_private_access(path)?;
    }
    Ok(metadata)
}

/// Publish one new executable from an already opened, caller-verified image.
/// The existing private parent is never recreated or repaired. Publication is
/// no-replace; any error may require read-only reconciliation of the destination.
/// The setup entrypoint supplies only its own current executable as `source`.
#[cfg(feature = "installation")]
pub fn private_create_executable(
    path: &Path,
    source: &mut File,
    sha256: &[u8; 32],
    size: u64,
) -> Result<PrivateExecutableSnapshot> {
    create_executable_with(path, source, sha256, size, || Ok(()), || Ok(()))
}

#[cfg(feature = "installation")]
fn create_executable_with(
    path: &Path,
    source: &mut File,
    expected: &[u8; 32],
    size: u64,
    before_publish: impl FnOnce() -> Result<()>,
    after_publish: impl FnOnce() -> Result<()>,
) -> Result<PrivateExecutableSnapshot> {
    use crate::installation_exchange::native_identity;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek};

    let parent = path
        .parent()
        .context("Executable needs an existing private parent")?;
    validate_private_directory(parent)?;
    let parent_identity = native_identity(parent, &fs::symlink_metadata(parent)?)?;
    ensure!(
        size > 0 && size <= MAX_PRIVATE_EXECUTABLE_BYTES,
        "Executable size is invalid"
    );
    match fs::symlink_metadata(path) {
        Ok(_) => return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let source_before = source.metadata()?;
    ensure!(
        source_before.is_file() && source_before.len() == size,
        "Source image changed"
    );
    source.rewind()?;
    let temporary = parent.join(format!(".driver-{}.tmp", uuid::Uuid::new_v4()));
    let mut output = create_private_executable_file(&temporary)?;
    let temporary_identity = native_identity(&temporary, &fs::symlink_metadata(&temporary)?)?;
    ensure!(
        opened_file_matches_path(&output, &temporary)?,
        "Executable temporary identity changed"
    );
    let result = (|| -> Result<PrivateExecutableSnapshot> {
        let mut hasher = Sha256::new();
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            copied += count as u64;
            ensure!(copied <= size, "Source image grew during copying");
            hasher.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        let source_after = source.metadata()?;
        ensure!(
            copied == size
                && source_after.len() == size
                && source_before.modified()? == source_after.modified()?
                && <[u8; 32]>::from(hasher.finalize()) == *expected,
            "Source image changed or differs from expected SHA256"
        );
        output.sync_all()?;
        drop(output);
        before_publish()?;
        validate_private_directory(parent)?;
        ensure!(
            native_identity(parent, &fs::symlink_metadata(parent)?)? == parent_identity,
            "Executable parent changed before publication"
        );
        ensure!(
            native_identity(&temporary, &fs::symlink_metadata(&temporary)?)? == temporary_identity,
            "Executable temporary changed before publication"
        );
        let prepared = private_executable_snapshot(&temporary)?;
        ensure!(
            prepared.identity == temporary_identity
                && prepared.sha256 == *expected
                && prepared.size == size,
            "Executable temporary changed before publication"
        );
        commit(&temporary, path, true)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        after_publish()?;
        validate_private_directory(parent)?;
        ensure!(
            native_identity(parent, &fs::symlink_metadata(parent)?)? == parent_identity,
            "Executable parent changed after publication"
        );
        let observed = private_executable_snapshot(path)?;
        ensure!(
            observed.identity == temporary_identity
                && observed.sha256 == *expected
                && observed.size == size,
            "Published executable identity or contents changed"
        );
        Ok(observed)
    })();
    // Only remove our unpublished temporary. Never remove or repair a destination
    // after uncertain publication, nor a replacement at the temporary name.
    if fs::symlink_metadata(&temporary)
        .ok()
        .is_some_and(|metadata| {
            native_identity(&temporary, &metadata).ok().as_ref() == Some(&temporary_identity)
        })
    {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(all(feature = "installation", unix))]
fn create_private_executable_file(path: &Path) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    Ok(fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(path)?)
}

#[cfg(all(feature = "installation", windows))]
fn create_private_executable_file(path: &Path) -> Result<File> {
    create_private_file(path)
}
enum Publication {
    CreateOnly,
    ReplaceClosed,
    ReplaceData,
}
/// Exclusively create one private directory under an existing parent.
/// Existing entries and their policies are never changed; parents are not created.
pub fn private_create_dir(path: &Path) -> std::io::Result<()> {
    create_private_directory(path)
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
pub(crate) fn create_private_directory(path: &Path) -> std::io::Result<()> {
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
pub(crate) struct PrivateSecurity(windows_sys::Win32::Security::PSECURITY_DESCRIPTOR);
#[cfg(windows)]
impl PrivateSecurity {
    pub(crate) fn new(directory: bool) -> Result<Self> {
        use std::ptr;
        use windows_sys::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
        };
        let sid = current_user_sid()?;
        let inherit = if directory { "OICI" } else { "" };
        // The token's default owner can be an administrator group even when
        // TokenUser identifies this installation's user. Bind new artifacts to
        // that exact user at creation; never repair ownership after publication.
        let sddl: Vec<u16> = format!("O:{sid}D:P(A;{inherit};FA;;;SY)(A;{inherit};FA;;;{sid})")
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
    pub(crate) fn attributes(&self) -> windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
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
pub(crate) fn wide_path(path: &Path) -> Result<Vec<u16>> {
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
pub(crate) fn create_private_directory(path: &Path) -> std::io::Result<()> {
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
pub(crate) fn publish_directory_native(from: &Path, to: &Path) -> Result<()> {
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
pub(crate) fn publish_directory_native(from: &Path, to: &Path) -> Result<()> {
    commit(from, to, true)
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) fn publish_directory_native(_from: &Path, _to: &Path) -> Result<()> {
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
                OWNER_SECURITY_INFORMATION, SE_DACL_PROTECTED,
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
            let mut owner = ptr::null_mut();
            assert_eq!(
                GetSecurityInfo(
                    file.as_raw_handle(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION,
                    &mut owner,
                    ptr::null_mut(),
                    &mut acl,
                    ptr::null_mut(),
                    &mut descriptor
                ),
                0
            );
            assert!(!owner.is_null());
            let mut owner_text = ptr::null_mut();
            assert_ne!(ConvertSidToStringSidW(owner, &mut owner_text), 0);
            let mut owner_len = 0;
            while *owner_text.add(owner_len) != 0 {
                owner_len += 1;
            }
            let owner =
                String::from_utf16(std::slice::from_raw_parts(owner_text, owner_len)).unwrap();
            LocalFree(owner_text.cast());
            assert_eq!(owner, current_user_sid().unwrap());
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

    #[derive(Debug, PartialEq, Eq)]
    struct PolicySnapshot {
        owner: String,
        descriptor: String,
        identity: (u32, u32, u32),
    }
    fn policy_snapshot(path: &Path) -> PolicySnapshot {
        use std::{os::windows::io::AsRawHandle, ptr};
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::{
                Authorization::{
                    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
                    GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
                },
                DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
            },
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };
        unsafe fn string(value: *const u16) -> String {
            unsafe {
                let mut len = 0;
                while *value.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16(std::slice::from_raw_parts(value, len)).unwrap()
            }
        }
        let name = wide_path(path).unwrap();
        unsafe {
            let mut owner = ptr::null_mut();
            let mut descriptor = ptr::null_mut();
            let flags = DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION;
            assert_eq!(
                GetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    flags,
                    &mut owner,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut descriptor,
                ),
                0
            );
            let mut owner_text = ptr::null_mut();
            assert_ne!(ConvertSidToStringSidW(owner, &mut owner_text), 0);
            let owner = string(owner_text);
            LocalFree(owner_text.cast());
            let mut rendered = ptr::null_mut();
            assert_ne!(
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor,
                    SDDL_REVISION_1,
                    flags,
                    &mut rendered,
                    ptr::null_mut(),
                ),
                0
            );
            let descriptor_text = string(rendered);
            LocalFree(rendered.cast());
            LocalFree(descriptor);
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            let file = fs::OpenOptions::new()
                .access_mode(0)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)
                .unwrap();
            let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            assert_ne!(
                GetFileInformationByHandle(file.as_raw_handle(), &mut info),
                0
            );
            PolicySnapshot {
                owner,
                descriptor: descriptor_text,
                identity: (
                    info.dwVolumeSerialNumber,
                    info.nFileIndexHigh,
                    info.nFileIndexLow,
                ),
            }
        }
    }
    #[test]
    fn published_file_and_created_parent_have_explicit_current_user_owner() {
        let temp = tempfile::tempdir().unwrap();
        let baseline = policy_snapshot(temp.path());
        let parent = temp.path().join("new private parent");
        let file = parent.join("receipt.json");
        // Supported file publication creates the absent private parent through
        // create_private_directory, then atomically publishes the private file.
        private_write(&file, b"first receipt").unwrap();
        let sid = current_user_sid().unwrap();
        let parent_before = policy_snapshot(&parent);
        assert_eq!(parent_before.owner, sid);
        assert_eq!(policy_snapshot(&file).owner, sid);
        private_write(&file, b"replacement receipt").unwrap();
        assert_eq!(policy_snapshot(&file).owner, sid);
        assert_eq!(fs::read(&file).unwrap(), b"replacement receipt");
        assert_eq!(policy_snapshot(&parent), parent_before);
        assert_eq!(policy_snapshot(temp.path()), baseline);
    }
    #[cfg(feature = "installation")]
    #[test]
    fn executable_temporary_has_current_user_policy_before_contents() {
        let temp = tempfile::tempdir().unwrap();
        let before = policy_snapshot(temp.path());
        let path = temp.path().join("new executable");
        let file = create_private_executable_file(&path).unwrap();
        assert_eq!(file.metadata().unwrap().len(), 0);
        assert!(opened_file_matches_path(&file, &path).unwrap());
        assert!(
            crate::installation_exchange::native_identity(&path, &file.metadata().unwrap()).is_ok()
        );
        assert_eq!(policy_snapshot(&path).owner, current_user_sid().unwrap());
        validate_windows_private_access(&path).unwrap();
        assert_eq!(policy_snapshot(temp.path()), before);
    }
    #[cfg(feature = "installation")]
    #[test]
    fn new_lock_is_user_owned_and_reopening_preserves_existing_policy_and_identity() {
        use crate::locking::InstallationLock;
        let temp = tempfile::tempdir().unwrap();
        let new_path = temp.path().join("new.lock");
        let new_lock = InstallationLock::try_acquire(&new_path).unwrap().unwrap();
        let new_policy = policy_snapshot(&new_path);
        assert_eq!(new_policy.owner, current_user_sid().unwrap());
        drop(new_lock);
        fs::write(&new_path, b"retained lock contents").unwrap();
        let reopened = InstallationLock::try_acquire(&new_path).unwrap().unwrap();
        assert_eq!(policy_snapshot(&new_path), new_policy);
        drop(reopened);
        assert_eq!(fs::read(&new_path).unwrap(), b"retained lock contents");

        // The legacy file keeps its preexisting native/default policy too.
        // OPEN_ALWAYS must not replace it with the new-artifact descriptor.
        let legacy = temp.path().join("legacy.lock");
        fs::write(&legacy, b"legacy contents").unwrap();
        let before = policy_snapshot(&legacy);
        let held = InstallationLock::try_acquire(&legacy).unwrap().unwrap();
        assert_eq!(policy_snapshot(&legacy), before);
        assert!(InstallationLock::try_acquire(&legacy).unwrap().is_none());
        drop(held);
        assert_eq!(policy_snapshot(&legacy), before);
        assert_eq!(fs::read(&legacy).unwrap(), b"legacy contents");
    }
}

#[cfg(all(test, feature = "installation"))]
mod executable_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, File, [u8; 32]) {
        let temporary = tempfile::tempdir().unwrap();
        let parent = temporary.path().canonicalize().unwrap().join("private");
        private_create_dir(&parent).unwrap();
        let source = temporary.path().join("source");
        fs::write(&source, b"verified running image").unwrap();
        let digest = Sha256::digest(b"verified running image").into();
        (
            temporary,
            parent.join("driver"),
            File::open(source).unwrap(),
            digest,
        )
    }

    #[test]
    fn executable_copy_rechecks_hash_and_never_publishes_changed_source() {
        let (_temporary, path, mut source, digest) = fixture();
        let result = private_create_executable(&path, &mut source, &digest, 22);
        assert!(result.is_ok(), "{result:?}");
        let observed = result.unwrap();
        assert_eq!(observed.sha256, digest);
        assert_eq!(observed.size, 22);
        assert_eq!(private_executable_snapshot(&path).unwrap(), observed);
        let other = path.with_file_name("wrong");
        assert!(private_create_executable(&other, &mut source, &[0; 32], 22).is_err());
        assert!(!other.exists());
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn executable_copy_uses_verified_open_source_not_its_replaced_path() {
        let (temporary, path, mut source, digest) = fixture();
        let source_path = temporary.path().join("source");
        fs::rename(&source_path, temporary.path().join("original-source")).unwrap();
        fs::write(&source_path, b"untrusted replacement").unwrap();
        let observed = private_create_executable(&path, &mut source, &digest, 22).unwrap();
        assert_eq!(observed.sha256, digest);
        assert_eq!(fs::read(&path).unwrap(), b"verified running image");
        assert_eq!(fs::read(&source_path).unwrap(), b"untrusted replacement");
    }

    #[test]
    fn executable_source_mutation_after_preflight_never_publishes() {
        let (temporary, path, mut source, digest) = fixture();
        fs::write(temporary.path().join("source"), [b'x'; 22]).unwrap();
        assert!(private_create_executable(&path, &mut source, &digest, 22).is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 0);
    }

    #[test]
    fn executable_publication_does_not_replace_a_concurrent_destination() {
        let (_temporary, path, mut source, digest) = fixture();
        let result = create_executable_with(
            &path,
            &mut source,
            &digest,
            22,
            || {
                private_create(&path, b"concurrent owner data")?;
                Ok(())
            },
            || Ok(()),
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"concurrent owner data");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn executable_prepublication_content_drift_does_not_publish() {
        let (_temporary, path, mut source, digest) = fixture();
        let result = create_executable_with(
            &path,
            &mut source,
            &digest,
            22,
            || {
                let temporary = fs::read_dir(path.parent().unwrap())?
                    .next()
                    .unwrap()?
                    .path();
                fs::write(temporary, [b'x'; 22])?;
                Ok(())
            },
            || Ok(()),
        );
        assert!(result.is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn executable_prepublication_public_mode_does_not_publish_or_repair() {
        use std::os::unix::fs::PermissionsExt;
        let (_temporary, path, mut source, digest) = fixture();
        let result = create_executable_with(
            &path,
            &mut source,
            &digest,
            22,
            || {
                let temporary = fs::read_dir(path.parent().unwrap())?
                    .next()
                    .unwrap()?
                    .path();
                fs::set_permissions(temporary, fs::Permissions::from_mode(0o755))?;
                Ok(())
            },
            || Ok(()),
        );
        assert!(result.is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 0);
    }

    #[test]
    fn executable_parent_loss_does_not_recreate_any_directory() {
        let (_temporary, path, mut source, digest) = fixture();
        let parent = path.parent().unwrap();
        let moved = parent.with_file_name("moved");
        let result = create_executable_with(
            &path,
            &mut source,
            &digest,
            22,
            || {
                fs::rename(parent, &moved)?;
                Ok(())
            },
            || Ok(()),
        );
        assert!(result.is_err());
        assert!(!parent.exists());
        assert!(!moved.join("driver").exists());
    }

    #[test]
    fn executable_postpublication_drift_preserves_replacement() {
        let (_temporary, path, mut source, digest) = fixture();
        let original = path.with_file_name("published-original");
        let result = create_executable_with(
            &path,
            &mut source,
            &digest,
            22,
            || Ok(()),
            || {
                fs::rename(&path, &original)?;
                let mut replacement = create_private_executable_file(&path)?;
                replacement.write_all(b"verified running image")?;
                replacement.sync_all()?;
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"verified running image");
        assert_eq!(fs::read(&original).unwrap(), b"verified running image");
    }

    #[cfg(unix)]
    #[test]
    fn executable_mode_is_private_and_executable_before_any_write() {
        use std::os::unix::fs::MetadataExt;
        let (_temporary, path, _source, _digest) = fixture();
        let file = create_private_executable_file(&path).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.len(), 0);
        assert_eq!(metadata.mode() & 0o7777, 0o700);
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    }
}
