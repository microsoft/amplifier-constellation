# Local installation driver, version 1

Candidate interface for installation-owned clients. This is a local executable,
not a service, ACP extension, browser API, scheduler or restoration engine.
The installation owns its protected configuration and all commands in it.

This document owns the local wire interface and its Python binding. The linked
[service snapshot reference](SERVICE-SNAPSHOT.md) defines the result of
`service.snapshot`; it is part of this protocol, not another service or API.

`constellation-native-driver --config ABSOLUTE_CONFIG serve` exchanges one JSON
object per line on stdin/stdout. That process owns its kernel locks; EOF or its
death releases them. Use the [in-process native session](INPROCESS.md) when a
Python critical section must retain ownership across arbitrary caller-side work:
a helper-owned lock cannot protect Python code that continues after helper death.
Both boundaries use the same enrollment and operations. A lost reply never authorizes a
second dispatch. Requests are correlated, not deduplicated by the driver: the
existing domain transaction remains the durable source of operation identity.

## Initial local setup

Before a `Session` can open, installation setup creates its private directories
and publishes the reviewed owner configuration. The local command
`constellation-native-driver initialize` accepts exactly one bounded JSONL request
on stdin, followed by EOF, and returns one correlated version-1 outcome. This is a
local installer entry point, not an operation exposed by `serve`, an agent tool
or a web endpoint. Only trusted installation setup supplies these absolute paths
and profiles. The request is limited to 2 MiB; serialized Config v1 remains
limited to 1 MiB.

| Setup operation | Fields after version/id/op | Successful result |
|---|---|---|
| `setup.root` | `path`: absolute directory | `{path, created}` |
| `setup.config` | `path`: absolute config file, `config`: existing Config v1 object | `{path, created, sha256}` |

`setup.root` creates one directory with the native private storage policy beneath
an existing, safe parent. It never recursively creates parents or changes an
existing path's permissions. An existing directory must already be private to
this installation user; `created:false` does not prove an unused connection home.
The domain retains its fresh-installation and existing-state checks.

`setup.config` requires an already protected parent and applies the same Config v1
enrollment checks as `serve`, including existing roots and configured command and
application prerequisites. Full service registration/profile validation still
belongs to the later `service.register` call; publishing configuration does not
assert service readiness. Setup publishes a protected file without replacing an
existing destination. The file contains the driver's compact JSON serialization.
An exact existing configuration is validated and returns `created:false`;
different contents, ownership, permissions or links refuse without repair.
The returned SHA-256 binds the published bytes.

Setup calls do not register or start services, enable startup, copy credentials,
create sessions, launch configured commands or enroll an updater. The domain
creates required directories in explicit order, stages its verified package and
existing connection files, then publishes the complete configuration. It can then
open a normal `Session` and invoke its fixed no-start registration profiles.

Known preflight rejection is `refused`; uncertain mutation or acknowledgment loss
is `unknown`. No automatic retry, cleanup or ACL repair follows. After lost
acknowledgment, a subsequent explicit setup call may validate an exact existing
artifact without changing it; a changed artifact is not silently adopted.

The Python helpers are `initialize_root(driver_path, path, *, timeout=30.0)` and
`initialize_config(driver_path, config_path, config, *, timeout=30.0)`. Both use
this one-shot entry point, preserve the existing exception classes and perform no
Python filesystem or ACL setup. They do not alter `Session` timeout or lock
semantics. Successful shared initialization is not complete domain installation
qualification.

## Enrolled operations

Request: `{"version":1,"id":"opaque-id","op":"lock.acquire","name":"installation"}`.
Reply: `{"version":1,"id":"opaque-id","outcome":"ok","result":{}}`.
Outcomes are `ok`, `busy`, `refused`, `unknown`, and `error`. Non-ok replies carry
`error: {code, message}`. `busy` means kernel lock contention only. `refused`
means a known rejection before dispatch. An observation/storage failure is an
`error`; a possible mutation whose terminal state is unproved is `unknown`.
`ok` for a service request means accepted, not settled. Observe a fresh snapshot.

