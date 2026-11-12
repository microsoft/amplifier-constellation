#![cfg(feature = "installation")]

use amplifier_native_platform::{
    installation_tree::{self as tree, MutationOutcome},
    private_dir,
};
#[cfg(windows)]
use std::path::Path;
use std::{fs, path::PathBuf};

struct Fixture {
    _temporary: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        Self::with_home(|path| path)
    }
    fn with_home(transform: impl FnOnce(PathBuf) -> PathBuf) -> Self {
        let temporary = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let home = transform(temporary.path().canonicalize().unwrap());
        let root = home.join("stages");
        private_dir(&root).unwrap();
        Self {
            _temporary: temporary,
            home,
            root,
        }
    }
    fn candidate(&self) -> PathBuf {
        let path = self.root.join("candidate");
        private_dir(&path.join("nested")).unwrap();
        fs::write(path.join("nested/data"), "regenerable candidate").unwrap();
        path
    }
}

fn applied(outcome: MutationOutcome) {
    match outcome {
        MutationOutcome::Applied { state } => assert_eq!(state, serde_json::json!({"absent":true})),
        other => panic!("Expected observed absence: {other:?}"),
    }
}

#[test]
fn absent_inspection_does_not_create_and_removal_requires_nonnull_snapshot() {
    let fixture = Fixture::new();
    assert!(tree::inspect(&fixture.root, "absent").unwrap().is_null());
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    assert!(matches!(
        tree::remove(&fixture.root, "absent", &serde_json::Value::Null, &[]).unwrap(),
        MutationOutcome::Refused { .. }
    ));
}

#[test]
fn one_selected_tree_is_removed_and_siblings_root_and_files_are_preserved() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    fs::write(fixture.root.join("kept"), "unrelated receipt").unwrap();
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    assert_eq!(before["schema"], 1);
    assert_eq!(before["entry"], "candidate");
    applied(tree::remove(&fixture.root, "candidate", &before, &[]).unwrap());
    assert!(!candidate.exists());
    assert!(fixture.root.is_dir());
    assert_eq!(
        fs::read_to_string(fixture.root.join("kept")).unwrap(),
        "unrelated receipt"
    );
    assert!(tree::inspect(&fixture.root, "candidate").unwrap().is_null());
    assert!(matches!(
        tree::remove(&fixture.root, "candidate", &before, &[]).unwrap(),
        MutationOutcome::Refused { .. }
    ));
}

#[test]
fn invalid_child_names_and_ordinary_file_roots_refuse_without_writes() {
    let fixture = Fixture::new();
    fixture.candidate();
    for name in [
        "",
        ".",
        "..",
        "../candidate",
        "candidate/nested",
        "candidate\\nested",
        "/absolute",
        "C:stage",
        "candidate:stream",
        "candidate\0",
        "candidate.",
        "candidate ",
    ] {
        assert!(
            tree::inspect(&fixture.root, name).is_err(),
            "accepted {name:?}"
        );
    }
    fs::write(fixture.root.join("file"), "not a tree").unwrap();
    assert!(tree::inspect(&fixture.root, "file").is_err());
    assert!(tree::inspect(&fixture.root.join("file"), "candidate").is_err());
    assert_eq!(
        fs::read_to_string(fixture.root.join("file")).unwrap(),
        "not a tree"
    );
    assert!(fixture.root.join("candidate/nested/data").exists());
}

