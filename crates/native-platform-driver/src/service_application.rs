//! Fixed service launcher and observed application identity. This is separate
//! from bounded verification commands: the native manager owns its lifetime.
use crate::{
    config::{self, Enrollment},
    supervision,
};
use amplifier_native_platform::{locking::InstallationLock, process};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::time::{Duration, Instant};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub identity_path: PathBuf,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.executable.is_absolute() && self.executable.is_file(),
            "Application executable must be an installed absolute file"
        );
        ensure!(
            self.cwd.is_absolute() && self.cwd.is_dir(),
            "Application working directory must exist"
        );
        ensure!(
            self.args.iter().all(|s| !s.contains('\0')),
            "Invalid application argument"
        );
        config::path_chain(&self.identity_path)?;
        config::protected_directory(
            self.identity_path
                .parent()
                .context("Identity receipt parent missing")?,
        )?;
        if self.identity_path.exists() {
            config::protected_file(&self.identity_path)?;
        }
        Ok(())
    }
}
fn profile<'a>(enrollment: &'a Enrollment, name: &str) -> Result<&'a Profile> {
    enrollment
        .config
        .service_applications
        .get(name)
        .context("Unenrolled service application")
}
fn fingerprint(enrollment: &Enrollment, name: &str) -> Result<String> {
    let bytes = serde_json::to_vec(&(
        profile(enrollment, name)?,
        enrollment.config.services.get(name),
        enrollment.config.registrations.get(name),
    ))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn receipt(
    enrollment: &Enrollment,
    name: &str,
    supervisor: Value,
    application: Value,
) -> Result<()> {
    let value = json!({"schema":1,"service":name,"profile_sha256":fingerprint(enrollment,name)?,"supervisor":supervisor,"application":application});
    amplifier_native_platform::private_write(
        &profile(enrollment, name)?.identity_path,
        &serde_json::to_vec(&value)?,
    )
}

/// No service start is accepted here. Only a service manager's fixed registered
/// command should invoke this entrypoint; snapshots independently prove the link.
pub fn run(enrollment: &Enrollment, name: &str) -> Result<i32> {
    let application = profile(enrollment, name)?;
    let lock_path = application.identity_path.with_extension("invocation.lock");
    config::path_chain(&lock_path)?;
    if lock_path.exists() {
        config::protected_file(&lock_path)?;
    }
    let _lock = InstallationLock::try_acquire(&lock_path)?
        .context("Application supervisor already owns this service")?;
    // Never overwrite a still-live prior application after uncertain teardown.
    if application.identity_path.exists() {
        let prior: Value = serde_json::from_slice(&fs::read(&application.identity_path)?)?;
        ensure!(
            prior["schema"] == 1 && prior["service"] == name,
            "Unrecognized prior application receipt"
        );
        for key in ["supervisor", "application"] {
            ensure!(
                !process::matches(&prior[key])?,
                "Prior service process remains live"
            );
        }
    }
    #[cfg(windows)]
    supervision::install_job()?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--config")
        .arg(&enrollment.path)
        .arg("internal-service-execute")
        .arg("--service")
        .arg(name)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(unix)]
    let (ack_read, ack_write) = exec_ack(&mut command)?;
    let mut child = command.spawn()?;
    #[cfg(unix)]
    drop(ack_write);
    let mut input = child.stdin.take().context("Application gate missing")?;
    #[cfg(unix)]
    let mut watcher = match supervision::spawn_watch(
        enrollment,
        &process::identity(child.id())?.context("Application launcher ended")?,
        None,
    ) {
        Ok(watcher) => watcher,
        Err(error) => {
            let _ = child.kill();
            let _ = supervision::reap_bounded(&mut child);
            return Err(error);
        }
    };
    input.write_all(b"RUN\n")?;
    input.flush()?;
    drop(input);
    #[cfg(unix)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = ack_read.take(64).read_to_end(&mut bytes).map(|_| bytes);
            let _ = tx.send(result);
        });
        let acknowledged = rx.recv_timeout(Duration::from_secs(10))?;
        ensure!(
            acknowledged? == b"EXEC\n",
            "Application executable launch was not confirmed"
        );
        receipt(
            enrollment,
            name,
            process::identity(std::process::id())?.context("Supervisor generation missing")?,
            process::identity(child.id())?.context("Application exited before launch proof")?,
        )?;
    }
    // Only this long-running, manager-owned process waits. JSONL commands never do.
    let status = child.wait()?;
    #[cfg(unix)]
    {
        drop(watcher.stdin.take());
        let until = Instant::now() + Duration::from_secs(3);
        while watcher.try_wait()?.is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        ensure!(
            watcher.try_wait()?.is_some_and(|s| s.success()),
            "Application descendants remain unconfirmed"
        );
    }
    Ok(status.code().unwrap_or(1))
}

