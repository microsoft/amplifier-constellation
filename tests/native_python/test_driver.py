"""Actual local driver fixtures. They never touch an enrolled live installation."""

from __future__ import annotations

import csv
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

SUPPORT = Path(__file__).resolve().parents[2] / "support" / "python"
sys.path.insert(0, str(SUPPORT))
from constellation_native import Busy, DriverError, Refused, Session  # noqa: E402

DRIVER = os.environ.get("CONSTELLATION_NATIVE_DRIVER")


def protect(path: Path) -> None:
    """Set private fixture permissions, using the actual platform mechanism."""
    if os.name != "nt":
        path.chmod(0o700 if path.is_dir() else 0o600)
        return
    rows = subprocess.check_output(
        ["whoami.exe", "/user", "/fo", "csv", "/nh"], text=True
    )
    sid = next(csv.reader(io.StringIO(rows.strip())))[1]
    inheritance = "(OI)(CI)" if path.is_dir() else ""
    subprocess.run(
        [
            "icacls.exe",
            str(path),
            "/inheritance:r",
            "/grant:r",
            f"*{sid}:{inheritance}F",
            f"*S-1-5-18:{inheritance}F",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )


@unittest.skipUnless(
    DRIVER, "Set CONSTELLATION_NATIVE_DRIVER to test the actual native binary"
)
class ActualDriverTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(
            prefix="constellation-native-test-",
            dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT"),
        )
        self.addCleanup(self.temp.cleanup)
        # macOS's temporary path can have a /var -> /private/var ancestor.
        self.root = Path(self.temp.name).resolve()
        protect(self.root)
        self.state = self.root / "state"
        self.state.mkdir()
        protect(self.state)
        self.config = self.root / "owner.json"
        self.data = {
            "version": 1,
            "roots": {"state": str(self.state)},
            "locks": {"installation": str(self.state / "installation.lock")},
            "services": {},
            "commands": {},
        }
        self.save_config()

    def save_config(self):
        self.config.write_text(json.dumps(self.data), encoding="utf-8")
        protect(self.config)

    def session(self):
        session = Session(str(Path(DRIVER).resolve()), self.config, timeout=10)
        self.addCleanup(session.close)
        return session

    def test_describe_and_generation_bound_process_observation(self):
        session = self.session()
        description = session.describe()
        self.assertEqual(description["protocol"], 1)
        identity = session.process_identity(os.getpid())
        self.assertEqual(identity["pid"], os.getpid())
        self.assertIs(session.process_matches(identity), True)
        self.assertIs(session.process_status(identity)["matches"], True)
        self.assertGreater(len(identity), 1)
        generation = next(
            key
            for key in ("start_ticks", "start_seconds", "creation_time_100ns")
            if key in identity
        )
        changed = dict(identity)
        changed[generation] = str(int(changed[generation]) + 1)
        self.assertIs(session.process_matches(changed), False)
        if sys.platform.startswith("linux"):
            self.assertEqual(set(identity), {"pid", "start_ticks", "boot_id"})
            self.assertIsInstance(identity["start_ticks"], str)

    def test_two_real_driver_invocations_contend_on_same_existing_lock(self):
        first, second = self.session(), self.session()
        with first.lock("installation"):
            with self.assertRaises(Busy):
                with second.lock("installation"):
                    self.fail("Two independent invocations acquired one lock")
            original = (self.state / "installation.lock").stat()
        with second.lock("installation"):
            current = (self.state / "installation.lock").stat()
            self.assertEqual(
                (original.st_dev, original.st_ino), (current.st_dev, current.st_ino)
            )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "Existing flock interoperability is Linux-specific",
    )
    def test_existing_python_flock_and_native_lock_interoperate(self):
        import fcntl

        path = self.state / "installation.lock"
        fd = os.open(path, os.O_CREAT | os.O_RDWR, 0o600)
        self.addCleanup(os.close, fd)
        native = self.session()
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try:
            with self.assertRaises(Busy):
                with native.lock("installation"):
                    self.fail("Native lock ignored the existing flock owner")
        finally:
            fcntl.flock(fd, fcntl.LOCK_UN)
        with native.lock("installation"):
            with self.assertRaises(BlockingIOError):
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        fcntl.flock(fd, fcntl.LOCK_UN)

    def test_python_client_crash_releases_invocation_lock(self):
        code = """
import sys, time
sys.path.insert(0, sys.argv[1])
from constellation_native import Session
with Session(sys.argv[2], sys.argv[3]) as native:
    with native.lock("installation"):
        print("locked", flush=True)
        time.sleep(60)
"""
        process = subprocess.Popen(
            [
                sys.executable,
                "-c",
                code,
                str(SUPPORT),
                str(Path(DRIVER).resolve()),
                str(self.config),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            import threading
            import queue

            ready = queue.Queue()
            threading.Thread(
                target=lambda: ready.put(process.stdout.readline()), daemon=True
            ).start()
            self.assertEqual(ready.get(timeout=10), "locked\n")
            second = self.session()
            with self.assertRaises(Busy):
                with second.lock("installation"):
                    self.fail("Client crashed before the contender observed contention")
            process.kill()
            process.wait(timeout=5)
            deadline = time.monotonic() + 5
            while True:
                try:
                    with second.lock("installation"):
                        break
                except Busy:
                    if time.monotonic() > deadline:
                        self.fail(
                            "Driver retained the lock after the Python client died"
                        )
                    time.sleep(0.05)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)
            process.stdout.close()
            process.stderr.close()

    def test_storage_preserves_bytes_and_refuses_traversal(self):
        native = self.session()
        data = b'{"session":"original", "authority":"unchanged"}\n'
        native.private_write("state", "receipt.json", data, create_only=True)
        self.assertEqual((self.state / "receipt.json").read_bytes(), data)
        with self.assertRaises(DriverError):
            native.private_write(
                "state", "receipt.json", b"replacement", create_only=True
            )
        self.assertEqual((self.state / "receipt.json").read_bytes(), data)
        for path in ("../escape.json", str(self.root / "escape.json")):
            with self.subTest(path=path):
                with self.assertRaises(Refused):
                    native.private_write("state", path, b"escape")
        self.assertFalse((self.root / "escape.json").exists())
        native.durable_remove("state", "receipt.json")
        self.assertFalse((self.state / "receipt.json").exists())

    def test_private_directory_then_receipt_preserves_existing_private_directory(self):
        native = self.session()
        parent_mode = self.state.stat().st_mode
        self.assertEqual(native.private_dir("state", "transaction"), {})
        directory = self.state / "transaction"
        self.assertTrue(directory.is_dir())
        original = directory.stat()
        if os.name != "nt":
            self.assertEqual(original.st_mode & 0o077, 0)
        native.private_write("state", "transaction/receipt.json", b"original")
        self.assertEqual(native.private_dir("state", "transaction"), {})
        current = directory.stat()
        self.assertEqual(
            (current.st_dev, current.st_ino, current.st_mode),
            (original.st_dev, original.st_ino, original.st_mode),
        )
        self.assertEqual((directory / "receipt.json").read_bytes(), b"original")
        self.assertEqual(self.state.stat().st_mode, parent_mode)

    def test_private_directory_refuses_missing_parent_and_existing_file(self):
        native = self.session()
        native.private_write("state", "receipt", b"existing")
        for path in ("missing/transaction", "../escape", "receipt"):
            with self.subTest(path=path):
                with self.assertRaises(Refused):
                    native.private_dir("state", path)
        self.assertFalse((self.state / "missing").exists())
        self.assertFalse((self.root / "escape").exists())
        self.assertEqual((self.state / "receipt").read_bytes(), b"existing")

    @unittest.skipIf(
        os.name == "nt", "POSIX permission drift; Windows ACL coverage is native"
    )
    def test_private_directory_refuses_to_change_existing_nonprivate_permissions(self):
        directory = self.state / "existing"
        directory.mkdir(mode=0o700)
        directory.chmod(0o755)
        native = self.session()
        with self.assertRaises(Refused):
            native.private_dir("state", "existing")
        self.assertEqual(directory.stat().st_mode & 0o777, 0o755)

    @unittest.skipIf(os.name == "nt", "Windows reparse fixture needs symlink privilege")
    def test_storage_refuses_symlink_ancestor(self):
        outside = self.root / "outside"
        outside.mkdir(mode=0o700)
        (self.state / "link").symlink_to(outside, target_is_directory=True)
        native = self.session()
        with self.assertRaises(Refused):
            native.private_write("state", "link/escaped.json", b"no")
        self.assertFalse((outside / "escaped.json").exists())

    def test_config_change_refuses_subsequent_dispatch(self):
        native = self.session()
        native.describe()
        self.data["roots"]["second"] = str(self.state)
        self.save_config()
        with self.assertRaises(Refused):
            native.private_write("state", "receipt.json", b"no")
        self.assertFalse((self.state / "receipt.json").exists())

    def test_unknown_request_fields_refuse_before_any_dispatch(self):
        native = self.session()
        for op, fields in (
            ("describe", {"argv": ["not-an-enrolled-command"]}),
            (
                "lock.acquire",
                {"name": "installation", "path": str(self.state / "other.lock")},
            ),
            ("process.launch", {"name": "uninstalled", "shell": "never execute"}),
            (
                "storage.mkdir",
                {"root": "state", "path": "transaction", "parents": True},
            ),
        ):
            with self.subTest(op=op):
                with self.assertRaises(Refused) as caught:
                    native._call(op, mutating=True, **fields)
                self.assertEqual(caught.exception.code, "invalid_request")
        with native.lock("installation"):
            self.assertFalse((self.state / "other.lock").exists())

    def test_changed_config_still_allows_owned_lock_release(self):
        native = self.session()
        with native.lock("installation"):
            self.data["roots"]["second"] = str(self.state)
            self.save_config()
        contender = self.session()
        with contender.lock("installation"):
            pass

    def test_detached_probe_survives_invocation_without_inheriting_its_lock(self):
        # This is an actual child-process lifetime fixture, not an ACP/model run.
        code = """
import json, os, sys, time
from pathlib import Path
result = Path(sys.argv[1])
result.with_suffix(".started").write_text(str(os.getpid()), encoding="utf-8")
while not result.with_suffix(".release").exists():
    time.sleep(0.02)
result.write_text(json.dumps({"completed": True}), encoding="utf-8")
"""
        self.data["commands"]["probe"] = {
            "executable": str(Path(sys.executable).resolve()),
            "args": ["-c", code, "{result}"],
            "cwd": str(self.root),
            "timeout_seconds": 15,
            "result_root": "state",
        }
        self.save_config()
        launcher = self.session()
        with launcher.lock("installation"):
            identity = launcher.launch("probe", result="probe.json")
            self.assertIsInstance(identity["pid"], int)
        launcher.close()
        observer = self.session()
        with observer.lock("installation"):
            # Lock acquisition must not wait for the detached command to finish.
            self.assertFalse((self.state / "probe.json").exists())
            (self.state / "probe.release").touch()
        deadline = time.monotonic() + 8
        while not (self.state / "probe.json").exists():
            if time.monotonic() > deadline:
                self.fail(
                    "Detached fixed command did not survive the launching invocation"
                )
            time.sleep(0.05)
        self.assertEqual(
            json.loads((self.state / "probe.json").read_text()), {"completed": True}
        )
        while observer.process_matches(identity):
            if time.monotonic() > deadline:
                self.fail("Detached supervisor remained after its command completed")
            time.sleep(0.05)


if __name__ == "__main__":
    unittest.main()
