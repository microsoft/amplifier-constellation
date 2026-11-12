#![cfg(feature = "installation")]

use amplifier_native_platform::{
    installation_exchange::{self as exchange, EntryRef, MutationOutcome, Profile},
    private_dir,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

struct Fixture {
    _temporary: tempfile::TempDir,
    home: PathBuf,
    profile: Profile,
}

impl Fixture {
    fn new() -> Self {
        let temporary = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let home = temporary.path().canonicalize().unwrap();
        for name in [
            "",
            "game",
            "stages",
            "stages/new",
            "stages/new/app",
            "prepared",
            "retained",
            "game/app",
        ] {
            private_dir(&home.join(name)).unwrap();
        }
        fs::write(home.join("game/app/identity.txt"), "original baseline").unwrap();
        fs::write(
            home.join("stages/new/app/identity.txt"),
            "reviewed candidate",
        )
        .unwrap();
        let profile = Profile {
            current: home.join("game/app"),
            roots: BTreeMap::from([
                ("game".into(), home.join("game")),
                ("stages".into(), home.join("stages")),
                ("prepared".into(), home.join("prepared")),
                ("retained".into(), home.join("retained")),
            ]),
        };
        Self {
            _temporary: temporary,
            home,
            profile,
        }
    }
    fn entry(root: &str, path: &str) -> EntryRef {
        EntryRef {
            root: root.into(),
            path: path.into(),
        }
    }
    fn target(&self) -> EntryRef {
        Self::entry("stages", "new/app")
    }
    fn prepared(&self) -> EntryRef {
        Self::entry("prepared", "candidate")
    }
    fn retained(&self) -> EntryRef {
        Self::entry("retained", "original")
    }
    fn prepare(&self) -> Value {
        let target = exchange::inspect(&self.profile, Some(&self.target())).unwrap();
        applied(
            exchange::prepare_pointer(&self.profile, &self.prepared(), &self.target(), &target)
                .unwrap(),
        )
    }
    fn current(&self) -> Value {
        exchange::inspect(&self.profile, None).unwrap()
    }
}

fn applied(outcome: MutationOutcome) -> Value {
    match outcome {
        MutationOutcome::Applied { state } => state,
        other => panic!("Expected applied: {other:?}"),
    }
}

#[test]
fn actual_pointer_and_real_baseline_exchange_then_explicit_rollback() {
    let fixture = Fixture::new();
    let original = fixture.current();
    let prepared = fixture.prepare();
    assert_eq!(prepared["kind"], "pointer");
    assert_eq!(
        fs::read_to_string(fixture.home.join("prepared/candidate/identity.txt")).unwrap(),
        "reviewed candidate"
    );
    #[cfg(unix)]
    assert!(
        fs::symlink_metadata(fixture.home.join("prepared/candidate"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        let handle = open_junction(
            &fixture.home.join("prepared/candidate"),
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        );
        assert_junction(&handle);
    }
    let state = applied(
        exchange::exchange(
            &fixture.profile,
            &fixture.prepared(),
            &fixture.retained(),
            &original,
            &prepared,
        )
        .unwrap(),
    );
    assert_eq!(state["current"], prepared);
    assert_eq!(state["retained"], original);
    assert!(state["incoming"].is_null());
    assert_eq!(
        fs::read_to_string(fixture.home.join("retained/original/identity.txt")).unwrap(),
        "original baseline"
    );
    assert_eq!(
        fs::read_to_string(fixture.profile.current.join("identity.txt")).unwrap(),
        "reviewed candidate"
    );
    let restored = applied(
        exchange::exchange(
            &fixture.profile,
            &fixture.retained(),
            &Fixture::entry("retained", "candidate-after-rollback"),
            &prepared,
            &original,
        )
        .unwrap(),
    );
    assert_eq!(restored["current"], original);
    assert_eq!(restored["retained"], prepared);
    assert_eq!(
        fs::read_to_string(fixture.profile.current.join("identity.txt")).unwrap(),
        "original baseline"
    );
    assert!(fixture.home.join("stages/new/app/identity.txt").exists());
}

#[test]
fn fresh_current_and_real_directory_incoming_are_supported_without_copying() {
    let mut fixture = Fixture::new();
    fixture.profile.current = fixture.home.join("game/fresh-app");
    let target = fixture.target();
    let expected = exchange::inspect(&fixture.profile, Some(&target)).unwrap();
    let state = applied(
        exchange::exchange(
            &fixture.profile,
            &target,
            &fixture.retained(),
            &Value::Null,
            &expected,
        )
        .unwrap(),
    );
    assert_eq!(state["current"], expected);
    assert!(state["retained"].is_null());
    assert!(!fixture.home.join("stages/new/app").exists());
    assert_eq!(
        fs::read_to_string(fixture.profile.current.join("identity.txt")).unwrap(),
        "reviewed candidate"
    );
}

#[test]
fn retained_unknown_entry_is_never_overwritten_even_when_empty() {
    let fixture = Fixture::new();
    let original = fixture.current();
    let prepared = fixture.prepare();
    private_dir(&fixture.home.join("retained/original")).unwrap();
    assert!(matches!(
        exchange::exchange(
            &fixture.profile,
            &fixture.prepared(),
            &fixture.retained(),
            &original,
            &prepared
        )
        .unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert_eq!(fixture.current(), original);
    assert_eq!(
        exchange::inspect(&fixture.profile, Some(&fixture.prepared())).unwrap(),
        prepared
    );
    assert!(fixture.home.join("retained/original").is_dir());
}

#[test]
fn changed_current_or_prepared_object_refuses_without_touching_either() {
    let fixture = Fixture::new();
    let original = fixture.current();
    let prepared = fixture.prepare();
    fs::rename(
        &fixture.profile.current,
        fixture.home.join("game/old-preserved"),
    )
    .unwrap();
    private_dir(&fixture.profile.current).unwrap();
    let changed = fixture.current();
    assert!(matches!(
        exchange::exchange(
            &fixture.profile,
            &fixture.prepared(),
            &fixture.retained(),
            &original,
            &prepared
        )
        .unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert_eq!(fixture.current(), changed);
    assert!(
        fixture
            .home
            .join("game/old-preserved/identity.txt")
            .exists()
    );
    fs::rename(
        fixture.home.join("prepared/candidate"),
        fixture.home.join("prepared/previous-pointer"),
    )
    .unwrap();
    let second = fixture.prepare();
    assert_ne!(second["identity"], prepared["identity"]);
    assert!(matches!(
        exchange::exchange(
            &fixture.profile,
            &fixture.prepared(),
            &fixture.retained(),
            &changed,
            &prepared
        )
        .unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert_eq!(fixture.current(), changed);
    assert!(fixture.home.join("prepared/previous-pointer").exists());
}

#[test]
fn prepare_refuses_reused_path_and_stale_target_identity() {
    let fixture = Fixture::new();
    let first = fixture.prepare();
    let target = exchange::inspect(&fixture.profile, Some(&fixture.target())).unwrap();
    assert!(matches!(
        exchange::prepare_pointer(
            &fixture.profile,
            &fixture.prepared(),
            &fixture.target(),
            &target
        )
        .unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert_eq!(
        exchange::inspect(&fixture.profile, Some(&fixture.prepared())).unwrap(),
        first
    );
    fs::rename(
        fixture.home.join("stages/new/app"),
        fixture.home.join("stages/new/previous"),
    )
    .unwrap();
    private_dir(&fixture.home.join("stages/new/app")).unwrap();
    assert!(matches!(
        exchange::prepare_pointer(
            &fixture.profile,
            &Fixture::entry("prepared", "fresh"),
            &fixture.target(),
            &target
        )
        .unwrap(),
        MutationOutcome::Refused { .. }
    ));
    assert!(!fixture.home.join("prepared/fresh").exists());
}

#[test]
fn overlapping_and_unenrolled_paths_are_rejected_before_rename() {
    let fixture = Fixture::new();
    let original = fixture.current();
    let prepared = fixture.prepare();
    for retained in [
        Fixture::entry("game", "app/inside"),
        Fixture::entry("game", "app"),
        Fixture::entry("retained", "../outside"),
        Fixture::entry("unknown", "entry"),
    ] {
        assert!(
            exchange::exchange(
                &fixture.profile,
                &fixture.prepared(),
                &retained,
                &original,
                &prepared
            )
            .is_err()
        );
        assert_eq!(fixture.current(), original);
    }
    assert!(
        exchange::inspect(
            &fixture.profile,
            Some(&Fixture::entry("prepared", "candidate/identity.txt"))
        )
        .is_err(),
        "Pointer ancestors must not route later filesystem operations"
    );
}

#[test]
fn external_pointer_target_and_regular_file_are_not_managed_entries() {
    let fixture = Fixture::new();
    private_dir(&fixture.home.join("outside")).unwrap();
    private_dir(&fixture.home.join("outside/target")).unwrap();
    let mut expanded = fixture.profile.clone();
    expanded
        .roots
        .insert("outside".into(), fixture.home.join("outside"));
    let target_ref = Fixture::entry("outside", "target");
    let expected = exchange::inspect(&expanded, Some(&target_ref)).unwrap();
    applied(
        exchange::prepare_pointer(&expanded, &fixture.prepared(), &target_ref, &expected).unwrap(),
    );
    assert!(exchange::inspect(&fixture.profile, Some(&fixture.prepared())).is_err());
    fs::write(fixture.home.join("prepared/file"), "not a directory").unwrap();
    assert!(
        exchange::inspect(&fixture.profile, Some(&Fixture::entry("prepared", "file"))).is_err()
    );
}

#[cfg(unix)]
struct BlockRename {
    parent: PathBuf,
    permissions: fs::Permissions,
}
#[cfg(unix)]
impl BlockRename {
    fn new(path: &Path) -> Self {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "Actual permission-failure test requires an unprivileged test user"
        );
        let parent = path.parent().unwrap().to_owned();
        let permissions = fs::metadata(&parent).unwrap().permissions();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
        Self {
            parent,
            permissions,
        }
    }
}
#[cfg(unix)]
impl Drop for BlockRename {
    fn drop(&mut self) {
        fs::set_permissions(&self.parent, self.permissions.clone()).unwrap();
    }
}

#[cfg(windows)]
struct BlockRename {
    _handle: fs::File,
}

#[cfg(windows)]
fn open_junction(path: &Path, share: u32) -> fs::File {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, OPEN_EXISTING,
        },
    };
    let path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // Actual read access establishes the sharing reservation needed by the
    // failure fixture. Access zero is only a metadata observation.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ,
            share,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(
        handle,
        INVALID_HANDLE_VALUE,
        "Open actual junction: {}",
        std::io::Error::last_os_error()
    );
    unsafe { fs::File::from_raw_handle(handle) }
}

#[cfg(windows)]
fn assert_junction(file: &fs::File) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::{IO::DeviceIoControl, Ioctl::FSCTL_GET_REPARSE_POINT};
    let mut data = vec![0u8; 16 * 1024];
    let mut returned = 0;
    assert_ne!(
        unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                FSCTL_GET_REPARSE_POINT,
                std::ptr::null(),
                0,
                data.as_mut_ptr().cast(),
                data.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        },
        0,
        "Handle must refer to the reparse entry itself: {}",
        std::io::Error::last_os_error()
    );
    assert!(returned >= 8, "Truncated native reparse record");
    assert_eq!(
        u32::from_le_bytes(data[..4].try_into().unwrap()),
        0xa000_0003,
        "Native pointer must be IO_REPARSE_TAG_MOUNT_POINT"
    );
}

#[cfg(windows)]
impl BlockRename {
    fn new(path: &Path) -> Self {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{
            Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION},
            Storage::FileSystem::{
                FILE_SHARE_READ, FILE_SHARE_WRITE, MOVEFILE_WRITE_THROUGH, MoveFileExW,
            },
        };
        let handle = open_junction(path, FILE_SHARE_READ | FILE_SHARE_WRITE);
        assert_junction(&handle);
        let probe = path.with_file_name("blocked-rename-probe");
        assert!(!probe.exists());
        let from: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let to: Vec<u16> = probe.as_os_str().encode_wide().chain([0]).collect();
        let moved = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        let error = std::io::Error::last_os_error();
        assert_eq!(
            moved, 0,
            "Failure fixture must actually block renaming the junction"
        );
        assert!(
            matches!(error.raw_os_error(), Some(code) if code == ERROR_SHARING_VIOLATION as i32 || code == ERROR_ACCESS_DENIED as i32),
            "Expected native sharing/access refusal, got {error}"
        );
        assert!(
            path.exists() && !probe.exists(),
            "Failed native rename must preserve the prepared pointer"
        );
        Self { _handle: handle }
    }
}

#[test]
fn actual_second_rename_failure_is_unknown_and_original_remains_recoverable() {
    let fixture = Fixture::new();
    let original = fixture.current();
    let prepared = fixture.prepare();
    let blocked = BlockRename::new(&fixture.home.join("prepared/candidate"));
    let result = exchange::exchange(
        &fixture.profile,
        &fixture.prepared(),
        &fixture.retained(),
        &original,
        &prepared,
    )
    .unwrap();
    assert!(
        matches!(
            result,
            MutationOutcome::Unknown {
                phase: "install_prepared",
                ..
            }
        ),
        "{result:?}"
    );
    drop(blocked);
    assert!(
        fixture.current().is_null(),
        "Do not invent automatic restoration"
    );
    assert_eq!(
        exchange::inspect(&fixture.profile, Some(&fixture.retained())).unwrap(),
        original
    );
    assert_eq!(
        exchange::inspect(&fixture.profile, Some(&fixture.prepared())).unwrap(),
        prepared
    );
    applied(
        exchange::exchange(
            &fixture.profile,
            &fixture.retained(),
            &Fixture::entry("retained", "recovery-displaced"),
            &Value::Null,
            &original,
        )
        .unwrap(),
    );
    assert_eq!(fixture.current(), original);
    assert!(
        fixture.home.join("prepared/candidate").exists(),
        "Uninstalled candidate is retained"
    );
}
