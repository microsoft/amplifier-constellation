# Native session ABI 1

An installation's Python action can hold the actual native kernel lock in its
own process. Losing a separate driver process cannot release that ownership
while Python continues its critical section. The shared dispatcher still owns
the fixed operations; the connector keeps its transaction journal and recovery.

The exact interface is [native_session.h](native_session.h). Build the private
library and fixed driver from the same source with `cargo build --locked`.
Library filenames are `libconstellation_native_driver.so` on Linux,
`libconstellation_native_driver.dylib` on macOS and
`constellation_native_driver.dll` on Windows. Package the matching architecture
with the connector's private interpreter. The library does not provision itself
or turn another installation's executable into an authorized command.

## Open, call, release

Open receives only `{version:1, config, driver}`. Both paths are absolute and
installation-owned. The config uses the existing protected
[enrollment schema](PROTOCOL.md); the driver is a protected ordinary file whose
canonical path and bytes are fixed for the session. An in-process host's
`current_exe` is its interpreter, so it cannot stand in for the native driver.
Changing enrollment or the fixed executable refuses new operations, while owned
lock release remains possible.

Request/response JSON and named operations are exactly the local driver protocol.
`open` and `close` responses use IDs `native.open` and `native.close`. ABI return
zero means a JSON outcome was returned; inspect its `outcome`, not just the C
return code. Handles are scoped to this process and library image. Copy each
returned buffer and free it exactly once through that same library. Keep the
library loaded until all handles and buffers are gone.

The Python binding must hold its reentrant mutex over the **whole**
`with session.lock(name)` body, including caller-side filesystem work. Native
calls from another thread while locks are held return `busy/thread_owned`.
Closing a session with locks held returns `busy/locks_held`; it never silently
ends the caller's critical section. Same-name nesting refuses; distinct enrolled
names may be held together. Release does not delete or replace the lock file.

A caught dispatch panic returns `unknown`, poisons further dispatch and retains
held locks. Valid `lock.release` and subsequent close remain available. Track the
attempted acquisition name before dispatch: an unknown acquisition can have
retained its lock before its acknowledgment failed. Explicitly release that name
when unwinding; do not retry acquisition or infer that nothing happened. A known
busy/refused acquisition creates no ownership.

Calls are synchronous. ABI 1 has no transport timeout argument and starts no
background mutation that outlives a timed-out call. Existing native operation
bounds and the external action worker's bounds remain in force. Killing that
owning worker releases its locks; it does not prove a dispatched effect failed.
Unknown outcomes reconcile through the domain's existing journal.

## Children and fork

Register the three fork callbacks **once per loaded library image**, using
Python `os.register_at_fork`. Keep that library loaded for the process lifetime;
Python cannot unregister the callbacks after its last session closes.
Preparation waits for active ABI calls, including
response allocation and buffer freeing, then freezes session changes. The parent
releases that gate. A child that could return to Python exits with code 125 if
any native critical section was active, preventing copied caller code from
continuing unlocked. Otherwise it abandons copied handles without destructors;
new sessions in that child are allowed. Registering callbacks per session would
deadlock and is invalid.

Ordinary subprocess fork/exec stays supported. Lock descriptors are
noninheritable and must not be passed explicitly to children. Do not use Python
`preexec_fn` inside a critical section. A fork-to-exec child can temporarily hold
inherited descriptors before close/exec; this is not a claim of instantaneous
descriptor removal. Foreign forks that continue running without the callbacks
are unsupported; native PID checks refuse inherited handles before waiting on
inherited synchronization, but cannot govern arbitrary foreign caller code.

`process.launch` still starts the fixed enrolled detached supervisor. Its
session probe may survive the short action and this native session, without
inheriting the action lock. The domain records its complete process generation
and observes its durable result later. A launch acknowledgment is not successful
session verification; a lost acknowledgment never causes a second dispatch.
On Windows the detached supervisor owns its job, not the Python ABI host.

## Existing state and qualification

`process.identity` and `process.matches` preserve complete generation identity.
[Service snapshots](SERVICE-SNAPSHOT.md) retain the original definition, native
enabled/disabled/masked data, activity and application identity. Domain owners
use `service.action` with the expected full observation and restore only prior
enrolled activity. There is no generic snapshot overwrite, automatic unmask or
enable that could reverse an independent administrator's change.

This ABI changes no lifecycle/provider wire, verification budget, task authority,
engine choice or account policy. The executable transport remains available, but
its process-owned lock is not equivalent for arbitrary direct Python work after
helper death. Qualification must identify which boundary was actually exercised.

Run actual library loading, caller/contender and death/fork tests on each target,
including the packaged Python on both Windows architectures. Treat package
builds, manager fixtures and an actual connector's install/Connect/update/rollback
journey as distinct evidence. Hosted CI is not a clean consumer machine claim.
