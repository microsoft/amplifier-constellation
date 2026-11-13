# Local native binding

`NativeSession` runs the installation's enrolled native operations in the Python
process, so that process owns the actual kernel locks while its critical section
also performs direct Python work. The installation supplies its verified private
library, matching fixed driver and protected configuration as absolute paths.
Callers select named services, locks, commands and storage roots from that
configuration. There is no network listener or third-party Python dependency.

The existing `Session(driver, config, timeout=...)` executable transport remains
available. Its locks belong to the helper process: helper death can release them
before the Python caller detects the loss. It does **not** provide caller-owned
exclusion for arbitrary Python effects. Keep existing in-process domain locks
until the replacement library boundary is qualified for that installation.

For first installation, `initialize_root(driver_path, path)` creates one private
directory beneath an existing safe parent.
`initialize_driver(driver_path, path, sha256, *, timeout=30.0)` copies only that
verified executing driver into an existing protected parent, using private
executable creation and no-replace publication. Supply the package's expected
lowercase SHA-256 after verifying the input driver before execution. The returned
`path`, `created`, `sha256`, `size` and nonempty opaque `identity` describe the
published or validated file; identity is not authority for a later replacement.
An exact already-private executable returns `created:false` without changing
bytes or policy. Public, foreign-owned, linked or conflicting targets refuse.
The helper does not copy the library or change its package-trust requirements.

`initialize_config(driver_path, config_path, config)` validates and publishes the
reviewed Config v1 without replacing an existing file. These helpers use the driver's
separate local `initialize` entry point, not a Session operation. Existing paths
are validated without changing permissions; a different configuration refuses.
Create directories in explicit order, stage the verified package and connection
prerequisites, then publish the configuration. No service starts or credentials
are copied. The [driver protocol](../../../crates/native-platform-driver/PROTOCOL.md#initial-local-setup)
owns the exact setup behavior and uncertainty rules.

Python path, digest and wait-budget validation happens before dispatch. Lost or
malformed acknowledgments remain `Unknown`; no automatic retry, rollback or
permission repair follows. A later explicit same-intent call may validate an
existing exact artifact. `created:false` does not establish an unused home or
authorize domain adoption. The initializer's one-shot timeout does not change
the synchronous `NativeSession` contract.

```python
from constellation_native import NativeSession, Busy, Unknown

with NativeSession(installed_library, installed_driver, owner_config) as native:
    with native.lock("installation"):
        before = native.service_snapshot("body")
        accepted = native.service_action("body", "stop", before)
        # Observe settlement through a fresh snapshot before exchanging source.
```

`NativeSession` keeps a reentrant mutex over the entire caller scope. Another
thread's close waits; close from inside the scope refuses without unlocking.
Same-name nesting refuses. A competing kernel owner raises `Busy`; inaccessible
storage raises `DriverError`. The native lock uses the existing file and kernel
namespace, including Python flock and Windows msvcrt peers.

Calls are synchronous. `NativeSession` and its `tree_remove` do not accept a
transport timeout; unsupported arguments reject before dispatch. The external
action worker retains its existing lifecycle/provider deadline and possible-effect
semantics. This ABI does not introduce background work that can keep mutating
after a caller timeout releases ownership. Explicit detached `process.launch`
probes retain their separate lifetime, generation identity and journal.

A panic or malformed response may poison dispatch, but retains native locks until
scope exit. An unknown or interrupted acquisition is released once during unwind,
without entering the body or retrying acquisition. Release remains available
after configuration drift or poison. If release itself cannot be confirmed, the
original exception remains primary and ownership remains unresolved; close never
silently drops a retained native lock beneath active caller work.

Fork hooks are registered once per loaded library image, including path aliases.
A Python-returning child exits 125 if any native lock was held. Otherwise copied
sessions reject before acquiring inherited Python mutexes; open a fresh session
in the child. Interrupted or overlapping fork preparation also fails closed in
the affected child; the parent retains its locks and unwinds only that attempt's
preparation. Loading another library during preparation refuses. Ordinary
subprocess exec is supported without inheriting locks. Do
not use `preexec_fn` in a native critical section; foreign uncoordinated forks
that continue Python execution are outside this boundary. The wrapper keeps the
library loaded for the process lifetime. The [ABI guide](../../../crates/native-platform-driver/INPROCESS.md)
defines native buffer, ownership and fork behavior.

`Refused` confirms a pre-dispatch rejection. `Unknown` means an effect may already
have happened, including a launch whose acknowledgment was lost. Reconcile the
existing domain journal and result; do not repeat the request. Neither binding
retries automatically. A broken executable `Session` transport closes permanently;
the in-process binding instead retains its locks through unwind. Accepted service
requests are not completed service transitions.

`private_dir` creates one private directory beneath an already prepared, enrolled
parent. An existing path must already be an ordinary private directory; it never
repairs an unrelated path's permissions or recursively invents parents.
`private_write` accepts the caller's already serialized bytes. The binding does
not rewrite receipts, choose restoration plans, unmask services or renew authority.
Both bindings expose explicit `private_write_data` and `private_read_data` for
protected data snapshots. Require the advertised `private_data_publication:1`
and `private_data_read:1` capabilities. The reader returns bytes or `None` for an
absent final file, with a maximum of 1 MiB and no truncation; an empty file is
`b""`. Policy and I/O failures stay distinct from absence. Windows production
readers allow delete sharing so writers can publish while an existing reader
retains its old snapshot. Strict `private_write`, create-only, executable and
directory replacement rules are unchanged. A malformed or interrupted native
snapshot response poisons dispatch while retaining caller locks through unwind.
Its [directory/pointer exchange operations](../../../crates/native-platform-driver/PROTOCOL.md)
use two native renames with an observable gap, not an atomic app/venv swap. The
domain owns the transaction journal, verification and explicit rollback.
`tree_inspect` and `tree_remove` operate only on explicitly enrolled regenerable
directories. The owner retains reference selection and pruning intent; the same
Session must hold the required lock through inspection and removal. The
[driver protocol](../../../crates/native-platform-driver/PROTOCOL.md#regenerable-trees)
owns the link, mount, identity and unknown-outcome rules.
For recovery after a lost removal result, use `tree_observe` under the same
enrolled lock and persist its scope with the exact profile and entry in the
original pruning intent. A fresh locked observation can establish absence for
that intent only when its scope matches. Changed scope, a remaining entry or a
legacy intent without scope grants no retry or automatic cleanup. Both bindings
advertise `regenerable_tree_observation:1`; the domain still owns reconciliation.
For the executable `Session` only, `tree_remove(..., timeout=120)` sets that call's response
deadline. Its default `None` retains the Session timeout, normally 30 seconds;
other calls are unchanged. Choose a positive finite duration within Python's
`threading.TIMEOUT_MAX` that leaves room
for invocation termination and reconciliation within the caller's remaining
provider budget. Timeout still raises `Unknown`, closes the invocation and
invalidates its locks; it never authorizes a retry.
Background commands own their lifetime
separately from the short invocation; neither binding's close cancels them.

The [serving driver](../../../crates/native-platform-driver/PROTOCOL.md) defines
the protocol and configuration. Tests
under `tests/native_python` distinguish injected transport failures from actual
driver and OS behavior. Mocked transport results are not native installation
qualification.
