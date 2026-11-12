//! Observation and compare-before-dispatch for installation-owned user services.
//!
//! The caller supplies protected enrollment, holds its installation lock, and owns
//! settlement/restoration. A successful command is acceptance, not a stopped or
//! running service. Managers do not expose atomic snapshot-CAS: an independent
//! administrator can still change state after the final comparison. No operation
//! here enables, unmasks, deletes a definition, or chooses an executable.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Six launchd reads or four systemd reads plus dispatch stay below 30 seconds.
// Windows observations/actions use the separately bounded native COM helper.
const COMMAND_LIMIT: Duration = Duration::from_secs(if cfg!(windows) { 8 } else { 3 });
const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceManager {
    SystemdUser,
    LaunchAgent,
    ScheduledTask,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub manager: ServiceManager,
    pub name: String,
    pub definition_path: Option<PathBuf>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Start,
    Stop,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionOutcome {
    Accepted,
    Refused { code: String, message: String },
    Unknown { code: String, message: String },
}
fn refused(code: &str, message: &str) -> ActionOutcome {
    ActionOutcome::Refused {
        code: code.into(),
        message: message.into(),
    }
}
fn unknown(code: &str, message: &str) -> ActionOutcome {
    ActionOutcome::Unknown {
        code: code.into(),
        message: message.into(),
    }
}
impl Service {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.is_empty() && self.name.len() <= 240,
            "Invalid enrolled service name"
        );
        ensure!(
            !self.name.starts_with('-'),
            "Service name cannot be an option"
        );
        ensure!(
            self.name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.@: \\".contains(&b)),
            "Invalid enrolled service name"
        );
        ensure!(
            !self.name.split('\\').any(|p| p == "." || p == ".."),
            "Invalid task path"
        );
        match self.manager {
            ServiceManager::SystemdUser => {
                ensure!(
                    cfg!(target_os = "linux"),
                    "systemd user services require Linux"
                );
                ensure!(
                    self.name.ends_with(".service") && !self.name.contains([' ', '\\']),
                    "Use one exact systemd service name"
                );
            }
            ServiceManager::LaunchAgent => {
                ensure!(cfg!(target_os = "macos"), "LaunchAgents require macOS");
                ensure!(
                    !self.name.contains([' ', '\\']),
                    "Use one exact LaunchAgent label"
                );
                ensure!(
                    self.definition_path.is_some(),
                    "An enrolled LaunchAgent plist is required"
                );
            }
            ServiceManager::ScheduledTask => {
                ensure!(cfg!(windows), "Scheduled Tasks require Windows")
            }
        }
        if let Some(path) = &self.definition_path {
            ensure!(
                path.is_absolute(),
                "Definition path must be absolute and installation-owned"
            );
            ensure!(
                !path.to_string_lossy().chars().any(char::is_control),
                "Invalid definition path"
            );
        }
        Ok(())
    }
}

/// Read-only failure remains an error; it is never evidence that a service stopped.
pub fn snapshot(service: &Service) -> Result<Value> {
    service.validate()?;
    // Two complete observations include process generation and effective definition.
    // An exited/reused PID, reload, or transition during observation cannot become a
    // stable snapshot merely because its numeric PID happened to match.
    let first = observe(service)?;
    let second = observe(service)?;
    ensure!(
        first == second,
        "Service changed during observation; obtain a fresh snapshot"
    );
    Ok(first)
}

pub fn request_state(
    service: &Service,
    action: ServiceAction,
    expected: &Value,
) -> Result<ActionOutcome> {
    request_state_checked(service, action, expected, |_| Ok(None))
}