| Operation | Fields after version/id/op | Result |
|---|---|---|
| `describe` | none | protocol, supported operations, platform |
| `lock.acquire`, `lock.release` | `name` | named lock held/released by this process |
| `process.identity` | `pid` | generation identity or null when absent |
| `process.matches`, `process.status` | `identity` | `{matches: boolean}`; observation errors remain errors |
| `service.register` | `name` | `{created: boolean}`; configured fresh registration without start |
| `service.snapshot` | `name` | exact native definition/state and process generation |
| `tree.inspect` | `name`, `entry` | opaque fixed-root/child filesystem identity, or null if absent |
| `tree.observe` | `name`, `entry` | `{scope, entry}`; fixed profile/root identity even for absent child; requires the enrolled lock |
| `tree.remove` | `name`, `entry`, `expected` | remove one owner-selected regenerable child under its required lock; observed absence only |
| `service.action` | `name`, `action: start\|stop`, `expected` | `{accepted:true}` after compare-before-dispatch |
| `storage.write` | `root`, `path`, `bytes` (array of octets), optional `create_only` | protected atomic file publication |
| `storage.write_data` | `root`, `path`, `bytes` (array of octets) | protected data snapshot publication; compatible open readers retain the old snapshot |
| `storage.remove` | `root`, `path` | durable ordinary-file removal |
| `storage.mkdir` | `root`, `path` | private child directory under an existing private parent; existing policies are not changed |
| `process.launch` | `name`, optional `result` | detached supervisor generation identity |
| `exchange.inspect` | `name`, optional `entry: {root,path}` | filesystem identity or null for absent; omitted entry selects configured current |
| `exchange.prepare_pointer` | `name`, `prepared`, `target`, `expected_target` | actual Unix directory symlink / Windows junction identity |
| `exchange.apply` | `name`, `incoming`, `retained`, `expected_current`, `expected_incoming` | observed current/incoming/retained after two no-replace renames |

Unknown fields and unenrolled names reject. `path` and `result` are relative
paths under configured storage roots, with no traversal, links or reparse
ancestors. No enrolled request accepts executable paths, shell, environment or service
names. `process.identity` is read-only; it never grants control of that process.
Linux generation identity retains `{pid, start_ticks, boot_id}` exactly, including
the string start_ticks used by existing receipts. Other platforms identify their
own generation format; consumers must not translate a retained Linux receipt.

Configuration has `version:1` and maps `roots`, `locks`, `services`, `commands`,
optional `registrations`, `service_applications`, and `exchanges`.
All filesystem locations are absolute, owner-controlled installation paths.
Services name a native manager (`systemd_user`, `launch_agent`, `scheduled_task`),
its fixed `name`, and optional fixed `definition_path`. Commands contain a fixed
absolute `executable`, `args`, `cwd`, `timeout_seconds` and optional `result_root`.
Only an entire argument equal to `{result}` is substituted, from the selected
root and caller's validated relative result path. No shell expansion occurs.

The detached supervisor owns one fixed command and its endpoint descendants;
it inherits no invocation lock handles. On Windows, its per-spawn handle list
contains only its NUL standard streams, excluding the caller's capture pipes and
unrelated inheritable handles. `exec --command NAME` is a raw-stdio
wrapper for an enrolled ACP endpoint, not JSONL; it monitors its launching
process generation and cleans up the endpoint on parent loss or the configured
deadline. The probe may outlive the short launching command. The domain records
launch intent before calling; absent acknowledgment remains unknown and STATUS
does not launch again. The existing ACP verifier still owns original-session
selection, inactivity limits, correlated rejection and result interpretation.

Service snapshots preserve manager-specific fields. Start/stop never edits,
enables, unmasks or restores definitions. A changed definition, state or process
generation refuses before dispatch. OS service managers do not offer a universal
atomic compare-and-swap: this check narrows drift but cannot lock out an external
administrator between observation and dispatch. The owner observes settlement
and owns restore planning; independent pauses stay paused.

Fresh service setup uses `registrations[name] = {executable,args,cwd,description,
autostart:false}` for an already configured service name. No request supplies these
values. Creation does not start the service; repeats only observe exact existing
definitions. Existing mismatches, masks and disable settings are preserved.
`autostart` selects future login behavior on fresh registration only.

