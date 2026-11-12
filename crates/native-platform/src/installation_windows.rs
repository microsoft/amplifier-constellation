//! Current-user Task Scheduler observation without a PowerShell host process.
//! The caller retains the service envelope, double snapshot and restore policy.

use crate::installation::{ActionOutcome, ServiceAction};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCancelCall, CoCreateInstance,
            CoDisableCallCancellation, CoEnableCallCancellation, CoInitializeEx, CoUninitialize,
        },
        TaskScheduler::{
            IExecAction, IRegisteredTask, IRunningTask, ITaskService, TASK_ACTION_EXEC,
            TASK_ACTION_TYPE, TASK_ENUM_HIDDEN, TASK_LOGON_TYPE, TASK_RUNLEVEL_TYPE, TaskScheduler,
        },
        Variant::VARIANT,
    },
    core::{BSTR, Interface},
};

const CALL_LIMIT: Duration = Duration::from_secs(8);
// A timed-out COM call may still be unwinding. Do not accumulate background
// manager calls or dispatch a second effect while its settlement is unknown.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct ActiveCall;
impl Drop for ActiveCall {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, Ordering::Release);
    }
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            let _ = CoDisableCallCancellation(None);
            CoUninitialize();
        }
    }
}

pub(crate) fn bounded<T: Send + 'static>(
    operation: impl FnOnce(&AtomicBool) -> Result<T> + Send + 'static,
) -> Result<T> {
    ensure!(
        IN_FLIGHT
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        "Previous native Task Scheduler observation or dispatch is still settling"
    );
    let deadline = Instant::now() + CALL_LIMIT;
    let expired = Arc::new(AtomicBool::new(false));
    let worker_expired = expired.clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel::<()>();
    let active = ActiveCall;
    thread::Builder::new()
        .name("native-task-manager".into())
        .spawn(move || {
            let thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
            let _ = ready_tx.send(thread_id);
            let result = (|| -> Result<T> {
                unsafe {
                    CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
                    // Cancellation remains best-effort. Timeout does not prove
                    // a manager mutation failed or permit another dispatch.
                    if let Err(error) = CoEnableCallCancellation(None) {
                        CoUninitialize();
                        return Err(error.into());
                    }
                }
                let _apartment = Apartment;
                live(&worker_expired)?;
                operation(&worker_expired)
            })();
            // The COM apartment and all its objects have settled before the
            // result becomes visible, so the next snapshot can start at once.
            drop(active);
            let _ = result_tx.send(result);
            // Retain this exact thread until the caller has either consumed the
            // result or requested cancellation, avoiding a recycled thread ID.
            let _ = finish_rx.recv();
        })
        .context("Start native Task Scheduler observation")?;
    let thread_id = match ready_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    {
        Ok(id) => id,
        Err(error) => {
            expired.store(true, Ordering::Release);
            drop(finish_tx);
            return Err(error).context("Native Task Scheduler worker did not start in time");
        }
    };
    let result = result_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    match result {
        Ok(result) => {
            drop(finish_tx);
            result
        }
        Err(error) => {
            expired.store(true, Ordering::Release);
            // Zero means do not add a further wait to the existing eight-second
            // bound. The worker checks expiry again before a service effect.
            if matches!(error, mpsc::RecvTimeoutError::Timeout) {
                unsafe {
                    let _ = CoCancelCall(thread_id, 0);
                }
            }
            drop(finish_tx);
            Err(error).context("Native Task Scheduler exceeded its bound; outcome may be unknown")
        }
    }
}

pub(crate) fn live(expired: &AtomicBool) -> Result<()> {
    ensure!(
        !expired.load(Ordering::Acquire),
        "Native Task Scheduler operation expired"
    );
    Ok(())
}

fn task(name: &str, expired: &AtomicBool) -> Result<(ITaskService, IRegisteredTask)> {
    live(expired)?;
    unsafe {
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        let empty = VARIANT::default();
        service.Connect(&empty, &empty, &empty, &empty)?;
        live(expired)?;
        let root = service.GetFolder(&BSTR::from("\\"))?;
        Ok((service, root.GetTask(&BSTR::from(name))?))
    }
}

