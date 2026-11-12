mod config;
mod service_application;
mod supervision;

use amplifier_native_platform::{
    installation::{self, ActionOutcome, ServiceAction},
    installation_exchange::{self, EntryRef, MutationOutcome},
    locking::InstallationLock,
    process,
    registration::{self, RegistrationOutcome},
};
use anyhow::{Context, Result, ensure};
use config::Enrollment;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
};

const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
struct Request {
    version: u32,
    id: String,
    #[serde(flatten)]
    operation: Operation,
}

#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum Operation {
    #[serde(rename = "describe")]
    Describe,
    #[serde(rename = "lock.acquire")]
    LockAcquire { name: String },
    #[serde(rename = "lock.release")]
    LockRelease { name: String },
    #[serde(rename = "process.identity")]
    ProcessIdentity { pid: u32 },
    #[serde(rename = "process.matches", alias = "process.status")]
    ProcessMatches { identity: Value },
    #[serde(rename = "service.register")]
    ServiceRegister { name: String },
    #[serde(rename = "exchange.inspect")]
    ExchangeInspect {
        name: String,
        entry: Option<EntryRef>,
    },
    #[serde(rename = "exchange.prepare_pointer")]
    ExchangePrepare {
        name: String,
        prepared: EntryRef,
        target: EntryRef,
        expected_target: Value,
    },
    #[serde(rename = "exchange.apply")]
    ExchangeApply {
        name: String,
        incoming: EntryRef,
        retained: EntryRef,
        expected_current: Value,
        expected_incoming: Value,
    },
    #[serde(rename = "service.snapshot")]
    ServiceSnapshot { name: String },
    #[serde(rename = "service.action")]
    ServiceAction {
        name: String,
        action: ServiceAction,
        expected: Value,
    },
    #[serde(rename = "storage.write")]
    StorageWrite {
        root: String,
        path: String,
        bytes: Vec<u8>,
        #[serde(default)]
        create_only: bool,
    },
    #[serde(rename = "storage.remove")]
    StorageRemove { root: String, path: String },
    #[serde(rename = "storage.mkdir")]
    StorageMkdir { root: String, path: String },
    #[serde(rename = "process.launch")]
    ProcessLaunch {
        name: String,
        result: Option<String>,
    },
}

struct Outcome {
    kind: &'static str,
    result: Option<Value>,
    error: Option<Value>,
}
impl Outcome {
    fn ok(value: Value) -> Self {
        Self {
            kind: "ok",
            result: Some(value),
            error: None,
        }
    }
    fn failure(kind: &'static str, code: &str, message: impl ToString) -> Self {
        Self {
            kind,
            result: None,
            error: Some(json!({"code":code,"message":message.to_string()})),
        }
    }
    fn reply(self, id: &str) -> Value {
        let mut reply = json!({"version":1,"id":id,"outcome":self.kind});
        if let Some(result) = self.result {
            reply["result"] = result;
        }
        if let Some(error) = self.error {
            reply["error"] = error;
        }
        reply
    }
}

