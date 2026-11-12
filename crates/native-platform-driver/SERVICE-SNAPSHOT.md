# Service snapshot, schema 1

This is the `service.snapshot` result reference within the [local driver protocol](PROTOCOL.md).
The request selects an enrolled service by name; the result preserves its native
manager state and process generations. Keep the entire result unchanged for
`service.action.expected`. The driver compares values, not serialized JSON key
order, and does not offer an atomic compare-and-swap with an external administrator.

The [base observer](../native-platform/src/installation.rs) reads two complete
observations and requires equality. Missing manager data, inaccessible processes
or a changed observation produce an error, not an inactive snapshot. An error or
timeout cannot establish that a service stopped. Optional application evidence
is then added by the [enrolled application observer](src/service_application.rs).

## Common keys

Every successful base snapshot contains all 13 keys below. `boolean | null`
means an actual JSON boolean or null, never a string. JSON null maps to Python
`None`; do not coerce it to false. Native values have the types specified in the
manager sections, even when a string resembles a number or boolean.

| Key | JSON type | Meaning |
| --- | --- | --- |
| `schema` | integer `1` | Snapshot shape version. |
| `service` | object | `{manager, name, definition_path}` from enrollment. Manager is `systemd_user`, `launch_agent`, or `scheduled_task`; name is a string; definition path is an absolute string or null. |
| `definition` | object or null | Observed enrolled file content, or the systemd mask-link form below. Null when no definition path is enrolled. |
| `effective_definition` | object or null | Manager-specific definition observation below. |
| `definition_matches` | boolean | Manager-specific association check; not universal configured/effective byte equality. |
| `enabled` | boolean or null | Native enablement interpretation below. Only systemd currently emits null. |
| `start_allowed` | boolean | Native availability permits requesting start; not authority or a guarantee of success. |
| `masked` | boolean | Actual systemd mask state. Fixed false on macOS/Windows for compatibility; not Linux enablement proof. Use native disabled data and `start_allowed` on those platforms. |
| `loaded` | boolean | Manager-specific presence/load observation below. |
| `active` | boolean or null | Observed activity; null preserves unknown or transitional native states. |
| `transitioning` | boolean | True when activity is transitional or unrecognized; it does not prove a transition is actually progressing. |
| `processes` | array of objects | Native process roles and complete generation identities; empty is permitted. |
| `native` | object | Original manager-specific fields below; no translated Linux fields. |

`application` is an optional fourteenth key, described separately below. It is
omitted entirely without the corresponding enrollment, not emitted as null.

### Definition content

An ordinary `definition` is `{path: string, sha256: string, bytes: integer[]}`.
`bytes` is the exact file content as octets, and `sha256` is its lowercase hex
SHA-256. This records content at a path, not an inode identity. The observer
requires an ordinary enrolled file; Unix ownership and write permissions are
checked. A masked systemd definition that is a symlink to `/dev/null` instead
uses `{path: string, mask_target: "/dev/null"}`, without invented bytes or hash.

The `{sha256, text}` content form used below hashes the UTF-8 bytes of the exact
returned text. Preserve text and hash together.

## systemd user service

`native` is a string-valued property map requested from `systemctl --user show`:

```text
Id Names LoadState ActiveState SubState UnitFileState FragmentPath SourcePath
DropInPaths MainPID ControlPID InvocationID NeedDaemonReload RefuseManualStart
RefuseManualStop ExecStart ExecStop Restart KillMode Type
```

Numeric-looking PIDs and values such as `NeedDaemonReload="no"` remain strings.
Unknown native state strings are retained. `Id` must identify the exact enrolled
unit, not an alias to another unit.

| Common value | Native mapping |
| --- | --- |
| `enabled` | True for `enabled`/`enabled-runtime`; false for `disabled`/`masked`/`masked-runtime`; null for other `UnitFileState` values. |
| `start_allowed` | True for `enabled`, `enabled-runtime`, `linked`, `linked-runtime`, `static`, `indirect`, or `alias`; false otherwise. |
| `masked` | True only for `UnitFileState` `masked` or `masked-runtime`. |
| `loaded` | True only for `LoadState` `loaded`. |
| `active` | True for `ActiveState` `active`; false for `inactive`/`failed`; null otherwise. |
| `transitioning` | False for `active`/`inactive`/`failed`; true otherwise. |

`effective_definition` is `{sha256: string, text: string}` from `systemctl cat`,
including its main file and ordered drop-ins. The native map also preserves
effective loaded execution properties. When masked, `effective_definition` is
null. `definition_matches` requires matching enrolled `FragmentPath` when a path
is supplied (the masked case bypasses this path comparison), `NeedDaemonReload`
equal to `no`, and `LoadState` equal to `loaded` or `masked`. It is not a byte
comparison between two definition documents.

`processes` contains nonzero observed `MainPID` and `ControlPID` entries as
`{role: "MainPID" | "ControlPID", identity: ProcessIdentity}`. If an observed
process disappears before its identity can be read, observation fails.

## macOS LaunchAgent

`native` always has this shape:

```text
{domain: string, disabled_override: boolean | null, properties: object}
```

