#![cfg(feature = "installation")]
//! Fresh registration only: unique sleep-only services, explicit disposable-CI opt-in.
use amplifier_native_platform::{
    installation::{self, ActionOutcome, Service, ServiceAction, ServiceManager},
    registration::{self, Registration, RegistrationOutcome, RetirementOutcome},
};
use serde_json::{Value, json};
#[cfg(unix)]
use std::fs;
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
        assert!(
            !matches!(
                registration::retire(&fixture.service, &fixture.profile, &paused),
                Ok(RetirementOutcome::Retired { .. })
            ),
            "Retirement removed an independent pause"
        );
        assert_eq!(settled_snapshot(&fixture.service), paused);
        println!(
            "native no-start registration: autostart={autostart}, fresh/repeat/mismatch/independent-pause passed"
        );
    }
}

#[test]
#[ignore = "creates, starts, settles and retires unique disposable current-user services"]
fn checked_retirement_preserves_active_changed_and_partial_states() {
    assert_eq!(
        std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref(),
        Ok("1")
    );
    let fixture = Fixture::new(false);
    assert!(!installation::manager_registration_exists(&fixture.service).unwrap());
    assert!(registration::is_retired(&fixture.service).unwrap());
    assert_eq!(
        registration::register(&fixture.service, &fixture.profile).unwrap(),
        RegistrationOutcome::Registered { created: true }
    );
    let registered = settled_snapshot(&fixture.service);
    #[cfg(target_os = "macos")]
    assert!(
        !installation::manager_registration_exists(&fixture.service).unwrap(),
        "No-start LaunchAgent was unexpectedly loaded"
    );
    #[cfg(not(target_os = "macos"))]
    assert!(installation::manager_registration_exists(&fixture.service).unwrap());
    assert!(!registration::is_retired(&fixture.service).unwrap());
    #[cfg(windows)]
    {
        let mut manager_only = fixture.service.clone();
        manager_only.definition_path = None;
        assert!(installation::manager_registration_exists(&manager_only).unwrap());
    }
    let definition = std::fs::read(fixture.service.definition_path.as_ref().unwrap()).unwrap();
    for change in ["args", "exe", "cwd", "description", "autostart"] {
        let mut changed = fixture.profile.clone();
        match change {
            "args" => changed.args.push("different".into()),
            "exe" => changed.executable = std::env::current_exe().unwrap(),
            "cwd" => changed.cwd = fixture._temp.path().canonicalize().unwrap(),
            "description" => changed.description.push_str(" changed"),
            "autostart" => changed.autostart = true,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                registration::retire(&fixture.service, &changed, &registered).unwrap(),
                RetirementOutcome::Refused { .. }
            ),
            "Accepted changed {change}"
        );
        assert_eq!(
            std::fs::read(fixture.service.definition_path.as_ref().unwrap()).unwrap(),
            definition
        );
    }
    assert_eq!(
        installation::request_state(&fixture.service, ServiceAction::Start, &registered).unwrap(),
        ActionOutcome::Accepted
    );
    let active = wait_activity(&fixture.service, true);
    assert!(matches!(
        registration::retire(&fixture.service, &fixture.profile, &active).unwrap(),
        RetirementOutcome::Refused { .. }
    ));
    assert_eq!(
        settled_snapshot(&fixture.service)["active"],
        true,
        "Retirement stopped a live operation"
    );
    assert_eq!(
        installation::request_state(&fixture.service, ServiceAction::Stop, &active).unwrap(),
        ActionOutcome::Accepted
    );
    let inactive = wait_activity(&fixture.service, false);
    assert_eq!(
        registration::retire(&fixture.service, &fixture.profile, &inactive).unwrap(),
        RetirementOutcome::Retired { removed: true }
    );
    assert!(registration::is_retired(&fixture.service).unwrap());
    assert!(!installation::manager_registration_exists(&fixture.service).unwrap());
    assert_eq!(
        registration::retire(&fixture.service, &fixture.profile, &inactive).unwrap(),
        RetirementOutcome::Retired { removed: false }
    );
    // An exported/remaining file does not become manager presence. Neither
    // absence observation is permission to create or run this operation again.
    amplifier_native_platform::private_create(
        fixture.service.definition_path.as_ref().unwrap(),
        &definition,
    )
    .unwrap();
    #[cfg(target_os = "macos")]
    assert!(!registration::is_retired(&fixture.service).unwrap());
    #[cfg(windows)]
    {
        let mut manager_only = fixture.service.clone();
        manager_only.definition_path = None;
        assert!(!installation::manager_registration_exists(&manager_only).unwrap());
        assert!(registration::is_retired(&fixture.service).is_err());
    }
    let mut invalid = fixture.service.clone();
    invalid.name = "--not-a-service".into();
    assert!(installation::manager_registration_exists(&invalid).is_err());
}
fn wait_activity(service: &Service, active: bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let observed = settled_snapshot(service);
        if observed["active"] == active && (active || observed["processes"] == json!([])) {
            return observed;
        }
        assert!(
            Instant::now() < deadline,
            "Service did not settle active={active}: {observed}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(windows)]
#[test]
#[ignore = "changes only a fresh UUID disposable task's disabled future trigger"]
fn retirement_checks_full_native_task_not_only_unchanged_action() {
    use windows::{
        Win32::System::TaskScheduler::{
            TASK_IGNORE_REGISTRATION_TRIGGERS, TASK_LOGON_INTERACTIVE_TOKEN, TASK_TRIGGER_TIME,
            TASK_UPDATE,
        },
        core::BSTR,
    };
    assert_eq!(
        std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref(),
        Ok("1")
    );
    let fixture = Fixture::new(false);
    assert_eq!(
        registration::register(&fixture.service, &fixture.profile).unwrap(),
        RegistrationOutcome::Registered { created: true }
    );
    let exported = std::fs::read(fixture.service.definition_path.as_ref().unwrap()).unwrap();
    let changed = with_task(&fixture, |scheduler, task| unsafe {
        let definition = task.Definition().unwrap();
        let trigger = definition
            .Triggers()
            .unwrap()
            .Create(TASK_TRIGGER_TIME)
            .unwrap();
        trigger.SetEnabled(false.into()).unwrap();
        trigger
            .SetStartBoundary(&BSTR::from("2099-12-31T12:00:00"))
            .unwrap();
        let empty = windows::Win32::System::Variant::VARIANT::default();
        let updated = scheduler
            .GetFolder(&BSTR::from("\\"))
            .unwrap()
            .RegisterTaskDefinition(
                &BSTR::from(&fixture.service.name),
                &definition,
                TASK_UPDATE.0 | TASK_IGNORE_REGISTRATION_TRIGGERS.0,
                &empty,
                &empty,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &empty,
            )
            .unwrap();
        String::from_utf16(&updated.Xml().unwrap()).unwrap()
    });
    let observed = settled_snapshot(&fixture.service);
    assert!(
        registration::profile_matches(&fixture.service, &fixture.profile, &observed).unwrap(),
        "Fixed action is still the same; trigger differs"
    );
    assert!(matches!(
        registration::retire(&fixture.service, &fixture.profile, &observed).unwrap(),
        RetirementOutcome::Refused { .. }
    ));
    assert_eq!(
        with_task(&fixture, |_, task| unsafe {
            String::from_utf16(&task.Xml().unwrap()).unwrap()
        }),
        changed
    );
    assert_eq!(
        std::fs::read(fixture.service.definition_path.as_ref().unwrap()).unwrap(),
        exported
    );
}
#[cfg(windows)]
fn with_task<T>(
    fixture: &Fixture,
    inspect: impl FnOnce(
        &windows::Win32::System::TaskScheduler::ITaskService,
        &windows::Win32::System::TaskScheduler::IRegisteredTask,
    ) -> T,
) -> T {
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
            unsafe { CoUninitialize() };
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        let _apartment = Apartment;
        let scheduler: ITaskService =
            CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER).unwrap();
        let empty = VARIANT::default();
        scheduler.Connect(&empty, &empty, &empty, &empty).unwrap();
        let task = scheduler
            .GetFolder(&BSTR::from("\\"))
            .unwrap()
            .GetTask(&BSTR::from(&fixture.service.name))
            .unwrap();
        inspect(&scheduler, &task)
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
        amplifier_native_platform::private_create_dir(&cwd).unwrap();
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
            let _ = Command::new("/bin/launchctl")
                .args([
                    "bootout",
                    &format!("gui/{}/{}", unsafe { libc::geteuid() }, self.service.name),
                ])
                .output();
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