pub fn execute(enrollment: &Enrollment, name: &str) -> Result<i32> {
    let mut barrier = [0; 4];
    #[cfg(unix)]
    {
        use std::os::fd::FromRawFd;
        let mut input = std::mem::ManuallyDrop::new(unsafe { fs::File::from_raw_fd(0) });
        input.read_exact(&mut barrier)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};
        let mut input = std::mem::ManuallyDrop::new(unsafe {
            fs::File::from_raw_handle(GetStdHandle(STD_INPUT_HANDLE))
        });
        input.read_exact(&mut barrier)?;
    }
    ensure!(&barrier == b"RUN\n", "Service application was not released");
    enrollment.unchanged()?;
    let application = profile(enrollment, name)?;
    #[cfg(windows)]
    let mut command = Command::new(&application.executable);
    #[cfg(windows)]
    command
        .args(&application.args)
        .current_dir(&application.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        // Only a successful exec closes this descriptor with exactly EXEC\n.
        // A failed exec appends failure evidence; the supervisor never certifies
        // a still-running trampoline merely because exec would keep its PID.
        let mut ack = unsafe { fs::File::from_raw_fd(3) };
        ensure!(
            unsafe { libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
            "Missing native exec acknowledgment"
        );
        let null = fs::File::open("/dev/null")?;
        ensure!(
            unsafe { libc::dup2(null.as_raw_fd(), 0) } == 0,
            "Cannot close application input"
        );
        ack.write_all(b"EXEC\n")?;
        let error =
            supervision::exec_exact(&application.executable, &application.args, &application.cwd);
        let _ = ack.write_all(b"FAILED\n");
        Err(error.into())
    }
    #[cfg(windows)]
    {
        // The actual application is a child of this fixed trampoline, in the
        // supervisor's non-inherited kill-on-close job. Record THAT generation.
        let mut child = command.spawn()?;
        if let Err(error) = (|| -> Result<()> {
            receipt(
                enrollment,
                name,
                process::parent_identity(
                    &process::identity(std::process::id())?.context("Launcher identity missing")?,
                )?
                .context("Supervisor identity missing")?,
                process::identity(child.id())?.context("Application already exited")?,
            )
        })() {
            let _ = child.kill();
            let _ = supervision::reap_bounded(&mut child);
            return Err(error);
        }
        Ok(child.wait()?.code().unwrap_or(1))
    }
}

fn chain_to(child: &Value, ancestor: &Value) -> Result<Vec<Value>> {
    let mut chain = vec![child.clone()];
    for _ in 0..32 {
        let current = chain.last().unwrap();
        ensure!(
            process::matches(current)?,
            "Process changed during service binding"
        );
        if current == ancestor {
            return Ok(chain);
        }
        let parent = process::parent_identity(current)?.context("No matching service ancestor")?;
        ensure!(!chain.contains(&parent), "Cyclic process ancestry");
        chain.push(parent);
    }
    anyhow::bail!("Service ancestry exceeds bounded observation")
}

