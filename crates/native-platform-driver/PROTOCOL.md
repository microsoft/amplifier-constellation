# Local installation driver, version 1

Candidate interface for installation-owned clients. This is a local executable,
not a service, ACP extension, browser API, scheduler or restoration engine.
The installation owns its protected configuration and all commands in it.

`constellation-native-driver --config ABSOLUTE_CONFIG serve` exchanges one JSON
object per line on stdin/stdout. Keep that process alive for the entire Python
critical section. EOF releases its kernel locks. A lost reply never authorizes a
second dispatch. Requests are correlated, not deduplicated by the driver: the
existing domain transaction remains the durable source of operation identity.

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
| `service.action` | `name`, `action: start\|stop`, `expected` | `{accepted:true}` after compare-before-dispatch |
| `storage.write` | `root`, `path`, `bytes` (array of octets), optional `create_only` | protected atomic file publication |
| `storage.remove` | `root`, `path` | durable ordinary-file removal |
| `storage.mkdir` | `root`, `path` | private child directory under an existing private parent; existing policies are not changed |
| `process.launch` | `name`, optional `result` | detached supervisor generation identity |
| `exchange.inspect` | `name`, optional `entry: {root,path}` | filesystem identity or null for absent; omitted entry selects configured current |
| `exchange.prepare_pointer` | `name`, `prepared`, `target`, `expected_target` | actual Unix directory symlink / Windows junction identity |
| `exchange.apply` | `name`, `incoming`, `retained`, `expected_current`, `expected_incoming` | observed current/incoming/retained after two no-replace renames |

Unknown fields and unenrolled names reject. `path` and `result` are relative
paths under configured storage roots, with no traversal, links or reparse
ancestors. No request accepts executable paths, shell, environment or service
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
it inherits no invocation lock handles. `exec --command NAME` is a raw-stdio
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

Snapshots retain `schema`, `service`, `definition`, `effective_definition`,
`definition_matches`, `enabled`, `start_allowed`, `masked`, `loaded`, `active`,
`transitioning`, `processes`, and `native`. Native fields/roles are not translated
into invented systemd values. Raw Task Scheduler engine identity is not controller
identity.

For a separately configured `service_applications[name] = {executable,args,cwd,
identity_path}`, the enrolled service runs this fixed driver as
`--config OWNER_CONFIG service-run --service NAME`. This long-running wrapper
owns the application and records its actual process generation under the protected
identity path. It has a service lifetime, independent of bounded probe commands.
POSIX launch proof requires successful executable replacement; invalid images
never fall back to a shell. The snapshot adds `application.status` as `running`,
`absent`, or `unconfirmed`. Running evidence includes the application `identity`,
`supervisor`, `manager_process`, both generation-checked ancestry chains, and
`profile_sha256`. An inactive native service with a retained live child remains
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

Tests must distinguish actual OS lifecycle/descendant behavior from fixtures,
and each of the six target OS/architecture runs from cross-compilation. No live
game or saved production session belongs in these tests.
