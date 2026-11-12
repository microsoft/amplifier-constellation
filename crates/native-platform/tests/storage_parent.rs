use amplifier_native_platform::{private_create, private_write};
use std::{fs, path::Path};

#[cfg(unix)]
fn make_public(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
#[cfg(unix)]
fn parent_policy(path: &Path) -> Vec<u8> {
    use std::os::unix::fs::PermissionsExt;
    (fs::metadata(path).unwrap().permissions().mode() & 0o777)
        .to_le_bytes()
        .to_vec()
}
#[cfg(windows)]
fn make_public(path: &Path) {
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
    let sid = amplifier_native_platform::current_user_sid().unwrap();
    let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)(A;OICI;GRGX;;;BU)")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        assert_ne!(
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut()
            ),
            0
        );
        let result = SetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        LocalFree(descriptor);
        assert_ne!(result, 0);
    }
}
#[cfg(windows)]
fn parent_policy(path: &Path) -> Vec<u8> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, GetFileSecurityW};
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut needed = 0;
        GetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            0,
            &mut needed,
        );
        assert!(needed > 0 && needed < 65536);
        let mut storage = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        assert_ne!(
            GetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION,
                storage.as_mut_ptr().cast(),
                needed,
                &mut needed
            ),
            0
        );
        std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), needed as usize).to_vec()
    }
}

#[test]
fn writing_or_creating_token_does_not_restrict_existing_public_parent() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("Shared documents — 使用者");
    fs::create_dir(&parent).unwrap();
    make_public(&parent);
    let before = parent_policy(&parent);
    let token = parent.join("token-out.json");
    private_create(&token, b"first private token").unwrap();
    assert_eq!(parent_policy(&parent), before);
    private_write(&token, b"replacement private token").unwrap();
    assert_eq!(parent_policy(&parent), before);
    assert_eq!(fs::read(token).unwrap(), b"replacement private token");
    assert_eq!(fs::read_dir(parent).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn new_directories_and_files_are_private_but_existing_ancestor_is_unchanged() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    make_public(temp.path());
    let before = parent_policy(temp.path());
    let parent = temp.path().join("new").join("nested");
    private_write(&parent.join("token"), b"secret fixture").unwrap();
    assert_eq!(parent_policy(temp.path()), before);
    for path in [temp.path().join("new"), parent.clone()] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert_eq!(
        fs::metadata(parent.join("token"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn concurrent_parent_creator_keeps_its_own_permissions() {
    use std::{
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        sync::{Arc, Barrier},
    };
    for _ in 0..32 {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("contended");
        let barrier = Arc::new(Barrier::new(2));
        let creator_barrier = barrier.clone();
        let created = parent.clone();
        let creator = std::thread::spawn(move || {
            creator_barrier.wait();
            fs::DirBuilder::new().mode(0o755).create(created).is_ok()
        });
        barrier.wait();
        private_create(&parent.join("token"), b"private").unwrap();
        let external_created = creator.join().unwrap();
        let mode = fs::metadata(parent).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, if external_created { 0o755 } else { 0o700 });
    }
}