/// Check additional installation-owned evidence between the two native reads.
/// The caller cannot dispatch from an unchecked snapshot or omit the second
/// observation. This keeps application proof inside the same compare-before-
/// action window without paying for a second complete pair of manager reads.
pub fn request_state_checked(
    service: &Service,
    action: ServiceAction,
    expected: &Value,
    validate: impl FnOnce(&Value) -> Result<Option<ActionOutcome>>,
) -> Result<ActionOutcome> {
    service.validate()?;
    let current = match action_snapshot(expected, || observe(service), validate)? {
        CheckedSnapshot::Confirmed(value) => value,
        CheckedSnapshot::Refused(outcome) => return Ok(outcome),
    };
    if let Some(refusal) = action_refusal(&current, action) {
        return Ok(refusal);
    }
    // Check generation once more immediately before handing control to the manager.
    for process in current["processes"]
        .as_array()
        .context("Missing process observation")?
    {
        if !crate::process::matches(&process["identity"])? {
            return Ok(refused(
                "stale_process",
                "An observed service process changed before dispatch",
            ));
        }
    }
    dispatch(service, action, &current)
}

enum CheckedSnapshot {
    Confirmed(Value),
    Refused(ActionOutcome),
}

fn action_snapshot(
    expected: &Value,
    mut observe: impl FnMut() -> Result<Value>,
    validate: impl FnOnce(&Value) -> Result<Option<ActionOutcome>>,
) -> Result<CheckedSnapshot> {
    let first = observe()?;
    if first != *expected {
        return Ok(CheckedSnapshot::Refused(refused(
            "stale_snapshot",
            "Service definition, availability, or process generation changed",
        )));
    }
    if let Some(outcome) = validate(&first)? {
        ensure!(
            matches!(outcome, ActionOutcome::Refused { .. }),
            "Service validation can only refuse before dispatch"
        );
        return Ok(CheckedSnapshot::Refused(outcome));
    }
    let second = observe()?;
    ensure!(
        first == second,
        "Service changed during observation; obtain a fresh snapshot"
    );
    Ok(CheckedSnapshot::Confirmed(second))
}

fn action_refusal(current: &Value, action: ServiceAction) -> Option<ActionOutcome> {
    if current["definition_matches"] != true {
        return Some(refused(
            "definition_mismatch",
            "The effective definition does not match the enrolled definition",
        ));
    }
    if current["transitioning"] != false {
        return Some(refused(
            "transitioning_or_unknown",
            "The service has a pending transition or unknown state",
        ));
    }
    if action == ServiceAction::Start
        && (current["start_allowed"] != true || current["masked"] != false)
    {
        return Some(refused(
            "disabled_or_unknown",
            "Starting cannot override independent disabled, masked, or unknown availability",
        ));
    }
    None
}

fn definition(path: &Path) -> Result<Value> {
    let metadata = fs::symlink_metadata(path).context("Read enrolled definition metadata")?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Definition must be a regular enrolled file"
    );
    ensure!(
        metadata.len() <= OUTPUT_LIMIT as u64,
        "Definition exceeds observation limit"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() } || metadata.uid() == 0,
            "Definition has an unexpected owner"
        );
        ensure!(
            metadata.mode() & 0o022 == 0,
            "Definition is writable by another user"
        );
    }
    let bytes = fs::read(path)?;
    ensure!(
        bytes.len() <= OUTPUT_LIMIT,
        "Definition exceeds observation limit"
    );
    Ok(json!({"path":path,"sha256":format!("{:x}", Sha256::digest(&bytes)),"bytes":bytes}))
}
fn content(text: &str) -> Value {
    json!({"sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"text":text})
}
fn process_observation(role: &str, pid: u32) -> Result<Option<Value>> {
    if pid == 0 {
        return Ok(None);
    }
    let identity =
        crate::process::identity(pid)?.context("Service process vanished during observation")?;
    Ok(Some(json!({"role":role,"identity":identity})))
}
fn observe(service: &Service) -> Result<Value> {
    match service.manager {
        ServiceManager::SystemdUser => systemd_snapshot(service),
        ServiceManager::LaunchAgent => launch_snapshot(service),
        ServiceManager::ScheduledTask => task_snapshot(service),
    }
}

#[derive(Debug)]
struct NotStarted(std::io::Error);
impl std::fmt::Display for NotStarted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fixed service-manager tool was not started: {}", self.0)
    }
}
impl std::error::Error for NotStarted {}

