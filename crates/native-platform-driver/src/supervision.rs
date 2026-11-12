use crate::config::Enrollment;
use amplifier_native_platform::process;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
#[cfg(unix)]
use serde_json::json;
#[cfg(unix)]
use std::io::BufRead;
use std::{
    io::{self, Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn reap_bounded(child: &mut Child) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "Owned process termination is unconfirmed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn invocation(
    enrollment: &Enrollment,
    mode: &str,
    name: &str,
    result: Option<&str>,
) -> Result<Command> {
    let mut c = Command::new(std::env::current_exe()?);
    c.arg("--config")
        .arg(&enrollment.path)
        .arg(mode)
        .arg("--command")
        .arg(name);
    if let Some(result) = result {
        c.arg("--result").arg(result);
    }
    Ok(c)
}

fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
    }
}

/// Launching does not interpret a domain terminal. Missing acknowledgment is unknown.
pub fn launch(enrollment: &Enrollment, name: &str, result: Option<&str>) -> Result<Value> {
    let mut c = invocation(enrollment, "internal-detached", name, result)?;
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut c);
    let mut child = c.spawn()?;
    let identity = process::identity(child.id())?
        .context("Launched supervisor exited before its generation was observed")?;
    // Reap while this short invocation is still alive; exit must not wait for the job.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(identity)
}

/// Own the fixed child, independently of the JSONL invocation and its locks.
pub fn run(
    enrollment: &Enrollment,
    name: &str,
    result: Option<&str>,
    attached: bool,
) -> Result<i32> {
    let (profile, _) = enrollment.command(name, result)?;
    let parent = if attached {
        Some(parent_identity()?)
    } else {
        None
    };
    #[cfg(windows)]
    install_job()?;
    let mut command = invocation(enrollment, "internal-execute", name, result)?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let mut input = child.stdin.take().context("Owned child has no input")?;
    // The trampoline cannot run the endpoint until its independent watcher is ready.
    #[cfg(unix)]
    let child_identity =
        process::identity(child.id())?.context("Owned child exited during setup")?;
    #[cfg(unix)]
    let mut watcher = match spawn_watch(enrollment, &child_identity, profile.timeout_seconds + 5) {
        Ok(w) => w,
        Err(e) => {
            let _ = child.kill();
            let _ = reap_bounded(&mut child);
            return Err(e);
        }
    };
    input.write_all(b"RUN\n")?;
    input.flush()?;
    let (sender, receiver) = std::sync::mpsc::channel();
    if attached {
        std::thread::spawn(move || {
            let outcome = io::copy(&mut io::stdin().lock(), &mut input);
            let _ = sender.send(outcome.map(|_| ()));
        });
    } else {
        drop(input);
    }
    let deadline = Instant::now() + Duration::from_secs(profile.timeout_seconds);
    let outcome = loop {
        if let Some(status) = child.try_wait()? {
            break Ok(status.code().unwrap_or(1));
        }
        if Instant::now() >= deadline {
            break Err(anyhow::anyhow!(
                "Owned command reached its absolute time bound"
            ));
        }
        if let Some(parent) = &parent {
            match process::matches(parent) {
                Ok(true) => (),
                Ok(false) => break Err(anyhow::anyhow!("Owning caller ended")),
                Err(e) => break Err(e.context("Cannot establish owning caller identity")),
            }
            if receiver.try_recv().is_ok() {
                break Err(anyhow::anyhow!("Owning caller closed input"));
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    #[cfg(unix)]
    {
        // Closing the control pipe asks the independent watcher to reap the entire
        // owned group, including children remaining after the command exited.
        drop(watcher.stdin.take());
        let until = Instant::now() + Duration::from_secs(3);
        while watcher.try_wait()?.is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        let status = watcher
            .try_wait()?
            .context("Owned process cleanup is unconfirmed")?;
        ensure!(
            status.success(),
            "Owned process watcher could not confirm cleanup"
        );
    }
    #[cfg(windows)]
    {
        // The process job is retained until this short supervisor exits. Windows
        // closes its non-inherited handle then kills every remaining descendant.
        let _ = child.kill();
    }
    reap_bounded(&mut child)?;
    outcome
}

/// The only dynamic input is a private start barrier. The command still comes
/// exclusively from the protected owner configuration.
pub fn execute(enrollment: &Enrollment, name: &str, result: Option<&str>) -> Result<i32> {
    let mut gate = [0u8; 4];
    // Rust Stdin is buffered: reading the barrier through it could prefetch ACP
    // bytes and then discard them at exec. Read exactly four raw descriptor bytes.
    #[cfg(unix)]
    {
        use std::os::fd::FromRawFd;
        let mut input = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(0) });
        input.read_exact(&mut gate)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};
        let mut input = std::mem::ManuallyDrop::new(unsafe {
            std::fs::File::from_raw_handle(GetStdHandle(STD_INPUT_HANDLE))
        });
        input.read_exact(&mut gate)?;
    }
    ensure!(&gate == b"RUN\n", "Owned command was not released");
    enrollment.unchanged()?;
    let (profile, args) = enrollment.command(name, result)?;
    let mut command = Command::new(profile.executable);
    command
        .args(args)
        .current_dir(profile.cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().into())
    }
    #[cfg(windows)]
    {
        // This trampoline and command inherit the supervisor's kill-on-close job.
        Ok(command.status()?.code().unwrap_or(1))
    }
}