fn text(value: BSTR) -> Result<String> {
    Ok(String::from_utf16(&value)?)
}

pub(crate) fn account_sid(account: &str) -> Result<String> {
    if account.starts_with("S-1-") {
        return Ok(account.to_owned());
    }
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, LocalFree},
        Security::{Authorization::ConvertSidToStringSidW, LookupAccountNameW},
    };
    let account: Vec<u16> = account.encode_utf16().chain(Some(0)).collect();
    let mut sid_bytes = 0;
    let mut domain_chars = 0;
    let mut sid_type = 0;
    unsafe {
        LookupAccountNameW(
            ptr::null(),
            account.as_ptr(),
            ptr::null_mut(),
            &mut sid_bytes,
            ptr::null_mut(),
            &mut domain_chars,
            &mut sid_type,
        );
        ensure!(
            std::io::Error::last_os_error().raw_os_error()
                == Some(ERROR_INSUFFICIENT_BUFFER as i32)
                && sid_bytes > 0,
            "Cannot resolve enrolled task principal"
        );
        // SID contains DWORD fields; keep the native buffer suitably aligned.
        let mut sid = vec![0u32; (sid_bytes as usize).div_ceil(4)];
        let mut domain = vec![0u16; domain_chars as usize];
        ensure!(
            LookupAccountNameW(
                ptr::null(),
                account.as_ptr(),
                sid.as_mut_ptr().cast(),
                &mut sid_bytes,
                domain.as_mut_ptr(),
                &mut domain_chars,
                &mut sid_type
            ) != 0,
            "Cannot resolve enrolled task principal"
        );
        let mut rendered = ptr::null_mut();
        ensure!(
            ConvertSidToStringSidW(sid.as_ptr().cast_mut().cast(), &mut rendered) != 0,
            "Cannot read enrolled task principal SID"
        );
        let mut length = 0;
        while *rendered.add(length) != 0 {
            length += 1;
        }
        let result = String::from_utf16(std::slice::from_raw_parts(rendered, length));
        LocalFree(rendered.cast());
        Ok(result?)
    }
}