pub(crate) struct Captured {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}
/// Fixed short-lived OS tools only. Drain both pipes with bounded retained output;
/// a timeout after spawn is ambiguous for a mutation, never a safe retry signal.
pub(crate) fn execute(mut command: Command) -> Result<Captured> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.env("LC_ALL", "C");
    let mut child = command.spawn().map_err(NotStarted)?;
    let (tx, rx) = mpsc::channel();
    fn reader(
        mut stream: impl Read + Send + 'static,
        index: usize,
        tx: mpsc::Sender<(usize, std::io::Result<Vec<u8>>)>,
    ) {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream
                .by_ref()
                .take((OUTPUT_LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send((index, result));
        });
    }
    reader(child.stdout.take().unwrap(), 0, tx.clone());
    reader(child.stderr.take().unwrap(), 1, tx);
    let deadline = Instant::now() + COMMAND_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let cleanup_deadline = Instant::now() + Duration::from_secs(1);
            while child.try_wait()?.is_none() && Instant::now() < cleanup_deadline {
                thread::sleep(Duration::from_millis(10));
            }
            bail!("Fixed service-manager operation exceeded its bound; outcome may be unknown");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let mut streams = [Vec::new(), Vec::new()];
    for _ in 0..2 {
        let (index, bytes) = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("Service-manager output did not close")?;
        streams[index] = bytes?;
        ensure!(
            streams[index].len() <= OUTPUT_LIMIT,
            "Service-manager output exceeds limit"
        );
    }
    Ok(Captured {
        status,
        stdout: String::from_utf8(streams[0].clone())?,
        stderr: String::from_utf8(streams[1].clone())?,
    })
}
pub(crate) fn checked(command: Command) -> Result<String> {
    let result = execute(command)?;
    ensure!(
        result.status.success(),
        "Service inspection failed ({:?}): {}",
        result.status.code(),
        result.stderr.trim()
    );
    Ok(result.stdout)
}
fn mutation(command: Command) -> ActionOutcome {
    match execute(command) {
        Ok(result) if result.status.success() => ActionOutcome::Accepted,
        Ok(result) => unknown(
            "manager_error",
            &format!(
                "Manager returned {:?}: {}; inspect before deciding whether to retry",
                result.status.code(),
                result.stderr.trim()
            ),
        ),
        Err(error) if error.downcast_ref::<NotStarted>().is_some() => {
            refused("manager_unavailable", &format!("{error:#}"))
        }
        Err(error) => unknown(
            "transport_unknown",
            &format!("{error:#}; inspect before deciding whether to retry"),
        ),
    }
}

