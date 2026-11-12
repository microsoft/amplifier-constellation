use amplifier_native_platform::*;
use std::{
    fs,
    path::Path,
    sync::{Arc, Barrier},
};

#[test]
fn replace_and_create_only_preserve_complete_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("private state").join("credential");
    private_create(&file, b"original").unwrap();
    assert!(private_create(&file, b"replacement").is_err());
    assert_eq!(fs::read(&file).unwrap(), b"original");
    private_write(&file, b"updated").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"updated");
    assert_eq!(fs::read_dir(file.parent().unwrap()).unwrap().count(), 1);
}
#[test]
fn concurrent_create_has_one_winner_and_no_partial_state() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("credential");
    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|n| {
            let path = file.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let bytes = vec![n; 32768];
                private_create(&path, &bytes).is_ok()
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter_map(|t| t.join().ok())
            .filter(|v| *v)
            .count(),
        1
    );
    let contents = fs::read(&file).unwrap();
    assert_eq!(contents.len(), 32768);
    assert!(contents.iter().all(|b| *b == contents[0]));
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}
#[test]
fn directory_is_never_replaced_with_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state");
    private_dir(&path).unwrap();
    assert!(private_write(&path, b"no").is_err());
    assert!(path.is_dir());
}
#[cfg(unix)]
#[test]
fn unix_private_permissions_and_symlink_refusal() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("owned");
    private_dir(&home).unwrap();
    private_write(&home.join("state"), b"ok").unwrap();
    assert_eq!(
        fs::metadata(&home).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(home.join("state"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    symlink(home.join("state"), home.join("redirect")).unwrap();
    assert!(private_write(&home.join("redirect"), b"no").is_err());
    assert!(private_create(&home.join("redirect"), b"no").is_err());
    assert_eq!(fs::read(home.join("state")).unwrap(), b"ok");
}
#[cfg(windows)]
#[test]
fn windows_private_dacl_survives_atomic_replacement() {
    use std::{collections::BTreeSet, os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            ACCESS_ALLOWED_ACE, Authorization::ConvertSidToStringSidW, DACL_SECURITY_INFORMATION,
            GetAce, GetFileSecurityW, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
            INHERITED_ACE, SE_DACL_PROTECTED,
        },
        Storage::FileSystem::FILE_ALL_ACCESS,
    };
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("private folder").join("secret");
    private_create(&path, b"first").unwrap();
    private_write(&path, b"second").unwrap();
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        // Inspect native SIDs, not SDDL display aliases (e.g. LA for the current
        // runner's built-in Administrator account). Exactly user + SYSTEM.
        let mut needed = 0;
        GetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            0,
            &mut needed,
        );
        assert!(needed > 0);
        let mut storage = vec![0_usize; (needed as usize).div_ceil(size_of::<usize>())];
        let descriptor = storage.as_mut_ptr().cast();
        assert_ne!(
            GetFileSecurityW(
                wide.as_ptr(),
                DACL_SECURITY_INFORMATION,
                descriptor,
                needed,
                &mut needed
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
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = ptr::null_mut();
        assert_ne!(
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted),
            0
        );
        assert_ne!(present, 0);
        assert!(!acl.is_null());
        assert_eq!((*acl).AceCount, 2);
        let mut principals = BTreeSet::new();
        for index in 0..(*acl).AceCount {
            let mut raw = ptr::null_mut();
            assert_ne!(GetAce(acl, u32::from(index), &mut raw), 0);
            let ace = &*raw.cast::<ACCESS_ALLOWED_ACE>();
            assert_eq!(ace.Header.AceType, 0, "ACCESS_ALLOWED_ACE required");
            assert_eq!(u32::from(ace.Header.AceFlags) & INHERITED_ACE, 0);
            assert_eq!(ace.Mask, FILE_ALL_ACCESS);
            let mut text = ptr::null_mut();
            let sid = ptr::addr_of!(ace.SidStart).cast_mut().cast();
            assert_ne!(ConvertSidToStringSidW(sid, &mut text), 0);
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            principals.insert(String::from_utf16_lossy(std::slice::from_raw_parts(
                text, len,
            )));
            LocalFree(text.cast());
        }
        assert_eq!(
            principals,
            BTreeSet::from([current_user_sid().unwrap(), "S-1-5-18".to_owned()])
        );
    }
}
#[test]
fn application_profiles_keep_startup_names_and_commands_separate() {
    let home = Path::new("/Users/One Person/state & 100%");
    let exe = Path::new("/Users/One Person/application");
    let helper = systemd_unit(&MACHINE_HELPER, exe, home).unwrap();
    assert!(helper.contains("Description=Constellation computer connection\n"));
    assert!(helper.contains(" run\n"));
    assert!(helper.contains("100%%"));
    let server = systemd_unit(&OBSERVATORY, exe, home).unwrap();
    assert!(server.contains(" serve\n"));
    assert_ne!(
        installed_binary(&OBSERVATORY, home),
        installed_binary(&MACHINE_HELPER, home)
    );
    let plist = launch_agent(&OBSERVATORY, "own", exe, home).unwrap();
    assert!(plist.contains("&amp;"));
    assert!(plist.contains("<string>serve</string>"));
    assert!(!plist.contains("shell"));
    assert!(systemd_unit(&OBSERVATORY, Path::new("bad\npath"), home).is_err());
}
#[test]
fn startup_ownership_rejects_other_commands_user_home_or_elevation() {
    let exe = Path::new(r"C:\Users\A Person\application.exe");
    let home = Path::new(r"C:\Users\A Person\state & own");
    let sid = "S-1-5-21-100-200-300-1001";
    let text = scheduled_task(&OBSERVATORY, exe, home)
        .unwrap()
        .replace("<LogonType>", &format!("<UserId>{sid}</UserId><LogonType>"));
    assert!(task_matches(&OBSERVATORY, &text, exe, home, sid).unwrap());
    assert!(!task_matches(&MACHINE_HELPER, &text, exe, home, sid).unwrap());
    for (old, new) in [
        (sid, "S-1-5-18"),
        ("LeastPrivilege", "HighestAvailable"),
        ("InteractiveToken", "ServiceAccount"),
        (" serve</Arguments>", " run</Arguments>"),
    ] {
        assert!(!task_matches(&OBSERVATORY, &text.replace(old, new), exe, home, sid).unwrap());
    }
    assert!(!task_matches(&OBSERVATORY, &text, exe, Path::new(r"C:\Elsewhere"), sid).unwrap());
    let omitted = text.replace("<RunLevel>LeastPrivilege</RunLevel>", "");
    assert!(task_matches(&OBSERVATORY, &omitted, exe, home, sid).unwrap());
    let duplicate = text.replace(
        "</Actions>",
        "<Exec><Command>other.exe</Command></Exec></Actions>",
    );
    assert!(!task_matches(&OBSERVATORY, &duplicate, exe, home, sid).unwrap());
}
#[test]
fn task_owner_is_matched_in_both_principal_and_logon_trigger() {
    let exe = Path::new(r"C:\Program Files\Constellation\application.exe");
    let home = Path::new(r"C:\Users\A Person\state & own");
    let sid = "S-1-5-21-100-200-300-1001";
    let legacy = scheduled_task(&OBSERVATORY, exe, home)
        .unwrap()
        .replace("<LogonType>", &format!("<UserId>{sid}</UserId><LogonType>"));
    let owned = legacy.replace(
        "</LogonTrigger>",
        &format!("<UserId>{sid}</UserId></LogonTrigger>"),
    );
    assert_eq!(
        scheduled_task_for_user(&OBSERVATORY, exe, home, sid).unwrap(),
        owned
    );
    assert!(task_matches(&OBSERVATORY, &owned, exe, home, sid).unwrap());
    // Exact known legacy entries keep their identity for reuse and removal.
    assert!(task_matches(&OBSERVATORY, &legacy, exe, home, sid).unwrap());
    for document in [&legacy, &owned] {
        for enabled in ["<Enabled>false</Enabled>", ""] {
            assert!(
                task_matches(
                    &OBSERVATORY,
                    &document.replace("<Enabled>true</Enabled>", enabled),
                    exe,
                    home,
                    sid
                )
                .unwrap()
            );
        }
    }
    for changed in [
        owned.replace(
            &format!("<UserId>{sid}</UserId></LogonTrigger>"),
            "<UserId>S-1-5-18</UserId></LogonTrigger>",
        ),
        owned.replace(
            &format!("<UserId>{sid}</UserId><LogonType>"),
            "<UserId>S-1-5-18</UserId><LogonType>",
        ),
        owned.replace(
            "</LogonTrigger>",
            &format!("<UserId>{sid}</UserId></LogonTrigger>"),
        ),
        owned.replace(
            "</Principal>",
            &format!("<UserId>{sid}</UserId></Principal>"),
        ),
        owned.replace(
            "</Principals>",
            &format!("<Principal id=\"Other\"><UserId>{sid}</UserId></Principal></Principals>"),
        ),
        owned.replace("</Task>", &format!("<UserId>{sid}</UserId></Task>")),
        owned.replace(
            "<RunLevel>LeastPrivilege</RunLevel>",
            "<RunLevel>LeastPrivilege</RunLevel><RunLevel>HighestAvailable</RunLevel>",
        ),
        legacy.replace(
            "</Triggers>",
            "<LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>",
        ),
        legacy.replace("</Triggers>", "<LogonTrigger /></Triggers>"),
        legacy.replace("<Triggers>", "<Triggers><LogonTriggerExtra/>"),
        legacy.replace("<Principals>", "<Principals><Principal/>"),
        legacy.replace(
            "</Triggers>",
            "<BootTrigger><Enabled>true</Enabled></BootTrigger></Triggers>",
        ),
        legacy
            .replace("<LogonTrigger>", "<BootTrigger>")
            .replace("</LogonTrigger>", "</BootTrigger>"),
        legacy.replace(
            "<Enabled>true</Enabled>",
            "<Enabled>true</Enabled><Enabled>false</Enabled>",
        ),
        owned.replace("<Enabled>true</Enabled>", "<Enabled>unknown</Enabled>"),
    ] {
        assert!(!task_matches(&OBSERVATORY, &changed, exe, home, sid).unwrap());
    }
    for invalid in [
        "",
        "S-1-",
        "S-1-5--18",
        "S-1-5-18</UserId>",
        "DOMAIN\\owner",
    ] {
        assert!(scheduled_task_for_user(&OBSERVATORY, exe, home, invalid).is_err());
    }
}

