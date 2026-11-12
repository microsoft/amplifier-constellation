#![cfg(feature = "installation")]

use amplifier_native_platform::locking::InstallationLock;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// Unix fork copies every open descriptor until exec applies CLOEXEC. Serialize
// lock lifetimes with sibling tests' spawning, so a simultaneous fork cannot
// transiently own an unrelated test's lock. The explicit executed-child case
// below independently tests that a real child does not retain the descriptor.
static TEST_LIFETIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct TestChild(Child);
impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn child(root: &Path, mode: &str) -> TestChild {
    TestChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "lock_child", "--nocapture"])
            .env("NATIVE_LOCK_TEST_ROOT", root)
            .env("NATIVE_LOCK_TEST_MODE", mode)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn ready(root: &Path, child: &mut TestChild) {
    let until = Instant::now() + Duration::from_secs(10);
    while !root.join("ready").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "Child exited before ready"
        );
        assert!(Instant::now() < until, "Child did not become ready");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn lock_child() {
    let Some(root) = std::env::var_os("NATIVE_LOCK_TEST_ROOT").map(PathBuf::from) else {
        return;
    };
    let mode = std::env::var("NATIVE_LOCK_TEST_MODE").unwrap();
    let _lock = (mode == "hold").then(|| {
        InstallationLock::try_acquire(&root.join("installation.lock"))
            .unwrap()
            .expect("Child must acquire")
    });
    fs::write(root.join("ready"), b"ready").unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while !root.join("exit").exists() && Instant::now() < until {
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn existing_file_is_unchanged_and_only_contention_is_none() {
    let _serial = TEST_LIFETIME.lock().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("installation.lock");
    fs::write(&path, b"legacy lock file contents").unwrap();
    let held = InstallationLock::try_acquire(&path).unwrap().unwrap();
    assert!(InstallationLock::try_acquire(&path).unwrap().is_none());
    drop(held);
    let next = InstallationLock::try_acquire(&path).unwrap().unwrap();
    drop(next);
    assert_eq!(fs::read(&path).unwrap(), b"legacy lock file contents");
    assert!(InstallationLock::try_acquire(home.path()).is_err());
    assert!(InstallationLock::try_acquire(&home.path().join("missing/lock")).is_err());
}

#[test]
fn another_process_contends_and_crash_releases_without_replacing_file() {
    let _serial = TEST_LIFETIME.lock().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("installation.lock");
    fs::write(&path, b"original").unwrap();
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(&path).unwrap().ino()
    };
    let mut holder = child(home.path(), "hold");
    ready(home.path(), &mut holder);
    assert!(InstallationLock::try_acquire(&path).unwrap().is_none());
    holder.0.kill().unwrap();
    holder.0.wait().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let next = loop {
        if let Some(lock) = InstallationLock::try_acquire(&path).unwrap() {
            break lock;
        }
        assert!(Instant::now() < until, "Exited process retained its lock");
        thread::sleep(Duration::from_millis(10));
    };
    drop(next);
    assert_eq!(fs::read(&path).unwrap(), b"original");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    }
}

#[test]
fn executed_child_does_not_keep_parent_lock_alive() {
    let _serial = TEST_LIFETIME.lock().unwrap();
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("installation.lock");
    let held = InstallationLock::try_acquire(&path).unwrap().unwrap();
    let mut observer = child(home.path(), "observe");
    ready(home.path(), &mut observer);
    drop(held);
    let _next = InstallationLock::try_acquire(&path).unwrap().unwrap();
    assert!(observer.0.try_wait().unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn existing_flock_and_native_lock_exclude_each_other_on_the_same_inode() {
    let _serial = TEST_LIFETIME.lock().unwrap();
    use std::{fs::OpenOptions, os::fd::AsRawFd};
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("installation.lock");
    let existing = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(existing.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert!(InstallationLock::try_acquire(&path).unwrap().is_none());
    assert_eq!(
        unsafe { libc::flock(existing.as_raw_fd(), libc::LOCK_UN) },
        0
    );
    let native = InstallationLock::try_acquire(&path).unwrap().unwrap();
    assert_eq!(
        unsafe { libc::flock(existing.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().kind(),
        std::io::ErrorKind::WouldBlock
    );
    drop(native);
    assert_eq!(
        unsafe { libc::flock(existing.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
}

#[cfg(unix)]
#[test]
fn symlink_and_fifo_are_refused_without_touching_target() {
    let _serial = TEST_LIFETIME.lock().unwrap();
    use std::{
        ffi::CString,
        os::unix::{
            ffi::OsStrExt,
            fs::{PermissionsExt, symlink},
        },
    };
    let home = tempfile::tempdir().unwrap();
    let target = home.path().join("target");
    fs::write(&target, b"preserved").unwrap();
    let link = home.path().join("link");
    symlink(&target, &link).unwrap();
    assert!(InstallationLock::try_acquire(&link).is_err());
    assert_eq!(fs::read(target).unwrap(), b"preserved");
    let fifo = home.path().join("fifo");
    let c_fifo = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c_fifo.as_ptr(), 0o600) }, 0);
    assert!(InstallationLock::try_acquire(&fifo).is_err());
    let regular = home.path().join("private.lock");
    let _lock = InstallationLock::try_acquire(&regular).unwrap().unwrap();
    assert_eq!(
        fs::metadata(regular).unwrap().permissions().mode() & 0o077,
        0
    );
}
