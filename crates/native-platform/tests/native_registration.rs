#![cfg(feature = "installation")]
//! Fresh registration only: unique sleep-only services, explicit disposable-CI opt-in.
use amplifier_native_platform::{
    installation::{self, Service, ServiceManager},
    registration::{self, Registration, RegistrationOutcome},
};
use serde_json::{Value, json};
#[cfg(unix)]
use std::fs;
#[cfg(not(target_os = "macos"))]
use std::process::Command;
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

#[test]
fn registration_profile_has_no_implicit_autostart_or_request_command_fields() {
    let profile: Registration = serde_json::from_value(
        json!({"executable":"/fixed/app","args":[],"cwd":"/fixed/home","description":"owned"}),
    )
    .unwrap();
    assert!(!profile.autostart);
    assert!(serde_json::from_value::<Registration>(json!({"executable":"/fixed/app","args":[],"cwd":"/fixed/home","description":"owned","shell":"unexpected"})).is_err());
}

#[test]
#[ignore = "creates unique no-start current-user services; disposable native CI only"]
fn fresh_registration_never_starts_or_replaces_existing_profiles_and_pauses() {
    assert_eq!(
        std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref(),
        Ok("1")
    );
    for autostart in [false, true] {
        let fixture = Fixture::new(autostart);
        assert_eq!(
            registration::register(&fixture.service, &fixture.profile).unwrap(),
            RegistrationOutcome::Registered { created: true }
        );
        let registered = settled_snapshot(&fixture.service);
        assert_eq!(
            registered["active"], false,
            "Registration started a service"
        );
        assert_eq!(registered["processes"], json!([]));
        println!(
            "registration observation: definition_matches={} load={} need_reload={} drop_ins={}",
            registered["definition_matches"],
            registered["native"]["LoadState"],
            registered["native"]["NeedDaemonReload"],
            registered["native"]["DropInPaths"]
        );
        #[cfg(windows)]
        if autostart {
            print_task_identity_difference(&fixture);
        }
        assert_eq!(
            registration::register(&fixture.service, &fixture.profile).unwrap(),
            RegistrationOutcome::Registered { created: false }
        );
        assert_eq!(
            settled_snapshot(&fixture.service),
            registered,
            "Repeat registration changed service state"
        );
        let mut changed = fixture.profile.clone();
        changed.args.push("unexpected-new-argument".into());
        assert!(matches!(
            registration::register(&fixture.service, &changed).unwrap(),
            RegistrationOutcome::Refused { .. }
        ));
        assert_eq!(
            settled_snapshot(&fixture.service),
            registered,
            "Mismatching profile overwrote a definition"
        );
        fixture.pause();
        let paused = settled_snapshot(&fixture.service);
        assert_eq!(paused["start_allowed"], false);
        assert!(matches!(
            registration::register(&fixture.service, &fixture.profile).unwrap(),
            RegistrationOutcome::Registered { created: false }
                | RegistrationOutcome::Refused { .. }
        ));
        assert_eq!(
            settled_snapshot(&fixture.service),
            paused,
            "Repeat setup changed an independent pause"
        );
        println!(
            "native no-start registration: autostart={autostart}, fresh/repeat/mismatch/independent-pause passed"
        );
    }
}
fn settled_snapshot(service: &Service) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match installation::snapshot(service) {
            Ok(value) if value["transitioning"] == false => return value,
            result if Instant::now() >= deadline => {
                panic!("Registration did not become observable: {result:?}")
            }
            _ => thread::sleep(Duration::from_millis(100)),
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn run(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "Disposable fixture operation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
struct Fixture {
    service: Service,
    profile: Registration,
    _temp: tempfile::TempDir,
}
impl Fixture {
    fn new(autostart: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path().canonicalize().unwrap().join("literal $cwd");
        amplifier_native_platform::private_dir(&cwd).unwrap();
        let label = format!("constellation-registration-test-{}", uuid::Uuid::new_v4());
        #[cfg(target_os = "linux")]
        let service = {
            let name = format!("{label}.service");
            Service {
                manager: ServiceManager::SystemdUser,
                definition_path: Some(
                    amplifier_native_platform::user_home()
                        .unwrap()
                        .join(".config/systemd/user")
                        .join(&name),
                ),
                name,
            }
        };
        #[cfg(target_os = "macos")]
        let service = Service {
            manager: ServiceManager::LaunchAgent,
            definition_path: Some(
                amplifier_native_platform::user_home()
                    .unwrap()
                    .join("Library/LaunchAgents")
                    .join(format!("{label}.plist")),
            ),
            name: label,
        };
        #[cfg(windows)]
        let service = Service {
            manager: ServiceManager::ScheduledTask,
            definition_path: Some(cwd.join("task.xml")),
            name: label,
        };
        #[cfg(unix)]
        let (executable, args) = (PathBuf::from("/bin/sleep"), vec!["120".into()]);
        #[cfg(windows)]
        let (executable, args) = (
            system_tool("WindowsPowerShell/v1.0/powershell.exe"),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                "Start-Sleep -Seconds 120".into(),
            ],
        );
        let profile = Registration {
            executable,
            args,
            cwd,
            description: "Disposable no-start service α".into(),
            autostart,
        };
        Self {
            service,
            profile,
            _temp: temp,
        }
    }
    fn pause(&self) {
        #[cfg(target_os = "linux")]
        if self.profile.autostart {
            run(Command::new("/usr/bin/systemctl").args([
                "--user",
                "disable",
                "--",
                &self.service.name,
            ]));
        } else {
            let path = self.service.definition_path.as_ref().unwrap();
            let bytes = fs::read(path).unwrap();
            let backup = self._temp.path().join("original.service");
            amplifier_native_platform::private_create(&backup, &bytes).unwrap();
            fs::remove_file(path).unwrap();
            std::os::unix::fs::symlink("/dev/null", path).unwrap();
            run(Command::new("/usr/bin/systemctl").args(["--user", "daemon-reload"]));
            assert_eq!(fs::read(backup).unwrap(), bytes);
        }
        #[cfg(target_os = "macos")]
        {
            let path = self.service.definition_path.as_ref().unwrap();
            let text = fs::read_to_string(path).unwrap().replacen(
                "<dict>",
                "<dict><key>Disabled</key><true/>",
                1,
            );
            amplifier_native_platform::private_write(path, text.as_bytes()).unwrap();
        }
        #[cfg(windows)]
        run(Command::new(system_tool("schtasks.exe")).args([
            "/Change",
            "/TN",
            &self.service.name,
            "/Disable",
        ]));
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
            let _ = fs::remove_file(self.service.definition_path.as_ref().unwrap());
        }
        #[cfg(windows)]
        {
            let _ = Command::new(system_tool("schtasks.exe"))
                .args(["/Delete", "/TN", &self.service.name, "/F"])
                .output();
        }
    }
}
#[cfg(windows)]
fn system_tool(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32")
        .join(name)
}