The [service snapshot reference](SERVICE-SNAPSHOT.md) specifies every common key,
its type, native manager fields, process identities and optional application
evidence. Preserve the entire returned snapshot as `service.action.expected`.
Do not normalize unknown values, substitute Linux states, or reconstruct a
snapshot from a subset of fields. Raw Task Scheduler engine identity is not
controller identity.

For a separately configured `service_applications[name] = {executable,args,cwd,
identity_path}` plus its matching `registrations[name]`, the enrolled service runs this fixed driver as
`--config OWNER_CONFIG service-run --service NAME`. This long-running wrapper
owns the application and records its actual process generation under the protected
identity path. It has a service lifetime, independent of bounded probe commands.
POSIX launch proof requires successful executable replacement; invalid images
never fall back to a shell. The snapshot adds `application.status` as `running`,
`absent`, or `unconfirmed`. Running evidence includes the application `identity`,
`supervisor`, `manager_process`, both generation-checked ancestry chains, and
`profile_sha256`. Native action metadata must match the fixed wrapper, and Windows
requires one proved task instance with no ambiguous shared engine identity.
An inactive native service with a retained live child remains
unconfirmed. Old installations without this optional profile retain raw snapshots.

Source exchange is an explicit operation, separate from JSON publication.
`exchanges[name] = {lock: ENROLLED_LOCK, profile: {current: ABSOLUTE_PATH,
roots: {NAME: ABSOLUTE_DIRECTORY}}}` selects fixed owner paths. An entry is a
relative `{root,path}` under those roots. The configured lock must remain held
for preparation/exchange and the caller's surrounding transaction. Inspection
identifies objects and targets; it does not certify source contents.

Prepare creates a real directory pointer without replacing anything. Apply checks
expected identities and an absent retained destination, then moves current to
retained and incoming to current. These are **two** no-replace native renames,
with an observable gap; app+venv is not an atomic pair. A partial error is unknown
and leaves all surviving entries inspectable. Explicit rollback uses retained as
the next incoming and a fresh retention destination. No deletion, automatic retry,
or whole-state rollback is provided. Domain owners still journal operations,
verify source fingerprints, close admission and settle services.

## Data snapshot publication

`describe` advertises `private_data_publication:1` and `storage.write_data` when
the explicit data writer is available. It uses the same protected enrolled-root,
ordinary-file, owner and path checks as `storage.write`; it adds no path authority.
The installation chooses this operation only for serialized data snapshots such
as journals. Existing strict `storage.write`, create-only writes, executable
replacement and directory exchange retain their original behavior.

On Windows, this reuses the native library's `private_write_data`: compatible
readers must grant delete sharing. An already-open reader retains the old file;
a fresh open sees the complete replacement. There is no missing-file fallback,
automatic retry, permission override or implicit change to consumer readers.
A reader denying delete sharing or a read-only destination still causes failure.
Consumer qualification must exercise its actual reader behavior.

`private_data_read:1` and `storage.read_data` advertise the corresponding
installation-owned reader. Its request is `{root,path,max_bytes}`, with an
integer limit from 1 through 1,048,576 bytes. It uses the same enrolled-root,
ordinary-file, owner and path validation. It does not create parents, repair
permissions, acquire a transaction lock or change the journal. The installation
retains responsibility for its surrounding transaction and reference checks.

The result is either `{present:true,bytes:[...]}` or `{present:false}`. Only a
missing final file at the actual open returns absence; an empty file is present
with an empty byte list. Invalid paths or policies are refused. Read/open errors
remain errors, and data exceeding the requested limit is refused without
returning a truncated snapshot. A missing or unsafe parent is not file absence.

On Windows this production reader explicitly grants read, write and delete
sharing. Its open handle keeps the selected file snapshot while a compatible
data publication replaces the directory entry. This does not provide snapshot
isolation from concurrent in-place writes. It rejects non-ordinary files
and reparse points; the Unix opener also refuses symbolic links and special
files. There is no retry across replacement and no promise of an atomic check
against a hostile same-owner writer. Strict writes, executable replacement,
create-only publication and directory exchange remain unchanged. Existing
Python `Path.read_text`/`read_bytes` calls are not converted by advertising this
operation; consumers must select and qualify the production reader explicitly.