const SYSTEMD_PROPERTIES: &str = "Id,Names,LoadState,ActiveState,SubState,UnitFileState,FragmentPath,SourcePath,DropInPaths,MainPID,ControlPID,InvocationID,NeedDaemonReload,RefuseManualStart,RefuseManualStop,ExecStart,ExecStop,Restart,KillMode,Type";
pub(crate) fn systemctl(args: &[&str]) -> Command {
    let mut command = Command::new("/usr/bin/systemctl");
    command
        .args(["--user", "--no-pager", "--no-ask-password"])
        .args(args);
    command
}
fn properties(text: &str) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').context("Malformed service property")?;
        ensure!(
            !key.is_empty() && map.insert(key.into(), value.into()).is_none(),
            "Duplicate service property"
        );
    }
    Ok(map)
}
fn systemd_snapshot(service: &Service) -> Result<Value> {
    let native = properties(&checked(systemctl(&[
        "show",
        "--all",
        "--property",
        SYSTEMD_PROPERTIES,
        "--",
        &service.name,
    ]))?)?;
    let get = |key: &str| {
        native
            .get(key)
            .map(String::as_str)
            .with_context(|| format!("Missing systemd property {key}"))
    };
    ensure!(
        get("Id")? == service.name,
        "Service alias does not identify the enrolled unit"
    );
    let mut processes = Vec::new();
    for key in ["MainPID", "ControlPID"] {
        if let Some(p) = process_observation(key, get(key)?.parse()?)? {
            processes.push(p);
        }
    }
    let _ = get("DropInPaths")?;
    let state = get("UnitFileState")?;
    let masked = matches!(state, "masked" | "masked-runtime");
    let enabled = match state {
        "enabled" | "enabled-runtime" => Some(true),
        "disabled" | "masked" | "masked-runtime" => Some(false),
        _ => None,
    };
    let configured = service
        .definition_path
        .as_ref()
        .map(|path| {
            if masked && path.is_symlink() {
                let target = fs::read_link(path)?;
                ensure!(
                    target == Path::new("/dev/null"),
                    "Masked enrolled unit has an unexpected link target"
                );
                Ok(json!({"path":path,"mask_target":target}))
            } else {
                definition(path)
            }
        })
        .transpose()?;
    let fragment = get("FragmentPath")?;
    let path_matches = masked
        || service
            .definition_path
            .as_ref()
            .is_none_or(|path| path == Path::new(fragment));
    // cat captures the complete installed main file plus ordered drop-in contents;
    // ExecStart/ExecStop/etc above retain the manager's effective loaded values.
    let effective = if masked {
        Value::Null
    } else {
        content(&checked(systemctl(&["cat", "--", &service.name]))?)
    };
    let active_state = get("ActiveState")?;
    let known = matches!(active_state, "active" | "inactive" | "failed");
    Ok(
        json!({"schema":1,"service":service,"definition":configured,"effective_definition":effective,
        "definition_matches":path_matches && get("NeedDaemonReload")? == "no" && matches!(get("LoadState")?,"loaded"|"masked"),
        "enabled":enabled,"start_allowed":matches!(state,"enabled"|"enabled-runtime"|"linked"|"linked-runtime"|"static"|"indirect"|"alias"),"masked":masked,"loaded":get("LoadState")? == "loaded",
        "active":match active_state {"active"=>Some(true),"inactive"|"failed"=>Some(false),_=>None},
        "transitioning":!known,"processes":processes,"native":native}),
    )
}

