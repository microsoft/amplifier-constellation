#![cfg(unix)]

use amplifier_native_platform::{private_dir, private_write};
use constellation_native_driver::ffi::*;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
};

const DRIVER: &str = env!("CARGO_BIN_EXE_constellation-native-driver");

fn call(handle: u64, op: &str) -> Value {
    let mut value = json!({"version":1,"id":"fixture","op":op});
    if op.starts_with("lock.") {
        value["name"] = json!("owner");
    }
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut out = NativeBuffer {
        data: std::ptr::null_mut(),
        len: 0,
    };
    let code =
        unsafe { constellation_native_request(handle, bytes.as_ptr(), bytes.len(), &mut out) };
    assert_eq!(code, 0);
    consume(out)
}

fn consume(out: NativeBuffer) -> Value {
    let value =
        serde_json::from_slice(unsafe { std::slice::from_raw_parts(out.data, out.len) }).unwrap();
    unsafe {
        constellation_native_buffer_free(out);
    }
    value
}

fn open(options: &[u8]) -> u64 {
    let mut handle = 0;
    let mut out = NativeBuffer {
        data: std::ptr::null_mut(),
        len: 0,
    };
    assert_eq!(
        unsafe {
            constellation_native_open(options.as_ptr(), options.len(), &mut handle, &mut out)
        },
        0
    );
    assert_eq!(consume(out)["outcome"], "ok");
    handle
}

fn close(handle: u64) {
    let mut out = NativeBuffer {
        data: std::ptr::null_mut(),
        len: 0,
    };
    assert_eq!(unsafe { constellation_native_close(handle, &mut out) }, 0);
    assert_eq!(consume(out)["outcome"], "ok");
}

#[test]
fn fork_case() {
    let Ok(mode) = std::env::var("NATIVE_INPROCESS_FORK_CASE") else {
        return;
    };
    let directory = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
        Some(root) => tempfile::tempdir_in(root).unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
    let home = directory.path().canonicalize().unwrap();
    private_dir(&home).unwrap();
    let driver = home.join("fixed-driver");
    private_write(&driver, &fs::read(DRIVER).unwrap()).unwrap();
    fs::set_permissions(&driver, fs::Permissions::from_mode(0o700)).unwrap();
    let config = home.join("owner.json");
    private_write(
        &config,
        &serde_json::to_vec(
            &json!({"version":1,"roots":{"state":home},"locks":{"owner":home.join("owner.lock")}}),
        )
        .unwrap(),
    )
    .unwrap();
    let options =
        serde_json::to_vec(&json!({"version":1,"config":config,"driver":driver})).unwrap();
    let handle = open(&options);
    if mode != "no_lock" {
        assert_eq!(call(handle, "lock.acquire")["outcome"], "ok");
    }
    if mode == "exec" {
        // std subprocess fork/exec must not trigger a blanket pthread child exit.
        let mut child = Command::new(&driver)
            .args(["--config", config.to_str().unwrap(), "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                b"{\"version\":1,\"id\":\"child\",\"op\":\"lock.acquire\",\"name\":\"owner\"}\n",
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["outcome"], "busy");
    } else {
        // This is the same coordinated sequence used by Python register_at_fork.
        unsafe {
            constellation_native_fork_prepare();
        }
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            unsafe {
                constellation_native_fork_child();
            }
            // Only a no-lock child can reach here. Old handles are invalid and
            // freshly opened process-owned sessions work normally.
            let old = call(handle, "describe");
            let fresh = open(&options);
            let acquired = call(fresh, "lock.acquire");
            let released = call(fresh, "lock.release");
            close(fresh);
            let success = old["error"]["code"] == "invalid_handle"
                && acquired["outcome"] == "ok"
                && released["outcome"] == "ok";
            unsafe {
                libc::_exit(if success { 0 } else { 124 });
            }
        }
        unsafe {
            constellation_native_fork_parent();
        }
        let mut status = 0;
        assert_eq!(unsafe { libc::waitpid(child, &mut status, 0) }, child);
        assert!(libc::WIFEXITED(status));
        assert_eq!(
            libc::WEXITSTATUS(status),
            if mode == "no_lock" { 0 } else { 125 }
        );
    }
    if mode == "no_lock" {
        assert_eq!(call(handle, "lock.acquire")["outcome"], "ok");
    }
    let contender = open(&options);
    assert_eq!(call(contender, "lock.acquire")["outcome"], "busy");
    assert_eq!(call(handle, "lock.release")["outcome"], "ok");
    assert_eq!(call(contender, "lock.acquire")["outcome"], "ok");
    assert_eq!(call(contender, "lock.release")["outcome"], "ok");
    close(handle);
    close(contender);
}

#[test]
fn coordinated_fork_and_subprocess_lifetimes() {
    for mode in ["held_lock", "no_lock", "exec"] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fork_case", "--nocapture"])
            .env("NATIVE_INPROCESS_FORK_CASE", mode)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
