//! Disposable CI fixture control, not an installation API or release artifact.
//! Python owns the 30-second process deadline. Native Stop settlement retains
//! its 25-second limit; descendant generations are checked separately there.

use anyhow::{Result, bail, ensure};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Pause,
    StopAndWait,
    Delete,
}

fn request(args: &[String]) -> Result<(Action, &str)> {
    ensure!(args.len() == 2, "Expected action and disposable task name");
    let action = match args[0].as_str() {
        "pause" => Action::Pause,
        "stop-and-wait" => Action::StopAndWait,
        "delete" => Action::Delete,
        _ => bail!("Unsupported fixture action"),
    };
    let name = &args[1];
    let suffix = name
        .strip_prefix("constellation-driver-test-")
        .ok_or_else(|| anyhow::anyhow!("Only disposable fixture tasks are accepted"))?;
    ensure!(
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "Expected a 32-character lowercase hexadecimal fixture identity"
    );
    Ok((action, name))
}

fn main() -> Result<()> {
    ensure!(
        std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref() == Ok("1"),
        "Disposable service fixture requires explicit CI opt-in"
    );
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (action, name) = request(&args)?;
    execute(action, name)
}

#[cfg(not(windows))]
fn execute(_: Action, _: &str) -> Result<()> {
    bail!("This test-only fixture requires Windows")
}

#[cfg(windows)]
fn execute(action: Action, name: &str) -> Result<()> {
    use std::time::{Duration, Instant};
    use windows::{
        Win32::{
            Foundation::VARIANT_BOOL,
            System::{
                Com::{
                    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                    CoUninitialize,
                },
                TaskScheduler::{ITaskService, TaskScheduler},
                Variant::VARIANT,
            },
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
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let _apartment = Apartment;
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let empty = VARIANT::default();
        service.Connect(&empty, &empty, &empty, &empty)?;
        let folder = service.GetFolder(&BSTR::from("\\"))?;
        let task = match folder.GetTask(&BSTR::from(name)) {
            Ok(task) => task,
            Err(error) if error.code().0 == 0x80070002_u32 as i32 && action != Action::Pause => {
                println!("Disposable task is already absent: {name}");
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        match action {
            Action::Pause => task.SetEnabled(VARIANT_BOOL(0))?,
            Action::StopAndWait => {
                task.Stop(0)?;
                let deadline = Instant::now() + Duration::from_secs(25);
                while task.GetInstances(0)?.Count()? != 0 {
                    ensure!(Instant::now() < deadline, "Disposable task did not stop");
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            Action::Delete => {
                ensure!(
                    task.GetInstances(0)?.Count()? == 0,
                    "Refusing to delete a running disposable task"
                );
                folder.DeleteTask(&BSTR::from(name), 0)?;
            }
        }
        println!("Disposable task {action:?} completed: {name}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_disposable_names_and_fixed_actions_are_accepted() {
        let name = "constellation-driver-test-0123456789abcdef0123456789abcdef";
        for action in ["pause", "stop-and-wait", "delete"] {
            assert!(request(&[action.into(), name.into()]).is_ok());
        }
        for action in ["start", "enable", "register", ""] {
            assert!(request(&[action.into(), name.into()]).is_err());
        }
        for invalid in [
            "controller",
            "\\constellation-driver-test-0123456789abcdef0123456789abcdef",
            "constellation-driver-test-0123456789abcdef0123456789abcde",
            "constellation-driver-test-0123456789abcdef0123456789abcdef0",
            "constellation-driver-test-0123456789ABCDEF0123456789ABCDEF",
            "constellation-driver-test-0123456789abcdef0123456789abcdeg",
        ] {
            assert!(request(&["delete".into(), invalid.into()]).is_err());
        }
        assert!(request(&[]).is_err());
        assert!(request(&["delete".into(), name.into(), "extra".into()]).is_err());
    }
}
