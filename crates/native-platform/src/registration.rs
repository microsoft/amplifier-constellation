//! Fresh, no-start registration of an owner-enrolled user service.
//!
//! This is setup, not a new command API: the caller selects a protected fixed
//! profile. Existing definitions are only observed; a mismatch or independent
//! pause is never repaired by overwriting, enabling, unmasking, or reloading it.
use crate::installation::{self, Service, ServiceManager};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "retirement.rs"]
mod retirement;
pub use retirement::{RetirementOutcome, is_retired, retire};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub description: String,
    #[serde(default)]
    pub autostart: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RegistrationOutcome {
    Registered { created: bool },
    Refused { code: String, message: String },
    Unknown { code: String, message: String },
}
fn refused(code: &str, message: &str) -> RegistrationOutcome {
    RegistrationOutcome::Refused {
        code: code.into(),
        message: message.into(),
    }
}
fn unconfirmed(error: impl std::fmt::Display) -> RegistrationOutcome {
    RegistrationOutcome::Unknown {
        code: "registration_unconfirmed".into(),
        message: format!(
            "{error}; inspect the existing definition before deciding whether to retry"
        ),
    }
}
impl Registration {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.executable.is_absolute() && self.executable.is_file(),
            "Registration executable must be an installed absolute file"
        );
        ensure!(
            self.cwd.is_absolute() && self.cwd.is_dir(),
            "Registration working directory must exist and be absolute"
        );
        ensure!(
            !self.description.is_empty(),
            "Registration description is required"
        );
        ensure!(self.args.len() <= 256, "Too many fixed service arguments");
        for value in std::iter::once(self.description.as_str())
            .chain(self.args.iter().map(String::as_str))
            .chain([text(&self.executable)?, text(&self.cwd)?])
        {
            ensure!(
                !value.chars().any(char::is_control),
                "Service definitions cannot contain control characters"
            );
        }
        ensure!(
            self.args.iter().map(String::len).sum::<usize>() + self.description.len() <= 64 * 1024,
            "Fixed service profile is too large"
        );
        Ok(())
    }
}
fn text(path: &Path) -> Result<&str> {
    path.to_str()
        .context("Service path must be representable as text")
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn systemd_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}
fn systemd_exec_quote(value: &str) -> String {
    systemd_quote(value).replace('$', "$$")
}
#[cfg(any(windows, test))]
fn windows_quote(value: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            result.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
            result.push('"');
        } else {
            result.extend(std::iter::repeat_n('\\', slashes));
            result.push(ch);
        }
        slashes = 0;
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}
fn unix_definition(service: &Service, profile: &Registration) -> Result<String> {
    Ok(match service.manager {
        ServiceManager::SystemdUser => {
            let command = std::iter::once(text(&profile.executable)?)
                .chain(profile.args.iter().map(String::as_str))
                .map(systemd_exec_quote)
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "[Unit]\nDescription={}\n[Service]\nType=simple\nExecStart={}\nWorkingDirectory={}\nRestart=no\nUMask=0077\n{}",
                profile.description.replace('%', "%%"),
                command,
                text(&profile.cwd)?.replace('%', "%%"),
                if profile.autostart {
                    "[Install]\nWantedBy=default.target\n"
                } else {
                    ""
                }
            )
        }
        ServiceManager::LaunchAgent => {
            let args = std::iter::once(text(&profile.executable)?)
                .chain(profile.args.iter().map(String::as_str))
                .map(|value| format!("<string>{}</string>", xml(value)))
                .collect::<String>();
            format!(
                "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>{}</string><key>ProgramArguments</key><array>{}</array><key>WorkingDirectory</key><string>{}</string><key>RunAtLoad</key><{}/><key>KeepAlive</key><false/></dict></plist>\n",
                xml(&service.name),
                args,
                xml(text(&profile.cwd)?),
                if profile.autostart { "true" } else { "false" }
            )
        }
        ServiceManager::ScheduledTask => {
            anyhow::bail!("Scheduled Tasks require the native registration path")
        }
    })
}
#[cfg(any(windows, test))]
pub(crate) fn windows_arguments(profile: &Registration) -> String {
    profile
        .args
        .iter()
        .map(|arg| windows_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(any(windows, test))]
fn task_xml(service: &Service, profile: &Registration, sid: &str) -> Result<String> {
    let args = windows_arguments(profile);
    let trigger = if profile.autostart {
        format!(
            "<Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{}</UserId></LogonTrigger></Triggers>",
            xml(sid)
        )
    } else {
        "<Triggers/>".into()
    };
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-16"?><Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><RegistrationInfo><Description>{}</Description><URI>\{}</URI></RegistrationInfo>{}<Principals><Principal id="Author"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><Enabled>true</Enabled><ExecutionTimeLimit>PT0S</ExecutionTimeLimit></Settings><Actions Context="Author"><Exec><Command>{}</Command><Arguments>{}</Arguments><WorkingDirectory>{}</WorkingDirectory></Exec></Actions></Task>"#,
        xml(&profile.description),
        xml(&service.name),
        trigger,
        xml(sid),
        xml(text(&profile.executable)?),
        xml(&args),
        xml(text(&profile.cwd)?)
    ))
}
fn destination(service: &Service) -> Result<PathBuf> {
    let path = service
        .definition_path
        .as_ref()
        .context("Fresh registration requires an enrolled definition_path")?;
    let expected = match service.manager {
        ServiceManager::SystemdUser => crate::user_home()?
            .join(".config/systemd/user")
            .join(&service.name),
        ServiceManager::LaunchAgent => crate::user_home()?
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", service.name)),
        ServiceManager::ScheduledTask => path.clone(),
    };
    ensure!(
        *path == expected,
        "Use this user's fixed service-definition location"
    );
    ensure!(
        !service.name.contains(['\\', '/']),
        "Fresh registration uses one per-installation service name, not a task folder"
    );
    Ok(path.clone())
}
fn existing_file(path: &Path, expected: &[u8]) -> Result<Option<RegistrationOutcome>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Ok(Some(refused(
                "existing_definition",
                "Existing mask, link, or non-file definition was preserved",
            )))
        }
        Ok(metadata) => {
            ensure!(
                metadata.len() <= 2 * 1024 * 1024,
                "Existing service definition exceeds observation limit"
            );
            if fs::read(path)? == expected {
                Ok(Some(RegistrationOutcome::Registered { created: false }))
            } else {
                Ok(Some(refused(
                    "definition_mismatch",
                    "Existing service definition differs and was preserved",
                )))
            }
        }
    }
}
fn prepare_parent(path: &Path) -> Result<()> {
    let parent = path.parent().context("Definition parent is missing")?;
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    let metadata = fs::symlink_metadata(parent)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Definition parent must be an existing real directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o022 == 0,
            "Definition directory is not controlled by this user"
        );
    }
    Ok(())
}