#[cfg(target_os = "macos")]
fn launchctl(args: &[&str]) -> Command {
    let mut command = Command::new("/bin/launchctl");
    command.args(args);
    command
}
// Keep only complete top-level properties. Counters such as runs and CPU time
// are observations, not definition/process identity, and are not a CAS token.
#[cfg(any(target_os = "macos", test))]
fn launch_properties(text: &str) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    let mut key = None;
    let mut value = String::new();
    let mut depth: i32 = 0;
    for line in text.lines().skip(1) {
        let trimmed = line.trim();
        if depth > 0 {
            value.push_str(line);
            value.push('\n');
            depth += trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
            if depth == 0 {
                result.insert(key.take().unwrap(), std::mem::take(&mut value));
            }
            continue;
        }
        if trimmed == "}" || trimmed.is_empty() {
            continue;
        }
        let Some((name, body)) = trimmed.split_once(" = ") else {
            continue;
        };
        if [
            "path",
            "type",
            "state",
            "pid",
            "program",
            "arguments",
            "environment",
            "default environment",
            "inherited environment",
            "properties",
            "domain",
            "minimum runtime",
            "exit timeout",
        ]
        .contains(&name)
        {
            ensure!(!result.contains_key(name), "Duplicate launchd property");
            if body == "{" {
                key = Some(name.to_owned());
                value = "{\n".into();
                depth = 1;
            } else {
                result.insert(name.into(), body.into());
            }
        }
    }
    ensure!(depth == 0, "Incomplete launchd observation");
    ensure!(
        result.contains_key("state") && result.contains_key("path"),
        "Missing launchd service state/definition"
    );
    Ok(result)
}
#[cfg(any(target_os = "macos", test))]
pub(crate) fn disabled_override(text: &str, name: &str) -> Result<Option<bool>> {
    ensure!(
        text.contains("disabled services = {"),
        "Unrecognized launchd disabled-state response"
    );
    let key = format!("\"{name}\" => ");
    let mut found = None;
    for line in text.lines() {
        if let Some(value) = line.trim().strip_prefix(&key) {
            ensure!(found.is_none(), "Duplicate launchd disable override");
            found = Some(match value.trim_end_matches(',') {
                "true" | "disabled" => true,
                "false" | "enabled" => false,
                _ => bail!("Unknown launchd disable override"),
            });
        }
    }
    Ok(found)
}
#[cfg(target_os = "macos")]
fn launch_snapshot(service: &Service) -> Result<Value> {
    let path = service
        .definition_path
        .as_ref()
        .context("Missing LaunchAgent plist")?;
    let configured = definition(path)?;
    let mut command = Command::new("/usr/bin/plutil");
    command
        .args(["-convert", "json", "-o", "-", "--"])
        .arg(path);
    let plist: Value = serde_json::from_str(&checked(command)?)?;
    ensure!(
        plist["Label"] == service.name,
        "Enrolled plist Label differs from service"
    );
    let domain = format!("gui/{}", unsafe { libc::geteuid() });
    let target = format!("{domain}/{}", service.name);
    let override_state = disabled_override(
        &checked(launchctl(&["print-disabled", &domain]))?,
        &service.name,
    )?;
    let configured_disabled = match plist.get("Disabled") {
        None => false,
        Some(value) => value
            .as_bool()
            .context("Malformed plist Disabled setting")?,
    };
    let disabled = override_state.unwrap_or(configured_disabled);
    let response = execute(launchctl(&["print", &target]))?;
    // 113 is launchctl's service-not-found status. Any other failure, including
    // unavailable GUI bootstrap domain, remains an inspection error. print-disabled
    // above independently required that exact user domain to exist.
    let loaded = response.status.success();
    ensure!(
        loaded || response.status.code() == Some(113),
        "Cannot inspect LaunchAgent: {}",
        response.stderr.trim()
    );
    let native = if loaded {
        launch_properties(&response.stdout)?
    } else {
        BTreeMap::new()
    };
    let mut processes = Vec::new();
    if let Some(pid) = native.get("pid")
        && let Some(p) = process_observation("launchd_pid", pid.parse()?)?
    {
        processes.push(p);
    }
    let state = native.get("state").map(String::as_str);
    let active = if !loaded {
        Some(false)
    } else {
        match state {
            Some("running") => Some(true),
            Some("not running") | Some("waiting") | Some("exited") => Some(false),
            _ => None,
        }
    };
    let path_matches = !loaded
        || native
            .get("path")
            .is_some_and(|actual| Path::new(actual) == path);
    Ok(
        json!({"schema":1,"service":service,"definition":configured,"effective_definition":plist,
        "definition_matches":path_matches,"enabled":!disabled,"start_allowed":!disabled,"masked":false,"loaded":loaded,
        "active":active,"transitioning":active.is_none(),"processes":processes,
        "native":{"domain":domain,"disabled_override":override_state,"properties":native}}),
    )
}
#[cfg(not(target_os = "macos"))]
fn launch_snapshot(_service: &Service) -> Result<Value> {
    bail!("LaunchAgents require macOS")
}

#[cfg(windows)]
fn task_snapshot(service: &Service) -> Result<Value> {
    let native = crate::installation_windows::observe(&service.name)?;
    let xml = native["xml"]
        .as_str()
        .context("Missing ScheduledTask XML")?;
    let configured = service
        .definition_path
        .as_ref()
        .map(|p| definition(p))
        .transpose()?;
    let mut processes = Vec::new();
    for instance in native["instances"]
        .as_array()
        .context("Missing task instances")?
    {
        let pid = u32::try_from(
            instance["engine_pid"]
                .as_u64()
                .context("Missing task engine PID")?,
        )?;
        if let Some(mut p) = process_observation("scheduled_task_engine", pid)? {
            p["instance_guid"] = instance["instance_guid"].clone();
            processes.push(p);
        }
    }
    let state = native["state"].as_u64().context("Missing task state")?;
    Ok(
        json!({"schema":1,"service":service,"definition":configured,"effective_definition":content(xml),
        "definition_matches":true,"enabled":native["enabled"],"start_allowed":native["enabled"],"masked":false,"loaded":true,
        "active":match state {4=>Some(true),1|3=>Some(false),_=>None},"transitioning":!matches!(state,1|3|4),
        "processes":processes,"native":native}),
    )
}
#[cfg(not(windows))]
fn task_snapshot(_service: &Service) -> Result<Value> {
    bail!("Scheduled Tasks require Windows")
}