#[test]
fn stale_child_identity_refuses_and_preserves_both_generations() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    fs::rename(&candidate, fixture.root.join("retained")).unwrap();
    fixture.candidate();
    assert!(matches!(
        tree::remove(&fixture.root, "candidate", &before, &[]).unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert!(candidate.join("nested/data").exists());
    assert!(fixture.root.join("retained/nested/data").exists());
}

#[test]
fn stale_fixed_root_identity_refuses_even_with_same_child_name() {
    let fixture = Fixture::new();
    fixture.candidate();
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    fs::rename(&fixture.root, fixture.home.join("retained-stages")).unwrap();
    fixture.candidate();
    assert!(matches!(
        tree::remove(&fixture.root, "candidate", &before, &[]).unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert!(fixture.root.join("candidate/nested/data").exists());
    assert!(
        fixture
            .home
            .join("retained-stages/candidate/nested/data")
            .exists()
    );
}

#[test]
fn protected_control_parent_identity_refuses_before_removing_any_contents() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let control = candidate.join("control");
    private_dir(&control).unwrap();
    fs::write(control.join("installation.lock"), "held lock identity").unwrap();
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    // Protect the observed object, not merely one textual spelling of its path.
    let protected = candidate.join(".").join("control");
    let outcome = tree::remove(&fixture.root, "candidate", &before, &[protected]).unwrap();
    match outcome {
        MutationOutcome::Refused { reason } => {
            assert!(reason.contains("protected control directory"))
        }
        other => panic!("Expected protected parent refusal: {other:?}"),
    }
    assert!(candidate.join("nested/data").exists());
    assert_eq!(
        fs::read_to_string(control.join("installation.lock")).unwrap(),
        "held lock identity"
    );
    assert_eq!(tree::inspect(&fixture.root, "candidate").unwrap(), before);
}

#[test]
fn selected_tree_cannot_itself_be_a_protected_parent() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    assert!(matches!(
        tree::remove(
            &fixture.root,
            "candidate",
            &before,
            std::slice::from_ref(&candidate)
        )
        .unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert!(candidate.join("nested/data").exists());
}

#[cfg(unix)]
#[test]
fn descendant_links_are_removed_without_following_targets_and_root_links_refuse() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let outside = fixture.home.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), "outside unchanged").unwrap();
    symlink(&outside, candidate.join("outside-directory")).unwrap();
    symlink(outside.join("sentinel"), candidate.join("outside-file")).unwrap();
    symlink(outside.join("missing"), candidate.join("dangling")).unwrap();
    symlink(&candidate, fixture.root.join("root-link")).unwrap();
    symlink(&fixture.root, fixture.home.join("parent-link")).unwrap();
    assert!(tree::inspect(&fixture.root, "root-link").is_err());
    assert!(tree::inspect(&fixture.home.join("parent-link"), "candidate").is_err());
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    applied(tree::remove(&fixture.root, "candidate", &before, &[]).unwrap());
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "outside unchanged"
    );
    assert!(
        fs::symlink_metadata(fixture.root.join("root-link"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn unsupported_fifo_refuses_before_any_tree_entry_is_removed() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    // A real unsupported entry without Unix socket path-length constraints,
    // including when the caller selects a deep private fixture root.
    let pipe = CString::new(candidate.join("owned.fifo").as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(pipe.as_ptr(), 0o600) }, 0);
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    assert!(matches!(
        tree::remove(&fixture.root, "candidate", &before, &[]).unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert!(candidate.join("nested/data").exists());
    assert!(candidate.join("owned.fifo").exists());
}

#[cfg(windows)]
fn junction(path: &Path, target: &Path, home: &Path) {
    use amplifier_native_platform::installation_exchange::{self as exchange, EntryRef, Profile};
    use std::collections::BTreeMap;
    let profile = Profile {
        current: home.join("unused-current"),
        roots: BTreeMap::from([("home".into(), home.to_owned())]),
    };
    let entry = |path: &Path| EntryRef {
        root: "home".into(),
        path: path.strip_prefix(home).unwrap().to_owned(),
    };
    let target_ref = entry(target);
    let before = exchange::inspect(&profile, Some(&target_ref)).unwrap();
    assert!(matches!(
        exchange::prepare_pointer(&profile, &entry(path), &target_ref, &before).unwrap(),
        MutationOutcome::Applied { .. }
    ));
}

#[cfg(windows)]
#[test]
fn deep_ordinary_unicode_paths_remove_junction_leaves_without_touching_targets() {
    use std::os::windows::ffi::OsStrExt;
    let fixture = Fixture::with_home(|home| {
        let text = home.to_str().unwrap();
        let mut home = match text.strip_prefix(r"\\?\UNC\") {
            Some(rest) => PathBuf::from(format!(r"\\{rest}")),
            None => PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text)),
        };
        while home.as_os_str().encode_wide().count() <= 340 {
            home.push("installation-café-雪-🧭-0123456789");
        }
        home
    });
    assert!(!fixture.root.to_str().unwrap().starts_with(r"\\?\"));
    let candidate = fixture.candidate();
    let outside = fixture.home.join("outside");
    private_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), "outside unchanged").unwrap();
    junction(&candidate.join("outside-junction"), &outside, &fixture.home);
    junction(&fixture.root.join("root-junction"), &outside, &fixture.home);
    assert!(tree::inspect(&fixture.root, "root-junction").is_err());
    let before = tree::inspect(&fixture.root, "candidate").unwrap();
    applied(tree::remove(&fixture.root, "candidate", &before, &[]).unwrap());
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "outside unchanged"
    );
    assert!(fixture.root.join("root-junction").is_dir());
    println!(
        "Ordinary Unicode tree root {} UTF-16 units; junction targets and root junction preserved",
        fixture.root.as_os_str().encode_wide().count()
    );
}
