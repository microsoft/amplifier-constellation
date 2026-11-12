//! Retire only a completed fixed no-autostart registration. Caller-owned locks
//! and the original operation receipt remain required. Native managers provide
//! no atomic snapshot-CAS: a same-owner administrator can race the final read.
use super::{Registration, destination, profile_matches, unix_definition};
use crate::installation::{self, ActionOutcome, CheckedSnapshot, Service, ServiceManager};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RetirementOutcome {
    Retired { removed: bool },
    Refused { code: String, message: String },
    Unknown { code: String, message: String },
}
fn refused(code: &str, message: impl std::fmt::Display) -> RetirementOutcome {
    RetirementOutcome::Refused {
        code: code.into(),
        message: message.to_string(),
    }
}
fn unknown(error: impl std::fmt::Display) -> RetirementOutcome {
    RetirementOutcome::Unknown {
        code: "retirement_unconfirmed".into(),
        message: format!(
            "{error}; retain the original operation receipt and observe; do not redispatch automatically"
        ),
    }
}

fn file_present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("Observe fixed definition presence"),
    }
}
fn definition(path: &Path) -> Result<Value> {
    let metadata = fs::symlink_metadata(path)?;
    let parent_path = path.parent().context("Missing definition parent")?;
    let parent = fs::symlink_metadata(parent_path)?;
    ensure!(
        parent.is_dir() && !crate::installation_exchange::is_pointer(&parent),
        "Definition parent is not a real directory"
    );
    ensure!(
        metadata.is_file() && !crate::installation_exchange::is_pointer(&metadata),
        "Retirement requires an ordinary definition file, never a mask or link"
    );
    ensure!(
        metadata.len() <= 2 * 1024 * 1024,
        "Definition exceeds inspection bound"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o022 == 0,
            "Definition is not controlled by the current user"
        );
        ensure!(
            parent.uid() == unsafe { libc::geteuid() } && parent.mode() & 0o022 == 0,
            "Definition parent is not controlled by the current user"
        );
    }
    #[cfg(windows)]
    {
        crate::validate_private_directory(parent_path)?;
        crate::storage::validate_windows_private_access(path)?;
    }
    let parent_identity = crate::installation_exchange::native_identity(parent_path, &parent)?;
    let identity = crate::installation_exchange::native_identity(path, &metadata)?;
    let bytes = fs::read(path)?;
    ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "Definition exceeds inspection bound"
    );
    let after = fs::symlink_metadata(path)?;
    ensure!(
        identity == crate::installation_exchange::native_identity(path, &after)?
            && after.len() == metadata.len(),
        "Definition changed during observation"
    );
    ensure!(
        parent_identity
            == crate::installation_exchange::native_identity(
                parent_path,
                &fs::symlink_metadata(parent_path)?
            )?,
        "Definition parent changed during observation"
    );
    Ok(json!({"identity":identity,"parent_identity":parent_identity,"bytes":bytes}))
}

fn presence(service: &Service, path: &Path) -> Result<(bool, bool)> {
    let before = file_present(path)?;
    let manager = installation::manager_registration_exists(service)?;
    ensure!(
        before == file_present(path)?,
        "Definition changed during absence observation"
    );
    Ok((manager, before))
}

/// Read-only absence for the service in the caller's preserved operation receipt.
/// True requires explicit native not-found AND a missing definition. An unloaded
/// LaunchAgent plist is a known remaining definition, also the normal no-start
/// state. False cannot distinguish that from unfinished cleanup and grants no
/// ownership/retry permission. Other partial/uninspectable state is an error.
pub fn is_retired(service: &Service) -> Result<bool> {
    service.validate()?;
    let path = destination(service)?;
    match presence(service, &path)? {
        (false, false) => Ok(true),
        (true, true) => Ok(false),
        (false, true) if service.manager == ServiceManager::LaunchAgent => {
            definition(&path)?;
            Ok(false)
        }
        _ => {
            anyhow::bail!("Only part of the registration is absent; preserve the operation receipt")
        }
    }
}