/// Prove that an already observed service launches this protected fixed profile.
/// This is read-only: it does not register, reload, enable, or repair anything.
/// Independent enabled/masked state is deliberately not normalized away.
pub fn profile_matches(
    service: &Service,
    profile: &Registration,
    snapshot: &Value,
) -> Result<bool> {
    service.validate()?;
    profile.validate()?;
    if snapshot["service"] != serde_json::to_value(service)?
        || snapshot["definition_matches"] != true
    {
        return Ok(false);
    }
    match service.manager {
        ServiceManager::SystemdUser | ServiceManager::LaunchAgent => {
            let expected = unix_definition(service, profile)?.into_bytes();
            let matches = snapshot["definition"]["bytes"] == serde_json::to_value(expected)?;
            Ok(matches
                && (service.manager != ServiceManager::SystemdUser
                    || snapshot["native"]["DropInPaths"] == ""))
        }
        #[cfg(windows)]
        ServiceManager::ScheduledTask => task_profile_matches(profile, snapshot),
        #[cfg(not(windows))]
        ServiceManager::ScheduledTask => Ok(false),
    }
}
#[cfg(any(windows, test))]
fn task_profile_matches(profile: &Registration, snapshot: &Value) -> Result<bool> {
    let Some(actions) = snapshot["native"]["actions"].as_array() else {
        return Ok(false);
    };
    if actions.len() != 1 {
        return Ok(false);
    }
    let action = &actions[0];
    Ok(action["type"] == 0
        && action["path"] == text(&profile.executable)?
        && action["args"] == windows_arguments(profile)
        && action["cwd"] == text(&profile.cwd)?)
}