#[test]
fn browser_target_is_local_without_embedded_credentials() {
    for value in [
        "http://127.0.0.1:1234/#local-capability",
        "https://localhost:8443/",
        "http://[::1]:8333/",
    ] {
        assert!(validate_local_url(value).is_ok(), "{value}");
    }
    for value in [
        "https://example.com/",
        "file:///tmp/file",
        "http://127.0.0.1.example.com:1234/",
        "http://user:password@localhost:1234/",
    ] {
        assert!(validate_local_url(value).is_err(), "{value}");
    }
}

#[test]
fn configured_https_origin_is_separate_from_local_setup() {
    assert!(validate_https_origin("https://observatory.example:8443/").is_ok());
    assert!(validate_local_url("https://observatory.example:8443/").is_err());
    for value in [
        "http://remote.example/",
        "file:///tmp/setup",
        "mailto:user@example.com",
        "https://user:pass@example.com/",
        "https://example.com/path",
        "https://example.com/?token=secret",
        "https://example.com/#token",
    ] {
        assert!(validate_https_origin(value).is_err(), "{value}");
    }
}

#[test]
fn staged_directory_publication_preserves_even_empty_existing_destination() {
    let temp = tempfile::tempdir().unwrap();
    let from = temp.path().join("stage");
    let to = temp.path().join("installed");
    fs::create_dir(&from).unwrap();
    fs::write(from.join("identity"), b"preserve").unwrap();
    fs::create_dir(&to).unwrap();
    assert!(publish_directory_if_absent(&from, &to).is_err());
    assert_eq!(fs::read(from.join("identity")).unwrap(), b"preserve");
    assert_eq!(fs::read_dir(&to).unwrap().count(), 0);
    fs::remove_dir(&to).unwrap();
    publish_directory_if_absent(&from, &to).unwrap();
    assert!(!from.exists());
    assert_eq!(fs::read(to.join("identity")).unwrap(), b"preserve");
}
#[test]
fn concurrent_destination_creation_is_never_replaced() {
    for _ in 0..16 {
        let temp = tempfile::tempdir().unwrap();
        let from = temp.path().join("stage");
        let to = temp.path().join("installed");
        fs::create_dir(&from).unwrap();
        fs::write(from.join("identity"), b"new installation").unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let start = barrier.clone();
        let target = to.clone();
        let other = std::thread::spawn(move || {
            start.wait();
            fs::create_dir(&target).is_ok()
        });
        barrier.wait();
        let published = publish_directory_if_absent(&from, &to).is_ok();
        let created = other.join().unwrap();
        assert_ne!(published, created);
        if created {
            assert!(from.join("identity").exists());
            assert_eq!(fs::read_dir(&to).unwrap().count(), 0);
        } else {
            assert!(!from.exists());
            assert_eq!(fs::read(to.join("identity")).unwrap(), b"new installation");
        }
    }
}

