#![cfg(feature = "installation")]
//! Opt-in integration against a unique, disposable current-user service. These
//! tests do not select any existing installation or launch an agent/model.
use amplifier_native_platform::installation::{
    self, ActionOutcome, Service, ServiceAction, ServiceManager,
};
use serde_json::{Value, json};
#[cfg(windows)]
use std::path::PathBuf;
use std::{
    fs,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn snapshot_rejects_cross_platform_manager_and_option_names_before_execution() {
    let manager = if cfg!(windows) {
        ServiceManager::SystemdUser
    } else {
        ServiceManager::ScheduledTask
    };
    let service = Service {
        manager,
        name: "not-a-real-enrollment".into(),
        definition_path: None,
    };
    assert!(installation::snapshot(&service).is_err());
    let service = Service {
        manager: local_manager(),
        name: "--help".into(),
        definition_path: None,
    };
    assert!(installation::snapshot(&service).is_err());
}

#[test]
#[ignore = "creates a unique current-user service; explicit disposable-runner opt-in required"]
fn native_service_snapshot_compare_start_stop_and_independent_disable() {
    assert_eq!(
        std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref(),
        Ok("1")
    );
    let fixture = Fixture::create();
    let stopped = wait(&fixture.service, false);
    assert_eq!(stopped["definition_matches"], true);
    assert_eq!(stopped["processes"], json!([]));
    assert!(
        matches!(installation::request_state(&fixture.service,ServiceAction::Start,&json!({"stale":true})).unwrap(),ActionOutcome::Refused{code,..} if code=="stale_snapshot")
    );
    assert!(matches!(
        installation::request_state(&fixture.service, ServiceAction::Start, &stopped).unwrap(),
        ActionOutcome::Accepted
    ));
    let running = wait(&fixture.service, true);
    assert!(!running["processes"].as_array().unwrap().is_empty());
    assert!(
        matches!(installation::request_state(&fixture.service,ServiceAction::Stop,&stopped).unwrap(),ActionOutcome::Refused{code,..} if code=="stale_snapshot")
    );
    // Neither a stale request nor an unknown action can stop a new generation.
    assert_eq!(wait(&fixture.service, true)["active"], true);
    assert!(matches!(
        installation::request_state(&fixture.service, ServiceAction::Stop, &running).unwrap(),
        ActionOutcome::Accepted
    ));
    let stopped_again = wait(&fixture.service, false);
    assert_eq!(stopped_again["definition"], stopped["definition"]);
    let boundary = fixture.python_boundary();
    if let Some(boundary) = &boundary {
        boundary.run("cycle", None);
    }
    let before_edit = wait(&fixture.service, false);
    fixture.change_definition();
    let changed = wait(&fixture.service, false);
    assert!(
        changed["definition"] != before_edit["definition"]
            || changed["effective_definition"] != before_edit["effective_definition"]
    );
    assert!(
        matches!(installation::request_state(&fixture.service,ServiceAction::Start,&before_edit).unwrap(),ActionOutcome::Refused{code,..} if code=="stale_snapshot")
    );
    assert_eq!(installation::snapshot(&fixture.service).unwrap(), changed);
    if let Some(boundary) = &boundary {
        boundary.run("definition_changed", Some(&before_edit));
    }
    fixture.disable();
    let disabled = installation::snapshot(&fixture.service).unwrap();
    assert_eq!(disabled["enabled"], false);
    assert!(
        matches!(installation::request_state(&fixture.service,ServiceAction::Start,&disabled).unwrap(),ActionOutcome::Refused{code,..} if code=="disabled_or_unknown")
    );
    assert_eq!(installation::snapshot(&fixture.service).unwrap(), disabled);
    if let Some(boundary) = &boundary {
        boundary.run("disabled", None);
    }
    #[cfg(target_os = "linux")]
    {
        fixture.mask();
        let masked = installation::snapshot(&fixture.service).unwrap();
        assert_eq!(masked["masked"], true);
        assert_eq!(masked["native"]["LoadState"], "masked");
        assert_eq!(masked["native"]["UnitFileState"], "masked");
        assert!(
            matches!(installation::request_state(&fixture.service,ServiceAction::Start,&masked).unwrap(),ActionOutcome::Refused{code,..} if code=="disabled_or_unknown")
        );
        assert_eq!(installation::snapshot(&fixture.service).unwrap(), masked);
        if let Some(boundary) = &boundary {
            boundary.run("masked", None);
        }
    }
}

fn local_manager() -> ServiceManager {
    if cfg!(target_os = "linux") {
        ServiceManager::SystemdUser
    } else if cfg!(target_os = "macos") {
        ServiceManager::LaunchAgent
    } else {
        ServiceManager::ScheduledTask
    }
}
fn wait(service: &Service, active: bool) -> Value {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let started = Instant::now();
        let observed = installation::snapshot(service);
        println!(
            "native service observation: elapsed_ms={} success={} desired_active={active}",
            started.elapsed().as_millis(),
            observed.is_ok()
        );
        match observed {
            Ok(value) if value["active"] == active && value["transitioning"] == false => {
                return value;
            }
            result if Instant::now() >= until => {
                panic!("Service did not settle to {active}: {result:?}")
            }
            _ => thread::sleep(Duration::from_millis(100)),
        }
    }
}
fn run(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "Disposable setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
struct Fixture {
    service: Service,
    _temporary: tempfile::TempDir,
}
impl Fixture {
    fn create() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let label = format!("constellation-installation-test-{}", uuid::Uuid::new_v4());
        #[cfg(target_os = "linux")]
        let fixture = {
            let directory = amplifier_native_platform::user_home()
                .unwrap()
                .join(".config/systemd/user");
            fs::create_dir_all(&directory).unwrap();
            let name = format!("{label}.service");
            let path = directory.join(&name);
            let definition = "[Unit]\nDescription=Disposable installation conformance\n[Service]\nType=simple\nExecStart=/bin/sleep 120\nRestart=no\n[Install]\nWantedBy=default.target\n";
            amplifier_native_platform::private_create(&path, definition.as_bytes()).unwrap();
            let fixture = Self {
                service: Service {
                    manager: ServiceManager::SystemdUser,
                    name,
                    definition_path: Some(path),
                },
                _temporary: temp,
            };
            run(Command::new("/usr/bin/systemctl").args(["--user", "daemon-reload"]));
            run(Command::new("/usr/bin/systemctl").args([
                "--user",
                "enable",
                "--",
                &fixture.service.name,
            ]));
            fixture
        };
        #[cfg(target_os = "macos")]
        let fixture = {
            let directory = amplifier_native_platform::user_home()
                .unwrap()
                .join("Library/LaunchAgents");
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join(format!("{label}.plist"));
            let plist = format!(
                "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>{label}</string><key>ProgramArguments</key><array><string>/bin/sleep</string><string>120</string></array><key>RunAtLoad</key><false/><key>KeepAlive</key><false/></dict></plist>"
            );
            amplifier_native_platform::private_create(&path, plist.as_bytes()).unwrap();
            let fixture = Self {
                service: Service {
                    manager: ServiceManager::LaunchAgent,
                    name: label,
                    definition_path: Some(path),
                },
                _temporary: temp,
            };
            run(Command::new("/bin/launchctl")
                .args(["bootstrap", &domain()])
                .arg(fixture.service.definition_path.as_ref().unwrap()));
            fixture
        };
        #[cfg(windows)]
        let fixture = {
            let path = temp.path().join("task.xml");
            let executable = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let sid = amplifier_native_platform::current_user_sid().unwrap();
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-16"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Principals><Principal id="Author"><UserId>{sid}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><ExecutionTimeLimit>PT0S</ExecutionTimeLimit></Settings><Actions Context="Author"><Exec><Command>{}</Command><Arguments>-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 120"</Arguments></Exec></Actions></Task>"#,
                xml_escape(&executable.to_string_lossy())
            );
            let bytes: Vec<u8> = std::iter::once(0xfeff)
                .chain(xml.encode_utf16())
                .flat_map(u16::to_le_bytes)
                .collect();
            amplifier_native_platform::private_create(&path, &bytes).unwrap();
            let fixture = Self {
                service: Service {
                    manager: ServiceManager::ScheduledTask,
                    name: label,
                    definition_path: Some(path),
                },
                _temporary: temp,
            };
            run(Command::new(schtasks())
                .args(["/Create", "/TN", &fixture.service.name, "/XML"])
                .arg(fixture.service.definition_path.as_ref().unwrap()));
            fixture
        };
        fixture
    }
    fn python_boundary(&self) -> Option<PythonBoundary> {
        let Some(driver) = std::env::var_os("CONSTELLATION_NATIVE_DRIVER") else {
            println!(
                "Python/CLI service boundary not exercised: CONSTELLATION_NATIVE_DRIVER is absent"
            );
            return None;
        };
        let driver = std::path::PathBuf::from(driver)
            .canonicalize()
            .expect("Installed test driver must exist")
            .into_os_string();
        let python = std::env::var_os("PYTHON").expect(
            "PYTHON must select the CI interpreter when CONSTELLATION_NATIVE_DRIVER is set",
        );
        let root = self
            ._temporary
            .path()
            .canonicalize()
            .unwrap()
            .join("python-boundary");
        amplifier_native_platform::private_dir(&root).unwrap();
        let state = root.join("state");
        amplifier_native_platform::private_dir(&state).unwrap();
        let config = root.join("owner.json");
        let value = json!({"version":1,"roots":{"state":state},"locks":{"installation":state.join("installation.lock")},"services":{"body":self.service},"commands":{}});
        amplifier_native_platform::private_create(&config, &serde_json::to_vec(&value).unwrap())
            .unwrap();
        Some(PythonBoundary {
            driver,
            python,
            config,
            state,
        })
    }
    fn change_definition(&self) {
        let path = self.service.definition_path.as_ref().unwrap();
        #[cfg(target_os = "linux")]
        {
            let text = fs::read_to_string(path).unwrap().replace(
                "Description=Disposable installation conformance",
                "Description=Changed disposable installation conformance",
            );
            amplifier_native_platform::private_write(path, text.as_bytes()).unwrap();
            run(Command::new("/usr/bin/systemctl").args(["--user", "daemon-reload"]));
        }
        #[cfg(target_os = "macos")]
        {
            // Stop booted out this unique job; edit its future loaded definition.
            let text = fs::read_to_string(path)
                .unwrap()
                .replace("<string>120</string>", "<string>121</string>");
            amplifier_native_platform::private_write(path, text.as_bytes()).unwrap();
        }
        #[cfg(windows)]
        {
            let bytes = fs::read(path).unwrap();
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]]))
                .collect();
            let text = String::from_utf16(&units)
                .unwrap()
                .replace("Start-Sleep -Seconds 120", "Start-Sleep -Seconds 121");
            let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            amplifier_native_platform::private_write(path, &bytes).unwrap();
            // Change the real registered definition, not only the exported file.
            run(Command::new(schtasks())
                .args(["/Create", "/F", "/TN", &self.service.name, "/XML"])
                .arg(path));
        }
    }
    #[cfg(target_os = "linux")]
    fn mask(&self) {
        // A runtime mask loses to this fixture's higher-priority ~/.config unit.
        // Preserve the exact unique test definition before replacing that same
        // enrolled path with a real local mask. No production unit is selected.
        let path = self.service.definition_path.as_ref().unwrap();
        let original = fs::read(path).unwrap();
        let backup = self._temporary.path().join("original-before-mask.service");
        amplifier_native_platform::private_create(&backup, &original).unwrap();
        assert_eq!(fs::read(&backup).unwrap(), original);
        fs::remove_file(path).unwrap();
        std::os::unix::fs::symlink("/dev/null", path).unwrap();
        run(Command::new("/usr/bin/systemctl").args(["--user", "daemon-reload"]));
        assert_eq!(
            fs::read_link(path).unwrap(),
            std::path::Path::new("/dev/null")
        );
        assert_eq!(fs::read(&backup).unwrap(), original);
    }
    fn disable(&self) {
        #[cfg(target_os = "linux")]
        run(Command::new("/usr/bin/systemctl").args([
            "--user",
            "disable",
            "--",
            &self.service.name,
        ]));
        #[cfg(target_os = "macos")]
        {
            // Already booted out by Stop: alter only this disposable plist. Do not
            // leave persistent launchd disable overrides behind on the test user.
            let path = self.service.definition_path.as_ref().unwrap();
            let text = fs::read_to_string(path).unwrap().replacen(
                "<dict>",
                "<dict><key>Disabled</key><true/>",
                1,
            );
            amplifier_native_platform::private_write(path, text.as_bytes()).unwrap();
        }
        #[cfg(windows)]
        run(Command::new(schtasks()).args(["/Change", "/TN", &self.service.name, "/Disable"]));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("/usr/bin/systemctl")
                .args(["--user", "--no-block", "stop", "--", &self.service.name])
                .output();
            let _ = Command::new("/usr/bin/systemctl")
                .args(["--user", "disable", "--", &self.service.name])
                .output();
            let _ = fs::remove_file(self.service.definition_path.as_ref().unwrap());
            let _ = Command::new("/usr/bin/systemctl")
                .args(["--user", "daemon-reload"])
                .output();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("/bin/launchctl")
                .args(["bootout", &format!("{}/{}", domain(), self.service.name)])
                .output();
            let _ = fs::remove_file(self.service.definition_path.as_ref().unwrap());
        }
        #[cfg(windows)]
        {
            let _ = Command::new(schtasks())
                .args(["/End", "/TN", &self.service.name])
                .output();
            let _ = Command::new(schtasks())
                .args(["/Delete", "/TN", &self.service.name, "/F"])
                .output();
        }
    }
}
#[cfg(target_os = "macos")]
fn domain() -> String {
    format!("gui/{}", unsafe { libc::geteuid() })
}
#[cfg(windows)]
fn schtasks() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/schtasks.exe")
}
#[cfg(windows)]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct PythonBoundary {
    driver: std::ffi::OsString,
    python: std::ffi::OsString,
    config: std::path::PathBuf,
    state: std::path::PathBuf,
}
impl PythonBoundary {
    fn run(&self, phase: &str, expected: Option<&Value>) {
        let receipt = self.state.join("expected.json");
        if let Some(expected) = expected {
            amplifier_native_platform::private_write(
                &receipt,
                &serde_json::to_vec(expected).unwrap(),
            )
            .unwrap();
        }
        let helper = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/service_boundary.py");
        let output = Command::new(&self.python)
            .arg(helper)
            .arg(&self.driver)
            .arg(&self.config)
            .arg(phase)
            .arg(receipt)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Actual Python service boundary {phase} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["phase"], phase);
        assert!(result["passed"].as_u64().unwrap() > 0);
        println!("actual Python/CLI service boundary: {result}");
    }
}