/// Register only this fixed owner profile; never start it. Ambiguous failure
/// after publication remains Unknown. An existing registration receives no writes.
pub fn register(service: &Service, profile: &Registration) -> Result<RegistrationOutcome> {
    service.validate()?;
    profile.validate()?;
    #[cfg(windows)]
    if service.manager == ServiceManager::ScheduledTask {
        return register_task(service, profile, service.definition_path.as_deref());
    }
    let path = destination(service)?;
    let expected = unix_definition(service, profile)?;
    if let Some(outcome) = existing_file(&path, expected.as_bytes())? {
        if matches!(outcome, RegistrationOutcome::Registered { .. }) {
            let observed = installation::snapshot(service)?;
            if !profile_matches(service, profile, &observed)? {
                return Ok(refused(
                    "effective_definition_mismatch",
                    "Existing effective definition or overrides were preserved",
                ));
            }
        }
        return Ok(outcome);
    }
    match service.manager {
        ServiceManager::SystemdUser => {
            let observed = installation::execute(installation::systemctl(&[
                "show",
                "--property=LoadState,FragmentPath",
                "--",
                &service.name,
            ]))?;
            let absent = observed
                .stdout
                .lines()
                .any(|line| line == "LoadState=not-found");
            if !absent {
                ensure!(
                    observed.status.success(),
                    "Could not establish service absence: {}",
                    observed.stderr.trim()
                );
                return Ok(refused(
                    "existing_service",
                    "A loaded, installed, or masked service already owns this name",
                ));
            }
        }
        #[cfg(target_os = "macos")]
        ServiceManager::LaunchAgent => {
            let domain = format!("gui/{}", unsafe { libc::geteuid() });
            let mut disabled = std::process::Command::new("/bin/launchctl");
            disabled.args(["print-disabled", &domain]);
            let text = installation::checked(disabled)?;
            if installation::disabled_override(&text, &service.name)? == Some(true) {
                return Ok(refused(
                    "independently_disabled",
                    "Existing launchd disable override was preserved",
                ));
            }
            let mut query = std::process::Command::new("/bin/launchctl");
            query.args(["print", &format!("{domain}/{}", service.name)]);
            let observed = installation::execute(query)?;
            if observed.status.success() {
                return Ok(refused(
                    "existing_service",
                    "A loaded LaunchAgent already owns this label",
                ));
            }
            ensure!(
                observed.status.code() == Some(113),
                "Could not establish LaunchAgent absence: {}",
                observed.stderr.trim()
            );
        }
        #[allow(unreachable_patterns)]
        _ => anyhow::bail!("Registration is unsupported on this platform"),
    }
    prepare_parent(&path)?;
    // No clobber publication. Once this begins, errors can follow a committed
    // write; do not convert them into permission to replay the registration.
    let result = (|| -> Result<()> {
        crate::private_create(&path, expected.as_bytes())?;
        if service.manager == ServiceManager::SystemdUser {
            installation::checked(installation::systemctl(&["daemon-reload"]))?;
            if profile.autostart {
                installation::checked(installation::systemctl(&["enable", "--", &service.name]))?;
            }
        }
        // A LaunchAgent plist is deliberately not bootstrapped here.
        Ok(())
    })();
    Ok(match result {
        Ok(()) => RegistrationOutcome::Registered { created: true },
        Err(error) => unconfirmed(error),
    })
}