struct Session {
    enrollment: Enrollment,
    locks: BTreeMap<String, InstallationLock>,
}
impl Session {
    fn dispatch(&mut self, op: Operation) -> Result<Outcome> {
        // Releasing this process's existing handle needs no authority from a new
        // configuration and must not strand a lock after an owner update.
        if let Operation::LockRelease { name } = op {
            if self.locks.remove(&name).is_none() {
                return Ok(Outcome::failure(
                    "refused",
                    "not_held",
                    "This invocation does not hold that lock",
                ));
            }
            return Ok(Outcome::ok(json!({})));
        }
        // Never continue against an owner's replaced configuration.
        if let Err(e) = self.enrollment.unchanged() {
            return Ok(Outcome::failure("refused", "configuration_changed", e));
        }
        let cfg = &self.enrollment.config;
        match op {
            Operation::Describe => Ok(Outcome::ok(
                json!({"protocol":1,"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"operations":["describe","lock.acquire","lock.release","process.identity","process.matches","process.status","service.snapshot","service.action","service.register","exchange.inspect","exchange.prepare_pointer","exchange.apply","storage.write","storage.remove","storage.mkdir","process.launch"],"source_pointer_exchange":1,"service_application_identity":1}),
            )),
            Operation::LockAcquire { name } => {
                let path = cfg.locks.get(&name).context("Unenrolled lock")?;
                config::protected_directory(path.parent().context("Lock parent missing")?)?;
                config::path_chain(path)?;
                if self.locks.contains_key(&name) {
                    return Ok(Outcome::failure(
                        "refused",
                        "already_held",
                        "This invocation already holds that lock",
                    ));
                }
                match InstallationLock::try_acquire(path)? {
                    Some(lock) => {
                        self.locks.insert(name, lock);
                        Ok(Outcome::ok(json!({})))
                    }
                    None => Ok(Outcome::failure(
                        "busy",
                        "lock_contended",
                        "Another invocation holds the installation lock",
                    )),
                }
            }
            Operation::LockRelease { .. } => {
                unreachable!("release handled before enrollment check")
            }
            Operation::ProcessIdentity { pid } => {
                Ok(Outcome::ok(process::identity(pid)?.unwrap_or(Value::Null)))
            }
            Operation::ProcessMatches { identity } => {
                Ok(Outcome::ok(json!({"matches":process::matches(&identity)?})))
            }
            Operation::ServiceRegister { name } => {
                let service = cfg.services.get(&name).context("Unenrolled service")?;
                let profile = cfg
                    .registrations
                    .get(&name)
                    .context("Unenrolled service registration")?;
                Ok(match registration::register(service, profile)? {
                    RegistrationOutcome::Registered { created } => {
                        Outcome::ok(json!({"created":created}))
                    }
                    RegistrationOutcome::Refused { code, message } => {
                        Outcome::failure("refused", &code, message)
                    }
                    RegistrationOutcome::Unknown { code, message } => {
                        Outcome::failure("unknown", &code, message)
                    }
                })
            }
            Operation::ExchangeInspect { name, entry } => {
                let exchange = cfg
                    .exchanges
                    .get(&name)
                    .context("Unenrolled source exchange")?;
                Ok(Outcome::ok(installation_exchange::inspect(
                    &exchange.profile,
                    entry.as_ref(),
                )?))
            }
            Operation::ExchangePrepare {
                name,
                prepared,
                target,
                expected_target,
            } => {
                let exchange = cfg
                    .exchanges
                    .get(&name)
                    .context("Unenrolled source exchange")?;
                if !self.locks.contains_key(&exchange.lock) {
                    return Ok(Outcome::failure(
                        "refused",
                        "lock_not_held",
                        "Hold the configured exchange lock for the whole critical section",
                    ));
                }
                Ok(exchange_outcome(installation_exchange::prepare_pointer(
                    &exchange.profile,
                    &prepared,
                    &target,
                    &expected_target,
                )?))
            }
            Operation::ExchangeApply {
                name,
                incoming,
                retained,
                expected_current,
                expected_incoming,
            } => {
                let exchange = cfg
                    .exchanges
                    .get(&name)
                    .context("Unenrolled source exchange")?;
                if !self.locks.contains_key(&exchange.lock) {
                    return Ok(Outcome::failure(
                        "refused",
                        "lock_not_held",
                        "Hold the configured exchange lock for the whole critical section",
                    ));
                }
                Ok(exchange_outcome(installation_exchange::exchange(
                    &exchange.profile,
                    &incoming,
                    &retained,
                    &expected_current,
                    &expected_incoming,
                )?))
            }
            Operation::ServiceSnapshot { name } => {
                let mut snapshot =
                    installation::snapshot(cfg.services.get(&name).context("Unenrolled service")?)?;
                service_application::augment(&self.enrollment, &name, &mut snapshot)?;
                Ok(Outcome::ok(snapshot))
            }
            Operation::ServiceAction {
                name,
                action,
                expected,
            } => {
                let service = cfg.services.get(&name).context("Unenrolled service")?;
                let mut expected_native = expected.clone();
                if cfg.service_applications.contains_key(&name) {
                    let mut current = installation::snapshot(service)?;
                    service_application::augment(&self.enrollment, &name, &mut current)?;
                    if current != expected {
                        return Ok(Outcome::failure(
                            "refused",
                            "stale_snapshot",
                            "Service/application generation changed",
                        ));
                    }
                    if current["application"]["status"] == "unconfirmed" {
                        return Ok(Outcome::failure(
                            "refused",
                            "application_unconfirmed",
                            "Cannot establish owned service application",
                        ));
                    }
                    expected_native
                        .as_object_mut()
                        .context("Snapshot must be an object")?
                        .remove("application");
                }
                match installation::request_state(service, action, &expected_native)? {
                    ActionOutcome::Accepted => Ok(Outcome::ok(json!({"accepted":true}))),
                    ActionOutcome::Refused { code, message } => {
                        Ok(Outcome::failure("refused", &code, message))
                    }
                    ActionOutcome::Unknown { code, message } => {
                        Ok(Outcome::failure("unknown", &code, message))
                    }
                }
            }
            Operation::StorageWrite {
                root,
                path,
                bytes,
                create_only,
            } => {
                let path = match self.enrollment.receipt(&root, &path) {
                    Ok(p) => p,
                    Err(e) => return Ok(Outcome::failure("refused", "invalid_receipt", e)),
                };
                if create_only && path.exists() {
                    return Ok(Outcome::failure(
                        "refused",
                        "already_exists",
                        "Receipt already exists",
                    ));
                }
                let written = if create_only {
                    amplifier_native_platform::private_create(&path, &bytes)
                } else {
                    amplifier_native_platform::private_write(&path, &bytes)
                };
                Ok(match written {
                    Ok(()) => Outcome::ok(json!({})),
                    Err(e) => Outcome::failure("unknown", "write_unconfirmed", e),
                })
            }
            Operation::StorageMkdir { root, path } => {
                let path = match self.enrollment.directory(&root, &path) {
                    Ok(path) => path,
                    Err(error) => {
                        return Ok(Outcome::failure("refused", "invalid_directory", error));
                    }
                };
                if path.exists() {
                    return Ok(Outcome::ok(json!({})));
                }
                Ok(match amplifier_native_platform::private_dir(&path) {
                    Ok(()) => Outcome::ok(json!({})),
                    Err(error) => Outcome::failure("unknown", "directory_unconfirmed", error),
                })
            }
            Operation::StorageRemove { root, path } => {
                let path = match self.enrollment.receipt(&root, &path) {
                    Ok(p) => p,
                    Err(e) => return Ok(Outcome::failure("refused", "invalid_receipt", e)),
                };
                if !path.exists() {
                    return Ok(Outcome::ok(json!({"absent":true})));
                }
                let removal = (|| -> Result<()> {
                    fs::remove_file(&path)?;
                    #[cfg(unix)]
                    fs::File::open(path.parent().context("Missing parent")?)?.sync_all()?;
                    // Windows DeleteFile completion commits removal through the filesystem;
                    // it has no portable directory-fsync equivalent.
                    Ok(())
                })();
                Ok(match removal {
                    Ok(()) => Outcome::ok(json!({"absent":true})),
                    Err(e) => Outcome::failure("unknown", "remove_unconfirmed", e),
                })
            }
            Operation::ProcessLaunch { name, result } => {
                if let Err(e) = self.enrollment.command(&name, result.as_deref()) {
                    return Ok(Outcome::failure("refused", "invalid_command", e));
                }
                Ok(
                    match supervision::launch(&self.enrollment, &name, result.as_deref()) {
                        Ok(identity) => Outcome::ok(identity),
                        Err(e) => Outcome::failure("unknown", "launch_unconfirmed", e),
                    },
                )
            }
        }
    }
}

fn exchange_outcome(outcome: MutationOutcome) -> Outcome {
    match outcome {
        MutationOutcome::Applied { state } => Outcome::ok(state),
        MutationOutcome::Refused { reason } => {
            Outcome::failure("refused", "source_changed", reason)
        }
        MutationOutcome::Unknown { phase, error } => Outcome::failure("unknown", phase, error),
    }
}

fn serve(enrollment: Enrollment) -> Result<()> {
    let mut session = Session {
        enrollment,
        locks: BTreeMap::new(),
    };
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    loop {
        let mut line = Vec::new();
        // Limit while reading, rather than allocating an unbounded line first.
        let read = io::Read::take(&mut input, MAX_FRAME as u64 + 1).read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        ensure!(
            line.len() <= MAX_FRAME && line.last() == Some(&b'\n'),
            "Invalid or oversized request frame"
        );
        let parsed: Value = serde_json::from_slice(&line)?;
        let id = parsed
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let fields = match parsed.get("op").and_then(Value::as_str) {
            Some("describe") => &["version", "id", "op"][..],
            Some("lock.acquire" | "lock.release" | "service.snapshot") => {
                &["version", "id", "op", "name"]
            }
            Some("process.identity") => &["version", "id", "op", "pid"],
            Some("process.matches" | "process.status") => &["version", "id", "op", "identity"],
            Some("exchange.inspect") => &["version", "id", "op", "name", "entry"],
            Some("exchange.prepare_pointer") => &[
                "version",
                "id",
                "op",
                "name",
                "prepared",
                "target",
                "expected_target",
            ],
            Some("exchange.apply") => &[
                "version",
                "id",
                "op",
                "name",
                "incoming",
                "retained",
                "expected_current",
                "expected_incoming",
            ],
            Some("service.action") => &["version", "id", "op", "name", "action", "expected"],
            Some("storage.write") => &[
                "version",
                "id",
                "op",
                "root",
                "path",
                "bytes",
                "create_only",
            ],
            Some("storage.remove" | "storage.mkdir") => &["version", "id", "op", "root", "path"],
            Some("process.launch") => &["version", "id", "op", "name", "result"],
            _ => &[],
        };
        let known_fields = parsed
            .as_object()
            .is_some_and(|m| m.keys().all(|k| fields.contains(&k.as_str())));
        let response = if !known_fields {
            Outcome::failure(
                "refused",
                "invalid_request",
                "Unrecognized operation or request field",
            )
            .reply(&id)
        } else {
            match serde_json::from_value::<Request>(parsed) {
                Ok(request)
                    if request.version == 1
                        && !request.id.is_empty()
                        && request.id.len() <= 128 =>
                {
                    match session.dispatch(request.operation) {
                        Ok(outcome) => outcome.reply(&id),
                        Err(e) => Outcome::failure("error", "observation_failed", e).reply(&id),
                    }
                }
                Ok(_) => Outcome::failure(
                    "refused",
                    "invalid_request",
                    "Unsupported version or invalid request identity",
                )
                .reply(&id),
                Err(e) => Outcome::failure("refused", "invalid_request", e).reply(&id),
            }
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

fn run() -> Result<i32> {
    let mut args = std::env::args_os().skip(1);
    ensure!(
        args.next().as_deref() == Some(std::ffi::OsStr::new("--config")),
        "Usage: constellation-native-driver --config PATH serve|exec --command NAME"
    );
    let path = PathBuf::from(args.next().context("Configuration path required")?);
    let enrollment = Enrollment::load(&path)?;
    let mode = args.next().context("Operation mode required")?;
    if mode == "serve" {
        ensure!(args.next().is_none(), "Unexpected argument");
        serve(enrollment)?;
        return Ok(0);
    }
    if mode == "service-run" || mode == "internal-service-execute" {
        ensure!(
            args.next().as_deref() == Some(std::ffi::OsStr::new("--service")),
            "Service name required"
        );
        let name = args
            .next()
            .context("Missing service")?
            .into_string()
            .map_err(|_| anyhow::anyhow!("Invalid service name"))?;
        ensure!(args.next().is_none(), "Unexpected argument");
        return if mode == "service-run" {
            service_application::run(&enrollment, &name)
        } else {
            service_application::execute(&enrollment, &name)
        };
    }
    #[cfg(unix)]
    if mode == "internal-watch" {
        ensure!(args.next().is_none(), "Unexpected argument");
        return supervision::watch();
    }
    ensure!(
        mode == "exec" || mode == "internal-detached" || mode == "internal-execute",
        "Unknown mode"
    );
    ensure!(
        args.next().as_deref() == Some(std::ffi::OsStr::new("--command")),
        "Command name required"
    );
    let name = args
        .next()
        .context("Command name missing")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("Invalid command name"))?;
    let result = match args.next() {
        Some(flag) if flag == "--result" => Some(
            args.next()
                .context("Result path missing")?
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid result path"))?,
        ),
        Some(_) => anyhow::bail!("Unexpected argument"),
        None => None,
    };
    ensure!(args.next().is_none(), "Unexpected argument");
    if mode == "internal-execute" {
        return supervision::execute(&enrollment, &name, result.as_deref());
    }
    supervision::run(&enrollment, &name, result.as_deref(), mode == "exec")
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("Native installation driver: {error:#}");
            std::process::exit(1);
        }
    }
}
