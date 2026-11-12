//! Opt-in registration/query/removal smoke for disposable platform CI runners.
//! It never launches the registered test executable or pairs a real computer.
// Match the downloaded desktop applications, including Windows child-process
// output encoding when their parent has no console.
#![cfg_attr(windows, windows_subsystem = "windows")]
use amplifier_native_platform::{self as platform, Application, MACHINE_HELPER, OBSERVATORY};
use std::{fs, process::Command};
#[test]
#[ignore = "writes a unique current-user startup entry; run only in disposable CI"]
fn native_startup_registration_query_and_removal() {
    exercise(&MACHINE_HELPER);
    exercise(&OBSERVATORY);
}
fn exercise(app: &Application) {
    assert_eq!(
        std::env::var("CONSTELLATION_STARTUP_SMOKE").as_deref(),
        Ok("1"),
        "Explicit disposable-runner opt-in required"
    );
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("helper café 子 with spaces");
    fs::create_dir(&home).unwrap();
    // Installed services canonicalize their home before calling this API.
    // On Windows this is an extended-length path, unlike the input spelling.
    let home = home.canonicalize().unwrap();
    let id = uuid::Uuid::new_v4();
    let label = format!("{}-{id}", app.startup_prefix);
    platform::install_startup(app, &home, &id).unwrap();
    #[cfg(windows)]
    let _cleanup = OwnedTask(label.clone());
    #[cfg(target_os = "linux")]
    {
        assert!(
            Command::new("systemctl")
                .args(["--user", "is-enabled", &format!("{label}.service")])
                .status()
                .unwrap()
                .success()
        );
        let path = platform::user_home()
            .unwrap()
            .join(".config/systemd/user")
            .join(format!("{label}.service"));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            platform::systemd_unit(app, &platform::installed_binary(app, &home), &home).unwrap()
        );
    }
    #[cfg(target_os = "macos")]
    {
        let path = platform::user_home()
            .unwrap()
            .join("Library/LaunchAgents")
            .join(format!("{label}.plist"));
        assert!(
            Command::new("plutil")
                .arg("-lint")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            platform::launch_agent(app, &label, &platform::installed_binary(app, &home), &home)
                .unwrap()
        );
    }
    #[cfg(windows)]
    {
        let observed = task_xml(&label);
        let trigger = observed
            .split_once("<LogonTrigger>")
            .expect("one actual logon trigger")
            .1
            .split_once("</LogonTrigger>")
            .unwrap()
            .0;
        let sid = platform::current_user_sid().unwrap();
        assert_eq!(trigger.matches("<UserId>").count(), 1);
        assert!(trigger.contains(&format!("<UserId>{sid}</UserId>")));
        assert!(
            platform::task_matches(
                app,
                &observed,
                &platform::installed_binary(app, &home),
                &home,
                &sid
            )
            .unwrap()
        );
        // Even an administrative runner must prove the task is restricted to
        // this user's logon. An all-user trigger can register there while it
        // fails with AccessDenied for an ordinary Windows desktop account.
    }
    // A second registration is exact/idempotent and never replaces another task.
    platform::install_startup(app, &home, &id).unwrap();
    platform::remove_startup(app, &home, &id).unwrap();
    #[cfg(target_os = "linux")]
    assert!(
        !platform::user_home()
            .unwrap()
            .join(".config/systemd/user")
            .join(format!("{label}.service"))
            .exists()
    );
    #[cfg(target_os = "macos")]
    assert!(
        !platform::user_home()
            .unwrap()
            .join("Library/LaunchAgents")
            .join(format!("{label}.plist"))
            .exists()
    );
    #[cfg(windows)]
    assert!(
        !Command::new("schtasks.exe")
            .args(["/Query", "/TN", &label])
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(windows)]
fn task_xml(label: &str) -> String {
    let label = label.to_owned();
    std::thread::spawn(move || unsafe {
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
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        let result = {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER).unwrap();
            let empty = VARIANT::default();
            service.Connect(&empty, &empty, &empty, &empty).unwrap();
            let folder = service.GetFolder(&BSTR::from("\\")).unwrap();
            let task = folder.GetTask(&BSTR::from(label)).unwrap();
            String::from_utf16(&task.Xml().unwrap()).unwrap()
        };
        CoUninitialize();
        result
    })
    .join()
    .unwrap()
}

// Only the exact fresh UUID registration created above belongs to this fixture.
// Keep cleanup independent of the identity decoder under test, including failure.
#[cfg(windows)]
struct OwnedTask(String);
#[cfg(windows)]
impl Drop for OwnedTask {
    fn drop(&mut self) {
        let _ = Command::new("schtasks.exe")
            .args(["/Delete", "/TN", &self.0, "/F"])
            .output();
    }
}