`domain` is the current user's `gui/<uid>`. `disabled_override` is the exact
entry from `launchctl print-disabled`, or null when no override exists. An
unrecognized response or override is an error. Effective disabled state uses
that override when present; otherwise it uses the plist's boolean `Disabled`,
defaulting to false when absent. A non-boolean plist value is an error.

`enabled` and `start_allowed` are the inverse of effective disabled state.
`masked` is the fixed compatibility value false; no systemd state is inferred.
`loaded` is true after successful `launchctl print`. Only its service-not-found
exit status 113 establishes unloaded state; an inaccessible GUI domain remains
an error. An unloaded observation has `properties:{}`, `active:false` and
`transitioning:false`.

For a loaded service, `properties` contains retained top-level string values
from this list; `path` and `state` are required:

```text
path type state pid program arguments environment default environment
inherited environment properties domain minimum runtime exit timeout
```

Multi-line blocks such as arguments/environment remain strings, not parsed JSON
arrays/maps. Volatile counters such as runs/CPU time are excluded. State `running`
maps to `active:true`; `not running`, `waiting`, and `exited` map to false;
other state strings map to null with `transitioning:true`.

`definition` is the enrolled plist's exact file content. `effective_definition`
is the JSON object produced by converting that plist; its `Label` must match the
enrolled name. Loaded native properties are retained separately in `native`.
`definition_matches` means unloaded or the loaded native `path` equals the
enrolled plist path; it is not a universal loaded/configured byte comparison.
When a nonzero native `pid` is present, `processes` includes
`{role:"launchd_pid", identity:ProcessIdentity}`.

## Windows Scheduled Task

The [native COM observer](../native-platform/src/installation_windows.rs) returns:

```text
native = {
  path: string, xml: string, enabled: boolean, state: integer,
  owner_sid: string, logon_type: integer, security_descriptor: string,
  instances: Instance[], actions: Action[]
}
Instance = {
  instance_guid: string, engine_pid: integer, state: integer,
  current_action: string, engine_instance_guids: string[]
}
Action = {type: integer, id: string}
```

Executable actions additionally contain string `path`, `args`, and `cwd` keys.
Instances are ordered by instance GUID; each engine's associated GUID list comes
from the full running-task enumeration, including hidden tasks. Inaccessible
enumeration is an error, not evidence of a unique engine. The task must belong
to the current user's SID and use the least-privilege run level.

`enabled` and `start_allowed` copy `native.enabled`; `loaded` is true for a
successful observation. `masked` is the fixed compatibility value false, not a
claim about a Linux mask. State 4 (running) maps to `active:true`; 1 (disabled)
and 3 (ready) map to false; other states, including 0 (unknown) and 2 (queued),
map to null with `transitioning:true`. Disabled state is preserved, not enabled
on behalf of the caller.

`definition` is the optional enrolled file observation, or null.
`effective_definition` is `{sha256: string, text: string}` from `native.xml`.
The raw snapshot currently sets `definition_matches:true`; this does **not**
assert that the enrolled file equals the registered XML. Complete snapshot
comparison detects changes to either captured value. Registration and optional
application binding perform their own enrolled-profile checks.

Each nonzero engine PID produces
`{role:"scheduled_task_engine", identity:ProcessIdentity, instance_guid:string}`
in `processes`. This identifies the scheduler engine, which may be shared; it
is not the application's process identity.

## Process identities

The [process observer](../native-platform/src/process.rs) emits these exact shapes:

```text
Linux = {pid: integer, start_ticks: string, boot_id: string}
macOS = {platform: "macos", pid: integer, start_seconds: string,
         start_microseconds: string, boot_seconds: string,
         boot_microseconds: string}
Windows = {platform: "windows", pid: integer,
           creation_time_100ns: string, session_id: integer}
```

Time/tick fields are decimal strings; Linux boot ID is a UUID string. Keep all
fields and their types. A PID alone is not a generation identity, and a retained
Linux identity must not be translated into a different platform's format.

## Optional application evidence

Only a configured `service_applications[name]` adds `application`. Its variants
are:

```text
{status: "absent"}
{status: "unconfirmed", reason: string}
{status: "running", identity: ProcessIdentity, supervisor: ProcessIdentity,
 manager_process: ProcessEntry, application_chain: ProcessIdentity[],
 manager_chain: ProcessIdentity[], profile_sha256: string}
```

`manager_process` preserves the exact native process entry above. The application
chain runs from application to supervisor; the manager chain runs from supervisor
to the native manager process. Each includes both endpoints. `profile_sha256`
binds the configured application, service and registration profile; it is not an
executable-content hash.

Running evidence requires the registered action to match the enrolled wrapper,
current generation-checked ancestry, and an unchanged protected application
receipt. Windows additionally requires one proved task instance with an engine
GUID list containing only that instance. Native process entries are never
renamed to pretend they identify the application.

Absent requires inactive, non-transitioning native state, an empty native process
list, and no live supervisor/application in any retained receipt. An inactive
manager with a retained live child is unconfirmed. Missing, stale, inaccessible
or ambiguous binding evidence is unconfirmed with a reason; native observation
errors still fail the snapshot operation. The driver refuses service actions
when application status is unconfirmed. No profile means the key is omitted;
it does not establish absence or failed proof.
