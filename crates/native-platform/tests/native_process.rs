#![cfg(feature = "installation")]

use amplifier_native_platform::process::{identity, matches};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct TestChild(Child);
impl Drop for TestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn process_child() {
    let Some(root) = std::env::var_os("NATIVE_PROCESS_TEST_ROOT").map(PathBuf::from) else {
        return;
    };
    fs::write(root.join("ready"), b"ready").unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while Instant::now() < until {
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn self_identity_is_stable_and_generation_is_required() {
    let current = identity(std::process::id()).unwrap().unwrap();
    assert_eq!(current["pid"], std::process::id());
    assert_eq!(identity(std::process::id()).unwrap().unwrap(), current);
    assert!(matches(&current).unwrap());
    let mut changed = current.clone();
    #[cfg(target_os = "linux")]
    let generation = "start_ticks";
    #[cfg(target_os = "macos")]
    let generation = "start_seconds";
    #[cfg(windows)]
    let generation = "creation_time_100ns";
    changed[generation] = json!(
        (current[generation]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            + 1)
        .to_string()
    );
    assert!(
        !matches(&changed).unwrap(),
        "Matching a PID must not ignore its generation"
    );
    assert!(matches(&json!({"pid": std::process::id()})).is_err());
    assert!(matches(&json!({"pid": 0})).is_err());
    assert!(matches(&json!({"pid": "1"})).is_err());
    assert!(identity(0).is_err());
    let mut extra = current;
    extra["unrecognized"] = json!(true);
    assert!(matches(&extra).is_err());
}

fn child(home: &Path) -> TestChild {
    let mut child = TestChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process_child", "--nocapture"])
            .env("NATIVE_PROCESS_TEST_ROOT", home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let until = Instant::now() + Duration::from_secs(10);
    while !home.join("ready").exists() {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < until, "Child did not become ready");
        thread::sleep(Duration::from_millis(10));
    }
    child
}

#[test]
fn child_generation_matches_until_confirmed_exit() {
    let home = tempfile::tempdir().unwrap();
    let mut child = child(home.path());
    let receipt = identity(child.0.id()).unwrap().unwrap();
    assert!(matches(&receipt).unwrap());
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    assert!(!matches(&receipt).unwrap());
    // Even if the OS immediately reuses the number, it cannot match this receipt.
    if let Some(reused) = identity(child.0.id()).unwrap() {
        assert_ne!(reused, receipt);
    }
}

#[test]
fn exited_child_is_not_alive_before_parent_reaps_it() {
    let home = tempfile::tempdir().unwrap();
    let mut child = child(home.path());
    let receipt = identity(child.0.id()).unwrap().unwrap();
    child.0.kill().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    // Do not call wait/try_wait here: on Unix this deliberately leaves a zombie
    // with the same PID/start time, and on Windows the child handle stays open.
    while identity(child.0.id()).unwrap().is_some() {
        assert!(
            Instant::now() < until,
            "Terminated child reported as running"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!matches(&receipt).unwrap());
    #[cfg(target_os = "linux")]
    {
        let stat = fs::read_to_string(format!("/proc/{}/stat", child.0.id())).unwrap();
        assert_eq!(
            stat.rsplit_once(')').unwrap().1.split_whitespace().next(),
            Some("Z")
        );
    }
    child.0.wait().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_identity_preserves_existing_python_record_exactly() {
    let pid = std::process::id();
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let ticks = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .nth(19)
        .unwrap();
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    assert_eq!(
        identity(pid).unwrap().unwrap(),
        json!({
            "pid": pid, "start_ticks": ticks, "boot_id": boot.trim()
        })
    );
    let mut forged_boot = identity(pid).unwrap().unwrap();
    forged_boot["boot_id"] = json!("00000000-0000-0000-0000-000000000000");
    assert!(!matches(&forged_boot).unwrap());
}
