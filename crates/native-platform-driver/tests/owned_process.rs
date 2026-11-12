use amplifier_native_platform::{private_dir, private_write, process};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DRIVER: &str = env!("CARGO_BIN_EXE_constellation-native-driver");

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    _directory: tempfile::TempDir,
    home: PathBuf,
    config: PathBuf,
}

impl Fixture {
    fn new(seconds: u64) -> Self {
        Self::with_bounds(seconds, seconds)
    }

    fn with_bounds(endpoint_seconds: u64, probe_seconds: u64) -> Self {
        // Namespaced local test environments may need a private root instead of
        // system /tmp; do not weaken the serving driver's ownership checks.
        let directory = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::Builder::new()
                .prefix(".native-owned-process-")
                .tempdir_in(root)
                .unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let home = directory.path().canonicalize().unwrap();
        private_dir(&home).unwrap();
        let config = home.join("owner.json");
        let profile = json!({
            "executable": std::env::current_exe().unwrap(),
            "args": ["--exact", "process_fixture", "--nocapture"],
            "cwd": home, "timeout_seconds": endpoint_seconds, "result_root": null
        });
        let mut probe_profile = profile.clone();
        probe_profile["timeout_seconds"] = json!(probe_seconds);
        private_write(
            &config,
            &serde_json::to_vec(&json!({
                "version": 1, "roots": {"state": home},
                "commands": {"endpoint": profile, "probe": probe_profile}
            }))
            .unwrap(),
        )
        .unwrap();
        Self {
            _directory: directory,
            home,
            config,
        }
    }

    fn command(&self, mode: &str, entry: &str, name: Option<&str>) -> Command {
        let mut command = Command::new(DRIVER);
        command.arg("--config").arg(&self.config).arg(entry);
        if let Some(name) = name {
            command.arg("--command").arg(name);
        }
        command
            .env("OWNED_PROCESS_FIXTURE_HOME", &self.home)
            .env("OWNED_PROCESS_FIXTURE_CONFIG", &self.config)
            .env("OWNED_PROCESS_FIXTURE_MODE", mode)
            .env("OWNED_PROCESS_FIXTURE_DRIVER", DRIVER)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        command
    }

    fn endpoint(&self) -> Value {
        read_record(&self.home.join("endpoint.json"))
    }
}

fn write_record(root: &Path, name: &str, record: &Value) {
    private_write(&root.join(name), &serde_json::to_vec(record).unwrap()).unwrap();
}

fn read_record(path: &Path) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(bytes) = fs::read(path)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "Missing fixture record: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_exit(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "Owned invocation did not exit");
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_absent(receipts: &[&Value]) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let live: Vec<&Value> = receipts
            .iter()
            .copied()
            .filter(|receipt| process::matches(receipt).expect("Process observation must succeed"))
            .collect();
        if live.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Owned descendants survived cleanup: {live:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn helper(root: &Path, mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "process_fixture", "--nocapture"])
        .env("OWNED_PROCESS_FIXTURE_HOME", root)
        .env("OWNED_PROCESS_FIXTURE_MODE", mode)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

// This same executable supplies fixed, disposable endpoint/probe commands. It
// never invokes a model, network service, shell, or installed application.
#[test]
#[allow(
    clippy::zombie_processes,
    reason = "Fixture deliberately leaves a descendant for the real supervisor to reap"
)]
fn process_fixture() {
    let Some(root) = std::env::var_os("OWNED_PROCESS_FIXTURE_HOME").map(PathBuf::from) else {
        return;
    };
    let mode = std::env::var("OWNED_PROCESS_FIXTURE_MODE").unwrap();
    if mode == "capture" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        private_write(&root.join("captured.bin"), &bytes).unwrap();
        return;
    }
    if mode == "endpoint" || mode == "endpoint-exits" {
        let mut leaf = helper(&root, "leaf").spawn().unwrap();
        write_record(
            &root,
            "endpoint.json",
            &json!({
                "endpoint": process::identity(std::process::id()).unwrap().unwrap(),
                "leaf": process::identity(leaf.id()).unwrap().unwrap()
            }),
        );
        if mode == "endpoint-exits" {
            return;
        }
        // A normal endpoint may never read stdin. The supervisor still must
        // close its entire process tree on upstream EOF or caller loss.
        let _ = leaf.wait();
        return;
    }
    if mode == "caller" || mode == "probe" {
        let mut endpoint = Command::new(std::env::var_os("OWNED_PROCESS_FIXTURE_DRIVER").unwrap());
        endpoint
            .arg("--config")
            .arg(std::env::var_os("OWNED_PROCESS_FIXTURE_CONFIG").unwrap())
            .args(["exec", "--command", "endpoint"])
            .env("OWNED_PROCESS_FIXTURE_MODE", "endpoint")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        if mode == "probe" {
            use std::os::unix::process::CommandExt;
            unsafe {
                endpoint.pre_exec(|| {
                    if libc::setsid() < 0 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok(())
                    }
                });
            }
        }
        let mut endpoint = endpoint.spawn().unwrap();
        write_record(
            &root,
            "caller.json",
            &json!({
                "caller": process::identity(std::process::id()).unwrap().unwrap(),
                "wrapper": process::identity(endpoint.id()).unwrap().unwrap()
            }),
        );
        let _input = endpoint
            .stdin
            .take()
            .expect("Fixture caller owns endpoint input");
        let _ = endpoint.wait();
        return;
    }
    assert_eq!(mode, "leaf");
    thread::sleep(Duration::from_secs(30));
}

