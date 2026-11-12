# Native Python checks

Run with the actual locally built driver:

```sh
CONSTELLATION_NATIVE_DRIVER=/absolute/path/constellation-native-driver \
  python -m unittest discover -s tests/native_python -v
```

`test_transport.py` injects only the JSONL transport. It proves bounded request and
response waiting, matching replies, truthful uncertainty and no automatic retry;
it does not prove a native service implementation.

`test_driver.py` starts the actual binary against a fresh protected temporary
installation. It checks lock contention/lifetime, client-crash cleanup, process
generation, protected byte writes and configuration/path rejection. Linux also
checks interoperability with the current Python `flock` implementation. Windows
uses actual user/SYSTEM ACLs; macOS resolves its system temporary-directory alias
before enrollment. No live agent, original game session or production service is
opened. Missing binary coverage is reported as skipped, not passed.

`test_setup.py` exercises the production local setup entry point: fresh protected
directories and configuration are created through the actual driver, without
fixture ACL creation. It then opens a normal Session and tests protected storage
and locking. Refusal cases check that existing paths, contents and deliberately
broadened permissions are preserved. Transport fixtures test lost or malformed
acknowledgments separately from actual filesystem qualification.

If a local sandbox remaps the system temporary root to an untrusted owner, set
`CONSTELLATION_NATIVE_TEST_ROOT` to an existing directory owned by the test user.
The fixture still creates its own protected subdirectory; it does not relax the
driver's ancestor checks.

Run these checks separately on each supported OS and architecture. Service
start/stop and owned endpoint-tree qualification additionally require the native
crate/driver tests; these transport and storage checks alone are not a combined
installation qualification.

`test_exchange.py` uses actual directory pointers, native renames and retained
baselines through the Python client, including explicit rollback and observation
after a lost reply. Native Rust tests induce a real second-rename failure and
require unknown settlement with the original and candidate preserved.

`test_service.py` is separately enabled by `CONSTELLATION_INSTALLATION_SMOKE=1`
on disposable runners. It creates only a unique test registration, proves no-start
setup, starts the fixed application, verifies its actual PID/generation separately
from the service manager, checks stale/forged receipt refusal, restarts into a new
generation, and preserves an independent pause. It then removes its own test
registration. The workflow also runs native registration/state fixtures for both
architectures of Linux, macOS and Windows. Hosted Windows x64 uses Windows Server;
that evidence does not claim Windows 11 desktop first-run UX qualification.

Both Windows jobs additionally run this suite, including the actual service
fixture, through Minecraft's exact Python Build Standalone 3.12.16 interpreters.
`run_packaged_python.py` uses the test-only `packaged-python-lock.json`, verifies
archive and executable hashes, and reuses the shared bounded archive reader in a
fresh disposable directory. It selects that interpreter for child processes and
records every test result, skips, version, architecture, driver/executable hashes,
and fixture ownership diagnostics in `native-packaged-python-result.json` and its
log. The regular Python 3.13 pass remains; the production packaging lock and global
PATH are unchanged. This qualifies the local driver/Python boundary, not the full
Minecraft dependency graph, game lifecycle, or fresh consumer desktop experience.