/// Preserve native roles. The application field is an additional proved link,
/// never a renamed TaskScheduler engine/MainPID or a best-effort live PID.
pub fn augment(enrollment: &Enrollment, name: &str, snapshot: &mut Value) -> Result<()> {
    let Some(application) = enrollment.config.service_applications.get(name) else {
        return Ok(());
    };
    let proof = (|| -> Result<Value> {
        if snapshot["active"] == false {
            ensure!(
                snapshot["transitioning"] == false
                    && snapshot["processes"]
                        .as_array()
                        .is_some_and(|p| p.is_empty()),
                "Native service still has process/transition evidence"
            );
            if application.identity_path.exists() {
                config::protected_file(&application.identity_path)?;
                let retained: Value =
                    serde_json::from_slice(&fs::read(&application.identity_path)?)?;
                ensure!(
                    retained["schema"] == 1 && retained["service"] == name,
                    "Unrecognized retained service receipt"
                );
                for key in ["supervisor", "application"] {
                    ensure!(
                        !process::matches(&retained[key])?,
                        "Native service is inactive but an owned process remains live"
                    );
                }
            }
            return Ok(json!({"status":"absent"}));
        }
        ensure!(
            snapshot["active"] == true && snapshot["transitioning"] == false,
            "Native service activity is unconfirmed"
        );
        let registration = enrollment
            .config
            .registrations
            .get(name)
            .context("Service application needs its fixed registered wrapper profile")?;
        ensure!(
            fs::canonicalize(&registration.executable)?
                == fs::canonicalize(std::env::current_exe()?)?,
            "Service registration does not select this installed native wrapper"
        );
        let args = vec![
            "--config".to_owned(),
            enrollment
                .path
                .to_str()
                .context("Configuration path is not text")?
                .to_owned(),
            "service-run".to_owned(),
            "--service".to_owned(),
            name.to_owned(),
        ];
        ensure!(
            registration.args == args,
            "Service registration does not select this application enrollment"
        );
        ensure!(
            amplifier_native_platform::registration::profile_matches(
                enrollment
                    .config
                    .services
                    .get(name)
                    .context("Unenrolled service")?,
                registration,
                snapshot
            )?,
            "Native registered action changed from the enrolled wrapper"
        );
        #[cfg(windows)]
        {
            let instances = snapshot["native"]["instances"]
                .as_array()
                .context("Missing task instances")?;
            ensure!(
                instances.len() == 1,
                "Task application invocation is ambiguous"
            );
            let instance = &instances[0];
            ensure!(
                instance["instance_guid"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()),
                "Missing task invocation identity"
            );
            let shared = instance["engine_instance_guids"]
                .as_array()
                .context("Missing task engine ownership evidence")?;
            ensure!(
                shared.len() == 1 && shared[0] == instance["instance_guid"],
                "Task engine belongs to multiple or unconfirmed invocations"
            );
        }
        config::protected_file(&application.identity_path)?;
        let bytes = fs::read(&application.identity_path)?;
        ensure!(bytes.len() <= 16384, "Application receipt exceeds bound");
        let record: Value = serde_json::from_slice(&bytes)?;
        ensure!(
            record["schema"] == 1
                && record["service"] == name
                && record["profile_sha256"] == fingerprint(enrollment, name)?,
            "Application receipt does not match enrollment"
        );
        let child_chain = chain_to(&record["application"], &record["supervisor"])?;
        for manager in snapshot["processes"]
            .as_array()
            .context("Missing native process observations")?
        {
            if !matches!(
                manager["role"].as_str(),
                Some("MainPID" | "launchd_pid" | "scheduled_task_engine")
            ) {
                continue;
            }
            if let Ok(supervisor_chain) = chain_to(&record["supervisor"], &manager["identity"]) {
                ensure!(
                    process::matches(&record["application"])?
                        && process::matches(&manager["identity"])?,
                    "Service generation changed after binding"
                );
                ensure!(
                    fs::read(&application.identity_path)? == bytes,
                    "Application receipt changed during binding"
                );
                return Ok(
                    json!({"status":"running","identity":record["application"],"supervisor":record["supervisor"],"manager_process":manager,"application_chain":child_chain,"manager_chain":supervisor_chain,"profile_sha256":record["profile_sha256"]}),
                );
            }
        }
        anyhow::bail!("Application is not bound to the observed native service invocation")
    })();
    snapshot["application"] = match proof {
        Ok(value) => value,
        Err(error) => json!({"status":"unconfirmed","reason":error.to_string()}),
    };
    Ok(())
}

#[cfg(unix)]
fn exec_ack(command: &mut Command) -> Result<(fs::File, fs::File)> {
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    };
    let mut fds = [0; 2];
    ensure!(
        unsafe { libc::pipe(fds.as_mut_ptr()) } == 0,
        "Cannot create exec acknowledgment"
    );
    let read = unsafe { fs::File::from_raw_fd(fds[0]) };
    let write = unsafe { fs::File::from_raw_fd(fds[1]) };
    for fd in fds {
        ensure!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
            "Cannot protect exec acknowledgment"
        );
    }
    let writer = write.as_raw_fd();
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(writer, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok((read, write))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactive_manager_does_not_hide_live_application_receipt() {
        let temporary = match std::env::var_os("CONSTELLATION_NATIVE_TEST_ROOT") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        };
        let root = temporary.path().canonicalize().unwrap();
        amplifier_native_platform::private_dir(&root).unwrap();
        let config_path = root.join("owner.json");
        let receipt_path = root.join("application.json");
        let mut data = json!({"version":1,"services":{"controller":{"manager":if cfg!(windows){"scheduled_task"}else if cfg!(target_os="macos"){"launch_agent"}else{"systemd_user"},"name":"unused-fixture","definition_path":null}},
            "service_applications":{"controller":{"executable":std::env::current_exe().unwrap(),"args":[],"cwd":root,"identity_path":receipt_path}}});
        data["registrations"] = json!({"controller":{"executable":std::env::current_exe().unwrap(),"args":["--config",config_path,"service-run","--service","controller"],"cwd":root,"description":"Fixture service","autostart":false}});
        amplifier_native_platform::private_write(&config_path, &serde_json::to_vec(&data).unwrap())
            .unwrap();
        let enrollment = Enrollment::load(&config_path).unwrap();
        let live = process::identity(std::process::id()).unwrap().unwrap();
        amplifier_native_platform::private_write(
            &receipt_path,
            &serde_json::to_vec(
                &json!({"schema":1,"service":"controller","supervisor":live,"application":live}),
            )
            .unwrap(),
        )
        .unwrap();
        let mut snapshot = json!({"active":false,"transitioning":false,"processes":[]});
        augment(&enrollment, "controller", &mut snapshot).unwrap();
        assert_eq!(snapshot["application"]["status"], "unconfirmed");
        assert!(process::matches(&live).unwrap());
    }
}