#[test]
fn upstream_input_close_reaps_endpoint_and_descendant() {
    let fixture = Fixture::new(20);
    let mut wrapper = OwnedChild(
        fixture
            .command("endpoint", "exec", Some("endpoint"))
            .spawn()
            .unwrap(),
    );
    let endpoints = fixture.endpoint();
    drop(wrapper.0.stdin.take());
    assert!(!wait_exit(&mut wrapper.0).success());
    wait_absent(&[&endpoints["endpoint"], &endpoints["leaf"]]);
}

#[test]
fn crashed_supervisor_cannot_leave_endpoint_descendants() {
    let fixture = Fixture::new(20);
    let mut wrapper = OwnedChild(
        fixture
            .command("endpoint", "exec", Some("endpoint"))
            .spawn()
            .unwrap(),
    );
    let endpoints = fixture.endpoint();
    wrapper.0.kill().unwrap();
    wrapper.0.wait().unwrap();
    wait_absent(&[&endpoints["endpoint"], &endpoints["leaf"]]);
}

#[test]
fn crashed_upstream_caller_closes_its_endpoint_tree() {
    let fixture = Fixture::new(20);
    let mut caller = helper(&fixture.home, "caller");
    caller
        .env("OWNED_PROCESS_FIXTURE_DRIVER", DRIVER)
        .env("OWNED_PROCESS_FIXTURE_CONFIG", &fixture.config);
    let mut caller = OwnedChild(caller.spawn().unwrap());
    let owner = read_record(&fixture.home.join("caller.json"));
    let endpoints = fixture.endpoint();
    caller.0.kill().unwrap();
    caller.0.wait().unwrap();
    wait_absent(&[
        &owner["wrapper"],
        &endpoints["endpoint"],
        &endpoints["leaf"],
    ]);
}

#[test]
fn absolute_bound_reaps_children_even_with_input_open() {
    let fixture = Fixture::new(2);
    let mut wrapper = OwnedChild(
        fixture
            .command("endpoint", "exec", Some("endpoint"))
            .spawn()
            .unwrap(),
    );
    let endpoints = fixture.endpoint();
    assert!(wrapper.0.stdin.is_some());
    assert!(!wait_exit(&mut wrapper.0).success());
    wait_absent(&[&endpoints["endpoint"], &endpoints["leaf"]]);
}

#[test]
fn normal_command_exit_also_cleans_remaining_descendants() {
    let fixture = Fixture::new(20);
    let mut wrapper = OwnedChild(
        fixture
            .command("endpoint-exits", "exec", Some("endpoint"))
            .spawn()
            .unwrap(),
    );
    let endpoints = fixture.endpoint();
    assert!(wait_exit(&mut wrapper.0).success());
    wait_absent(&[&endpoints["endpoint"], &endpoints["leaf"]]);
}

#[test]
fn detached_probe_and_new_session_endpoint_cleanup_follow_parent_loss() {
    let fixture = Fixture::with_bounds(20, 3);
    let mut serve_command = fixture.command("probe", "serve", None);
    serve_command.stdout(Stdio::piped());
    let mut serve = OwnedChild(serve_command.spawn().unwrap());
    writeln!(
        serve.0.stdin.as_mut().unwrap(),
        "{}",
        json!({"version":1,"id":"launch","op":"process.launch","name":"probe","result":null})
    )
    .unwrap();
    serve.0.stdin.as_mut().unwrap().flush().unwrap();
    let mut reply = String::new();
    BufReader::new(serve.0.stdout.as_mut().unwrap())
        .read_line(&mut reply)
        .unwrap();
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["outcome"], "ok", "{reply}");
    let caller = read_record(&fixture.home.join("caller.json"));
    let endpoints = fixture.endpoint();
    #[cfg(unix)]
    {
        let outer = caller["caller"]["pid"].as_u64().unwrap() as i32;
        let inner = caller["wrapper"]["pid"].as_u64().unwrap() as i32;
        assert_ne!(
            unsafe { libc::getpgid(outer) },
            unsafe { libc::getpgid(inner) },
            "Fixture must escape the outer group"
        );
        assert_eq!(
            unsafe { libc::getsid(inner) },
            inner,
            "Inner endpoint wrapper must lead its own session"
        );
    }
    // JSONL invocation exit is independent of the detached probe's lifetime.
    drop(serve.0.stdin.take());
    assert!(wait_exit(&mut serve.0).success());
    assert!(process::matches(&reply["result"]).unwrap());
    assert!(process::matches(&caller["caller"]).unwrap());
    assert!(process::matches(&caller["wrapper"]).unwrap());
    assert!(process::matches(&endpoints["endpoint"]).unwrap());
    assert!(process::matches(&endpoints["leaf"]).unwrap());
    // The outer probe's short bound ends its group. The independent endpoint
    // wrapper must notice that caller loss despite its separate Unix session.
    wait_absent(&[
        &reply["result"],
        &caller["caller"],
        &caller["wrapper"],
        &endpoints["endpoint"],
        &endpoints["leaf"],
    ]);
}