The Python method `private_read_data(root, path, max_bytes=1048576)` returns
`bytes` or `None` for observed absence. Its limit is validated before dispatch.
A transport failure remains a failed observation and invalidates executable
`Session` locks; it is not absence or authority to retry a mutation. Invalid
result shapes also invalidate that transport rather than supplying data or
absence. `NativeSession` uses the same operations and bounded result validation
inside the caller process: a malformed or interrupted response poisons dispatch
while retaining its kernel locks until scope unwind. Neither binding changes
the domain's recovery decisions.

Rejected preflight remains `refused`. Any publication or flush error is
`unknown`/`write_unconfirmed`, including a flush failure after the replacement
may have committed. Persist intent before mutation; after a lost reply, unwind
the Session-held critical section and reacquire before freshly reading the same
intent. Cached old reader bytes do not prove publication failed. This operation
does not settle the domain journal or authorize a second dispatch.

## Python binding

Import `Session`, `DriverError`, `Busy`, `Refused`, and `Unknown` from
`constellation_native`. The synchronous [implementation](../../support/python/constellation_native/client.py)
starts one local driver process; it does not connect to a daemon. Its constructor is:

```python
Session(driver_path: os.PathLike[str] | str,
        config_path: os.PathLike[str] | str, *, timeout: float = 30.0)
```

Both paths must be absolute and supplied by installation setup. `timeout` is a
positive finite number of seconds; booleans are invalid. Relative paths or invalid timeout
raise `ValueError` before launch. Failure to launch raises
`DriverError("driver_start", message)`. Construction is not a successful protocol
handshake; call `describe()` when capability discovery is needed. The request
deadline covers its pipe write and response wait, after obtaining the session's
thread mutex. It is not a work deadline or a promise that cleanup takes zero time.

`with Session(...) as native:` returns the session and closes it on exit.
`close() -> None` is idempotent, closes this invocation and releases its locks;
it does not cancel independently launched commands or manager-owned applications.
The session serializes requests. It never reconnects, retries a request, or
replays a request ID after a lost response. A forked child must create a new
session; it cannot use or retain the parent's driver connection.

For an application-bound service action, one native observation precedes
application validation and a second follows it. Both must match the expected
native snapshot before availability and process-generation checks and dispatch.
This avoids repeating a complete observation pair. Individual manager-call caps
are not a strict end-to-end wall-clock guarantee: local filesystem/process proof
and cleanup also take time. A client deadline remains conservative `Unknown`
after a possibly mutating dispatch; it never authorizes a retry.

The following methods return the wire `result` unchanged unless stated otherwise.
JSON objects, arrays and null become Python dictionaries, lists and `None`.
Names select protected enrollment entries, not native service names or arbitrary
executables supplied by a request.