fn inactive(current: &Value, manager: &ServiceManager) -> Option<ActionOutcome> {
    let safe = current["active"] == false
        && current["transitioning"] == false
        && current["masked"] == false
        && current["processes"].as_array().is_some_and(Vec::is_empty)
        && current["start_allowed"] == true
        && match manager {
            ServiceManager::SystemdUser => {
                current["native"]["UnitFileState"] == "static"
                    && current["native"]["DropInPaths"] == ""
                    && current["native"]["ActiveState"] == "inactive"
            }
            ServiceManager::LaunchAgent => current["native"]["disabled_override"].is_null(),
            ServiceManager::ScheduledTask => {
                current["enabled"] == true
                    && current["native"]["instances"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
            }
        };
    (!safe).then(|| ActionOutcome::Refused {
        code: "not_retirable".into(),
        message:
            "Registration is active, transitioning, independently paused/overridden, or unconfirmed"
                .into(),
    })
}

/// Retire a fixed no-autostart profile after its original operation has settled.
/// It never stops, enables, unmasks, repairs, or recursively cleans anything.
/// Callers serialize cooperating writers. Native managers have no atomic CAS;
/// same-owner edits after the final observation remain a documented race.
pub fn retire(
    service: &Service,
    profile: &Registration,
    expected: &Value,
) -> Result<RetirementOutcome> {
    service.validate()?;
    profile.validate()?;
    if profile.autostart {
        return Ok(refused(
            "autostart_profile",
            "Only no-autostart operation profiles may be retired",
        ));
    }
    if expected["service"] != serde_json::to_value(service)?
        || !profile_matches(service, profile, expected)?
    {
        return Ok(refused(
            "profile_mismatch",
            "Expected snapshot is not bound to this fixed profile",
        ));
    }
    let path = destination(service)?;
    if presence(service, &path)? == (false, false) {
        return Ok(RetirementOutcome::Retired { removed: false });
    }
    let file = definition(&path)?;
    if file["bytes"] != expected["definition"]["bytes"] {
        return Ok(refused("definition_mismatch", "Definition bytes changed"));
    }
    let checked = installation::action_snapshot(
        expected,
        || installation::observe(service),
        |current| {
            if !profile_matches(service, profile, current)? {
                return Ok(Some(ActionOutcome::Refused {
                    code: "profile_mismatch".into(),
                    message: "Current profile differs".into(),
                }));
            }
            Ok(inactive(current, &service.manager))
        },
    )?;
    let current = match checked {
        CheckedSnapshot::Confirmed(current) => current,
        CheckedSnapshot::Refused(ActionOutcome::Refused { code, message }) => {
            return Ok(RetirementOutcome::Refused { code, message });
        }
        _ => unreachable!("snapshot validation only refuses"),
    };
    if definition(&path)? != file {
        return Ok(refused(
            "definition_changed",
            "Definition identity or bytes changed",
        ));
    }
    #[cfg(windows)]
    if service.manager == ServiceManager::ScheduledTask {
        return retire_task(service, profile, &current, &file);
    }
    let wanted = unix_definition(service, profile)?.into_bytes();
    if file["bytes"] != serde_json::to_value(wanted)? {
        return Ok(refused(
            "definition_mismatch",
            "Definition does not match the generated fixed profile",
        ));
    }
    retire_unix(service, &path, &current, &file)
}

fn remove_definition(path: &Path, expected: &Value) -> Result<()> {
    ensure!(
        definition(path)? == *expected,
        "Definition changed before removal"
    );
    fs::remove_file(path)?;
    #[cfg(unix)]
    fs::File::open(path.parent().context("Missing parent")?)?.sync_all()?;
    Ok(())
}
fn settle_retirement(mutate: impl FnOnce() -> Result<()>) -> RetirementOutcome {
    match mutate() {
        Ok(()) => RetirementOutcome::Retired { removed: true },
        Err(error) => unknown(error),
    }
}
fn retire_unix(
    service: &Service,
    path: &Path,
    current: &Value,
    file: &Value,
) -> Result<RetirementOutcome> {
    let outcome = settle_retirement(|| -> Result<()> {
        match service.manager {
            ServiceManager::SystemdUser => {
                remove_definition(path, file)?;
                installation::checked(installation::systemctl(&["daemon-reload"]))?;
            }
            #[cfg(target_os = "macos")]
            ServiceManager::LaunchAgent => {
                if current["loaded"] == true {
                    let target = format!("gui/{}/{}", unsafe { libc::geteuid() }, service.name);
                    let mut command = std::process::Command::new("/bin/launchctl");
                    command.args(["bootout", &target]);
                    installation::checked(command)?;
                }
                remove_definition(path, file)?;
            }
            #[allow(unreachable_patterns)]
            _ => anyhow::bail!("Unsupported retirement platform"),
        }
        ensure!(
            is_retired(service)?,
            "Registration remains after retirement"
        );
        Ok(())
    });
    #[cfg(not(target_os = "macos"))]
    let _ = current;
    Ok(outcome)
}

#[cfg(windows)]
fn retire_task(
    service: &Service,
    profile: &Registration,
    current: &Value,
    file: &Value,
) -> Result<RetirementOutcome> {
    use windows::{
        Win32::System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            TaskScheduler::{ITaskService, TaskScheduler},
            Variant::VARIANT,
        },
        core::{BSTR, HRESULT},
    };
    let (service, profile, current, file) = (
        service.clone(),
        profile.clone(),
        current.clone(),
        file.clone(),
    );
    let result = crate::installation_windows::bounded(move |expired| unsafe {
        let live = || crate::installation_windows::live(expired);
        live()?;
        let sid = crate::current_user_sid()?;
        let scheduler: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let empty = VARIANT::default();
        scheduler.Connect(&empty, &empty, &empty, &empty)?;
        let root = scheduler.GetFolder(&BSTR::from("\\"))?;
        let name = BSTR::from(&service.name);
        let task = root.GetTask(&name)?;
        let desired = scheduler.NewTask(0)?;
        let xml = super::task_xml(&service, &profile, &sid)?;
        desired.SetXmlText(&BSTR::from(&xml))?;
        let expected_xml = super::normalized_task_definition(&desired, &sid)?;
        let actual_xml = super::normalized_task_definition(&task.Definition()?, &sid)?;
        let exported: Vec<u8> = std::iter::once(0xfeff)
            .chain(xml.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        if expected_xml.is_none()
            || actual_xml != expected_xml
            || file["bytes"] != serde_json::to_value(exported)?
        {
            return Ok(refused(
                "task_definition_mismatch",
                "Full current-user no-trigger task definition differs",
            ));
        }
        let observed = crate::installation_windows::view(&scheduler, &task, expired)?.0;
        if observed != current["native"] {
            return Ok(refused("stale_snapshot", "Task changed before retirement"));
        }
        let path = service
            .definition_path
            .as_ref()
            .context("Missing exported definition")?;
        if definition(path)? != file {
            return Ok(refused("definition_changed", "Exported definition changed"));
        }
        live()?;
        root.DeleteTask(&name, 0)?;
        live()?;
        remove_definition(path, &file)?;
        live()?;
        match root.GetTask(&name) {
            Err(error) if error.code() == HRESULT::from_win32(2) => (),
            Err(error) => return Err(error).context("Cannot establish task removal"),
            Ok(_) => anyhow::bail!("Task remains after deletion"),
        }
        ensure!(!file_present(path)?, "Exported definition remains");
        Ok(RetirementOutcome::Retired { removed: true })
    });
    Ok(match result {
        Ok(value) => value,
        Err(error) => unknown(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_requires_known_inactive_unpaused_no_process_state() {
        let original = json!({"active":false,"transitioning":false,"masked":false,
            "processes":[],"start_allowed":true,"enabled":true,
            "native":{"UnitFileState":"static","ActiveState":"inactive","DropInPaths":"",
                      "disabled_override":null,"instances":[]}});
        for manager in [
            ServiceManager::SystemdUser,
            ServiceManager::LaunchAgent,
            ServiceManager::ScheduledTask,
        ] {
            assert!(inactive(&original, &manager).is_none());
            for (key, value) in [
                ("active", json!(true)),
                ("active", Value::Null),
                ("transitioning", json!(true)),
                ("masked", json!(true)),
                ("processes", json!([{"pid":123}])),
                ("processes", Value::Null),
                ("start_allowed", json!(false)),
            ] {
                let mut changed = original.clone();
                changed[key] = value;
                assert!(
                    inactive(&changed, &manager).is_some(),
                    "{manager:?} accepted {key}"
                );
            }
        }
        for (manager, key, value) in [
            (
                ServiceManager::SystemdUser,
                "DropInPaths",
                json!("/changed.conf"),
            ),
            (
                ServiceManager::SystemdUser,
                "UnitFileState",
                json!("enabled"),
            ),
            (ServiceManager::SystemdUser, "ActiveState", json!("failed")),
            (
                ServiceManager::LaunchAgent,
                "disabled_override",
                json!(true),
            ),
            (
                ServiceManager::LaunchAgent,
                "disabled_override",
                json!(false),
            ),
            (
                ServiceManager::ScheduledTask,
                "instances",
                json!([{"guid":"still-running"}]),
            ),
        ] {
            let mut changed = original.clone();
            changed["native"][key] = value;
            assert!(inactive(&changed, &manager).is_some());
        }
        let mut disabled = original;
        disabled["enabled"] = json!(false);
        assert!(inactive(&disabled, &ServiceManager::ScheduledTask).is_some());
    }

    fn definition_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("protected");
        crate::private_create_dir(&parent).unwrap();
        let path = parent.join("fixed-definition");
        crate::private_create(&path, b"exact fixed profile").unwrap();
        (temp, path)
    }
    #[test]
    fn same_bytes_replacement_is_not_the_observed_definition() {
        let (_temp, path) = definition_fixture();
        let captured = definition(&path).unwrap();
        let replacement = path.with_file_name("replacement");
        crate::private_create(&replacement, b"exact fixed profile").unwrap();
        fs::remove_file(&path).unwrap();
        fs::rename(replacement, &path).unwrap();
        assert!(remove_definition(&path, &captured).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"exact fixed profile");
    }
    #[test]
    fn failure_after_unlink_is_unknown_without_recreating_or_retrying() {
        let (_temp, path) = definition_fixture();
        let captured = definition(&path).unwrap();
        let outcome = settle_retirement(|| {
            remove_definition(&path, &captured)?;
            anyhow::bail!("injected manager reload failure after unlink")
        });
        assert!(matches!(outcome, RetirementOutcome::Unknown { .. }));
        assert!(!path.exists());
    }
    #[cfg(unix)]
    #[test]
    fn writable_or_linked_parent_is_preserved_and_refused() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (temp, path) = definition_fixture();
        let parent = path.parent().unwrap();
        fs::set_permissions(parent, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(definition(&path).is_err());
        assert_eq!(
            fs::metadata(parent).unwrap().permissions().mode() & 0o777,
            0o777
        );
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
        let linked = temp.path().join("linked");
        symlink(parent, &linked).unwrap();
        assert!(definition(&linked.join("fixed-definition")).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"exact fixed profile");
    }
    #[cfg(windows)]
    #[test]
    fn private_file_in_writable_parent_is_refused_without_acl_repair() {
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::{
                Authorization::{
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
                },
                DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SetFileSecurityW,
            },
        };
        let (_temp, path) = definition_fixture();
        let parent = path.parent().unwrap();
        let sid = crate::current_user_sid().unwrap();
        let sddl: Vec<u16> = format!("D:P(A;;FA;;;{sid})(A;;FA;;;SY)(A;;FA;;;WD)")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = std::ptr::null_mut();
        unsafe {
            assert_ne!(
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    std::ptr::null_mut()
                ),
                0
            );
            let changed = SetFileSecurityW(
                crate::windows_native_path(parent).unwrap().as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor,
            );
            LocalFree(descriptor.cast());
            assert_ne!(changed, 0);
        }
        assert!(definition(&path).is_err());
        // A second inspection must still reject the same broad policy; retirement
        // neither tightens the parent nor removes the protected child.
        assert!(crate::validate_private_directory(parent).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"exact fixed profile");
    }
}
