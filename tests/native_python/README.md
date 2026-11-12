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

If a local sandbox remaps the system temporary root to an untrusted owner, set
`CONSTELLATION_NATIVE_TEST_ROOT` to an existing directory owned by the test user.
The fixture still creates its own protected subdirectory; it does not relax the
driver's ancestor checks.

Run these checks separately on each supported OS and architecture. Service
start/stop and owned endpoint-tree qualification additionally require the native
crate/driver tests; these transport and storage checks alone are not a combined
installation qualification.