#[cfg(windows)]
fn normalized_task_definition(
    definition: &windows::Win32::System::TaskScheduler::ITaskDefinition,
    current_sid: &str,
) -> Result<Option<windows::core::BSTR>> {
    use windows::{
        Win32::System::TaskScheduler::{ILogonTrigger, TASK_TRIGGER_LOGON, TASK_TRIGGER_TYPE2},
        core::{BSTR, Interface},
    };
    // ITaskDefinition is a detached description. Changes become persistent only
    // through RegisterTaskDefinition, which the existing-task branch never calls.
    unsafe {
        let triggers = definition.Triggers()?;
        let mut count = 0;
        triggers.Count(&mut count)?;
        ensure!((0..=256).contains(&count), "Unexpected task trigger count");
        for index in 1..=count {
            let trigger = triggers.get_Item(index)?;
            let mut kind = TASK_TRIGGER_TYPE2::default();
            trigger.Type(&mut kind)?;
            if kind == TASK_TRIGGER_LOGON {
                let logon: ILogonTrigger = trigger.cast()?;
                let mut account = BSTR::new();
                logon.UserId(&mut account)?;
                let account = String::from_utf16(&account)?;
                if !account.is_empty() {
                    let sid = crate::installation_windows::account_sid(&account)?;
                    if sid != current_sid {
                        return Ok(None);
                    }
                    logon.SetUserId(&BSTR::from(sid))?;
                }
            }
        }
        let mut xml = BSTR::new();
        definition.XmlText(&mut xml)?;
        Ok(Some(xml))
    }
}

