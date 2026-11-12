//! Read-only process-generation observations. A PID alone is never an identity.
//!
//! These observations are not handles authorizing later termination. A command
//! supervisor must retain its own child/process handle when controlling a child.
//! Access and observation failures stay errors, distinct from confirmed absence.

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

/// Observe the native generation of a process. Zero is never a process target.
pub fn identity(pid: u32) -> Result<Option<Value>> {
    ensure!(pid > 0, "Invalid process ID");
    #[cfg(unix)]
    ensure!(pid <= i32::MAX as u32, "Invalid process ID");
    platform_identity(pid)
}

/// Compare a complete, well-formed saved observation with the current process.
/// A missing or different process is false; malformed evidence is an error.
pub fn matches(expected: &Value) -> Result<bool> {
    let object = expected
        .as_object()
        .context("Process identity must be an object")?;
    let pid = object
        .get("pid")
        .and_then(Value::as_u64)
        .filter(|pid| *pid > 0 && *pid <= u32::MAX as u64)
        .context("Invalid process identity PID")? as u32;
    validate_identity(expected)?;
    Ok(identity(pid)?.as_ref() == Some(expected))
}

/// Observe a direct parent only while both generations still match. A recycled
/// numeric parent PID cannot become evidence of service/application ownership.
pub fn parent_identity(child: &Value) -> Result<Option<Value>> {
    ensure!(
        matches(child)?,
        "Child generation changed before parent observation"
    );
    let pid = child["pid"].as_u64().context("Missing child PID")? as u32;
    let parent_pid = platform_parent(pid)?;
    if parent_pid == 0 || parent_pid == pid {
        return Ok(None);
    }
    let parent = identity(parent_pid)?;
    ensure!(
        matches(child)? && platform_parent(pid)? == parent_pid,
        "Child or parent changed during observation"
    );
    if let Some(parent) = &parent {
        ensure!(
            birth_order(parent)? <= birth_order(child)?,
            "Parent PID has been reused"
        );
        ensure!(
            matches(parent)?,
            "Parent generation changed during observation"
        );
    }
    Ok(parent)
}

fn birth_order(identity: &Value) -> Result<u128> {
    #[cfg(target_os = "linux")]
    return Ok(identity["start_ticks"]
        .as_str()
        .context("Missing start ticks")?
        .parse()?);
    #[cfg(target_os = "macos")]
    return Ok(identity["start_seconds"]
        .as_str()
        .context("Missing start seconds")?
        .parse::<u128>()?
        * 1_000_000
        + identity["start_microseconds"]
            .as_str()
            .context("Missing start microseconds")?
            .parse::<u128>()?);
    #[cfg(windows)]
    return Ok(identity["creation_time_100ns"]
        .as_str()
        .context("Missing creation time")?
        .parse()?);
}

#[cfg(target_os = "linux")]
fn platform_parent(pid: u32) -> Result<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields = stat
        .rsplit_once(')')
        .context("Malformed native process record")?
        .1;
    Ok(fields
        .split_whitespace()
        .nth(1)
        .context("Missing parent PID")?
        .parse()?)
}

#[cfg(target_os = "macos")]
fn platform_parent(pid: u32) -> Result<u32> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>();
    let read = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size as i32,
        )
    };
    ensure!(
        read == size as i32 && info.pbi_pid == pid,
        "Cannot establish native parent"
    );
    Ok(info.pbi_ppid)
}

#[cfg(windows)]
fn platform_parent(pid: u32) -> Result<u32> {
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
            "Cannot inspect native process parent"
        );
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut parent = None;
        let mut next = Process32FirstW(handle, &mut entry);
        while next != 0 {
            if entry.th32ProcessID == pid {
                parent = Some(entry.th32ParentProcessID);
                break;
            }
            next = Process32NextW(handle, &mut entry);
        }
        CloseHandle(handle);
        parent.context("Native process disappeared before parent observation")
    }
}

fn decimal(value: &Value, key: &str) -> Result<()> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Missing {key}"))?;
    ensure!(
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) && text.parse::<u64>().is_ok(),
        "Invalid {key}"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_identity(value: &Value) -> Result<()> {
    ensure!(
        value.as_object().is_some_and(|o| o.len() == 3),
        "Invalid Linux process identity shape"
    );
    decimal(value, "start_ticks")?;
    let boot = value
        .get("boot_id")
        .and_then(Value::as_str)
        .context("Missing boot_id")?;
    ensure!(uuid::Uuid::parse_str(boot).is_ok(), "Invalid boot_id");
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_identity(pid: u32) -> Result<Option<Value>> {
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .context("Read native boot identity")?;
    let boot = boot.trim();
    ensure!(
        uuid::Uuid::parse_str(boot).is_ok(),
        "Invalid native boot identity"
    );
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("Read native process generation"),
    };
    // A process name can contain spaces and closing parentheses. Field 22 starts
    // at index 19 after the LAST ')', matching the existing Python receipt shape.
    let (prefix, fields) = stat
        .rsplit_once(')')
        .context("Malformed native process record")?;
    let recorded_pid = prefix
        .split_once(' ')
        .context("Missing native process ID")?
        .0;
    ensure!(
        recorded_pid.parse::<u32>()? == pid,
        "Native process identity changed"
    );
    let state = fields
        .split_whitespace()
        .next()
        .context("Missing native process state")?;
    if matches!(state, "Z" | "X" | "x") {
        return Ok(None);
    }
    let ticks = fields
        .split_whitespace()
        .nth(19)
        .context("Missing native start ticks")?;
    let result = json!({"pid": pid, "start_ticks": ticks, "boot_id": boot});
    validate_identity(&result)?;
    Ok(Some(result))
}

