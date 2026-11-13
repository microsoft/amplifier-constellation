"""Actual Instance caller/NativeSession lock conformance, no services or models.

Fetch the pinned Microsoft source outside this repository and supply its root
through CONSTELLATION_INSTANCE_SOURCE. No copied or substitute worker lock is
used: its actual flock/msvcrt single_worker and subprocess methods are imported.
The child domain adapter is intentionally a bounded local fixture; connector
install/update/Connect state machines remain separately qualified by owners.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "support" / "python"))
from test_driver import protect  # noqa: E402

FIXTURE = HERE / "fixtures" / "worker_boundary.py"
PIN = json.loads((HERE / "fixtures" / "worker_source.json").read_text())


class ActualWorkerNativeSessionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        required = ("CONSTELLATION_NATIVE_LIBRARY", "CONSTELLATION_NATIVE_DRIVER",
                    "CONSTELLATION_INSTANCE_SOURCE")
        missing = [name for name in required if not os.environ.get(name)]
        if missing:
            message = "Actual Worker/native coverage requires " + ", ".join(missing)
            if os.environ.get("CI", "").lower() in ("1", "true"):
                raise RuntimeError(message)
            raise unittest.SkipTest(message)
        cls.library = Path(os.environ[required[0]]).resolve()
        cls.driver = Path(os.environ[required[1]]).resolve()
        cls.instance_source = Path(os.environ[required[2]]).resolve()
        for path in (cls.library, cls.driver, cls.instance_source / PIN["path"]):
            if not path.is_file():
                raise RuntimeError("Missing actual conformance input: " + str(path))
        digest = hashlib.sha256((cls.instance_source / PIN["path"]).read_bytes()).hexdigest()
        if digest != PIN["sha256"]:
            raise RuntimeError("Instance source differs from " + PIN["commit"])
        from constellation_native import NativeSession, Busy
        cls.NativeSession = NativeSession
        cls.Busy = Busy

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="native-worker-boundary-",
                                               dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT"))
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        protect(self.root)
        self.worker_root = self.root / "worker"
        self.domain_root = self.root / "domain"
        self.envelopes = self.root / "envelopes"
        for directory in (self.worker_root, self.domain_root, self.envelopes):
            directory.mkdir()
            protect(directory)
        self.executable = self.root / self.driver.name
        shutil.copyfile(self.driver, self.executable)
        protect(self.executable)
        if os.name != "nt":
            self.executable.chmod(0o700)
        self.config = self.root / "owner.json"
        self.config.write_text(json.dumps({
            "version": 1,
            "roots": {"worker": str(self.worker_root), "state": str(self.domain_root)},
            "locks": {"worker": str(self.worker_root / "worker.lock"),
                      "installation": str(self.domain_root / "installation.lock")},
            "services": {}, "commands": {},
        }))
        protect(self.config)
        # Keep the existing worker/native lock file identity throughout each
        # test. Worker writes its byte before msvcrt locking on Windows.
        for path in (self.worker_root / "worker.lock", self.domain_root / "installation.lock"):
            path.write_bytes(b"0")
            protect(path)
        self.journal = self.domain_root / "fixture-state.json"
        self.journal.write_bytes(b'{"original":"unchanged"}')
        protect(self.journal)
        for name in ("fixture.json", "fixture.provider.json"):
            (self.envelopes / name).write_bytes(b"{}")
        self.plan = {
            "library": str(self.library), "driver": str(self.executable),
            "native_config": str(self.config), "instance_source": str(self.instance_source),
            "worker_root": str(self.worker_root), "domain_root": str(self.domain_root),
            "journal": str(self.journal), "envelopes": str(self.envelopes),
            "transaction": str(uuid.uuid4()), "entered": str(self.root / "entered.json"),
            "release": str(self.root / "release"), "refused": str(self.root / "refused.json"),
        }
        self.plan_file = self.root / "plan.json"
        self.plan_file.write_text(json.dumps(self.plan))
        self.initial_identity = self.identity()

    def identity(self):
        return [(path.stat().st_dev, path.stat().st_ino)
                for path in (self.worker_root / "worker.lock", self.domain_root / "installation.lock")]

    def session(self):
        return self.NativeSession(self.library, self.executable, self.config)

    def command(self, mode, role=None):
        argv = [sys.executable, str(FIXTURE), mode, str(self.plan_file)]
        if role is not None:
            argv.extend(("--role", role))
        return argv

    def completed(self, mode, role=None):
        result = subprocess.run(self.command(mode, role), capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return json.loads(result.stdout)

    def started(self, mode, role=None):
        process = subprocess.Popen(self.command(mode, role), stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        def cleanup():
            if process.poll() is None:
                # Only this exact disposable Popen child; no PID lookup or
                # production process. Normally the release marker settles it.
                process.terminate()
            try:
                process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate(timeout=5)
        self.addCleanup(cleanup)
        return process

    def entered(self, process):
        deadline = time.monotonic() + 8
        path = Path(self.plan["entered"])
        while True:
            if path.exists():
                try:
                    return json.loads(path.read_text())
                except json.JSONDecodeError:
                    # The independent observer may see create before the
                    # fixture's small write/flush completes.
                    pass
            if process.poll() is not None:
                out, err = process.communicate()
                self.fail("Fixture exited before its critical section: " + out + err)
            if time.monotonic() >= deadline:
                self.fail("Fixture did not enter its bounded critical section")
            time.sleep(0.02)

    def release(self, process):
        Path(self.plan["release"]).write_bytes(b"release")
        out, err = process.communicate(timeout=8)
        self.assertEqual(process.returncode, 0, out + err)
        self.assertEqual(self.identity(), self.initial_identity)
        return json.loads(out)

    def test_actual_worker_lock_blocks_native_then_releases_same_file(self):
        process = self.started("worker-hold")
        self.assertEqual(self.entered(process)["status"], "worker_held")
        with self.session() as native:
            with self.assertRaises(self.Busy):
                with native.lock("worker"):
                    self.fail("Actual worker lock must contend")
        self.release(process)
        with self.session() as native:
            with native.lock("worker"):
                self.assertEqual(self.identity(), self.initial_identity)

    def test_native_lock_blocks_actual_worker_before_domain_dispatch(self):
        before = self.journal.read_bytes()
        with self.session() as native:
            with native.lock("worker"):
                self.assertEqual(self.completed("worker-probe")["status"], "worker_busy")
                self.assertEqual(self.completed("worker-dispatch", "provider")["status"], "worker_busy")
                self.assertFalse(Path(self.plan["entered"]).exists())
                with native.lock("installation"):
                    self.assertEqual(self.journal.read_bytes(), before)
        self.assertEqual(self.completed("worker-probe")["status"], "worker_acquired")
        self.assertEqual(self.identity(), self.initial_identity)

    def _actual_dispatch_holds_both(self, role):
        process = self.started("worker-dispatch", role)
        observed = self.entered(process)
        self.assertEqual(observed["role"], role)
        self.assertTrue(observed["worker_lock_held_before_domain"])
        with self.session() as native:
            for name in ("worker", "installation"):
                with self.assertRaises(self.Busy):
                    with native.lock(name):
                        self.fail("Both actual critical sections must remain owned")
        result = self.release(process)
        self.assertEqual(result["status"], "dispatched")
        self.assertEqual(self.journal.read_bytes(), b'{"original":"unchanged"}\nfixture ' + role.encode())
        with self.session() as native:
            with native.lock("worker"):
                with native.lock("installation"):
                    self.assertEqual(self.identity(), self.initial_identity)

    def test_actual_provider_invocation_preserves_worker_before_domain_order(self):
        self._actual_dispatch_holds_both("provider")

    def test_actual_participant_invocation_preserves_worker_before_domain_order(self):
        self._actual_dispatch_holds_both("participant")

    def test_actual_connection_invocation_preserves_worker_before_domain_order(self):
        self._actual_dispatch_holds_both("connection")

    def test_domain_contention_refuses_all_three_actual_worker_invocations(self):
        before = self.journal.read_bytes()
        with self.session() as native:
            with native.lock("installation"):
                for role in ("provider", "participant", "connection"):
                    with self.subTest(role=role):
                        self.assertEqual(self.completed("worker-dispatch", role),
                                         {"status": "domain_busy", "role": role})
                        self.assertFalse(Path(self.plan["entered"]).exists())
                        self.assertEqual(self.journal.read_bytes(), before)
                        Path(self.plan["refused"]).unlink()
                        # Failed invocation released the outer real worker lock;
                        # the still-running caller retains its domain lock.
                        with native.lock("worker"):
                            self.assertEqual(self.identity(), self.initial_identity)
        self.assertEqual(self.journal.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
