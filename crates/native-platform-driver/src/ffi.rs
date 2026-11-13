//! Small synchronous C ABI. Domain transaction and recovery policy stay with callers.

use crate::session::Session;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::UnsafeCell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    ptr,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    thread::{self, ThreadId},
};

const MAX_REQUEST: usize = 16 * 1024 * 1024;
const MAX_RESPONSE: usize = 32 * 1024 * 1024;

#[repr(C)]
pub struct NativeBuffer {
    pub data: *mut u8,
    pub len: usize,
}

struct Entry {
    session: Session,
    owner_thread: Option<ThreadId>,
    poisoned: bool,
}

impl Entry {
    fn dispatch(&mut self, id: &str, operation: impl FnOnce(&mut Session) -> Value) -> Value {
        // Catch before Entry can be dropped. Caller code may still be executing
        // inside a critical section when the error reaches it.
        let response = catch_unwind(AssertUnwindSafe(|| operation(&mut self.session)));
        self.owner_thread = (self.session.held_locks() != 0).then(|| thread::current().id());
        match response {
            Ok(value) => value,
            Err(_) => {
                self.poisoned = true;
                reply(
                    id,
                    "unknown",
                    "native_panic",
                    "Native dispatch panicked; retained locks require explicit release",
                )
            }
        }
    }
}

#[derive(Default)]
struct Registry {
    entries: BTreeMap<u64, Entry>,
    next: u32,
}

// All access to the slot is serialized by GATE. No Rust mutex/guard or allocator
// operation is used by the fork child callback. A no-lock child abandons its
// copied registry without destructors; inherited handles never become usable.
struct Slot(UnsafeCell<Option<Registry>>);
// SAFETY: every read/write of the slot is protected by GATE, including fork
// preparation. The child resets it only while it owns the inherited gate.
unsafe impl Sync for Slot {}
static REGISTRY: Slot = Slot(UnsafeCell::new(None));
static GATE: AtomicBool = AtomicBool::new(false);
static OWNER_PID: AtomicU32 = AtomicU32::new(0);
#[cfg(unix)]
static FORK_PREPARED: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
static FORK_HAS_LOCKS: AtomicBool = AtomicBool::new(false);

struct Gate;
impl Gate {
    fn acquire() -> Self {
        while GATE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        Self
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        GATE.store(false, Ordering::Release);
    }
}

fn reply(id: &str, outcome: &str, code: &str, message: &str) -> Value {
    json!({"version":1,"id":id,"outcome":outcome,"error":{"code":code,"message":message}})
}

fn foreign_process() -> bool {
    let owner = OWNER_PID.load(Ordering::Acquire);
    owner != 0 && owner != std::process::id()
}

