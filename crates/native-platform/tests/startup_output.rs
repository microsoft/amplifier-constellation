//! Keep OS-manager diagnostics out of a native helper's JSON response.
//! Ordinary Linux runs use an inert manager in a private HOME. Actual OS
//! registration requires the same explicit disposable-CI opt-in as startup smoke.
#![cfg_attr(windows, windows_subsystem = "windows")]
use amplifier_native_platform::{self as platform, MACHINE_HELPER};
use anyhow::{Context, Result, ensure};
use std::{env, fs, path::Path, process::Command};

const ACTION: &str = "CONSTELLATION_STARTUP_OUTPUT_ACTION";
const FIXTURE_HOME: &str = "CONSTELLATION_STARTUP_OUTPUT_HOME";
const ID: &str = "CONSTELLATION_STARTUP_OUTPUT_ID";

fn child(action: &str) -> Result<()> {
    let home = env::var_os(FIXTURE_HOME).context("Missing fixture home")?;
    let id = uuid::Uuid::parse_str(&env::var(ID)?)?;
    let enabled = match action {
        "enable" => true,
        "disable" => false,
        _ => anyhow::bail!("Unknown fixture action"),
    };
    if enabled {
        platform::install_startup(&MACHINE_HELPER, Path::new(&home), &id)?;
    } else {
        platform::remove_startup(&MACHINE_HELPER, Path::new(&home), &id)?;
    }
    println!("{{\"configured\":{enabled}}}");
    Ok(())
}

fn exercise(real_manager: bool) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let home = temporary.path().join("JSON helper café 子");
    fs::create_dir(&home)?;
    let home = home.canonicalize()?;
    let id = uuid::Uuid::new_v4().to_string();
    let tools = temporary.path().join("inert-tools");
    let user = temporary.path().join("private-user");
    if !real_manager {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::create_dir(&tools)?;
            fs::create_dir(&user)?;
            let manager = tools.join("systemctl");
            fs::write(
                &manager,
                "#!/bin/sh\nprintf 'startup manager stdout noise\\n'\nprintf 'startup manager diagnostic\\n' >&2\nexit \"${CONSTELLATION_FIXTURE_MANAGER_EXIT:-0}\"\n",
            )?;
            fs::set_permissions(manager, fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("Inert manager fixture is Linux-only");
    }
    let invoke = |action: &str, failure: bool| {
        let mut command = Command::new(env::current_exe()?);
        command
            .env(ACTION, action)
            .env(FIXTURE_HOME, &home)
            .env(ID, &id);
        if !real_manager {
            command.env("HOME", &user).env("PATH", &tools);
            command.env(
                "CONSTELLATION_FIXTURE_MANAGER_EXIT",
                if failure { "7" } else { "0" },
            );
        }
        command.output().map_err(anyhow::Error::from)
    };
    let result = (|| -> Result<()> {
        for action in ["enable", "enable", "disable"] {
            let output = invoke(action, false)?;
            ensure!(output.status.success(), "Helper failed: {:?}", output);
            let expected = format!("{{\"configured\":{}}}\n", action == "enable");
            ensure!(
                output.stdout == expected.as_bytes(),
                "Helper stdout is not the sole JSON response: {:?}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        if !real_manager {
            let failed = invoke("enable", true)?;
            ensure!(!failed.status.success(), "Manager failure became success");
            ensure!(
                failed.stdout.is_empty(),
                "Failure emitted a success response"
            );
            ensure!(
                String::from_utf8_lossy(&failed.stderr).contains("startup manager diagnostic"),
                "Manager diagnostics must remain available on stderr"
            );
        }
        Ok(())
    })();
    // Always remove this unique fixture registration, including fail-before
    // output assertions. The helper captures output rather than polluting this
    // parent's stream. No registered executable is started by the fixture.
    let cleanup = invoke("disable", false)?;
    ensure!(
        cleanup.status.success(),
        "Fixture removal failed: {:?}",
        cleanup
    );
    result?;
    ensure!(cleanup.stdout == b"{\"configured\":false}\n");
    Ok(())
}

fn main() -> Result<()> {
    if let Ok(action) = env::var(ACTION) {
        return child(&action);
    }
    let real_manager = env::var("CONSTELLATION_STARTUP_SMOKE").as_deref() == Ok("1");
    if !real_manager && !cfg!(target_os = "linux") {
        println!("Actual startup JSON check requires disposable-CI opt-in on this OS");
        return Ok(());
    }
    exercise(real_manager)
}
