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

The Administrators-owner fixtures require an elevated disposable Windows token.
Ordinary non-admin accounts explicitly skip those cases; GitHub Actions Windows
runs fail if that required token is missing. The nested-storage regression changes
only a short-lived child's default owner and then exercises production setup,
directory creation and writes. It never changes the parent token or repairs an
existing installation's ownership.

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

Each `native-installation-*` artifact includes both exact debug executables: the
driver and the application selected by `NATIVE_SERVICE_FIXTURE`. Their separate
`native-installation-result.json` and `native-service-fixture-result.json` records
use the same source-pinned binary inspector before testing and verify unchanged
bytes afterward, including architecture and ordinary/delayed Windows CRT imports.
Consumer qualification can reuse these tested binaries without rebuilding.

## In-process library qualification

The [native session ABI](../../crates/native-platform-driver/INPROCESS.md) is
qualified against the exact library built with the driver. Set
`CONSTELLATION_NATIVE_LIBRARY` to its absolute native path (`.so`, `.dylib`, or
`.dll`) alongside `CONSTELLATION_NATIVE_DRIVER`. The library inspector does not
load code:

```sh
python tests/native_python/inspect_library.py \
  --library "$CONSTELLATION_NATIVE_LIBRARY" --output native-library-result.json
# Run actual in-process tests on this OS/architecture, then verify unchanged bytes.
python tests/native_python/inspect_library.py \
  --library "$CONSTELLATION_NATIVE_LIBRARY" --output native-library-result.json --verify
```

`inspect_library.py` retains architecture, file hash and dynamic dependencies.
It requires an ELF shared object without an interpreter/PIE or extra search path,
an actual macOS `MH_DYLIB` with deployment metadata and system dependencies, or a
PE DLL passing the same ordinary/delayed external Visual C++ runtime guard as the
unchanged executable inspector. Metadata inspection is not actual library loading
or ABI proof. Linux's glibc system-library list does not establish a universal
glibc version floor, and hosted tests do not establish a fresh consumer OS.

`test_inprocess_lock.py` loads the real library and checks caller-owned exclusion,
helper and caller death, poison/unwind, thread close, and the fixed detached probe.
The Unix fork checks also cover interrupted preparation and competing attempts;
signals and malformed replies are explicitly injected at the Python boundary.
These tests retain actual kernel locks but do not claim arbitrary foreign-fork
support or a kernel-wide cancellation deadline.

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

For this pass `CONSTELLATION_NATIVE_LIBRARY` and the immutable Microsoft Instance
checkout at `CONSTELLATION_INSTANCE_SOURCE` are required. The parent binds the library's
exact SHA256 into the isolated child, which loads that file, observes ABI 1,
executes the actual in-process tests and verifies unchanged library bytes afterward.
`NativeSessionTests.test_actual_library_abi_and_named_operations` and
`NativeSessionTests.test_helper_death_does_not_release_python_owned_lock` must both
pass, as must all six `ActualWorkerNativeSessionTests` cases using the exact
`fixtures/worker_source.json` pin. Missing, skipped or expected-failure results
cannot qualify this boundary. Worker-source and library hashes are checked before
and after the run; the existing 300-second packaged qualification bound is unchanged.
The receipt separates exact interpreter/library loading from metadata inspection
and records other OS-specific skips explicitly. Inspection-only metadata fixtures
are never counted as actual ABI, lock ownership or packaged-interpreter proof.

The combined capability cases in `test_inprocess_capabilities.py` exercise both
bindings against the same protected journal and tree enrollment. They retain
caller ownership through malformed or interrupted snapshot decoding, and persist
an actual cleanup intent before one committed removal whose response is lost.
Fresh locked reconciliation must match the saved scope; replacing the root
invalidates that comparison. Packaged Windows qualification also requires the
actual in-process writer to preserve a held old reader while a new reader sees
the replacement. These cases qualify composition with ABI 1, without changing
the external provider/lifecycle deadlines or qualifying a domain's state machine.

`test_tree.py` uses production setup and the actual driver to remove only an
explicitly enrolled regenerable child. It checks lock contention, stale root and
child identities, invalid paths, outside sentinels behind links/junctions, ordinary
long Windows paths, and read-only reconciliation after a genuinely committed but
discarded removal reply. The Linux bind-mount fixture runs in a separate mount
namespace; unavailable unprivileged and noninteractive privileged namespaces are
reported as skipped, never mount-protection proof.

`run_tree_scale.py` runs once per native target, separately from both unittest
passes: 58,924 files, 1,064 real links/junctions and 12 nested directories total
60,000 descendants. It records actual removal time with an explicit 120-second
cleanup-only deadline while the Session default remains 30 seconds. It also records
source/driver hashes, independent counts and outside-sentinel preservation. The original Windows
ARM run exceeded the default 30 seconds and retained an unknown outcome; the
cleanup caller must choose a bound within its remaining enclosing operation.
Failure retains an error/unknown receipt and performs no second removal request.
This is one measured package-shaped fixture, not a universal duration guarantee,
hostile-writer filesystem isolation, or completed Minecraft retention/lifecycle
qualification.