#[test]
fn captured_legacy_windows_task_retains_empty_logon_trigger() {
    // Captured Server 2025 Scheduler export, with identities/paths redacted,
    // retained byte-for-byte from Observatory machine-helper's native fixture.
    let text = include_str!("fixtures/windows-task-query.xml");
    let home = Path::new(r"C:\Users\Example\AppData\Local\Temp\fixture\helper with spaces");
    let exe = Path::new(
        r"C:\Users\Example\AppData\Local\Temp\fixture\helper with spaces\bin\constellation-machine-helper.exe",
    );
    let sid = "S-1-5-21-100-200-300-1001";
    for empty in ["<LogonTrigger />", "<LogonTrigger/>"] {
        let observed = text.replace("<LogonTrigger />", empty);
        assert!(task_matches(&MACHINE_HELPER, &observed, exe, home, sid).unwrap());
        assert!(!task_matches(&MACHINE_HELPER, &observed, exe, home, "S-1-5-18").unwrap());
        for altered in [
            observed.replace(empty, &format!("{empty}{empty}")),
            observed.replace(empty, &format!("{empty}<BootTrigger />")),
            observed.replace(
                empty,
                "<LogonTrigger><UserId>S-1-5-18</UserId></LogonTrigger>",
            ),
            observed.replace(empty, "<LogonTrigger bogus=\"true\" />"),
            observed.replace(empty, "<LogonTrigger /> </LogonTrigger>"),
            observed.replace(
                "</Principal>",
                "<RunLevel>HighestAvailable</RunLevel></Principal>",
            ),
            observed.replace("InteractiveToken", "ServiceAccount"),
        ] {
            assert!(!task_matches(&MACHINE_HELPER, &altered, exe, home, sid).unwrap());
        }
    }
}
