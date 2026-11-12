# Local native binding

`Session` connects installation-owned Python code to its bundled native driver.
It uses one local process and has no network listener or third-party Python
dependency. The installation supplies both absolute setup paths. Callers select
named services, locks, commands and storage roots from that protected configuration.

For first installation, `initialize_root(driver_path, path)` creates one private
directory beneath an existing safe parent, and
`initialize_config(driver_path, config_path, config)` validates and publishes the
reviewed Config v1 without replacing an existing file. Both use the driver's
separate local `initialize` entry point, not a Session operation. Existing paths
are validated without changing permissions; a different configuration refuses.
Create directories in explicit order, stage the verified package and connection
prerequisites, then publish the configuration. No service starts or credentials
are copied. The [driver protocol](../../../crates/native-platform-driver/PROTOCOL.md#initial-local-setup)
owns the exact setup behavior and uncertainty rules.

```python
from constellation_native import Session, Busy, Unknown

with Session(installed_driver, owner_config) as native:
    with native.lock("installation"):
        before = native.service_snapshot("body")
        accepted = native.service_action("body", "stop", before)
        # Observe settlement through a fresh snapshot before exchanging source.
```

The native process keeps the lock for the entire Python critical section. A
competing kernel owner raises `Busy`; inaccessible storage raises `DriverError`.
Closing or crashing the invocation releases its locks. Create a fresh `Session`
after a Python process fork; the child cannot reuse or retain its parent's pipes.

`Refused` confirms a pre-dispatch rejection. `Unknown` means an effect may already
have happened, including a launch whose acknowledgment was lost. Reconcile the
existing domain journal and result; do not repeat the request. A broken transport
closes permanently and never reconnects or retries automatically. Accepted service
requests are not completed service transitions.

`private_dir` creates one private directory beneath an already prepared, enrolled
parent. An existing path must already be an ordinary private directory; it never
repairs an unrelated path's permissions or recursively invents parents.
`private_write` accepts the caller's already serialized bytes. The binding does
not rewrite receipts, choose restoration plans, unmask services or renew authority.
Its [directory/pointer exchange operations](../../../crates/native-platform-driver/PROTOCOL.md)
use two native renames with an observable gap, not an atomic app/venv swap. The
domain owns the transaction journal, verification and explicit rollback.
`tree_inspect` and `tree_remove` operate only on explicitly enrolled regenerable
directories. The owner retains reference selection and pruning intent; the same
Session must hold the required lock through inspection and removal. The
[driver protocol](../../../crates/native-platform-driver/PROTOCOL.md#regenerable-trees)
owns the link, mount, identity and unknown-outcome rules.
Background commands own their lifetime
separately from the short invocation; `Session.close()` does not cancel them.

The [serving driver](../../../crates/native-platform-driver/PROTOCOL.md) defines
the protocol and configuration. Tests
under `tests/native_python` distinguish injected transport failures from actual
driver and OS behavior. Mocked transport results are not native installation
qualification.