#[cfg(unix)]
fn spawn_watch(enrollment: &Enrollment, child: &Value, seconds: u64) -> Result<Child> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--config")
        .arg(&enrollment.path)
        .arg("internal-watch");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    detach(&mut command);
    let mut watcher = command.spawn()?;
    let parent = process::identity(std::process::id())?.context("No supervisor identity")?;
    writeln!(
        watcher.stdin.as_mut().context("Missing watcher channel")?,
        "{}",
        json!({"parent":parent,"child":child,"seconds":seconds})
    )?;
    let output = watcher.stdout.take().context("Missing watcher reply")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = io::BufReader::new(output)
            .read_line(&mut line)
            .map(|_| line);
        let _ = tx.send(result);
    });
    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(line)) if line == "READY\n" => Ok(watcher),
        _ => {
            let _ = watcher.kill();
            let _ = reap_bounded(&mut watcher);
            anyhow::bail!("Owned process watcher did not acknowledge");
        }
    }
}

#[cfg(unix)]
pub fn watch() -> Result<i32> {
    let input = io::stdin();
    let mut input = input.lock();
    let mut line = String::new();
    io::Read::take(&mut input, 8192).read_line(&mut line)?;
    let value: Value = serde_json::from_str(&line)?;
    let parent = value
        .get("parent")
        .context("Missing supervisor identity")?
        .clone();
    let child = value
        .get("child")
        .context("Missing owned generation")?
        .clone();
    let seconds = value
        .get("seconds")
        .and_then(Value::as_u64)
        .filter(|s| (1..=305).contains(s))
        .context("Invalid watcher deadline")?;
    ensure!(
        process::matches(&parent)? && process::matches(&child)?,
        "Process ownership changed before start"
    );
    let pid = child
        .get("pid")
        .and_then(Value::as_u64)
        .filter(|p| *p > 1 && *p <= i32::MAX as u64)
        .context("Invalid child identity")? as i32;
    ensure!(
        unsafe { libc::getpgid(pid) } == pid,
        "Owned child is not its process group leader"
    );
    println!("READY");
    io::stdout().flush()?;
    // The sole writer is the supervisor. No probe/endpoint inherits this handle.
    let (tx, rx) = std::sync::mpsc::channel();
    drop(input);
    std::thread::spawn(move || {
        let mut byte = [0];
        let _ = io::stdin().read(&mut byte);
        let _ = tx.send(());
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut observation_error = None;
    loop {
        if rx.try_recv().is_ok() || Instant::now() >= deadline {
            break;
        }
        match process::matches(&parent) {
            Ok(true) => (),
            Ok(false) => break,
            Err(error) => {
                observation_error = Some(error);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    // A reused leader PID is never authority to signal a different process group.
    if let Some(current) = process::identity(pid as u32)? {
        ensure!(
            current == child,
            "Owned process generation changed; cleanup is unknown"
        );
    }
    let killed = unsafe { libc::kill(-pid, libc::SIGKILL) };
    if killed != 0 {
        let e = io::Error::last_os_error();
        ensure!(
            e.raw_os_error() == Some(libc::ESRCH),
            "Owned group cleanup failed: {e}"
        );
    }
    if let Some(error) = observation_error {
        return Err(error.context("Owned tree cleaned after supervisor observation failed"));
    }
    Ok(0)
}

fn parent_identity() -> Result<Value> {
    #[cfg(unix)]
    let pid = unsafe { libc::getppid() } as u32;
    #[cfg(windows)]
    let pid = windows_parent()?;
    process::identity(pid)?.context("Launching process generation is unavailable")
}

#[cfg(windows)]
fn windows_parent() -> Result<u32> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
    };
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        ensure!(
            handle != INVALID_HANDLE_VALUE,
            "Cannot inspect process owner"
        );
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = None;
        let mut ok = Process32FirstW(handle, &mut entry);
        while ok != 0 {
            if entry.th32ProcessID == std::process::id() {
                found = Some(entry.th32ParentProcessID);
                break;
            }
            ok = Process32NextW(handle, &mut entry);
        }
        CloseHandle(handle);
        found.context("Cannot identify process owner")
    }
}

#[cfg(windows)]
fn install_job() -> Result<()> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::GetCurrentProcess,
        },
    };
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        ensure!(!job.is_null(), "Cannot create owned process job");
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
            || AssignProcessToJobObject(job, GetCurrentProcess()) == 0
        {
            let e = io::Error::last_os_error();
            CloseHandle(job);
            return Err(e.into());
        }
        // Deliberately process-lifetime: closing this handle would kill this
        // supervisor too. OS teardown closes it on success, failure or crash.
        Ok(())
    }
}
