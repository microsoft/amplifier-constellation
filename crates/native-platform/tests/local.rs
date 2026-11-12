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
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("private folder").join("secret");
    private_create(&path, b"first").unwrap();
    private_write(&path, b"second").unwrap();
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-Acl -LiteralPath $env:NATIVE_TEST_PATH).Sddl",
        ])
        .env("NATIVE_TEST_PATH", &path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    let dacl = text.split("D:").nth(1).unwrap();
    assert!(dacl.starts_with('P'), "{text}");
    assert!(dacl.contains(&current_user_sid().unwrap()), "{text}");
    assert!(dacl.contains(";;;SY)"), "{text}");
    for principal in ["WD", "AU", "BU", "BA"] {
        assert!(!dacl.contains(&format!(";;;{principal})")), "{text}");
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