#[cfg(windows)]
fn print_task_identity_difference(fixture: &Fixture) {
    use windows::{
        Win32::System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            TaskScheduler::{ITaskService, TaskScheduler},
            Variant::VARIANT,
        },
        core::BSTR,
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        let _apartment = Apartment;
        let service: ITaskService =
            CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER).unwrap();
        let empty = VARIANT::default();
        service.Connect(&empty, &empty, &empty, &empty).unwrap();
        let bytes = std::fs::read(fixture.service.definition_path.as_ref().unwrap()).unwrap();
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|value| u16::from_le_bytes([value[0], value[1]]))
            .collect();
        let expected = String::from_utf16(&units).unwrap();
        let desired = service.NewTask(0).unwrap();
        desired
            .SetXmlText(&BSTR::from(expected.trim_start_matches('\u{feff}')))
            .unwrap();
        let existing = service
            .GetFolder(&BSTR::from("\\"))
            .unwrap()
            .GetTask(&BSTR::from(&fixture.service.name))
            .unwrap();
        let mut desired_xml = BSTR::new();
        let mut actual_xml = BSTR::new();
        desired.XmlText(&mut desired_xml).unwrap();
        existing
            .Definition()
            .unwrap()
            .XmlText(&mut actual_xml)
            .unwrap();
        let desired = String::from_utf16(&desired_xml).unwrap();
        let actual = String::from_utf16(&actual_xml).unwrap();
        let first = desired
            .chars()
            .zip(actual.chars())
            .position(|(a, b)| a != b);
        if let Some(index) = first {
            let start = index.saturating_sub(100);
            println!(
                "disposable task raw XML first difference at char {index}: expected={:?}, actual={:?}",
                desired.chars().skip(start).take(700).collect::<String>(),
                actual.chars().skip(start).take(700).collect::<String>()
            );
        } else {
            println!(
                "disposable task raw XML equal={} lengths={}/{}",
                desired == actual,
                desired.len(),
                actual.len()
            );
        }
    }
}
