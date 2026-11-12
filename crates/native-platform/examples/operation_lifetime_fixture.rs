//! Opt-in, disposable OS lifetime fixture. Not a product command or release asset.
#[cfg(feature = "installation")]
mod fixture {
    use amplifier_native_platform::{
        installation::{self, ActionOutcome, Service, ServiceAction, ServiceManager},
        process,
        registration::{self, Registration, RegistrationOutcome, RetirementOutcome},
    };
    use anyhow::{Context, Result, bail, ensure};
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    #[derive(Clone, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Plan {
        id: String,
        root: PathBuf,
        executable: PathBuf,
        executable_sha256: String,
        source: String,
        detached: bool,
    }
    fn hash(path: &Path) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
    }
    fn write(path: &Path, value: &Value) -> Result<()> {
        amplifier_native_platform::private_write_data(path, &serde_json::to_vec_pretty(value)?)
    }
    fn read(path: &Path) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }
    fn local_manager() -> ServiceManager {
        #[cfg(target_os = "linux")]
        return ServiceManager::SystemdUser;
        #[cfg(target_os = "macos")]
        return ServiceManager::LaunchAgent;
        #[cfg(windows)]
        return ServiceManager::ScheduledTask;
    }
    impl Plan {
        fn profile(&self, role: &str) -> Result<(Service, Registration)> {
            ensure!(
                ["a", "u", "b"].contains(&role),
                "Invalid fixed fixture role"
            );
            let label = format!("constellation-operation-test-{}-{role}", self.id);
            #[cfg(target_os = "linux")]
            let (name, path) = {
                let name = format!("{label}.service");
                let path = amplifier_native_platform::user_home()?
                    .join(".config/systemd/user")
                    .join(&name);
                (name, path)
            };
            #[cfg(target_os = "macos")]
            let (name, path) = (
                label.clone(),
                amplifier_native_platform::user_home()?
                    .join("Library/LaunchAgents")
                    .join(format!("{label}.plist")),
            );
            #[cfg(windows)]
            let (name, path) = (label, self.root.join(format!("{role}.xml")));
            Ok((
                Service {
                    manager: local_manager(),
                    name,
                    definition_path: Some(path),
                },
                Registration {
                    executable: self.executable.clone(),
                    args: vec![
                        "role".into(),
                        role.into(),
                        self.root.to_string_lossy().into(),
                    ],
                    cwd: self.root.clone(),
                    description: format!("Disposable lifetime {} {role}", self.id),
                    autostart: false,
                },
            ))
        }
        fn validate(&self) -> Result<()> {
            ensure!(
                uuid::Uuid::parse_str(&self.id)?.to_string() == self.id,
                "Invalid fixture UUID"
            );
            ensure!(
                self.root.is_absolute() && self.root.is_dir(),
                "Missing fixture root"
            );
            ensure!(
                self.root.file_name().and_then(|n| n.to_str())
                    == Some(format!("native-operation-{}", self.id).as_str()),
                "Not a fresh operation fixture root"
            );
            ensure!(
                self.executable == std::env::current_exe()?.canonicalize()?,
                "Wrong fixture executable"
            );
            ensure!(
                hash(&self.executable)? == self.executable_sha256,
                "Fixture executable changed"
            );
            ensure!(
                self.source.len() == 40 && self.source.bytes().all(|b| b.is_ascii_hexdigit()),
                "Exact fixture source required"
            );
            Ok(())
        }
        fn cancelled(&self) -> Result<()> {
            ensure!(
                !self.root.join("cleanup.json").exists(),
                "Fixture cleanup requested"
            );
            Ok(())
        }
    }
    fn control(plan: &Plan) -> Result<amplifier_native_platform::locking::InstallationLock> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(guard) = amplifier_native_platform::locking::InstallationLock::try_acquire(
                &plan.root.join("fixture-control.lock"),
            )? {
                return Ok(guard);
            }
            ensure!(
                Instant::now() < deadline,
                "Fixture control owner did not settle"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
    fn own_identity() -> Result<Value> {
        process::identity(std::process::id())?.context("Current fixture process has no identity")
    }
    fn wait<F>(plan: &Plan, label: &str, seconds: u64, mut observe: F) -> Result<Value>
    where
        F: FnMut() -> Result<Option<Value>>,
    {
        let until = Instant::now() + Duration::from_secs(seconds);
        let mut last = "not observed".to_owned();
        loop {
            plan.cancelled()?;
            match observe() {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => (),
                Err(error) => last = format!("{error:#}"),
            }
            ensure!(Instant::now() < until, "{label} did not settle: {last}");
            thread::sleep(Duration::from_millis(100));
        }
    }
    fn snapshot(plan: &Plan, role: &str, active: bool) -> Result<Value> {
        let (service, profile) = plan.profile(role)?;
        wait(plan, &format!("{role} active={active}"), 25, || {
            let value = installation::snapshot(&service)?;
            ensure!(
                registration::profile_matches(&service, &profile, &value)?,
                "Fixture profile changed"
            );
            Ok((value["active"] == active
                && value["transitioning"] == false
                && (active || value["processes"] == json!([])))
            .then_some(value))
        })
    }
    fn register(plan: &Plan, role: &str) -> Result<()> {
        let _guard = control(plan)?;
        plan.cancelled()?;
        let (service, profile) = plan.profile(role)?;
        ensure!(
            registration::register(&service, &profile)?
                == RegistrationOutcome::Registered { created: true },
            "Fresh fixture registration was not created"
        );
        write(
            &plan.root.join(format!("{role}-registered.json")),
            &json!({"service":service,"profile":profile}),
        )
    }
    fn action(plan: &Plan, role: &str, action: ServiceAction, expected: &Value) -> Result<()> {
        let _guard = control(plan)?;
        plan.cancelled()?;
        let (service, profile) = plan.profile(role)?;
        ensure!(
            registration::profile_matches(&service, &profile, expected)?,
            "Wrong operation profile"
        );
        let outcome = installation::request_state_checked(&service, action, expected, |value| {
            ensure!(
                registration::profile_matches(&service, &profile, value)?,
                "Profile changed before action"
            );
            Ok(None)
        })?;
        ensure!(
            outcome == ActionOutcome::Accepted,
            "Manager action not accepted: {outcome:?}"
        );
        Ok(())
    }
    fn ready(plan: &Plan, role: &str) -> Result<Value> {
        wait(plan, &format!("{role} ready"), 25, || {
            let path = plan.root.join(format!("{role}-heartbeat.json"));
            if !path.exists() {
                return Ok(None);
            }
            let value = read(&path)?;
            Ok(process::matches(&value["identity"])?.then_some(value))
        })
    }
    fn pulse(plan: &Plan, role: &str, identity: &Value, sequence: u64) -> Result<()> {
        write(
            &plan.root.join(format!("{role}-heartbeat.json")),
            &json!({"identity":identity,"sequence":sequence}),
        )
    }
    fn heartbeat(plan: &Plan, role: &str, identity: &Value) -> Result<()> {
        for sequence in 1..=900 {
            if plan.root.join("cleanup.json").exists() {
                return Ok(());
            }
            pulse(plan, role, identity, sequence)?;
            thread::sleep(Duration::from_millis(100));
        }
        bail!("Fixture heartbeat reached its bounded 90-second lifetime")
    }
    fn role(plan: &Plan, role: &str) -> Result<()> {
        plan.validate()?;
        let identity = own_identity()?;
        match role {
            "a" => {
                pulse(plan, "a", &identity, 0)?;
                wait(plan, "orchestrator start signal", 25, || {
                    Ok(plan.root.join("go.json").exists().then_some(json!(true)))
                })?;
                register(plan, "u")?;
                let mut detached_child = if plan.detached {
                    let _guard = control(plan)?;
                    plan.cancelled()?;
                    let mut child = Command::new(&plan.executable);
                    child
                        .args(["role", "u"])
                        .arg(&plan.root)
                        .current_dir(&plan.root)
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null());
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::CommandExt;
                        child.process_group(0);
                    }
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        child.creation_flags(0x08000000 | 0x00000200);
                    }
                    Some(child.spawn()?)
                } else {
                    action(
                        plan,
                        "u",
                        ServiceAction::Start,
                        &snapshot(plan, "u", false)?,
                    )?;
                    None
                };
                let result = heartbeat(plan, "a", &identity);
                // Only reached on fixture cancellation/timeout, never as proof of
                // the manager boundary. Retain the exact directly owned child.
                if let Some(child) = detached_child.as_mut() {
                    if child.try_wait()?.is_none() {
                        child.kill()?;
                    }
                    child.wait()?;
                }
                result
            }
            "u" => {
                write(
                    &plan.root.join("u-entry.json"),
                    &json!({"identity":identity,"mode":if plan.detached {"detached"} else {"os-managed"}}),
                )?;
                let parent = ready(plan, "a")?;
                action(plan, "a", ServiceAction::Stop, &snapshot(plan, "a", true)?)?;
                let stopped = snapshot(plan, "a", false)?;
                // A settled manager is not itself proof the application exited.
                // Capture the gap, then observe this exact generation without a
                // signal or cooperative cleanup that could manufacture success.
                write(
                    &plan.root.join("a-manager-inactive.json"),
                    &json!({"parent":stopped,"identity":parent["identity"],
                        "generation_still_matches":process::matches(&parent["identity"])?}),
                )?;
                let parent_exit = wait_exited(plan, "a", &parent["identity"])?;
                ensure!(process::matches(&identity)?, "Worker identity changed");
                write(
                    &plan.root.join("u-after-parent-stop.json"),
                    &json!({"identity":identity,"parent":stopped,"parent_exit":parent_exit}),
                )?;
                register(plan, "b")?;
                action(
                    plan,
                    "b",
                    ServiceAction::Start,
                    &snapshot(plan, "b", false)?,
                )?;
                let replacement = ready(plan, "b")?;
                let observed = snapshot(plan, "b", true)?;
                write(
                    &plan.root.join("u-complete.json"),
                    &json!({"identity":identity,"replacement":replacement,"replacement_snapshot":observed}),
                )
            }
            "b" => {
                pulse(plan, "b", &identity, 0)?;
                heartbeat(plan, "b", &identity)
            }
            _ => bail!("Unknown fixed fixture role"),
        }
    }
    fn wait_exited(plan: &Plan, role: &str, identity: &Value) -> Result<Value> {
        let started = Instant::now();
        wait(
            plan,
            &format!("captured {role} process generation exit"),
            25,
            || {
                if process::matches(identity)? {
                    return Ok(None);
                }
                Ok(Some(json!({"identity":identity,"exited":true,
                "elapsed_ms":started.elapsed().as_millis()})))
            },
        )
    }
    fn replacement_advances(plan: &Plan, expected: &Value) -> Result<Value> {
        let initial = read(&plan.root.join("b-heartbeat.json"))?;
        ensure!(
            initial["identity"] == *expected && process::matches(expected)?,
            "Replacement generation changed"
        );
        wait(plan, "replacement heartbeat", 10, || {
            let later = read(&plan.root.join("b-heartbeat.json"))?;
            ensure!(
                later["identity"] == *expected && process::matches(expected)?,
                "Replacement did not survive"
            );
            Ok((later["sequence"].as_u64() > initial["sequence"].as_u64()).then_some(later))
        })
    }
    fn retired(plan: &Plan, role: &str) -> Result<Value> {
        let (service, profile) = plan.profile(role)?;
        let observed = snapshot(plan, role, false)?;
        let result = registration::retire(&service, &profile, &observed)?;
        ensure!(
            matches!(result, RetirementOutcome::Retired { .. }),
            "Retirement not confirmed: {result:?}"
        );
        ensure!(
            registration::is_retired(&service)?,
            "Retirement absence not confirmed"
        );
        Ok(json!({"service":service,"outcome":result}))
    }
    fn scenario(plan: &Plan) -> Result<Value> {
        register(plan, "a")?;
        action(
            plan,
            "a",
            ServiceAction::Start,
            &snapshot(plan, "a", false)?,
        )?;
        let parent = ready(plan, "a")?;
        snapshot(plan, "a", true)?;
        write(&plan.root.join("go.json"), &json!({"id":plan.id}))?;
        let worker = wait(plan, "worker entry", 30, || {
            let path = plan.root.join("u-entry.json");
            path.exists().then(|| read(&path)).transpose()
        })?;
        if plan.detached {
            snapshot(plan, "a", false)?;
            let result = wait(plan, "detached worker outcome", 60, || {
                if plan.root.join("u-complete.json").exists() {
                    return Ok(Some(
                        json!({"worker_completed":true,"worker_survived_parent_stop":true}),
                    ));
                }
                Ok((!process::matches(&worker["identity"])?)
                    .then(|| json!({"worker_completed":false,"worker_survived_parent_stop":false})))
            })?;
            let parent_exited = !process::matches(&parent["identity"])?;
            let mut result = result;
            result["parent_generation_exited"] = json!(parent_exited);
            result["worker_generation_exited"] = json!(!process::matches(&worker["identity"])?);
            if !parent_exited {
                // A manager stop acknowledgement cannot establish the outcome
                // of a worker relative to an application that has not exited.
                result["worker_survived_parent_stop"] = Value::Null;
            }
            #[cfg(target_os = "linux")]
            ensure!(
                parent_exited && result["worker_survived_parent_stop"] == false,
                "Detached Linux control did not expose the old cgroup boundary"
            );
            return Ok(
                json!({"mode":"detached-control","parent":parent,"worker":worker,"observation":result,
                "claim":"Observed control only; no cross-platform kill assumption"}),
            );
        }
        let completion = wait(plan, "worker complete", 60, || {
            let path = plan.root.join("u-complete.json");
            path.exists().then(|| read(&path)).transpose()
        })?;
        ensure!(
            !process::matches(&parent["identity"])?,
            "Original parent generation remains"
        );
        snapshot(plan, "u", false)?;
        let worker_exit = wait_exited(plan, "u", &worker["identity"])?;
        let before = replacement_advances(plan, &completion["replacement"]["identity"])?;
        let retirement = retired(plan, "u")?;
        let after = replacement_advances(plan, &completion["replacement"]["identity"])?;
        snapshot(plan, "b", true)?;
        Ok(
            json!({"mode":"os-managed","parent":parent,"worker":worker,"completion":completion,
            "worker_exit":worker_exit,"worker_retirement":retirement,"replacement_before_retirement":before,"replacement_after_retirement":after,
            "worker_survived_parent_stop":true,"replacement_survived_worker_exit_and_retirement":true}),
        )
    }
    fn cleanup(plan: &Plan) -> Result<Value> {
        // Cancellation is fixture-only cooperative exit, never used to establish
        // the A→U→B proof. Original failures remain in the outer receipt.
        {
            let _guard = control(plan)?;
            write(&plan.root.join("cleanup.json"), &json!({"id":plan.id}))?;
        }
        let mut rows = Vec::new();
        let mut errors = Vec::new();
        for role in ["a", "u", "b"] {
            let result = (|| -> Result<Value> {
                let (service, profile) = plan.profile(role)?;
                if registration::is_retired(&service)? {
                    return Ok(json!({"role":role,"absent":true}));
                }
                let deadline = Instant::now() + Duration::from_secs(30);
                let observed = loop {
                    let observed = installation::snapshot(&service)?;
                    ensure!(
                        registration::profile_matches(&service, &profile, &observed)?,
                        "Cleanup profile changed"
                    );
                    if observed["active"] == false
                        && observed["transitioning"] == false
                        && observed["processes"] == json!([])
                    {
                        break observed;
                    }
                    ensure!(
                        Instant::now() < deadline,
                        "Fixture did not settle for cleanup"
                    );
                    thread::sleep(Duration::from_millis(100));
                };
                let outcome = registration::retire(&service, &profile, &observed)?;
                ensure!(
                    matches!(outcome, RetirementOutcome::Retired { .. }),
                    "Cleanup unconfirmed: {outcome:?}"
                );
                ensure!(
                    registration::is_retired(&service)?,
                    "Cleanup absence unconfirmed"
                );
                Ok(json!({"role":role,"retired":true,"outcome":outcome}))
            })();
            match result {
                Ok(row) => rows.push(row),
                Err(error) => errors.push(format!("{role}: {error:#}")),
            }
        }
        for name in ["a-heartbeat.json", "u-entry.json", "b-heartbeat.json"] {
            let path = plan.root.join(name);
            if path.exists() {
                match read(&path).and_then(|v| process::matches(&v["identity"])) {
                    Ok(false) => (),
                    Ok(true) => errors.push(format!("Recorded generation remains: {name}")),
                    Err(error) => {
                        errors.push(format!("Cannot confirm generation exit {name}: {error:#}"))
                    }
                }
            }
        }
        Ok(json!({"services":rows,"errors":errors,"complete":errors.is_empty()}))
    }
    fn run(output: &Path) -> Result<()> {
        ensure!(
            output.is_absolute() && !output.exists(),
            "Use a fresh absolute receipt path"
        );
        ensure!(
            std::env::var("CONSTELLATION_INSTALLATION_SMOKE").as_deref() == Ok("1"),
            "Explicit disposable service opt-in required"
        );
        let source =
            std::env::var("NATIVE_OPERATION_SOURCE").context("Exact source revision required")?;
        let executable = std::env::current_exe()?.canonicalize()?;
        let mut cases = Vec::new();
        for detached in [true, false] {
            let id = uuid::Uuid::new_v4().to_string();
            let parent = std::env::var_os("NATIVE_OPERATION_TEST_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .canonicalize()?;
            let root = parent.join(format!("native-operation-{id}"));
            amplifier_native_platform::private_create_dir(&root)?;
            let plan = Plan {
                id,
                root,
                executable: executable.clone(),
                executable_sha256: hash(&executable)?,
                source: source.clone(),
                detached,
            };
            plan.validate()?;
            amplifier_native_platform::private_create(
                &plan.root.join("plan.json"),
                &serde_json::to_vec_pretty(&plan)?,
            )?;
            let result = scenario(&plan).map_err(|e| format!("{e:#}"));
            let cleaned = cleanup(&plan).map_err(|e| format!("{e:#}"));
            let passed = result.is_ok() && cleaned.as_ref().is_ok_and(|v| v["complete"] == true);
            let mut role_errors = serde_json::Map::new();
            for role in ["a", "u", "b"] {
                let path = plan.root.join(format!("{role}-error.json"));
                if path.exists() {
                    role_errors.insert(
                        role.into(),
                        read(&path).unwrap_or_else(|e| json!({"unreadable":e.to_string()})),
                    );
                }
            }
            cases.push(json!({"plan":plan,"result":result,"cleanup":cleaned,"passed":passed,"role_errors":role_errors,
                "root_retained_for_evidence":true}));
            write(
                output,
                &json!({"source":source,"executable_sha256":hash(&executable)?,"cases":cases,"complete":false}),
            )?;
            if !passed {
                break;
            }
        }
        let passed = cases.len() == 2 && cases.iter().all(|v| v["passed"] == true);
        write(
            output,
            &json!({"source":source,"executable_sha256":hash(&executable)?,"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
            "cases":cases,"complete":true,"passed":passed,"limits":["Actual disposable service lifetime; no app bundle swap, browser, model, sign-in or production qualification","Only own fixed UUID profiles were registered; no policy change or global manager setup"]}),
        )?;
        ensure!(
            passed,
            "Native operation lifetime fixture failed; retained exact result at {}",
            output.display()
        );
        Ok(())
    }
    pub fn main() -> Result<()> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.as_slice() {
            [verb, output] if verb == "run" => run(Path::new(output)),
            [verb, selected, root]
                if verb == "role" && ["a", "u", "b"].contains(&selected.as_str()) =>
            {
                let plan: Plan =
                    serde_json::from_slice(&fs::read(Path::new(root).join("plan.json"))?)?;
                ensure!(
                    plan.root == Path::new(root),
                    "Fixture root differs from plan"
                );
                plan.validate()?;
                let result = role(&plan, selected);
                if let Err(error) = &result {
                    let _ = write(
                        &plan.root.join(format!("{selected}-error.json")),
                        &json!({"error":format!("{error:#}")}),
                    );
                }
                result
            }
            _ => bail!("Expected run OUTPUT or fixed role a|u|b ROOT"),
        }
    }
}
#[cfg(feature = "installation")]
fn main() -> anyhow::Result<()> {
    fixture::main()
}
#[cfg(not(feature = "installation"))]
fn main() {
    eprintln!("This disposable fixture requires the installation feature");
}