#[cfg(target_os = "macos")]
fn validate_identity(value: &Value) -> Result<()> {
    ensure!(
        value.as_object().is_some_and(|o| o.len() == 6) && value["platform"] == "macos",
        "Invalid macOS process identity shape"
    );
    for key in [
        "start_seconds",
        "start_microseconds",
        "boot_seconds",
        "boot_microseconds",
    ] {
        decimal(value, key)?;
    }
    for key in ["start_microseconds", "boot_microseconds"] {
        ensure!(
            value[key].as_str().unwrap().parse::<u64>()? < 1_000_000,
            "Invalid {key}"
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_identity(pid: u32) -> Result<Option<Value>> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>();
    // SAFETY: errno is thread-local, and a native-sized writable buffer is
    // supplied for the requested type. Clear stale errno before observing it.
    let read = unsafe {
        *libc::__error() = 0;
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size as i32,
        )
    };
    if read != size as i32 {
        let error = std::io::Error::last_os_error();
        // proc_pidinfo can report an exited, unreaped process as zero bytes
        // with ESRCH while kill(pid, 0) still sees its process-table entry.
        // Only that explicit native absence result is absence; permission,
        // interrupted, unset-errno and malformed/partial reads remain errors.
        if read <= 0 && error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(None);
        }
        anyhow::bail!("Read native process generation: {read}/{size} bytes: {error}");
    }
    ensure!(info.pbi_pid == pid, "Native process identity changed");
    if info.pbi_status == libc::SZOMB {
        return Ok(None);
    }
    let mut boot: libc::timeval = unsafe { std::mem::zeroed() };
    let mut boot_size = std::mem::size_of::<libc::timeval>();
    // SAFETY: kern.boottime returns a timeval into this exact-sized buffer.
    if unsafe {
        libc::sysctlbyname(
            c"kern.boottime".as_ptr(),
            (&mut boot as *mut libc::timeval).cast(),
            &mut boot_size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error()).context("Read native boot identity");
    }
    ensure!(
        boot_size == std::mem::size_of::<libc::timeval>(),
        "Unexpected native boot identity size"
    );
    let result = json!({
        "platform": "macos", "pid": pid,
        "start_seconds": info.pbi_start_tvsec.to_string(),
        "start_microseconds": info.pbi_start_tvusec.to_string(),
        "boot_seconds": boot.tv_sec.to_string(), "boot_microseconds": boot.tv_usec.to_string()
    });
    validate_identity(&result)?;
    Ok(Some(result))
}

#[cfg(windows)]
fn validate_identity(value: &Value) -> Result<()> {
    ensure!(
        value.as_object().is_some_and(|o| o.len() == 4) && value["platform"] == "windows",
        "Invalid Windows process identity shape"
    );
    decimal(value, "creation_time_100ns")?;
    ensure!(
        value["session_id"]
            .as_u64()
            .is_some_and(|v| v <= u32::MAX as u64),
        "Invalid session_id"
    );
    Ok(())
}

#[cfg(windows)]
fn platform_identity(pid: u32) -> Result<Option<Value>> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, FILETIME, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SYNCHRONIZE, WaitForSingleObject,
            },
        },
    };
    // SAFETY: this requests only read-only metadata; the handle is not inherited.
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            return Ok(None);
        }
        return Err(error).context("Open process for native identity");
    }
    // SAFETY: OpenProcess returned a valid handle, owned exactly once here.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    // Waiting with zero timeout is read-only and distinguishes an exited process
    // even if its numeric exit code happens to equal STILL_ACTIVE (259).
    match unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } {
        WAIT_OBJECT_0 => return Ok(None),
        WAIT_TIMEOUT => (),
        _ => return Err(std::io::Error::last_os_error()).context("Observe process exit state"),
    }
    let mut creation: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit: FILETIME = unsafe { std::mem::zeroed() };
    let mut kernel: FILETIME = unsafe { std::mem::zeroed() };
    let mut user: FILETIME = unsafe { std::mem::zeroed() };
    // SAFETY: all output buffers and the owned native handle are valid.
    if unsafe {
        GetProcessTimes(
            handle.as_raw_handle(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("Read native process generation");
    }
    if exit.dwHighDateTime != 0 || exit.dwLowDateTime != 0 {
        return Ok(None);
    }
    let mut session = 0;
    if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
        return Err(std::io::Error::last_os_error()).context("Read process session identity");
    }
    let creation = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    let result = json!({"platform": "windows", "pid": pid,
        "creation_time_100ns": creation.to_string(), "session_id": session});
    validate_identity(&result)?;
    Ok(Some(result))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn validate_identity(_: &Value) -> Result<()> {
    anyhow::bail!("Unsupported platform")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn platform_identity(_: u32) -> Result<Option<Value>> {
    anyhow::bail!("Unsupported platform")
}
