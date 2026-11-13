"""CI-only native-driver qualification with exact consumer-packaged Python.

This does not change the production Python lock, install global software, modify
PATH, or build another driver. The archive is extracted into a fresh disposable
directory; only qualification records and logs remain after the test.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import struct
import subprocess
import sys
import tempfile
import traceback
import unittest

TESTS = Path(__file__).resolve().parent
ROOT = TESTS.parents[1]
LOCK = TESTS / "packaged-python-lock.json"
# Isolated -I omits the script directory. This fixed qualification closure is
# needed for its inspectors/tests; no host or current-directory import is added.
if str(TESTS) not in sys.path:
    sys.path.insert(0, str(TESTS))
NATIVE_SESSION_ABI = 1
REQUIRED_INPROCESS_TESTS = (
    "NativeSessionTests.test_actual_library_abi_and_named_operations",
    "NativeSessionTests.test_helper_death_does_not_release_python_owned_lock",
    "ActualWorkerNativeSessionTests.test_actual_worker_lock_blocks_native_then_releases_same_file",
    "ActualWorkerNativeSessionTests.test_native_lock_blocks_actual_worker_before_domain_dispatch",
    "ActualWorkerNativeSessionTests.test_actual_provider_invocation_preserves_worker_before_domain_order",
    "ActualWorkerNativeSessionTests.test_actual_participant_invocation_preserves_worker_before_domain_order",
    "ActualWorkerNativeSessionTests.test_actual_connection_invocation_preserves_worker_before_domain_order",
    "ActualWorkerNativeSessionTests.test_domain_contention_refuses_all_three_actual_worker_invocations",
)


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_record(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


class RecordedResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.passed = []

    def addSuccess(self, test):
        self.passed.append(test.id())
        super().addSuccess(test)


def require_inprocess_passes(tests):
    """Missing, skipped or expected-failure ABI/ownership tests are not proof."""
    for required in REQUIRED_INPROCESS_TESTS:
        matching = [name for name in tests["passed"] if name.endswith("." + required)]
        if len(matching) != 1:
            raise ValueError("Required actual in-process qualification did not pass: " + required)
        for category in ("skipped", "failures", "errors", "expected_failures"):
            if any(row["test"].endswith("." + required) for row in tests.get(category, [])):
                raise ValueError("Required in-process qualification has an incomplete outcome: " + required)


def probe_library():
    """Actual ABI loading belongs to this test process, never the inspector."""
    import inspect_library

    library = Path(os.environ["CONSTELLATION_NATIVE_LIBRARY"])
    if not library.is_absolute():
        raise ValueError("Qualification requires an absolute native library path")
    library = library.resolve(strict=True)
    expected = os.environ["CONSTELLATION_NATIVE_LIBRARY_SHA256"]
    before = sha256(library)
    if before != expected:
        raise ValueError("Native library differs from the parent's exact qualification binding")
    inspection = inspect_library.inspect_library(library, system=platform.system(), machine=platform.machine())
    if inspection["library_sha256"] != before:
        raise ValueError("Native library changed before ABI loading")
    loaded = ctypes.CDLL(str(library))
    loaded.constellation_native_abi_version.argtypes = []
    loaded.constellation_native_abi_version.restype = ctypes.c_uint32
    observed = loaded.constellation_native_abi_version()
    if observed != NATIVE_SESSION_ABI:
        raise ValueError("Loaded native library ABI differs from the supported ABI")
    for name in ("constellation_native_open", "constellation_native_request", "constellation_native_close",
                 "constellation_native_buffer_free", "constellation_native_fork_prepare",
                 "constellation_native_fork_parent", "constellation_native_fork_child"):
        getattr(loaded, name)
    if sha256(library) != before:
        raise ValueError("Native library changed during ABI loading")
    return loaded, {"path": str(library), "sha256": before, "abi": observed,
                    "loaded_by_test_process": True, "inspection": inspection}


def execute_tests(output):
    """Runs inside the exact downloaded interpreter, including owned children."""
    # The receipt/log encoding is fixed independently of the runner's console
    # codepage; interpreter filesystem and subprocess behavior stay unchanged.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    os.environ["NATIVE_TEST_PYTHON"] = sys.executable
    os.environ["PYTHON"] = sys.executable
    os.environ["CONSTELLATION_INSTALLATION_SMOKE"] = "1"
    loaded_library, library_record = probe_library()
    result = unittest.TextTestRunner(verbosity=2, resultclass=RecordedResult).run(
        unittest.defaultTestLoader.discover(str(TESTS), pattern="test_*.py")
    )
    record = {
        "python_version": platform.python_version(),
        "sys_version": sys.version,
        "executable": sys.executable,
        "executable_sha256": sha256(sys.executable),
        "system": platform.system(),
        "machine": platform.machine(),
        "pointer_bits": struct.calcsize("P") * 8,
        "isolated": bool(sys.flags.isolated),
        "native_test_python": os.environ["NATIVE_TEST_PYTHON"],
        "installation_smoke": os.environ["CONSTELLATION_INSTALLATION_SMOKE"],
        "tests_run": result.testsRun,
        "successful": result.wasSuccessful(),
        "passed": result.passed,
        "failures": [{"test": test.id(), "traceback": trace} for test, trace in result.failures],
        "errors": [{"test": test.id(), "traceback": trace} for test, trace in result.errors],
        "skipped": [{"test": test.id(), "reason": reason} for test, reason in result.skipped],
        "expected_failures": [{"test": test.id(), "traceback": trace} for test, trace in result.expectedFailures],
        "unexpected_successes": [test.id() for test in result.unexpectedSuccesses],
        "native_library": library_record,
    }
    try:
        require_inprocess_passes(record)
        after = sha256(library_record["path"])
        record["native_library"]["sha256_after"] = after
        if after != library_record["sha256"]:
            raise ValueError("Native library changed during packaged Python tests")
        # Retain the same loaded image through the whole test suite; no handle
        # unloading or loading another artifact is part of this qualification.
        if loaded_library.constellation_native_abi_version() != NATIVE_SESSION_ABI:
            raise ValueError("Loaded native library ABI changed during tests")
    except Exception as error:
        record["successful"] = False
        record["inprocess_qualification_error"] = str(error)
    write_record(output, record)
    return 0 if record["successful"] else 1


def shared_python_support():
    path = ROOT / "packaging/python/python_support.py"
    spec = importlib.util.spec_from_file_location("native_test_python_support", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def pe_machine(path):
    with path.open("rb") as stream:
        header = stream.read(64)
        if len(header) != 64 or header[:2] != b"MZ":
            raise ValueError("Packaged Python is not a Windows executable")
        stream.seek(struct.unpack_from("<I", header, 60)[0])
        header = stream.read(6)
        if len(header) != 6 or header[:4] != b"PE\0\0":
            raise ValueError("Packaged Python PE header is invalid")
        return struct.unpack_from("<H", header, 4)[0]


def qualify(target, output, log):
    record = {"target": target, "outcome": "error", "production_operations": False}
    try:
        if os.name != "nt":
            raise ValueError("This exact consumer-interpreter qualification is Windows-only")
        lock = json.loads(LOCK.read_text(encoding="utf-8"))
        asset = next(asset for asset in lock["assets"] if asset["target"] == target)
        driver = Path(os.environ["CONSTELLATION_NATIVE_DRIVER"]).resolve(strict=True)
        service_fixture = Path(os.environ["NATIVE_SERVICE_FIXTURE"]).resolve(strict=True)
        library = Path(os.environ["CONSTELLATION_NATIVE_LIBRARY"])
        if not library.is_absolute():
            raise ValueError("Qualification requires an absolute native library path")
        library = library.resolve(strict=True)
        import inspect_library
        library_inspection = inspect_library.inspect_library(library, system=platform.system(), machine=platform.machine())
        instance_source = Path(os.environ["CONSTELLATION_INSTANCE_SOURCE"])
        if not instance_source.is_absolute() or not instance_source.is_dir():
            raise ValueError("Qualification requires the absolute pinned Instance source directory")
        instance_source = instance_source.resolve(strict=True)
        worker_pin_path = TESTS / "fixtures" / "worker_source.json"
        worker_pin = json.loads(worker_pin_path.read_text(encoding="utf-8"))
        worker_source_sha256 = sha256(instance_source / worker_pin["path"])
        if worker_source_sha256 != worker_pin["sha256"]:
            raise ValueError("Instance worker source differs from its exact qualification pin")
        record.update({
            "source": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "driver_path": str(driver),
            "driver_sha256": sha256(driver),
            "service_fixture_sha256": sha256(service_fixture),
            "library_path": str(library),
            "library_sha256": library_inspection["library_sha256"],
            "library_inspection": library_inspection,
            "required_native_session_abi": NATIVE_SESSION_ABI,
            "instance_source": str(instance_source),
            "worker_source_pin": worker_pin,
            "worker_source_pin_sha256": sha256(worker_pin_path),
            "worker_source_sha256": worker_source_sha256,
            "python_lock_sha256": sha256(LOCK),
            "packaging_support_sha256": sha256(ROOT / "packaging/python/python_support.py"),
            "python_version": lock["python_version"],
            "asset": asset,
            "provenance": lock["provenance"],
            "host_system": platform.system(),
            "host_machine": platform.machine(),
        })
        support = shared_python_support()
        with tempfile.TemporaryDirectory(prefix="constellation-pbs-312-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            scratch = Path(temporary).resolve()
            archive = support.download(asset, scratch / "cache")
            record["archive_sha256"] = sha256(archive)
            destination = scratch / "python"
            if destination.exists():
                raise ValueError("Packaged Python extraction destination must be fresh")
            # Reuse the bounded, checksummed archive reader. It creates files
            # exclusively and resolves only distributor-internal aliases.
            support.unpack_python(archive, destination)
            executable = destination / asset["executable"]
            if sha256(executable) != asset["executable_sha256"]:
                raise ValueError("Packaged Python differs from the retained consumer executable")
            if pe_machine(executable) != asset["pe_machine"]:
                raise ValueError("Packaged Python architecture differs from this target")
            child_record = scratch / "test-result.json"
            env = dict(os.environ, CONSTELLATION_NATIVE_DRIVER=str(driver), NATIVE_SERVICE_FIXTURE=str(service_fixture),
                       CONSTELLATION_NATIVE_LIBRARY=str(library),
                       CONSTELLATION_NATIVE_LIBRARY_SHA256=record["library_sha256"],
                       CONSTELLATION_INSTANCE_SOURCE=str(instance_source))
            with log.open("w", encoding="utf-8") as stream:
                run = subprocess.run(
                    [str(executable), "-I", "-B", str(Path(__file__).resolve()), "--execute", "--output", str(child_record)],
                    env=env, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT,
                    timeout=300, check=False,
                )
            record["returncode"] = run.returncode
            if not child_record.is_file():
                raise RuntimeError("Packaged interpreter did not produce its test result")
            tests = json.loads(child_record.read_text(encoding="utf-8"))
            record["tests"] = tests
            if tests["python_version"] != lock["python_version"] or tests["executable_sha256"] != asset["executable_sha256"]:
                raise ValueError("Executed interpreter provenance differs from the pinned package")
            if not tests["isolated"] or tests["native_test_python"] != tests["executable"]:
                raise ValueError("Tests did not select the isolated packaged interpreter")
            require_inprocess_passes(tests)
            if (tests["native_library"]["sha256"] != record["library_sha256"]
                    or tests["native_library"].get("sha256_after") != record["library_sha256"]
                    or tests["native_library"]["abi"] != NATIVE_SESSION_ABI
                    or tests["native_library"]["loaded_by_test_process"] is not True):
                raise ValueError("Actual packaged-Python library/ABI proof differs from the selected artifact")
            if not any("NativeServiceApplicationTests." in name for name in tests["passed"]):
                raise ValueError("Actual service/application fixture did not pass under packaged Python")
            if not any("WindowsFixtureOwnerTests." in name for name in tests["passed"]):
                raise ValueError("Actual Windows fixture ownership regression did not pass under packaged Python")
            owner_observations = []
            for line in log.read_text(encoding="utf-8").splitlines():
                try:
                    observation = json.loads(line)
                except ValueError:
                    continue
                if isinstance(observation, dict) and observation.get("windows_fixture_owner") == "passed":
                    owner_observations.append(observation)
            record["fixture_owner_observations"] = owner_observations
            if not owner_observations:
                raise ValueError("Windows fixture owner diagnostics were not retained")
            record["driver_sha256_after"] = sha256(driver)
            if record["driver_sha256_after"] != record["driver_sha256"]:
                raise ValueError("Native driver changed during packaged Python qualification")
            record["library_sha256_after"] = sha256(library)
            if record["library_sha256_after"] != record["library_sha256"]:
                raise ValueError("Native library changed during packaged Python qualification")
            record["worker_source_sha256_after"] = sha256(instance_source / worker_pin["path"])
            if record["worker_source_sha256_after"] != worker_source_sha256:
                raise ValueError("Pinned Instance worker changed during packaged Python qualification")
            record["outcome"] = "passed" if run.returncode == 0 and tests["successful"] else "failed"
    except Exception:
        record["error"] = traceback.format_exc()
        with log.open("a", encoding="utf-8") as stream:
            stream.write("\n" + record["error"])
    finally:
        if log.is_file():
            record["log_sha256"] = sha256(log)
        write_record(output, record)
    # Preserve fixture owner/DACL diagnostics in the ordinary CI log as well as
    # the artifact; the full test log is small and contains disposable state only.
    if log.is_file():
        print(log.read_text(encoding="utf-8"), end="")
    print(json.dumps({"target": target, "outcome": record["outcome"], "receipt": str(output)}))
    return 0 if record["outcome"] == "passed" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--log", type=Path)
    parser.add_argument("--execute", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.execute:
        raise SystemExit(execute_tests(args.output))
    if args.target is None or args.log is None:
        parser.error("--target and --log are required for qualification")
    raise SystemExit(qualify(args.target, args.output.resolve(), args.log.resolve()))