#[test]
fn start_barrier_preserves_bytes_already_waiting_for_endpoint() {
    let fixture = Fixture::new(20);
    let mut wrapper = OwnedChild(
        fixture
            .command("capture", "internal-execute", Some("endpoint"))
            .spawn()
            .unwrap(),
    );
    let payload = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
"#;
    let mut combined = b"RUN\n".to_vec();
    combined.extend_from_slice(payload);
    wrapper
        .0
        .stdin
        .take()
        .unwrap()
        .write_all(&combined)
        .unwrap();
    assert!(wait_exit(&mut wrapper.0).success());
    assert_eq!(
        fs::read(fixture.home.join("captured.bin")).unwrap(),
        payload,
        "Trampoline must not buffer and discard the first protocol frame"
    );
}

#[test]
fn service_receipt_identifies_application_and_crash_reaps_its_tree() {
    let fixture = Fixture::new(1);
    let mut config: Value = serde_json::from_slice(&fs::read(&fixture.config).unwrap()).unwrap();
    config["services"] = json!({"controller":{"manager":if cfg!(windows){"scheduled_task"}else if cfg!(target_os="macos"){"launch_agent"}else{"systemd_user"},"name":"disposable-controller","definition_path":null}});
    config["service_applications"] = json!({"controller":{"executable":std::env::current_exe().unwrap(),"args":["--exact","process_fixture","--nocapture"],"cwd":fixture.home,"identity_path":fixture.home.join("application.json")}});
    private_write(&fixture.config, &serde_json::to_vec(&config).unwrap()).unwrap();
    let mut command = fixture.command("endpoint", "service-run", None);
    command.args(["--service", "controller"]);
    let mut wrapper = OwnedChild(command.spawn().unwrap());
    let endpoints = fixture.endpoint();
    let record = read_record(&fixture.home.join("application.json"));
    assert_eq!(
        record["application"], endpoints["endpoint"],
        "Actual application must not be confused with launcher or scheduler"
    );
    assert_eq!(
        record["supervisor"],
        process::identity(wrapper.0.id()).unwrap().unwrap()
    );
    let mut current = record["application"].clone();
    let mut found = false;
    for _ in 0..4 {
        if current == record["supervisor"] {
            found = true;
            break;
        }
        current = process::parent_identity(&current).unwrap().unwrap();
    }
    assert!(found, "Application parent chain must reach its supervisor");
    // Bounded verifier settings do not turn an ordinary service into a timed job.
    thread::sleep(Duration::from_millis(1200));
    assert!(process::matches(&record["application"]).unwrap());
    let duplicate = command.output().unwrap();
    assert!(!duplicate.status.success());
    assert_eq!(read_record(&fixture.home.join("application.json")), record);
    wrapper.0.kill().unwrap();
    wrapper.0.wait().unwrap();
    wait_absent(&[&record["application"], &endpoints["leaf"]]);
}

#[test]
fn failed_application_exec_never_publishes_running_identity() {
    let fixture = Fixture::new(1);
    let bad = fixture.home.join(if cfg!(windows) {
        "invalid.exe"
    } else {
        "invalid"
    });
    private_write(&bad, b"not an executable image").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bad, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let config = json!({"version":1,"services":{"controller":{"manager":if cfg!(windows){"scheduled_task"}else if cfg!(target_os="macos"){"launch_agent"}else{"systemd_user"},"name":"disposable-controller","definition_path":null}},
        "service_applications":{"controller":{"executable":bad,"args":[],"cwd":fixture.home,"identity_path":fixture.home.join("application.json")}}});
    private_write(&fixture.config, &serde_json::to_vec(&config).unwrap()).unwrap();
    let mut command = fixture.command("endpoint", "service-run", None);
    command.args(["--service", "controller"]);
    let mut wrapper = OwnedChild(command.spawn().unwrap());
    assert!(!wait_exit(&mut wrapper.0).success());
    assert!(
        !fixture.home.join("application.json").exists(),
        "A failed executable must not receive a running receipt"
    );
}