fn with_registry(id: &str, f: impl FnOnce(&mut Registry) -> Value) -> Value {
    // This check MUST precede waiting for a gate inherited from a foreign fork.
    if foreign_process() {
        return reply(
            id,
            "refused",
            "forked_session",
            "Fork coordination is required; execute a fresh process",
        );
    }
    OWNER_PID.store(std::process::id(), Ordering::Release);
    // SAFETY: gate owns all access for the duration of this call.
    let registry = unsafe { &mut *REGISTRY.0.get() }.get_or_insert_with(Registry::default);
    f(registry)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenOptions {
    version: u32,
    config: String,
    driver: String,
}

fn open(options: &[u8], handle: &mut u64) -> Value {
    let parsed: OpenOptions = match serde_json::from_slice(options) {
        Ok(value) => value,
        Err(error) => {
            return reply(
                "native.open",
                "refused",
                "invalid_options",
                &error.to_string(),
            );
        }
    };
    if parsed.version != 1 {
        return reply(
            "native.open",
            "refused",
            "invalid_options",
            "Unsupported ABI options version",
        );
    }
    with_registry("native.open", |registry| {
        let session = match Session::new(Path::new(&parsed.config), Path::new(&parsed.driver)) {
            Ok(session) => session,
            Err(error) => {
                return reply(
                    "native.open",
                    "refused",
                    "invalid_enrollment",
                    &error.to_string(),
                );
            }
        };
        let Some(next) = registry.next.checked_add(1) else {
            return reply(
                "native.open",
                "refused",
                "handle_limit",
                "Process session identity space exhausted",
            );
        };
        registry.next = next;
        *handle = (u64::from(std::process::id()) << 32) | u64::from(next);
        registry.entries.insert(
            *handle,
            Entry {
                session,
                owner_thread: None,
                poisoned: false,
            },
        );
        json!({"version":1,"id":"native.open","outcome":"ok","result":{"abi":1}})
    })
}

fn request(handle: u64, frame: &[u8]) -> Value {
    let id = serde_json::from_slice::<Value>(frame)
        .ok()
        .and_then(|v| v["id"].as_str().map(str::to_owned))
        .unwrap_or_default();
    with_registry(&id, |registry| {
        let Some(entry) = registry.entries.get_mut(&handle) else {
            return reply(
                &id,
                "refused",
                "invalid_handle",
                "No open session with this process identity",
            );
        };
        let current_thread = thread::current().id();
        if entry
            .owner_thread
            .is_some_and(|owner| owner != current_thread)
        {
            return reply(
                &id,
                "busy",
                "thread_owned",
                "Another thread owns this session's critical section",
            );
        }
        if entry.poisoned && !Session::is_release(frame) {
            return reply(
                &id,
                "unknown",
                "session_poisoned",
                "Reconcile prior effects; only release and close remain available",
            );
        }
        entry.dispatch(&id, |session| session.request(frame))
    })
}

fn close(handle: u64) -> Value {
    with_registry("native.close", |registry| {
        let Some(entry) = registry.entries.get(&handle) else {
            return reply(
                "native.close",
                "refused",
                "invalid_handle",
                "No open session with this process identity",
            );
        };
        if entry.session.held_locks() != 0 {
            return reply(
                "native.close",
                "busy",
                "locks_held",
                "Release the caller's critical sections before closing",
            );
        }
        registry.entries.remove(&handle);
        json!({"version":1,"id":"native.close","outcome":"ok","result":{}})
    })
}

unsafe fn input<'a>(data: *const u8, len: usize) -> Option<&'a [u8]> {
    if len > MAX_REQUEST || (data.is_null() && len != 0) {
        return None;
    }
    if len == 0 {
        return Some(&[]);
    }
    // SAFETY: the ABI caller guarantees readable storage for len bytes.
    Some(unsafe { std::slice::from_raw_parts(data, len) })
}

unsafe fn emit(out: *mut NativeBuffer, f: impl FnOnce() -> Value) -> i32 {
    if out.is_null() {
        return 1;
    }
    // SAFETY: caller supplies writable, aligned output storage.
    unsafe {
        ptr::write(
            out,
            NativeBuffer {
                data: ptr::null_mut(),
                len: 0,
            },
        );
    }
    // Serialize parsing, dispatch and output allocation with coordinated fork,
    // not only the short registry mutation. Never wait on a foreign fork's gate.
    let foreign = foreign_process();
    let _gate = (!foreign).then(Gate::acquire);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if foreign {
            reply(
                "",
                "refused",
                "forked_session",
                "Fork coordination is required; execute a fresh process",
            )
        } else {
            f()
        }
    }))
    .unwrap_or_else(|_| {
        reply(
            "",
            "unknown",
            "abi_panic",
            "Native ABI failed; reconcile effects before any further mutation",
        )
    });
    let bytes = match serde_json::to_vec(&outcome) {
        Ok(bytes) if bytes.len() <= MAX_RESPONSE => bytes,
        Ok(_) => serde_json::to_vec(&reply(
            "",
            "unknown",
            "response_too_large",
            "Response exceeds the ABI limit; reconcile effects",
        ))
        .unwrap(),
        Err(_) => return 2,
    };
    let mut boxed = bytes.into_boxed_slice();
    let result = NativeBuffer {
        data: boxed.as_mut_ptr(),
        len: boxed.len(),
    };
    std::mem::forget(boxed);
    // SAFETY: caller output remains valid; ownership passes to buffer_free.
    unsafe {
        ptr::write(out, result);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn constellation_native_abi_version() -> u32 {
    1
}

/// # Safety
/// Input/output pointers must remain readable/writable and aligned for the call.
/// See native_session.h for buffer and process lifetime requirements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_open(
    options: *const u8,
    len: usize,
    out_handle: *mut u64,
    out: *mut NativeBuffer,
) -> i32 {
    if !out.is_null() {
        unsafe {
            ptr::write(
                out,
                NativeBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                },
            );
        }
    }
    if out_handle.is_null() || out.is_null() {
        return 1;
    }
    // SAFETY: valid writable handle storage is part of the ABI contract.
    unsafe {
        *out_handle = 0;
    }
    // SAFETY: input checks null/length; caller owns the readable input.
    let Some(options) = (unsafe { input(options, len) }) else {
        return 1;
    };
    // SAFETY: output pointers and input slice remain valid for this call.
    unsafe { emit(out, || open(options, &mut *out_handle)) }
}