fn dispatch(service: &Service, action: ServiceAction, current: &Value) -> Result<ActionOutcome> {
    match service.manager {
        ServiceManager::SystemdUser => {
            let verb = match action {
                ServiceAction::Start => "start",
                ServiceAction::Stop => "stop",
            };
            let refuse_key = match action {
                ServiceAction::Start => "RefuseManualStart",
                ServiceAction::Stop => "RefuseManualStop",
            };
            if current["native"][refuse_key] != "no" {
                return Ok(refused(
                    "manual_action_refused",
                    "Manager refuses or does not establish manual action permission",
                ));
            }
            Ok(mutation(systemctl(&[
                "--no-block",
                verb,
                "--",
                &service.name,
            ])))
        }
        #[cfg(target_os = "macos")]
        ServiceManager::LaunchAgent => {
            let domain = current["native"]["domain"]
                .as_str()
                .context("Missing bootstrap domain")?;
            let target = format!("{domain}/{}", service.name);
            let command = match action {
                ServiceAction::Stop if current["loaded"] == false => {
                    return Ok(ActionOutcome::Accepted);
                }
                ServiceAction::Stop => launchctl(&["bootout", &target]),
                ServiceAction::Start if current["loaded"] == true => {
                    launchctl(&["kickstart", &target])
                }
                ServiceAction::Start => {
                    let mut command = launchctl(&["bootstrap", domain]);
                    command.arg(service.definition_path.as_ref().context("Missing plist")?);
                    let accepted = mutation(command);
                    if accepted != ActionOutcome::Accepted {
                        return Ok(accepted);
                    }
                    // Loading alone need not run a job whose RunAtLoad is false.
                    // No retry follows a partial bootstrap/kickstart failure.
                    return Ok(mutation(launchctl(&["kickstart", &target])));
                }
            };
            Ok(mutation(command))
        }
        #[cfg(windows)]
        ServiceManager::ScheduledTask => {
            crate::installation_windows::dispatch(&service.name, action, &current["native"])
        }
        #[allow(unreachable_patterns)]
        _ => bail!("Service manager is unsupported on this platform"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_validation_is_bracketed_by_one_native_observation_pair() {
        use std::cell::RefCell;
        let order = RefCell::new(Vec::new());
        let expected = json!({"enabled":true,"process_generation":"original"});
        let result = action_snapshot(
            &expected,
            || {
                order.borrow_mut().push("observe");
                Ok(expected.clone())
            },
            |observed| {
                order.borrow_mut().push("application");
                assert_eq!(observed, &expected);
                Ok(None)
            },
        )
        .unwrap();
        assert!(matches!(result, CheckedSnapshot::Confirmed(value) if value == expected));
        assert_eq!(*order.borrow(), ["observe", "application", "observe"]);
    }

    #[test]
    fn native_drift_during_application_validation_cannot_be_confirmed() {
        use std::cell::RefCell;
        let expected = json!({"enabled":true,"process_generation":"original"});
        for changed in [
            json!({"enabled":false,"process_generation":"original"}),
            json!({"enabled":true,"process_generation":"replacement"}),
        ] {
            let observed = RefCell::new(expected.clone());
            let result = action_snapshot(
                &expected,
                || Ok(observed.borrow().clone()),
                |_| {
                    *observed.borrow_mut() = changed;
                    Ok(None)
                },
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn application_refusal_does_not_request_more_native_observations() {
        use std::cell::Cell;
        let reads = Cell::new(0);
        let expected = json!({"enabled":true});
        let result = action_snapshot(
            &expected,
            || {
                reads.set(reads.get() + 1);
                Ok(expected.clone())
            },
            |_| Ok(Some(refused("application_unconfirmed", "Unproved process"))),
        )
        .unwrap();
        assert!(
            matches!(result, CheckedSnapshot::Refused(ActionOutcome::Refused {code,..}) if code == "application_unconfirmed")
        );
        assert_eq!(reads.get(), 1);
        let result = action_snapshot(
            &expected,
            || Ok(json!({"enabled":false})),
            |_| panic!("Stale native state must refuse before application validation"),
        )
        .unwrap();
        assert!(
            matches!(result, CheckedSnapshot::Refused(ActionOutcome::Refused {code,..}) if code == "stale_snapshot")
        );
    }

    #[test]
    fn application_validator_cannot_claim_dispatch_or_settlement() {
        let expected = json!({"enabled":true});
        for outcome in [
            ActionOutcome::Accepted,
            unknown("not_a_dispatch", "Validation cannot dispatch"),
        ] {
            assert!(
                action_snapshot(&expected, || Ok(expected.clone()), |_| Ok(Some(outcome))).is_err()
            );
        }
    }

    #[test]
    fn availability_refusal_does_not_invent_enabled_or_settled() {
        let mut view = json!({"definition_matches":true,"transitioning":false,"enabled":true,"start_allowed":true,"masked":false});
        assert!(action_refusal(&view, ServiceAction::Start).is_none());
        for value in [Value::Null, json!(false)] {
            view["start_allowed"] = value;
            assert!(matches!(
                action_refusal(&view, ServiceAction::Start),
                Some(ActionOutcome::Refused { .. })
            ));
            assert!(action_refusal(&view, ServiceAction::Stop).is_none());
        }
        view["start_allowed"] = json!(true);
        view["masked"] = json!(true);
        assert!(action_refusal(&view, ServiceAction::Start).is_some());
        view["masked"] = json!(false);
        view["transitioning"] = Value::Null;
        assert!(action_refusal(&view, ServiceAction::Stop).is_some());
    }
    #[test]
    fn properties_preserve_unknown_and_reject_duplicate() {
        assert_eq!(
            properties("ActiveState=future-state\nMainPID=0\n").unwrap()["ActiveState"],
            "future-state"
        );
        assert!(properties("A=1\nA=2\n").is_err());
        assert!(properties("not a property").is_err());
    }
    #[test]
    fn launchd_parser_retains_effective_blocks_and_ignores_counters() {
        let raw = "gui/501/example = {\n\tpath = /Users/a/job.plist\n\tstate = running\n\tpid = 21\n\truns = 8\n\targuments = {\n\t\t/bin/sleep\n\t\t60\n\t}\n}\n";
        let parsed = launch_properties(raw).unwrap();
        assert_eq!(parsed["pid"], "21");
        assert!(!parsed.contains_key("runs"));
        assert!(parsed["arguments"].contains("/bin/sleep"));
        assert!(launch_properties("bad").is_err());
        assert_eq!(
            disabled_override("disabled services = {\n\"example\" => true\n}", "example").unwrap(),
            Some(true)
        );
        assert_eq!(
            disabled_override("disabled services = {\n\"other\" => true\n}", "example").unwrap(),
            None
        );
        assert!(disabled_override("unknown", "example").is_err());
    }
    #[test]
    fn definition_records_exact_bytes_and_rejects_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("unit");
        fs::write(&path, b"first\n").unwrap();
        let first = definition(&path).unwrap();
        fs::write(&path, b"second\n").unwrap();
        let second = definition(&path).unwrap();
        assert_ne!(first, second);
        assert_eq!(first["bytes"], json!([102, 105, 114, 115, 116, 10]));
        assert!(definition(tmp.path()).is_err());
    }
    #[test]
    fn missing_manager_is_known_no_dispatch() {
        let temp = tempfile::tempdir().unwrap();
        let command = Command::new(temp.path().join("does-not-exist"));
        assert!(
            matches!(mutation(command), ActionOutcome::Refused { code, .. } if code == "manager_unavailable")
        );
    }
    #[test]
    fn no_restore_or_arbitrary_action_in_request_shape() {
        for action in ["restore", "enable", "unmask", "delete", "/bin/sh"] {
            assert!(serde_json::from_value::<ServiceAction>(json!(action)).is_err());
        }
        assert_eq!(
            serde_json::to_value(ActionOutcome::Accepted).unwrap(),
            json!({"status":"accepted"})
        );
    }
}