| Python method | Result / meaning |
| --- | --- |
| `describe()` | Object containing `protocol:1`, `platform`, `architecture`, `operations`, `source_pointer_exchange:1`, and `service_application_identity:1`. These describe support, not enrollment. |
| `lock(name: str)` | Context manager yielding `None`; retains the named kernel lock across the caller's critical section. |
| `process_identity(pid: int)` | Native [process identity](SERVICE-SNAPSHOT.md#process-identities) or `None` for observed absence. |
| `process_matches(identity: Mapping[str, Any]) -> bool` | Extracts and validates the boolean `matches` result. Malformed evidence or observation failures raise; they are not false. |
| `process_status(identity: Mapping[str, Any])` | Returns the full `{"matches": bool}` object. It is the wire alias, not a second lifecycle/status model. |
| `service_register(name: str)` | `{"created": bool}`; registers a fresh enrolled definition without starting it. |
| `service_snapshot(name: str)` | Complete [snapshot](SERVICE-SNAPSHOT.md); an observation failure raises rather than manufacturing stopped state. |
| `tree_inspect(name: str, entry: str)` | Opaque ordinary-directory identity or null; no recursive scan or mutation. |
| `tree_observe(name: str, entry: str)` | `{scope, entry}` with stable profile/root evidence and the existing inspection result or null. Requires this Session's configured tree lock; advertised by `regenerable_tree_observation:1`. |
| `tree_remove(name: str, entry: str, expected: Mapping, *, timeout: float \| None = None)` | `{absent: true}` only after identity-checked removal and observed absence; requires this Session's configured lock. Optional timeout applies only to this call. |
| `service_action(name: str, action: str, expected: Mapping[str, Any])` | `{"accepted": True}` for `"start"` or `"stop"`; acceptance is not settlement. |
| `private_write(root: str, path: str, data: bytes, *, create_only: bool = False)` | `{}` after protected publication. Accepts already serialized `bytes` only; other data types raise `TypeError`. |
| `private_write_data(root: str, path: str, data: bytes)` | `{}` after protected data snapshot publication; accepts serialized bytes only. Requires advertised `private_data_publication:1` and `storage.write_data`. |
| `private_read_data(root: str, path: str, *, max_bytes: int = 1048576)` | `bytes` from one protected ordinary file, or `None` for observed final-file absence. Requires advertised `private_data_read:1` and `storage.read_data`; limits are 1 through 1,048,576 bytes, with no truncation. |
| `private_dir(root: str, path: str)` | `{}` after creating or validating one private child directory under an enrolled existing parent. |
| `durable_remove(root: str, path: str)` | `{"absent": True}` after removal or confirmed prior absence. |
| `launch(name: str, *, result: str \| None = None)` | Detached supervisor generation identity, not command completion or the endpoint's identity. |
| `exchange_inspect(name: str, entry: Mapping[str, str] \| None = None)` | Existing [source exchange](#source-exchange-binding) observation; `None` selects configured current. |
| `exchange_prepare_pointer(name: str, prepared: Mapping[str, str], target: Mapping[str, str], expected_target: Any)` | Existing source exchange pointer observation. |
| `exchange_apply(name: str, incoming: Mapping[str, str], retained: Mapping[str, str], expected_current: Any, expected_incoming: Any)` | Existing source exchange result containing observed `current`, `incoming`, and `retained`. |

### Exceptions and lock ownership

`DriverError(code: str, message: str)` subclasses `RuntimeError` and exposes
`.code`; `str(error)` is the message. `Busy`, `Refused`, and `Unknown` subclass
`DriverError` with the same constructor. Catch the specific subclasses before the
base class when their outcomes need different handling.

| Outcome | Python exception | Meaning |
| --- | --- | --- |
| `busy` | `Busy` | Kernel lock contention only; acquisition does not wait or retry. |
| `refused` | `Refused` | A known rejection before dispatch. |
| `unknown` | `Unknown` | An effect may have happened; reconcile the existing domain transaction before further related action. |
| `error` | `DriverError` | A driver/observation failure; it proves neither absence nor settled service state. |

Lost, malformed, mismatched or timed-out responses permanently close the session.
For a mutating request they raise `Unknown`; for a read-only request they raise
`DriverError`. Lock acquisition/release, registration, service action, storage
changes, tree removal, launch, pointer preparation and exchange count as mutations here.
Read-only failures do not settle earlier effects. Calls after close raise
`DriverError` with `closed_session`; use after fork raises `forked_session`.
Oversized requests and non-finite JSON values fail before dispatch with
`ValueError`; values that JSON cannot serialize may raise `TypeError`.

`with native.lock(name):` holds both the native kernel lock and this session's
thread mutex until exit, including while caller code runs. Calls by that same
thread are permitted; other threads cannot dispatch or close this session during
the critical section. Nested acquisition of the same name raises
`DriverError("lock_reentry", message)`; distinct enrolled locks may be nested.
On exit the binding requests release if the session still owns the lock. A failed
release raises rather than claiming success; a broken session closes its driver.
Driver exit releases its kernel locks. No lock file deletion is used to override
another owner. The caller still owns transaction journaling and settlement.

### Source exchange binding

The three Python exchange methods above pass through the existing operations;
entries remain `{root, path}`, and expected observations are retained unchanged.
The serving functions are [`inspect`, `prepare_pointer`, and `exchange`](../native-platform/src/installation_exchange.rs).
Pointer preparation and exchange require the configured exchange lock held in
this same driver invocation. They neither add a new receipt format nor replace
the domain journal. Keep the explicit two-rename and unknown-outcome behavior
described above when using them from Python.

### Regenerable trees

`describe.regenerable_tree_removal == 1` advertises this optional primitive.
It requires explicit protected Config v1 enrollment; ordinary storage permission
does not grant recursive cleanup:

```json
{"regenerable_trees":{"stages":{"root":"state","path":"stages","lock":"installation"}}}
```

`root` selects an existing protected storage root; `path` selects an existing
private subdirectory. The subdirectory cannot contain the enrollment file or any
configured lock. Requests supply its enrolled `name` and exactly one normal child
`entry`, never an absolute path, traversal, stream, or the capability root itself.
The child must be an ordinary directory. Root and ancestor links/reparse points
refuse. An inspection snapshot binds the fixed directory and child filesystem
identities; it is not a content fingerprint or permission to discard current work.

Before removal, the domain must hold the configured lock across its reference
checks, durable pruning intent and dispatch. The domain retains all current,
baseline, rollback, retained-plan and uncertain-transaction protection. An unknown
or unmarked directory does not become regenerable because inspection succeeds.
The shared primitive checks the expected identities and preflights the selected
tree, rejecting mount crossings and unsupported entries. Supported descendant
symlinks/junctions are leaves; their targets are never traversed. The shared
preflight also compares each directory's filesystem identity with the parents of
the enrollment file and all configured locks, so existing mount aliases cannot
hide those control files inside the selected tree. The standard
library performs removal synchronously in this invocation, with no detached
traversal or retry. A final fixed-root check and actual child absence are required
for `{absent:true}`. Errors after removal begins are `Unknown`, including partial
deletion and an unverifiable terminal state.

The installation lock coordinates enrolled writers; it is not a filesystem CAS
or a defense against hostile same-owner/privileged replacement or concurrent
mount-topology changes. Mount preflight observes the current process namespace,
including Linux same-device bind mounts. Already-dispatched kernel operations may
settle after transport loss; invocation termination is not proof of no effects.
Leave the pruning intent unresolved, unwind lost lock ownership, then open a fresh
Session/lock for read-only reconciliation. Bare null from `tree.inspect` cannot
settle a prior unknown removal: a replaced root can also have no such child.
Use the optional `tree.observe` operation when advertised. It returns
`{scope:{schema:1,profile_sha256,root,root_identity},entry:SNAPSHOT_OR_NULL}`.
The scope binds the protected profile name, storage root name/path, relative tree
path, lock name/path and the canonical tree root's filesystem identity. The root
is compared before and after observation. This operation requires the same
enrolled lock and performs no recursive traversal or mutation.
The digest covers that selected profile, not the whole configuration document;
unrelated configuration changes across fresh Sessions may preserve the scope.
The existing configuration-change guard still refuses changes within a Session.

Persist the complete scope with the original pruning intent before dispatch;
pass its non-null `entry` snapshot unchanged to the existing `tree.remove`.
After acknowledgment loss, observe the exact profile name and child entry saved
in that intent under a fresh lock, then compare the complete scope. The scope is
shared by a profile's children: an absent different child cannot settle this
intent. Only an equal scope and null entry for the original child support marking
that same intent absent. A changed root/profile, an existing or replaced child, a
legacy intent without saved scope, or an uncertain observation stays unresolved.
No automatic redispatch follows.
Existing `tree.inspect` and `tree.remove` results remain unchanged; capability
`regenerable_tree_removal:1` alone does not advertise this new observation.
The normal client deadline is unchanged. Python callers may explicitly pass
`tree_remove(..., timeout=120)` for a cleanup-only response deadline when that
duration, invocation termination and reconciliation fit within their remaining
enclosing provider budget. `None` uses the Session's configured timeout (30 seconds
by default); every other operation and the Session setting remain unchanged.
The override must be a positive finite number no greater than Python's
`threading.TIMEOUT_MAX`, is checked before dispatch, and is not a wire request
field. It covers request writing and response waiting under
the same Session lock. Expiry terminates that invocation and raises `Unknown`
without retry; it does not cancel already-dispatched effects. Neither entry count
nor successful prior timings guarantee completion within the selected deadline.

Tests must distinguish actual OS lifecycle/descendant behavior from fixtures,
and each of the six target OS/architecture runs from cross-compilation. No live
game or saved production session belongs in these tests.
