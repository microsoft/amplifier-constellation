//! The real synchronous ABI and fixed CLI contend for the same installation
//! lock. All children and files are disposable; no services or models are used.
use amplifier_native_platform::{private_dir, private_write, process};
use constellation_native_driver::ffi::{
    NativeBuffer, constellation_native_abi_version, constellation_native_buffer_free,
    constellation_native_close, constellation_native_open, constellation_native_request,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    ptr, thread,
    time::{Duration, Instant},
};

const DRIVER: &str = env!("CARGO_BIN_EXE_constellation-native-driver");

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    config: PathBuf,
    driver: PathBuf,
    document: Value,
}

impl Fixture {
    fn new() -> Self {
        let directory = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::Builder::new()
                .prefix(".native-inprocess-")
                .tempdir_in(root)
                .unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let home = directory.path().canonicalize().unwrap();
        private_dir(&home).unwrap();
        let config = home.join("owner.json");
        let driver = home.join(if cfg!(windows) {
            "native-driver.exe"
        } else {
            "native-driver"
        });
        private_write(&driver, &fs::read(DRIVER).unwrap()).unwrap();
        make_executable(&driver);
        let document = json!({
            "version": 1,
            "roots": {"state": home},
            "locks": {"owner": home.join("owner.lock")},
            "commands": {"probe": {
                "executable": std::env::current_exe().unwrap(),
                "args": ["--exact", "detached_probe_fixture", "--nocapture"],
                "cwd": home, "timeout_seconds": 15, "result_root": null
            }}
        });
        private_write(&config, &serde_json::to_vec(&document).unwrap()).unwrap();
        Self {
            _directory: directory,
            home,
            config,
            driver,
            document,
        }
    }

    fn open(&self) -> Abi {
        let (handle, reply) = open(&self.config, &self.driver);
        assert_eq!(reply["outcome"], "ok", "{reply}");
        assert_ne!(handle, 0);
        Abi { handle }
    }

    fn cli_lock(&self) -> Value {
        let mut child = Command::new(DRIVER)
            .arg("--config")
            .arg(&self.config)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{}", frame("lock.acquire")).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn empty_buffer() -> NativeBuffer {
    NativeBuffer {
        data: ptr::null_mut(),
        len: 0,
    }
}

fn decode(buffer: NativeBuffer) -> Value {
    assert!(!buffer.data.is_null());
    // SAFETY: this is the exact initialized buffer returned by this ABI. Copy
    // before freeing it, and pass its allocation/length back exactly once.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) }.to_vec();
    unsafe {
        constellation_native_buffer_free(buffer);
    }
    serde_json::from_slice(&bytes).unwrap()
}

fn open(config: &Path, driver: &Path) -> (u64, Value) {
    let options =
        serde_json::to_vec(&json!({"version":1,"config":config,"driver":driver})).unwrap();
    let mut handle = 0;
    let mut out = empty_buffer();
    // SAFETY: all pointers refer to aligned storage valid for the call.
    let code = unsafe {
        constellation_native_open(options.as_ptr(), options.len(), &mut handle, &mut out)
    };
    assert_eq!(code, 0);
    (handle, decode(out))
}

fn request(handle: u64, bytes: &[u8]) -> Value {
    let mut out = empty_buffer();
    // SAFETY: input and output remain valid until the synchronous call returns.
    let code =
        unsafe { constellation_native_request(handle, bytes.as_ptr(), bytes.len(), &mut out) };
    assert_eq!(code, 0);
    decode(out)
}

fn close(handle: u64) -> Value {
    let mut out = empty_buffer();
    // SAFETY: output is aligned and writable for this synchronous call.
    assert_eq!(unsafe { constellation_native_close(handle, &mut out) }, 0);
    decode(out)
}

fn frame(op: &str) -> Value {
    json!({"version":1,"id":"fixture","op":op,"name":"owner"})
}

struct Abi {
    handle: u64,
}
impl Abi {
    fn call(&self, value: Value) -> Value {
        request(self.handle, &serde_json::to_vec(&value).unwrap())
    }
}
impl Drop for Abi {
    fn drop(&mut self) {
        // Cleanup for test assertion failure; production close deliberately
        // does not release a caller's still-running critical section.
        let _ = self.call(frame("lock.release"));
        let _ = close(self.handle);
    }
}

#[test]
fn abi_describe_and_invalid_frames_use_existing_outcomes() {
    let fixture = Fixture::new();
    let abi = fixture.open();
    assert_eq!(constellation_native_abi_version(), 1);
    let reply = abi.call(json!({"version":1,"id":"describe","op":"describe"}));
    assert_eq!(reply["outcome"], "ok");
    assert_eq!(reply["result"]["protocol"], 1);
    let malformed = request(abi.handle, b"{not-json");
    assert_eq!(malformed["outcome"], "refused");
    assert_eq!(malformed["error"]["code"], "invalid_request");
    let unknown = abi.call(json!({"version":1,"id":"extra","op":"describe","command":"arbitrary"}));
    assert_eq!(unknown["outcome"], "refused");
    assert_eq!(close(abi.handle)["outcome"], "ok");
    assert_eq!(close(abi.handle)["error"]["code"], "invalid_handle");
}

#[test]
fn installed_driver_must_be_absolute_and_private() {
    let fixture = Fixture::new();
    let (handle, reply) = open(&fixture.config, Path::new("native-driver"));
    assert_eq!(handle, 0);
    assert_eq!(reply["outcome"], "refused");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fixture.driver, fs::Permissions::from_mode(0o755)).unwrap();
        let (handle, reply) = open(&fixture.config, &fixture.driver);
        assert_eq!(handle, 0);
        assert_eq!(reply["outcome"], "refused");
    }
}