fn view(
    service: &ITaskService,
    task: &IRegisteredTask,
    expired: &AtomicBool,
) -> Result<(Value, Vec<IRunningTask>)> {
    live(expired)?;
    unsafe {
        let definition = task.Definition()?;
        let principal = definition.Principal()?;
        let mut user = BSTR::new();
        principal.UserId(&mut user)?;
        let owner = account_sid(&text(user)?)?;
        let mut run_level = TASK_RUNLEVEL_TYPE::default();
        principal.RunLevel(&mut run_level)?;
        ensure!(
            owner == crate::current_user_sid()? && run_level.0 == 0,
            "Task is not a least-privilege current-user task"
        );
        let mut logon = TASK_LOGON_TYPE::default();
        principal.LogonType(&mut logon)?;
        let actions = definition.Actions()?;
        let mut action_count = 0;
        actions.Count(&mut action_count)?;
        ensure!(
            (0..=64).contains(&action_count),
            "Unexpected task action count"
        );
        let mut action_views = Vec::with_capacity(action_count as usize);
        for index in 1..=action_count {
            live(expired)?;
            let action = actions.get_Item(index)?;
            let mut id = BSTR::new();
            let mut kind = TASK_ACTION_TYPE::default();
            action.Id(&mut id)?;
            action.Type(&mut kind)?;
            let mut observation = json!({"type":kind.0,"id":text(id)?});
            if kind == TASK_ACTION_EXEC {
                let action: IExecAction = action.cast()?;
                let mut path = BSTR::new();
                let mut args = BSTR::new();
                let mut cwd = BSTR::new();
                action.Path(&mut path)?;
                action.Arguments(&mut args)?;
                action.WorkingDirectory(&mut cwd)?;
                observation["path"] = json!(text(path)?);
                observation["args"] = json!(text(args)?);
                observation["cwd"] = json!(text(cwd)?);
            }
            action_views.push(observation);
        }
        let collection = task.GetInstances(0)?;
        let count = collection.Count()?;
        ensure!(
            (0..=4096).contains(&count),
            "Unexpected task instance count"
        );
        let mut instances = Vec::with_capacity(count as usize);
        for index in 1..=count {
            live(expired)?;
            let running = collection.get_Item(&VARIANT::from(index))?;
            running.Refresh()?;
            let observation = json!({
                "instance_guid":text(running.InstanceGuid()?)?,
                "engine_pid":running.EnginePID()?,
                "state":running.State()?.0,
                "current_action":text(running.CurrentAction()?)?
            });
            instances.push((observation, running));
        }
        if !instances.is_empty() {
            // EnginePID identifies a scheduler engine, which can be shared.
            // Preserve the native GUID association instead of treating an
            // ancestry match to that engine as unique proof of this invocation.
            let wanted: BTreeSet<u64> = instances
                .iter()
                .map(|(value, _)| value["engine_pid"].as_u64().unwrap())
                .collect();
            // Excluding hidden tasks would make engine uniqueness unproven.
            // Unsupported full enumeration remains an error, never a fallback
            // to a smaller list that could silently omit another invocation.
            let running = service.GetRunningTasks(TASK_ENUM_HIDDEN.0)?;
            let count = running.Count()?;
            ensure!(
                (0..=4096).contains(&count),
                "Unexpected global running-task count"
            );
            let mut shared: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
            for index in 1..=count {
                live(expired)?;
                let instance = running.get_Item(&VARIANT::from(index))?;
                instance.Refresh()?;
                let pid = u64::from(instance.EnginePID()?);
                if wanted.contains(&pid) {
                    shared
                        .entry(pid)
                        .or_default()
                        .insert(text(instance.InstanceGuid()?)?);
                }
            }
            for (value, _) in &mut instances {
                let pid = value["engine_pid"].as_u64().unwrap();
                value["engine_instance_guids"] =
                    json!(shared.get(&pid).cloned().unwrap_or_default());
            }
        }
        instances.sort_by(|a, b| {
            a.0["instance_guid"]
                .as_str()
                .cmp(&b.0["instance_guid"].as_str())
        });
        let (observations, handles): (Vec<Value>, Vec<IRunningTask>) =
            instances.into_iter().unzip();
        let observation = json!({
            "path":text(task.Path()?)?, "xml":text(task.Xml()?)?,
            "enabled":task.Enabled()?.as_bool(), "state":task.State()?.0,
            "owner_sid":owner, "logon_type":logon.0,
            "security_descriptor":text(task.GetSecurityDescriptor(7)?)?,
            "instances":observations,"actions":action_views
        });
        live(expired)?;
        Ok((observation, handles))
    }
}

pub(crate) fn observe(name: &str) -> Result<Value> {
    let name = name.to_owned();
    bounded(move |expired| {
        let (service, task) = task(&name, expired)?;
        Ok(view(&service, &task, expired)?.0)
    })
}

pub(crate) fn dispatch(
    name: &str,
    action: ServiceAction,
    expected: &Value,
) -> Result<ActionOutcome> {
    let name = name.to_owned();
    let expected = expected.clone();
    let result = bounded(move |expired| {
        let (service, task) = task(&name, expired)?;
        let (current, instances) = view(&service, &task, expired)?;
        if current != expected {
            return Ok(ActionOutcome::Refused {
                code: "stale_task".into(),
                message: "Task changed before native manager dispatch".into(),
            });
        }
        live(expired)?;
        unsafe {
            match action {
                ServiceAction::Start => {
                    if !task.Enabled()?.as_bool() {
                        return Ok(ActionOutcome::Refused {
                            code: "independently_disabled".into(),
                            message: "Task is independently disabled".into(),
                        });
                    }
                    live(expired)?;
                    let _ = task.Run(&VARIANT::default())?;
                }
                ServiceAction::Stop => {
                    // Act only on the instances whose GUIDs were compared;
                    // do not enumerate and stop a newly appeared generation.
                    for instance in instances {
                        live(expired)?;
                        instance.Stop()?;
                    }
                }
            }
        }
        Ok(ActionOutcome::Accepted)
    });
    Ok(match result {
        Ok(outcome) => outcome,
        Err(error) => ActionOutcome::Unknown {
            code: "native_task_unconfirmed".into(),
            message: error.to_string(),
        },
    })
}
