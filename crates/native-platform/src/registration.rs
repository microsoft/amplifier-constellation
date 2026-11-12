//! Fresh, no-start registration of an owner-enrolled user service.
//!
//! This is setup, not a new command API: the caller selects a protected fixed
//! profile. Existing definitions are only observed; a mismatch or independent
//! pause is never repaired by overwriting, enabling, unmasking, or reloading it.
use crate::installation::{self, Service, ServiceManager};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
            .replace('$', "$$")
    )
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
                .map(systemd_quote)
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "[Unit]\nDescription={}\n[Service]\nType=simple\nExecStart={}\nWorkingDirectory={}\nRestart=no\nUMask=0077\n{}",
                profile.description.replace('%', "%%"),
                command,
                systemd_quote(text(&profile.cwd)?),
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
fn task_xml(service: &Service, profile: &Registration, sid: &str) -> Result<String> {
    let args = profile
        .args
        .iter()
        .map(|arg| windows_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
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
            if observed["definition_matches"] != true
                || (service.manager == ServiceManager::SystemdUser
                    && observed["native"]["DropInPaths"] != "")
            {
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
            let mut normalized = BSTR::new();
            desired.XmlText(&mut normalized)?;
            match root.GetTask(&BSTR::from(&service.name)) {
                Ok(existing) => {
                    let mut actual = BSTR::new();
                    existing.Definition()?.XmlText(&mut actual)?;
                    if actual == normalized && (path.is_none() || file.is_some()) {
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