#[test]
fn held_abi_lock_contends_with_cli_and_close_does_not_unlock() {
    let fixture = Fixture::new();
    let abi = fixture.open();
    assert_eq!(abi.call(frame("lock.acquire"))["outcome"], "ok");
    assert_eq!(fixture.cli_lock()["outcome"], "busy");
    assert_eq!(close(abi.handle)["error"]["code"], "locks_held");
    assert_eq!(fixture.cli_lock()["outcome"], "busy");
    assert_eq!(abi.call(frame("lock.release"))["outcome"], "ok");
    assert_eq!(fixture.cli_lock()["outcome"], "ok");
}

#[test]
fn another_thread_cannot_release_or_dispatch_inside_owned_scope() {
    let fixture = Fixture::new();
    let abi = fixture.open();
    assert_eq!(abi.call(frame("lock.acquire"))["outcome"], "ok");
    let handle = abi.handle;
    let reply = thread::spawn(move || {
        request(handle, &serde_json::to_vec(&frame("lock.release")).unwrap())
    })
    .join()
    .unwrap();
    assert_eq!(reply["error"]["code"], "thread_owned");
    assert_eq!(fixture.cli_lock()["outcome"], "busy");
    assert_eq!(abi.call(frame("lock.release"))["outcome"], "ok");
    let reply = thread::spawn(move || {
        request(
            handle,
            br#"{"version":1,"id":"other-thread","op":"describe"}"#,
        )
    })
    .join()
    .unwrap();
    assert_eq!(reply["outcome"], "ok");
}

#[test]
fn configuration_drift_refuses_dispatch_but_retains_release() {
    let mut fixture = Fixture::new();
    let abi = fixture.open();
    assert_eq!(abi.call(frame("lock.acquire"))["outcome"], "ok");
    fixture.document["roots"]["extra"] = json!(fixture.home);
    private_write(
        &fixture.config,
        &serde_json::to_vec(&fixture.document).unwrap(),
    )
    .unwrap();
    let reply = abi.call(json!({"version":1,"id":"drift","op":"describe"}));
    assert_eq!(reply["error"]["code"], "configuration_changed");
    assert_eq!(fixture.cli_lock()["outcome"], "busy");
    assert_eq!(abi.call(frame("lock.release"))["outcome"], "ok");
    assert_eq!(fixture.cli_lock()["outcome"], "ok");
}

#[test]
fn driver_byte_drift_refuses_dispatch_but_retains_release() {
    let fixture = Fixture::new();
    let abi = fixture.open();
    assert_eq!(abi.call(frame("lock.acquire"))["outcome"], "ok");
    let mut bytes = fs::read(&fixture.driver).unwrap();
    bytes.push(0);
    private_write(&fixture.driver, &bytes).unwrap();
    make_executable(&fixture.driver);
    let reply = abi.call(json!({"version":1,"id":"drift","op":"describe"}));
    assert_eq!(reply["error"]["code"], "driver_changed");
    assert_eq!(fixture.cli_lock()["outcome"], "busy");
    assert_eq!(abi.call(frame("lock.release"))["outcome"], "ok");
    assert_eq!(fixture.cli_lock()["outcome"], "ok");
}

