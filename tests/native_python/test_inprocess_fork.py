"""Bounded actual fork-preparation failures; disposable locks, no services."""

from __future__ import annotations

import contextlib
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_inprocess_lock as lock_cases

inprocess = lock_cases.inprocess
_ISOLATED = len(sys.argv) == 3 and sys.argv[1] == "--case"


def isolated(operation):
    """A broken native gate must fail this case, never hang the test runner."""
    def run(self):
        if _ISOLATED:
            return operation(self)
        child = subprocess.Popen(
            [lock_cases.PYTHON, "-I", str(Path(__file__).resolve()),
             "--case", operation.__name__],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, start_new_session=True,
        )
        timed_out = False
        try:
            stdout, stderr = child.communicate(timeout=20)
        except subprocess.TimeoutExpired:
            timed_out = True
            # This exact fixture-created process group contains only its own
            # bounded fork child. No PID search or unrelated process is used.
            os.killpg(child.pid, signal.SIGKILL)
            stdout, stderr = child.communicate(timeout=5)
        self.assertFalse(timed_out, "Fork fixture exceeded its 20-second bound")
        self.assertEqual(child.returncode, 0, (stdout + stderr).decode(errors="replace"))
    run.__name__ = operation.__name__
    return run


@unittest.skipUnless(hasattr(os, "fork"), "Unix Python-returning fork contract")
class NativeSessionForkTests(unittest.TestCase):
    # Reuse only setup/helpers. Do not inherit or re-export its twenty tests.
    setUpClass = classmethod(lock_cases.NativeSessionTests.setUpClass.__func__)
    session = lock_cases.NativeSessionTests.session
    assert_contended = lock_cases.NativeSessionTests.assert_contended
    assert_available = lock_cases.NativeSessionTests.assert_available

    def setUp(self):
        if _ISOLATED:
            lock_cases.NativeSessionTests.setUp(self)

    @contextlib.contextmanager
    def capture_unraisable(self):
        caught = []
        with mock.patch.object(sys, "unraisablehook", side_effect=caught.append):
            yield caught

    def child_must_not_enter(self):
        marker = self.state / "forbidden-child-body"
        child = os.fork()
        if child == 0:
            marker.write_bytes(b"copied caller work")
            os._exit(99)
        self.assertEqual(lock_cases._wait_fork(child), 125)
        self.assertFalse(marker.exists())

    @isolated
    def test_interrupted_cache_acquisition_fails_child_closed(self):
        class InterruptedMutex:
            def __enter__(self):
                raise KeyboardInterrupt("controlled cache-acquisition interruption")

            def __exit__(self, *_):
                raise AssertionError("Unacquired mutex must not be released")

        with self.first.lock("installation"):
            with self.capture_unraisable() as errors:
                with mock.patch.object(inprocess, "_LIBRARY_MUTEX", InterruptedMutex()):
                    self.child_must_not_enter()
            self.assertEqual([type(error.exc_value) for error in errors], [KeyboardInterrupt])
            self.assertFalse(inprocess._FORKING)
            self.assert_contended()
        self.assert_available()

    @isolated
    def test_interrupted_return_from_native_prepare_unwinds_parent(self):
        library = self.first._library
        native_prepare = library.fork_prepare
        calls = []

        def interrupted():
            native_prepare()
            calls.append("native_prepared")
            raise KeyboardInterrupt("controlled interruption after actual native prepare")

        with self.first.lock("installation"):
            with self.capture_unraisable() as errors:
                with mock.patch.object(library, "fork_prepare", side_effect=interrupted):
                    self.child_must_not_enter()
            self.assertEqual(calls, ["native_prepared"])
            self.assertEqual([type(error.exc_value) for error in errors], [KeyboardInterrupt])
            self.assertFalse(inprocess._FORKING)
            # This reaches the native gate; an incomplete parent unwind would
            # hang the isolated process and fail its outer 20-second bound.
            self.assert_contended()
        self.assert_available()

    @isolated
    def test_child_callback_exception_cannot_reach_caller_body(self):
        entered = self.state / "child-callback-entered"

        def failed_child():
            entered.write_bytes(b"controlled callback failure")
            raise KeyboardInterrupt("controlled child callback failure")

        with self.first.lock("installation"):
            with mock.patch.object(self.first._library, "fork_child", side_effect=failed_child):
                self.child_must_not_enter()
            self.assertEqual(entered.read_bytes(), b"controlled callback failure")
            self.assertFalse(inprocess._FORKING)
            self.assert_contended()
        self.assert_available()

    @isolated
    def test_competing_fork_preserves_preparing_thread_and_native_lock(self):
        import fcntl

        ready, release = threading.Event(), threading.Event()
        errors, observations = [], []

        def preparing_thread():
            try:
                inprocess._before_fork()
                attempt = inprocess._FORK_STATE.current
                observations.append((attempt.owns_preparation, attempt.prepared))
                ready.set()
                if not release.wait(10):
                    raise AssertionError("Competing fork did not release fixture preparation")
                observations.append((attempt.owns_preparation, attempt.prepared))
            except BaseException as error:
                errors.append(error)
            finally:
                try:
                    inprocess._after_fork_parent()
                except BaseException as error:
                    errors.append(error)
                ready.set()

        with self.first.lock("installation"):
            thread = threading.Thread(target=preparing_thread, daemon=True)
            thread.start()
            try:
                self.assertTrue(ready.wait(3))
                self.assertEqual(errors, [])
                self.assertTrue(inprocess._FORKING)
                # Even a cached path must refuse before using a prepared image.
                with self.assertRaises(lock_cases.Busy) as unavailable:
                    inprocess._load(Path(self.library))
                self.assertEqual(unavailable.exception.code, "fork_in_progress")
                self.child_must_not_enter()
                self.assertTrue(inprocess._FORKING)
                self.assertTrue(thread.is_alive())
                # Inspect real kernel exclusion without entering the native
                # gate intentionally held by the other thread's preparation.
                with (self.state / "installation.lock").open("r+b") as peer:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(peer.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            finally:
                release.set()
                thread.join(5)
            self.assertFalse(thread.is_alive())
            self.assertEqual(errors, [])
            self.assertEqual(observations, [(True, True), (True, True)])
            self.assertFalse(inprocess._FORKING)
            self.assert_contended()
        self.assert_available()


if __name__ == "__main__":
    if _ISOLATED:
        unittest.main(argv=[sys.argv[0], "NativeSessionForkTests." + sys.argv[2], "-v"])
    else:
        unittest.main()