/// # Safety
/// The request and output pointers must satisfy the buffer contract in the header.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_request(
    handle: u64,
    data: *const u8,
    len: usize,
    out: *mut NativeBuffer,
) -> i32 {
    if !out.is_null() {
        unsafe {
            ptr::write(
                out,
                NativeBuffer {
                    data: ptr::null_mut(),
                    len: 0,
                },
            );
        }
    }
    // SAFETY: caller supplies readable input for len bytes.
    let Some(frame) = (unsafe { input(data, len) }) else {
        return 1;
    };
    // SAFETY: emit validates the pointer; caller supplies writable output.
    unsafe { emit(out, || request(handle, frame)) }
}

/// # Safety
/// Output must be writable/aligned and all returned buffers freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_close(handle: u64, out: *mut NativeBuffer) -> i32 {
    // SAFETY: output requirements are passed through to emit.
    unsafe { emit(out, || close(handle)) }
}

/// # Safety
/// Pass exactly one unmodified buffer returned by this library, exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_buffer_free(buffer: NativeBuffer) {
    if !buffer.data.is_null() {
        // An uncoordinated foreign child cannot safely wait on inherited state.
        // Its process must exec/exit; deliberately do not free copied ownership.
        if foreign_process() {
            return;
        }
        let _gate = Gate::acquire();
        // SAFETY: this is the exact allocation/length produced by emit.
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                buffer.data,
                buffer.len,
            )));
        }
    }
}

/// # Safety
/// Register exactly once per loaded library image, with matching parent/child
/// callbacks. Do not invoke recursively or from an active ABI dispatch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_fork_prepare() {
    #[cfg(unix)]
    {
        if foreign_process() {
            // SAFETY: fail closed without inherited synchronization or destructors.
            unsafe {
                libc::_exit(125);
            }
        }
        let gate = Gate::acquire();
        // SAFETY: the held gate prevents any concurrent registry change.
        let held = unsafe { &*REGISTRY.0.get() }
            .as_ref()
            .is_some_and(|r| r.entries.values().any(|e| e.session.held_locks() != 0));
        FORK_HAS_LOCKS.store(held, Ordering::Release);
        FORK_PREPARED.store(true, Ordering::Release);
        // Ownership continues until exactly one parent/child callback.
        std::mem::forget(gate);
    }
}

/// # Safety
/// Only the parent of a fork prepared by fork_prepare may call this once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_fork_parent() {
    #[cfg(unix)]
    if FORK_PREPARED.swap(false, Ordering::AcqRel) {
        GATE.store(false, Ordering::Release);
    }
}

/// # Safety
/// Only the child of a fork prepared by fork_prepare may call this once, before
/// any other native operation or caller critical-section code.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constellation_native_fork_child() {
    #[cfg(unix)]
    {
        if !FORK_PREPARED.load(Ordering::Acquire) || FORK_HAS_LOCKS.load(Ordering::Acquire) {
            // No copied Python critical section may continue without ownership.
            // Process exit closes inherited lock descriptors without LOCK_UN.
            unsafe {
                libc::_exit(125);
            }
        }
        // SAFETY: preparation quiesced ABI calls; no locks are held. Abandon the
        // copied allocations without allocator/destructor/mutex activity here.
        unsafe {
            ptr::write(REGISTRY.0.get(), None);
        }
        OWNER_PID.store(std::process::id(), Ordering::Release);
        FORK_PREPARED.store(false, Ordering::Release);
        GATE.store(false, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "ffi_tests.rs"]
mod tests;
