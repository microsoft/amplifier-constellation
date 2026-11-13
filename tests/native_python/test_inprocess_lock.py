"""Actual caller-owned library locks in disposable directories; no live services."""

from __future__ import annotations

import contextlib
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock

TESTS = Path(__file__).resolve().parent
sys.path.insert(0, str(TESTS))
sys.path.insert(0, str(TESTS.parents[1] / "support/python"))
from constellation_native import Busy, DriverError, NativeSession, Refused, Session, Unknown
from constellation_native import inprocess
from test_driver import protect

LIBRARY = os.environ.get("CONSTELLATION_NATIVE_LIBRARY")
DRIVER = os.environ.get("CONSTELLATION_NATIVE_DRIVER")
PYTHON = os.environ.get("NATIVE_TEST_PYTHON", sys.executable)


def _wait_file(path, process, timeout=10):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        if path.is_file():
            return
        if process.poll() is not None:
            raise AssertionError(f"Owned fixture exited before readiness: {process.returncode}")
        time.sleep(0.01)
    raise AssertionError("Owned fixture did not reach readiness")


def _wait_fork(pid, timeout=5):
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        found, status = os.waitpid(pid, os.WNOHANG)
        if found:
            return os.waitstatus_to_exitcode(status)
        time.sleep(0.01)
    os.kill(pid, signal.SIGKILL)  # This exact test-created, unreaped child only.
    os.waitpid(pid, 0)
    raise AssertionError("Owned fork child exceeded its fixture deadline")


class NativeSessionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not LIBRARY or not DRIVER:
            if os.environ.get("CI", "").lower() == "true":
                raise AssertionError("CI must supply the exact native library and driver")
            raise unittest.SkipTest("Set CONSTELLATION_NATIVE_LIBRARY and CONSTELLATION_NATIVE_DRIVER")
        cls.library = str(Path(LIBRARY).resolve(strict=True))
        cls.driver_input = Path(DRIVER).resolve(strict=True)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(
            prefix="native-inprocess-", dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
        )
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        protect(self.root)
        self.state = self.root / "state"
        self.state.mkdir()
        protect(self.state)
        self.driver = self.root / ("driver.exe" if os.name == "nt" else "driver")
        shutil.copyfile(self.driver_input, self.driver)
        protect(self.driver)
        if os.name != "nt":
            self.driver.chmod(0o700)
        self.config = self.root / "owner.json"
        self.configuration = {
            "version": 1, "roots": {"state": str(self.state)},
            "locks": {"installation": str(self.state / "installation.lock"),
                      "other": str(self.state / "other.lock")},
            "services": {}, "commands": {},
        }
        self.config.write_text(json.dumps(self.configuration), encoding="utf-8")
        protect(self.config)
        self.first = self.session()
        self.second = self.session()

    def session(self):
        value = NativeSession(self.library, self.driver, self.config)
        self.addCleanup(value.close)
        return value

    def assert_contended(self):
        with self.assertRaises(Busy):
            with self.second.lock("installation"):
                self.fail("Contender entered another caller's critical section")

    def assert_available(self):
        with self.second.lock("installation"):
            self.second.private_write("state", "second.json", b'{"owner":"second"}')

    def test_actual_library_abi_and_named_operations(self):
        self.assertEqual(self.first._library.version(), 1)
        self.assertEqual(self.first.describe()["protocol"], 1)
        self.assertFalse(hasattr(self.first, "_process"))
        with self.first.lock("installation"):
            self.assert_contended()
            self.first.private_dir("state", "nested")
            self.first.private_write("state", "nested/state.json", b'{"observed":true}')
            self.assertEqual((self.state / "nested/state.json").read_bytes(), b'{"observed":true}')
            identity = self.first.process_identity(os.getpid())
            self.assertTrue(self.first.process_matches(identity))
            self.first.durable_remove("state", "nested/state.json")
        self.assert_available()

    def test_helper_death_does_not_release_python_owned_lock(self):
        # This child is a real transport driver, separate from the caller-owned
        # session. Its death must not release this Python process's kernel lock.
        helper = Session(self.driver, self.config, timeout=10)
        self.addCleanup(helper.close)
        helper.describe()
        with self.first.lock("installation"):
            with self.assertRaises(Busy):
                with helper.lock("installation"):
                    self.fail("Legacy transport bypassed the native owner")
            helper._process.kill()
            helper._process.wait(timeout=5)
            self.assert_contended()
            (self.state / "caller-still-owns.json").write_text("{}", encoding="utf-8")
            self.assert_contended()
        self.assert_available()

    def test_other_thread_close_waits_for_whole_python_section(self):
        entered, finished = threading.Event(), threading.Event()
        errors = []
        def closing():
            entered.set()
            try:
                self.first.close()
            except BaseException as error:
                errors.append(error)
            finally:
                finished.set()
        with self.first.lock("installation"):
            thread = threading.Thread(target=closing)
            thread.start()
            self.assertTrue(entered.wait(2))
            self.assertFalse(finished.wait(0.1))
            self.assert_contended()
            (self.state / "direct.json").write_text("{}", encoding="utf-8")
        thread.join(5)
        self.assertFalse(thread.is_alive())
        self.assertEqual(errors, [])
        self.assert_available()

    def test_close_and_same_name_nesting_cannot_unlock_live_scope(self):
        with self.first.lock("installation"):
            with self.assertRaises(Busy) as closed:
                self.first.close()
            self.assertEqual(closed.exception.code, "locks_held")
            with self.assertRaises(DriverError) as nested:
                with self.first.lock("installation"):
                    self.fail("Nested acquisition succeeded")
            self.assertEqual(nested.exception.code, "lock_reentry")
            with self.first.lock("other"):
                self.assert_contended()
            self.assert_contended()
        self.assert_available()

    def test_body_exception_releases_only_at_scope_exit(self):
        sentinel = RuntimeError("fixture caller failure")
        with self.assertRaises(RuntimeError) as caught:
            with self.first.lock("installation"):
                self.assert_contended()
                raise sentinel
        self.assertIs(caught.exception, sentinel)
        self.assert_available()

    def test_configuration_drift_refuses_work_but_release_remains_possible(self):
        original = self.config.read_bytes()
        with self.first.lock("installation"):
            self.config.write_bytes(original + b"\n")
            with self.assertRaises(Refused) as caught:
                self.first.describe()
            self.assertEqual(caught.exception.code, "configuration_changed")
            # Restoring bytes here is only this test's disposable setup. Release
            # below is deliberately executed while enrollment still differs.
        self.config.write_bytes(original)
        self.assert_available()

    def test_driver_drift_refuses_work_but_release_remains_possible(self):
        original = self.driver.read_bytes()
        with self.first.lock("installation"):
            self.driver.write_bytes(original + b"fixture-drift")
            with self.assertRaises(Refused) as caught:
                self.first.describe()
            self.assertEqual(caught.exception.code, "driver_changed")
        self.driver.write_bytes(original)
        self.assert_available()

    def test_lost_mutating_reply_poison_retains_lock_until_unwind(self):
        library = self.first._library
        invoke = library.invoke
        writes = []
        def lost_once(function, *args):
            raw = invoke(function, *args)
            if function is library.request:
                request = json.loads(args[1])
                if request["op"] == "storage.write":
                    writes.append(request["id"])
                    if len(writes) == 1:
                        return b'{"version":1,"id":"wrong","outcome":"ok","result":{}}'
            return raw
        with self.first.lock("installation"):
            with mock.patch.object(library, "invoke", side_effect=lost_once):
                with self.assertRaises(Unknown) as caught:
                    self.first.private_write("state", "committed.json", b"{}")
            self.assertEqual(caught.exception.code, "protocol_response")
            self.assertTrue((self.state / "committed.json").is_file())
            self.assert_contended()
            with self.assertRaises(Unknown):
                self.first.describe()
            (self.state / "owned-direct-write").write_bytes(b"still excluded")
            self.assert_contended()
        self.assertEqual(len(writes), 1)
        self.assert_available()

    def test_unknown_acquisition_unwinds_once_without_entering_body(self):
        library = self.first._library
        invoke = library.invoke
        operations = []
        def lost_acquire(function, *args):
            raw = invoke(function, *args)
            if function is library.request and args[0] == self.first._handle.value:
                op = json.loads(args[1])["op"]
                operations.append(op)
                if op == "lock.acquire":
                    return b"not-json"
            return raw
        with mock.patch.object(library, "invoke", side_effect=lost_acquire):
            with self.assertRaises(Unknown):
                with self.first.lock("installation"):
                    self.fail("Unacknowledged acquisition entered caller work")
        self.assertEqual(operations, ["lock.acquire", "lock.release"])
        self.assert_available()

    def test_decode_interrupt_after_write_retains_lock_and_poisons_dispatch(self):
        import inspect
        source, first_line = inspect.getsourcelines(NativeSession._call)
        call_line = first_line + next(
            index for index, line in enumerate(source) if "return self._guarded_decode(" in line
        )
        for phase in ("during_decode", "before_decode"):
            with self.subTest(phase=phase):
                session = self.first if phase == "during_decode" else self.session()
                interruption = KeyboardInterrupt("completed write " + phase)
                library = session._library
                invoke, decode = library.invoke, session._decode
                writes, decoded_writes, trace_hits = [], [], []

                def observed(function, *args):
                    raw = invoke(function, *args)
                    if function is library.request and args[0] == session._handle.value:
                        request = json.loads(args[1])
                        if request["op"] == "storage.write":
                            writes.append(request["id"])
                    return raw

                def interrupted(raw, request_id, *, mutating):
                    if request_id in writes:
                        decoded_writes.append(request_id)
                        if phase == "during_decode":
                            raise interruption
                    return decode(raw, request_id, mutating=mutating)

                def trace(frame, event, _arg):
                    if (event == "line" and frame.f_code is NativeSession._call.__code__
                            and frame.f_lineno == call_line
                            and frame.f_locals.get("op") == "storage.write"):
                        trace_hits.append(True)
                        raise interruption
                    return trace

                committed, forbidden = phase + ".json", phase + "-forbidden.json"
                with mock.patch.object(library, "invoke", side_effect=observed), \
                     mock.patch.object(session, "_decode", side_effect=interrupted):
                    with session.lock("installation"):
                        with self.assertRaises(KeyboardInterrupt) as caught:
                            previous_trace = sys.gettrace()
                            try:
                                if phase == "before_decode":
                                    sys.settrace(trace)
                                session.private_write("state", committed, b'{"committed":true}')
                            finally:
                                sys.settrace(previous_trace)
                        self.assertIs(caught.exception, interruption)
                        self.assertEqual((self.state / committed).read_bytes(), b'{"committed":true}')
                        self.assert_contended()
                        for operation in (
                            session.describe,
                            lambda: session.private_write("state", forbidden, b"{}"),
                        ):
                            with self.assertRaises(Unknown) as refused:
                                operation()
                            self.assertEqual(refused.exception.code, "session_poisoned")
                            self.assert_contended()
                        self.assertFalse((self.state / forbidden).exists())
                self.assertEqual(len(writes), 1)
                self.assertEqual(len(decoded_writes), 1 if phase == "during_decode" else 0)
                self.assertEqual(len(trace_hits), 1 if phase == "before_decode" else 0)
                self.assert_available()

    def test_interrupt_after_native_acquisition_frees_buffers_and_releases_once(self):
        library = self.first._library
        native_request = library.request
        native_free = library.free
        operations, buffers = [], []
        def interrupted(handle, raw, length, out):
            result = native_request(handle, raw, length, out)
            op = json.loads(raw)["op"]
            operations.append(op)
            if op == "lock.acquire":
                raise KeyboardInterrupt("after actual native acquisition")
            return result
        def freed(buffer):
            buffers.append((buffer.data, buffer.len))
            native_free(buffer)
        with mock.patch.object(library, "request", side_effect=interrupted), \
             mock.patch.object(library, "free", side_effect=freed):
            with self.assertRaises(KeyboardInterrupt):
                with self.first.lock("installation"):
                    self.fail("Interrupted acquisition entered caller work")
        self.assertEqual(operations, ["lock.acquire", "lock.release"])
        self.assertEqual(len(buffers), 2)
        self.assertTrue(all(pointer and length for pointer, length in buffers))
        self.assert_available()

    def test_release_reply_loss_preserves_primary_and_explicit_close_reconciles(self):
        sentinel = RuntimeError("original caller failure")
        library = self.first._library
        invoke = library.invoke
        def lost_release(function, *args):
            raw = invoke(function, *args)
            if function is library.request and json.loads(args[1])["op"] == "lock.release":
                return b"bad-release-reply"
            return raw
        with self.assertRaises(RuntimeError) as caught:
            with mock.patch.object(library, "invoke", side_effect=lost_release):
                with self.first:
                    with self.first.lock("installation"):
                        raise sentinel
        self.assertIs(caught.exception, sentinel)
        self.assert_available()

    def test_unsupported_timeouts_reject_before_loading_or_dispatch(self):
        with mock.patch.object(inprocess, "_load") as loading:
            with self.assertRaises(TypeError):
                NativeSession(self.library, self.driver, self.config, timeout=30)
            loading.assert_not_called()
        with mock.patch.object(self.first._library, "invoke") as called:
            with self.assertRaises(TypeError):
                self.first.tree_remove("stages", "entry", {}, timeout=120)
            with self.assertRaises(TypeError):
                self.first.tree_remove("stages", "entry", {}, timeout=None)
            called.assert_not_called()

    def test_invalid_library_path_and_request_reject_before_effect(self):
        with self.assertRaises(ValueError):
            NativeSession("relative-library", self.driver, self.config)
        with self.assertRaises(Refused):
            NativeSession(self.state, self.driver, self.config)
        with self.assertRaises(ValueError):
            self.first.private_write("state", "oversized", b"x" * (6 * 1024 * 1024))
        self.assertFalse((self.state / "oversized").exists())

    def test_actual_caller_death_releases_ownership(self):
        marker = self.state / "child-ready"
        with tempfile.TemporaryFile() as log:
            process = subprocess.Popen(
                [PYTHON, "-I", str(Path(__file__).resolve()), "--child-hold",
                 self.library, str(self.driver), str(self.config), str(marker)],
                stdin=subprocess.DEVNULL, stdout=log, stderr=log, close_fds=True,
            )
            try:
                _wait_file(marker, process)
                self.assert_contended()
                process.kill()
                process.wait(timeout=5)
                self.assert_available()
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)

    def test_subprocess_exec_does_not_unlock_parent(self):
        with self.first.lock("installation"):
            run = subprocess.run(
                [PYTHON, "-I", "-c", "print('owned-child')"],
                capture_output=True, timeout=5, close_fds=True,
            )
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assert_contended()

    def test_fixed_detached_probe_survives_close_without_inheriting_lock(self):
        self.first.close()
        self.second.close()
        code = """
import json, os, sys, time
from pathlib import Path
result = Path(sys.argv[1])
result.with_suffix('.started').write_text(str(os.getpid()), encoding='utf-8')
while not result.with_suffix('.release').exists():
    time.sleep(0.02)
result.write_text(json.dumps({'completed': True}), encoding='utf-8')
"""
        self.configuration["commands"]["probe"] = {
            "executable": str(Path(PYTHON).resolve()),
            "args": ["-I", "-c", code, "{result}"],
            "cwd": str(self.root), "timeout_seconds": 15, "result_root": "state",
        }
        self.config.write_text(json.dumps(self.configuration), encoding="utf-8")
        self.first, self.second = self.session(), self.session()
        identity = None
        try:
            with self.first.lock("installation"):
                identity = self.first.launch("probe", result="probe.json")
                self.assertIsInstance(identity["pid"], int)
            self.first.close()
            with self.second.lock("installation"):
                self.assertFalse((self.state / "probe.json").exists())
                # Acquiring this actual lock while the probe still waits proves
                # it inherited no action ownership from the Python process.
                (self.state / "probe.release").touch()
            until = time.monotonic() + 10
            while not (self.state / "probe.json").exists():
                if time.monotonic() > until:
                    self.fail("Fixed detached probe did not complete")
                time.sleep(0.02)
            self.assertEqual(json.loads((self.state / "probe.json").read_text()), {"completed": True})
        finally:
            if identity is not None:
                (self.state / "probe.release").touch(exist_ok=True)
                until = time.monotonic() + 25
                while self.second.process_matches(identity):
                    if time.monotonic() > until:
                        self.fail("Owned detached supervisor did not settle within its native bound")
                    time.sleep(0.05)

    @unittest.skipUnless(hasattr(os, "fork"), "Unix fork contract")
    def test_python_returning_fork_cannot_continue_a_locked_scope(self):
        marker = self.state / "forbidden-child-write"
        with self.first.lock("installation"):
            pid = os.fork()
            if pid == 0:
                marker.write_bytes(b"unsafe")
                os._exit(99)
            self.assertEqual(_wait_fork(pid), 125)
            self.assertFalse(marker.exists())
            self.assert_contended()
        self.assert_available()

    @unittest.skipUnless(hasattr(os, "fork"), "Unix fork contract")
    def test_child_rejects_pid_before_inherited_python_mutex(self):
        ready, finish = threading.Event(), threading.Event()
        def hold_python_mutex():
            with self.first._mutex:
                ready.set()
                finish.wait(10)
        thread = threading.Thread(target=hold_python_mutex)
        thread.start()
        self.assertTrue(ready.wait(2))
        try:
            pid = os.fork()
            if pid == 0:
                signal.alarm(3)
                try:
                    for operation in (self.first.describe, self.first.close):
                        try:
                            operation()
                        except DriverError as error:
                            if error.code != "forked_session":
                                os._exit(81)
                        else:
                            os._exit(82)
                except BaseException:
                    os._exit(83)
                os._exit(0)
            self.assertEqual(_wait_fork(pid), 0)
        finally:
            finish.set()
            thread.join(5)
        self.assertFalse(thread.is_alive())
        self.assertEqual(self.first.describe()["protocol"], 1)
        self.assert_available()

    @unittest.skipUnless(hasattr(os, "fork"), "Unix fork contract")
    def test_unlocked_fork_child_opens_fresh_native_session(self):
        # Separate the immediate inherited-mutex refusal from real driver hash
        # and filesystem work. Debug artifacts need several seconds for those
        # independent operations; this is a fixture bound, not an ABI timeout.
        pid = os.fork()
        if pid == 0:
            signal.alarm(10)
            try:
                with NativeSession(self.library, self.driver, self.config) as fresh:
                    with fresh.lock("installation"):
                        fresh.private_write("state", "fresh-child.json", b"{}")
            except BaseException:
                os._exit(83)
            os._exit(0)
        self.assertEqual(_wait_fork(pid, timeout=15), 0)
        self.assertEqual((self.state / "fresh-child.json").read_bytes(), b"{}")
        self.assert_available()

    @unittest.skipUnless(hasattr(os, "fork"), "Unix fork contract")
    def test_same_library_alias_registers_native_fork_hooks_once(self):
        # Exercise image-handle deduplication after a distinct path lookup,
        # rather than a normalized spelling that only hits the path cache.
        # The injected loader returns the already loaded *actual* native DLL;
        # this is a handle-selection test, not an OS alias-loading claim.
        alias = self.state / "loader-alias"
        alias.write_bytes(b"fixture loader identity")
        self.assertNotIn(str(alias.resolve()), inprocess._PATHS)
        with mock.patch.object(inprocess.ctypes, "CDLL", return_value=self.first._library.dll) as load:
            other = NativeSession(alias, self.driver, self.config)
        load.assert_called_once()
        self.addCleanup(other.close)
        self.assertIs(other._library, self.first._library)
        library = self.first._library
        prepare = library.fork_prepare
        calls = []
        def once():
            calls.append(True)
            prepare()
        with mock.patch.object(library, "fork_prepare", side_effect=once):
            pid = os.fork()
            if pid == 0:
                os._exit(0)
            self.assertEqual(_wait_fork(pid), 0)
        self.assertEqual(calls, [True])


if __name__ == "__main__":
    if len(sys.argv) == 6 and sys.argv[1] == "--child-hold":
        library, driver, config, marker = sys.argv[2:]
        with NativeSession(library, driver, config) as session:
            with session.lock("installation"):
                Path(marker).write_bytes(b"owned")
                time.sleep(30)
        raise SystemExit(0)
    unittest.main()
