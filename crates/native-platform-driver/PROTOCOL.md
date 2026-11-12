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
| `service.snapshot` | `name` | exact native definition/state and process generation |
| `service.action` | `name`, `action: start\|stop`, `expected` | `{accepted:true}` after compare-before-dispatch |
| `storage.write` | `root`, `path`, `bytes` (array of octets), optional `create_only` | protected atomic file publication |
| `storage.remove` | `root`, `path` | durable ordinary-file removal |
| `storage.mkdir` | `root`, `path` | private child directory under an existing private parent; existing policies are not changed |
| `process.launch` | `name`, optional `result` | detached supervisor generation identity |

Unknown fields and unenrolled names reject. `path` and `result` are relative
paths under configured storage roots, with no traversal, links or reparse
ancestors. No request accepts executable paths, shell, environment or service
names. `process.identity` is read-only; it never grants control of that process.
Linux generation identity retains `{pid, start_ticks, boot_id}` exactly, including
the string start_ticks used by existing receipts. Other platforms identify their
own generation format; consumers must not translate a retained Linux receipt.

Configuration has `version:1` and maps `roots`, `locks`, `services`, `commands`.
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

Protected JSON writes do **not** implement source-directory exchange. Managed
app/venv pointer swaps and rollback need their own explicit platform proof before
enrollment claims that capability. This protocol makes no such capability claim.

Tests must distinguish actual OS lifecycle/descendant behavior from fixtures,
and each of the six target OS/architecture runs from cross-compilation. No live
game or saved production session belongs in these tests.