#[cfg(windows)]
fn register_task(
    service: &Service,
    profile: &Registration,
    path: Option<&Path>,
) -> Result<RegistrationOutcome> {
    use windows::{
        Win32::System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            TaskScheduler::{
                ITaskService, TASK_CREATE, TASK_IGNORE_REGISTRATION_TRIGGERS,
                TASK_LOGON_INTERACTIVE_TOKEN, TaskScheduler,
            },
            Variant::VARIANT,
        },
        core::{BSTR, HRESULT},
    };
    let service = service.clone();
    let profile = profile.clone();
    let path = path.map(Path::to_owned);
    let result = crate::installation_windows::bounded(move |expired| {
        crate::installation_windows::live(expired)?;
        let sid = crate::current_user_sid()?;
        let xml = task_xml(&service, &profile, &sid)?;
        let bytes: Vec<u8> = std::iter::once(0xfeff)
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        let file = path
            .as_ref()
            .map(|path| existing_file(path, &bytes))
            .transpose()?
            .flatten();
        if let Some(ref outcome) = file
            && !matches!(outcome, RegistrationOutcome::Registered { .. })
        {
            return Ok(outcome.clone());
        }
        unsafe {
            let scheduler: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let empty = VARIANT::default();
            scheduler.Connect(&empty, &empty, &empty, &empty)?;
            let root = scheduler.GetFolder(&BSTR::from("\\"))?;
            let desired = scheduler.NewTask(0)?;
            desired.SetXmlText(&BSTR::from(&xml))?;
            let normalized = normalized_task_definition(&desired, &sid)?
                .context("Configured logon trigger is not for this user")?;
            match root.GetTask(&BSTR::from(&service.name)) {
                Ok(existing) => {
                    let actual = normalized_task_definition(&existing.Definition()?, &sid)?;
                    if actual.as_ref() == Some(&normalized) && (path.is_none() || file.is_some()) {
                        return Ok(RegistrationOutcome::Registered { created: false });
                    }
                    return Ok(refused(
                        "existing_service",
                        "Existing task definition or independent availability was preserved",
                    ));
                }
                Err(error) if error.code() == HRESULT::from_win32(2) => {}
                Err(error) => return Err(error).context("Cannot establish task absence"),
            }
            if file.is_some() {
                return Ok(refused(
                    "existing_definition",
                    "Existing exported definition has no task; inspect the previous registration outcome",
                ));
            }
            if let Some(path) = &path {
                prepare_parent(path)?;
                crate::installation_windows::live(expired)?;
                crate::private_create(path, &bytes)?;
            }
            crate::installation_windows::live(expired)?;
            // CREATE only: never UPDATE/CREATE_OR_UPDATE. No Run call, registration
            // trigger, or time trigger; logon is optional and applies in the future.
            root.RegisterTaskDefinition(
                &BSTR::from(&service.name),
                &desired,
                TASK_CREATE.0 | TASK_IGNORE_REGISTRATION_TRIGGERS.0,
                &empty,
                &empty,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &empty,
            )?;
            Ok(RegistrationOutcome::Registered { created: true })
        }
    });
    // A COM timeout can follow publication/registration. Preserve uncertainty;
    // neither caller nor this function may convert it into automatic redispatch.
    Ok(match result {
        Ok(outcome) => outcome,
        Err(error) => unconfirmed(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Registration) {
        let temp = tempfile::tempdir().unwrap();
        let profile = Registration {
            executable: std::env::current_exe().unwrap(),
            args: vec![
                "a space".into(),
                "quote\" and trailing\\".into(),
                "$HOME %value%".into(),
            ],
            cwd: temp.path().to_owned(),
            description: "Disposable & exact".into(),
            autostart: false,
        };
        (temp, profile)
    }
    #[test]
    fn no_start_templates_preserve_args_and_only_opt_in_autostart() {
        let (_temp, mut profile) = fixture();
        let mut service = Service {
            manager: ServiceManager::SystemdUser,
            name: "test.service".into(),
            definition_path: None,
        };
        let unit = unix_definition(&service, &profile).unwrap();
        assert!(!unit.contains("[Install]"));
        assert!(unit.contains("$$HOME %%value%%"));
        assert!(unit.contains("Restart=no"));
        profile.autostart = true;
        assert!(
            unix_definition(&service, &profile)
                .unwrap()
                .contains("WantedBy=default.target")
        );
        service.manager = ServiceManager::LaunchAgent;
        profile.autostart = false;
        let plist = unix_definition(&service, &profile).unwrap();
        assert!(plist.contains("<key>RunAtLoad</key><false/>"));
        assert!(plist.contains("<key>KeepAlive</key><false/>"));
        assert!(plist.contains("quote&quot; and trailing\\"));
        let task = task_xml(&service, &profile, "S-1-5-21-test").unwrap();
        assert!(task.contains("<Triggers/>"));
        assert!(!task.contains("RegistrationTrigger"));
        assert!(task.contains("<Enabled>true</Enabled>"));
        profile.autostart = true;
        assert!(
            task_xml(&service, &profile, "S-1-5-21-test")
                .unwrap()
                .contains("<LogonTrigger>")
        );
    }
    #[test]
    fn windows_argument_escaping_keeps_empty_quotes_and_trailing_slashes() {
        assert_eq!(windows_quote(""), "\"\"");
        assert_eq!(windows_quote("a\\"), "\"a\\\\\"");
        assert_eq!(windows_quote("a\"b"), "\"a\\\"b\"");
    }
    #[test]
    fn working_directory_keeps_literal_dollar_while_exec_arguments_escape_it() {
        let (temp, mut profile) = fixture();
        profile.cwd = temp.path().join("literal $cwd");
        fs::create_dir(&profile.cwd).unwrap();
        let service = Service {
            manager: ServiceManager::SystemdUser,
            name: "test.service".into(),
            definition_path: None,
        };
        let unit = unix_definition(&service, &profile).unwrap();
        let cwd = unit
            .lines()
            .find(|line| line.starts_with("WorkingDirectory="))
            .unwrap();
        assert!(cwd.contains("literal $cwd"));
        assert!(!cwd.contains("$$cwd"));
        assert!(unit.contains("$$HOME %%value%%"));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn actual_systemd_parser_rejects_quoted_cwd_and_accepts_literal_whole_path() {
        use std::process::Command;
        let analyzer = Path::new("/usr/bin/systemd-analyze");
        if !analyzer.is_file() {
            eprintln!("systemd-analyze unavailable; actual parser case not exercised");
            return;
        }
        let (temp, mut profile) = fixture();
        profile.cwd = temp.path().join("literal $cwd %mark");
        fs::create_dir(&profile.cwd).unwrap();
        let runtime = temp.path().join("runtime");
        crate::private_dir(&runtime).unwrap();
        let service = Service {
            manager: ServiceManager::SystemdUser,
            name: "after.service".into(),
            definition_path: None,
        };
        let valid = unix_definition(&service, &profile).unwrap();
        let valid_value = text(&profile.cwd).unwrap().replace('%', "%%");
        let invalid = valid.replace(
            &format!("WorkingDirectory={valid_value}\n"),
            &format!(
                "WorkingDirectory={}\n",
                systemd_quote(text(&profile.cwd).unwrap())
            ),
        );
        assert_ne!(valid, invalid);
        for (name, contents, expected_success) in [
            ("before.service", invalid, false),
            ("after.service", valid, true),
        ] {
            let path = temp.path().join(name);
            fs::write(&path, contents).unwrap();
            let result = Command::new(analyzer)
                .args(["--user", "--man=no", "verify"])
                .arg(&path)
                .env("XDG_RUNTIME_DIR", &runtime)
                .env("LC_ALL", "C")
                .output()
                .unwrap();
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert_eq!(
                result.status.success(),
                expected_success,
                "{name}: {diagnostic}"
            );
            if !expected_success {
                assert!(
                    diagnostic.contains("WorkingDirectory= path is not absolute"),
                    "{diagnostic}"
                );
            }
        }
    }
    #[test]
    fn windows_profile_match_requires_one_exact_exec_and_preserves_pause() {
        let (_temp, profile) = fixture();
        let mut view = serde_json::json!({"enabled":false,"native":{"actions":[{"type":0,"id":"action","path":profile.executable,"args":windows_arguments(&profile),"cwd":profile.cwd}]}});
        assert!(task_profile_matches(&profile, &view).unwrap());
        view["native"]["actions"][0]["args"] = serde_json::json!("changed");
        assert!(!task_profile_matches(&profile, &view).unwrap());
        view["native"]["actions"] = serde_json::json!([]);
        assert!(!task_profile_matches(&profile, &view).unwrap());
    }
    #[cfg(windows)]
    fn detached_logon_definition(
        check: impl FnOnce(
            &windows::Win32::System::TaskScheduler::ITaskDefinition,
            &windows::Win32::System::TaskScheduler::ILogonTrigger,
            &str,
            &windows::core::BSTR,
        ),
    ) {
        use windows::{
            Win32::System::{
                Com::{
                    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                    CoUninitialize,
                },
                TaskScheduler::{ILogonTrigger, ITaskService, TaskScheduler},
                Variant::VARIANT,
            },
            core::{BSTR, Interface},
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
            let (_temp, mut profile) = fixture();
            profile.autostart = true;
            let service = Service {
                manager: ServiceManager::ScheduledTask,
                name: "detached-normalization-test".into(),
                definition_path: None,
            };
            let sid = crate::current_user_sid().unwrap();
            let definition = scheduler.NewTask(0).unwrap();
            definition
                .SetXmlText(&BSTR::from(task_xml(&service, &profile, &sid).unwrap()))
                .unwrap();
            let expected = normalized_task_definition(&definition, &sid)
                .unwrap()
                .unwrap();
            let logon: ILogonTrigger = definition
                .Triggers()
                .unwrap()
                .get_Item(1)
                .unwrap()
                .cast()
                .unwrap();
            // NewTask and property setters operate on detached descriptions.
            // No RegisterTaskDefinition, existing task lookup, or service action.
            check(&definition, &logon, &sid, &expected);
        }
    }
    #[cfg(windows)]
    #[test]
    fn logon_normalization_preserves_any_user_as_unequal() {
        use windows::core::BSTR;
        detached_logon_definition(|definition, logon, sid, expected| unsafe {
            logon.SetUserId(&BSTR::new()).unwrap();
            let mut before = BSTR::new();
            definition.XmlText(&mut before).unwrap();
            assert_ne!(&before, expected);
            let actual = normalized_task_definition(definition, sid)
                .unwrap()
                .unwrap();
            assert_ne!(
                &actual, expected,
                "Any-user trigger must not match this user"
            );
            assert_eq!(actual, before, "An empty identity must not be filled in");
        });
    }
    #[cfg(windows)]
    #[test]
    fn logon_normalization_refuses_a_different_user_without_editing_it() {
        use windows::core::BSTR;
        detached_logon_definition(|definition, logon, sid, _| unsafe {
            let other_sid = if sid == "S-1-5-18" {
                "S-1-5-19"
            } else {
                "S-1-5-18"
            };
            logon.SetUserId(&BSTR::from(other_sid)).unwrap();
            let mut before = BSTR::new();
            definition.XmlText(&mut before).unwrap();
            assert!(
                normalized_task_definition(definition, sid)
                    .unwrap()
                    .is_none()
            );
            let mut after = BSTR::new();
            definition.XmlText(&mut after).unwrap();
            assert_eq!(after, before, "A different identity must be preserved");
        });
    }
    #[cfg(windows)]
    #[test]
    fn logon_normalization_preserves_changed_delay_as_unequal() {
        use windows::core::BSTR;
        detached_logon_definition(|definition, logon, sid, expected| unsafe {
            logon.SetDelay(&BSTR::from("PT1M")).unwrap();
            let mut before = BSTR::new();
            definition.XmlText(&mut before).unwrap();
            assert_ne!(&before, expected);
            let actual = normalized_task_definition(definition, sid)
                .unwrap()
                .unwrap();
            assert_ne!(&actual, expected, "Changed timing must remain a mismatch");
            assert_eq!(actual, before, "Normalization must preserve trigger timing");
        });
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn unix_profile_match_requires_explicit_empty_overrides_and_exact_bytes() {
        let (_temp, profile) = fixture();
        let service = Service {
            manager: ServiceManager::SystemdUser,
            name: "test.service".into(),
            definition_path: None,
        };
        let mut view = serde_json::json!({"service":service,"definition_matches":true,"enabled":false,"definition":{"bytes":unix_definition(&service,&profile).unwrap().into_bytes()},"native":{"DropInPaths":""}});
        assert!(profile_matches(&service, &profile, &view).unwrap());
        view["native"]["DropInPaths"] = Value::Null;
        assert!(!profile_matches(&service, &profile, &view).unwrap());
        view["native"]["DropInPaths"] = serde_json::json!("/owner/override.conf");
        assert!(!profile_matches(&service, &profile, &view).unwrap());
        view["native"]["DropInPaths"] = serde_json::json!("");
        view["definition"]["bytes"] = serde_json::json!([1]);
        assert!(!profile_matches(&service, &profile, &view).unwrap());
    }
    #[test]
    fn existing_files_are_observed_not_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("definition");
        assert_eq!(existing_file(&path, b"exact").unwrap(), None);
        crate::private_create(&path, b"exact").unwrap();
        assert_eq!(
            existing_file(&path, b"exact").unwrap(),
            Some(RegistrationOutcome::Registered { created: false })
        );
        assert!(matches!(
            existing_file(&path, b"different").unwrap(),
            Some(RegistrationOutcome::Refused { .. })
        ));
        assert_eq!(fs::read(path).unwrap(), b"exact");
    }
}
