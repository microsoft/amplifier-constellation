//! Windows consumers must not require a machine-wide long-path registry setting.
#![cfg(windows)]
use amplifier_native_platform::{
    private_create, private_dir, private_write, protect_file, publish_directory_if_absent,
};
use std::{fs, os::windows::ffi::OsStrExt, path::PathBuf};

fn deep_path(root: &std::path::Path) -> PathBuf {
    // Exercise ordinary caller paths, not a caller-supplied verbatim prefix.
    let text = root
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(root.to_str().unwrap());
    let mut path = PathBuf::from(text);
    while path.as_os_str().encode_wide().count() < 340 {
        path.push("installation-private-files-café-雪-0123456789");
    }
    path
}

#[test]
fn private_directory_and_existing_file_support_deep_unicode_paths() {
    let temporary = tempfile::tempdir().unwrap();
    let home = deep_path(temporary.path());
    private_dir(&home).expect("private directory beyond MAX_PATH");
    let token = home.join("credential.json");
    fs::write(&token, b"fixture").unwrap();
    protect_file(&token).expect("private ACL beyond MAX_PATH");
    assert_eq!(fs::read(token).unwrap(), b"fixture");
}

#[test]
fn atomic_private_create_and_replace_support_deep_paths_without_overwrite() {
    let temporary = tempfile::tempdir().unwrap();
    let home = deep_path(temporary.path());
    fs::create_dir_all(&home).unwrap();
    let token = home.join("receipt.json");
    private_create(&token, b"original").expect("create-only beyond MAX_PATH");
    assert!(private_create(&token, b"must not replace").is_err());
    assert_eq!(fs::read(&token).unwrap(), b"original");
    private_write(&token, b"replacement").expect("atomic replacement beyond MAX_PATH");
    assert_eq!(fs::read(&token).unwrap(), b"replacement");
    assert_eq!(fs::read_dir(home).unwrap().count(), 1);
}

#[test]
fn directory_publication_supports_deep_paths_and_preserves_existing_destination() {
    let temporary = tempfile::tempdir().unwrap();
    let home = deep_path(temporary.path());
    let from = home.join("staged");
    let to = home.join("installed");
    fs::create_dir_all(&from).unwrap();
    fs::write(from.join("app"), b"new executable").unwrap();
    fs::create_dir_all(&to).unwrap();
    assert!(publish_directory_if_absent(&from, &to).is_err());
    assert!(from.join("app").exists());
    fs::remove_dir(&to).unwrap();
    publish_directory_if_absent(&from, &to).expect("no-replace publication beyond MAX_PATH");
    assert_eq!(fs::read(to.join("app")).unwrap(), b"new executable");
    assert!(!from.exists());
}