fn read_record(path: &Path) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(bytes) = fs::read(path)
            && let Ok(record) = serde_json::from_slice(&bytes)
        {
            return record;
        }
        assert!(
            Instant::now() < deadline,
            "Missing fixture record: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

// The same test executable is the fixed disposable probe. Its cwd, command and
// lifetime bound come from the protected enrollment; no process-wide environment
// mutation. A post-close acknowledgment proves continued work before finish.
#[test]
fn detached_probe_fixture() {
    let home = std::env::current_dir().unwrap();
    if !home.join("probe-intent.json").exists() {
        return;
    }
    let identity = process::identity(std::process::id()).unwrap().unwrap();
    private_write(
        &home.join("probe-live.json"),
        &serde_json::to_vec(&identity).unwrap(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut acknowledged = false;
    loop {
        if !acknowledged && let Ok(request) = fs::read(home.join("probe-after-close.json")) {
            let request: Value = serde_json::from_slice(&request).unwrap();
            assert_eq!(request["phase"], "session_closed");
            private_write(
                &home.join("probe-after-close-ack.json"),
                &serde_json::to_vec(&json!({"identity":identity,"phase":"session_closed"}))
                    .unwrap(),
            )
            .unwrap();
            acknowledged = true;
        }
        if home.join("probe-finish.json").exists() {
            assert!(
                acknowledged,
                "Finish must follow the post-close acknowledgment"
            );
            private_write(&home.join("probe-terminal.json"), br#"{"settled":true}"#).unwrap();
            return;
        }
        if Instant::now() >= deadline {
            private_write(
                &home.join("probe-terminal.json"),
                &serde_json::to_vec(&json!({"settled":false,"reason":"fixture_deadline","after_close_acknowledged":acknowledged})).unwrap(),
            )
            .unwrap();
            panic!("Fixture owner did not finish probe before its own 10s deadline");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn detached_probe_survives_session_close_without_inheriting_lock() {
    const ISOLATED: &str = "CONSTELLATION_NATIVE_PROBE_ISOLATED";
    if std::env::var_os(ISOLATED).is_none() {
        // Other tests intentionally hash driver bytes while holding this
        // process's ABI registry gate. On slow CI, that unrelated contention
        // can consume the probe's entire 10s lifetime. A separate executable
        // gives this lifetime scenario its own registry without changing any
        // production timeout, native ownership or lock behavior.
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "detached_probe_survives_session_close_without_inheriting_lock",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ISOLATED, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut timed_out = false;
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            !timed_out && output.status.success(),
            "Isolated probe scenario failed (parent deadline: {timed_out}):\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let fixture = Fixture::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        detached_probe_case(&fixture);
    }));
    if let Err(error) = result {
        // Emit the small receipts into the CI log and retain the files. The
        // fixed native launcher intentionally sends child stdio to null.
        for name in [
            "probe-live.json",
            "probe-after-close.json",
            "probe-after-close-ack.json",
            "probe-finish.json",
            "probe-terminal.json",
        ] {
            match fs::read(fixture.home.join(name)) {
                Ok(bytes) => eprintln!("{name}: {}", String::from_utf8_lossy(&bytes)),
                Err(error) => eprintln!("{name}: unavailable ({error})"),
            }
        }
        let retained = fixture._directory.keep();
        eprintln!(
            "Detached probe failure evidence retained at {}",
            retained.display()
        );
        std::panic::resume_unwind(error);
    }
}

fn detached_probe_case(fixture: &Fixture) {
    private_write(&fixture.home.join("probe-intent.json"), b"{}").unwrap();
    let abi = fixture.open();
    assert_eq!(abi.call(frame("lock.acquire"))["outcome"], "ok");
    let launch = abi.call(json!({"version":1,"id":"probe","op":"process.launch","name":"probe"}));
    assert_eq!(launch["outcome"], "ok", "{launch}");
    let probe = read_record(&fixture.home.join("probe-live.json"));
    assert!(process::matches(&probe).unwrap());
    #[cfg(target_os = "linux")]
    for identity in [&probe, &launch["result"]] {
        let directory = PathBuf::from(format!("/proc/{}/fd", identity["pid"].as_u64().unwrap()));
        for entry in fs::read_dir(directory).unwrap() {
            if let Ok(target) = fs::read_link(entry.unwrap().path()) {
                assert_ne!(
                    target,
                    fixture.home.join("owner.lock"),
                    "Invocation lock leaked into detached child"
                );
            }
        }
    }
    assert_eq!(abi.call(frame("lock.release"))["outcome"], "ok");
    assert_eq!(close(abi.handle)["outcome"], "ok");
    private_write(
        &fixture.home.join("probe-after-close.json"),
        br#"{"phase":"session_closed"}"#,
    )
    .unwrap();
    let acknowledged = read_record(&fixture.home.join("probe-after-close-ack.json"));
    assert_eq!(acknowledged["identity"], probe);
    assert_eq!(acknowledged["phase"], "session_closed");
    assert!(
        process::matches(&probe).unwrap(),
        "Probe exited before explicit finish; inspect retained terminal"
    );
    assert_eq!(fixture.cli_lock()["outcome"], "ok");
    private_write(&fixture.home.join("probe-finish.json"), b"{}").unwrap();
    assert_eq!(
        read_record(&fixture.home.join("probe-terminal.json"))["settled"],
        true
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    while process::matches(&launch["result"]).unwrap() || process::matches(&probe).unwrap() {
        assert!(
            Instant::now() < deadline,
            "Completed detached fixture remained live"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
