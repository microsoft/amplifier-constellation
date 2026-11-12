"""Cleanup deadline transport fixtures; these do not qualify native deletion."""

from __future__ import annotations

import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

SUPPORT = Path(__file__).resolve().parents[2] / "support" / "python"
sys.path.insert(0, str(SUPPORT))
from constellation_native import DriverError, Session, Unknown  # noqa: E402


FIXTURE = r"""
import json, os, sys
from pathlib import Path
mode, capture = sys.argv[1:]
held = None
for line in sys.stdin.buffer:
    request = json.loads(line)
    with open(capture, "a", encoding="utf-8") as output:
        output.write(json.dumps(request) + "\n")
    if mode == "locked_timeout" and request["op"] == "lock.acquire":
        held = open(Path(capture).with_suffix(".lock"), "a+b", buffering=0)
        held.write(b"x")
        held.seek(0)
        if os.name == "nt":
            import msvcrt
            msvcrt.locking(held.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(held.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    if mode == "locked_timeout" and request["op"] == "tree.remove":
        # Test-only synchronization: the parent consumes this marker before
        # injecting deadline exhaustion. It is deliberately not an acknowledgment.
        print(json.dumps({"fixture": "tree_received"}), flush=True)
        sys.stdin.buffer.read()
        break
    print(json.dumps({"version": 1, "id": request["id"],
                      "outcome": "ok", "result": request}), flush=True)
"""


class TreeTimeoutTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.script = self.root / "fixture.py"
        self.script.write_text(FIXTURE, encoding="utf-8")
        self.capture = self.root / "requests.jsonl"
        self.config = self.root / "owner.json"
        self.config.write_text("{}", encoding="utf-8")

    def session(self, mode="echo", **options):
        real_popen = subprocess.Popen

        def spawn(argv, **kwargs):
            self.assertEqual(argv, [sys.executable, "--config", str(self.config), "serve"])
            return real_popen(
                [sys.executable, str(self.script), mode, str(self.capture)], **kwargs
            )

        with patch("constellation_native.client.subprocess.Popen", side_effect=spawn):
            session = Session(sys.executable, self.config, **options)
        self.addCleanup(session.close)
        return session

    def requests(self):
        if not self.capture.exists():
            return []
        return [json.loads(line) for line in self.capture.read_text(encoding="utf-8").splitlines()]

    def kernel_lock_available(self):
        with self.capture.with_suffix(".lock").open("r+b", buffering=0) as lock:
            if os.name == "nt":
                import msvcrt

                try:
                    msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                except OSError:
                    return False
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl

                try:
                    fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    return False
                fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
            return True

    def test_override_changes_only_tree_call_both_waits_and_never_the_wire(self):
        session = self.session()
        self.assertEqual(session.timeout, 30.0)
        response_get = session._responses.get
        response_waits = []
        write_waits = []
        real_queue = queue.Queue
        expected = {"schema": 1, "identity": "opaque fixture"}

        def receive(*, timeout):
            response_waits.append(timeout)
            self.assertEqual(session.timeout, 30.0)
            return response_get(timeout=timeout)

        def writing_queue():
            result = real_queue()
            original_get = result.get

            def written(*, timeout):
                write_waits.append(timeout)
                self.assertEqual(session.timeout, 30.0)
                return original_get(timeout=timeout)

            result.get = written
            return result

        # Freeze only the caller's deadline arithmetic; the actual child echoes
        # immediately. Record both bounded waits without waiting 30/120 seconds.
        with patch("constellation_native.client.time.monotonic", return_value=100.0), \
             patch.object(session._responses, "get", side_effect=receive), \
             patch("constellation_native.client.queue.Queue", side_effect=writing_queue):
            session.describe()
            session.tree_remove("stages", "candidate", expected, timeout=120)
            session.service_snapshot("body")
            session.tree_remove("stages", "candidate", expected)
            session.tree_remove("stages", "candidate", expected, timeout=None)
            session.tree_remove("stages", "candidate", expected, timeout=0.5)
        self.assertEqual(write_waits, [30.0, 120.0, 30.0, 30.0, 30.0, 0.5])
        self.assertEqual(response_waits, write_waits)
        self.assertEqual(session.timeout, 30.0)
        requests = self.requests()
        self.assertEqual(len(requests), 6)
        for request in requests:
            self.assertNotIn("timeout", request)
            self.assertNotIn("_timeout", request)
            if request["op"] == "tree.remove":
                self.assertEqual(set(request), {"version", "id", "op", "name", "entry", "expected"})
                self.assertEqual(request["expected"], expected)

    def test_none_uses_the_configured_session_timeout(self):
        session = self.session(timeout=7)
        response_get = session._responses.get
        with patch("constellation_native.client.time.monotonic", return_value=100.0), \
             patch.object(session._responses, "get", wraps=response_get) as receive:
            session.tree_remove("stages", "candidate", {}, timeout=None)
        receive.assert_called_once_with(timeout=7.0)
        self.assertEqual(session.timeout, 7.0)

    def test_writing_and_response_share_one_override_deadline(self):
        session = self.session()
        response_get = session._responses.get
        # Ten seconds pass before the write wait and another fifteen before
        # the reply wait. Both consume the same original 120-second budget.
        with patch("constellation_native.client.time.monotonic", side_effect=[100.0, 110.0, 125.0]), \
             patch.object(session._responses, "get", wraps=response_get) as receive:
            session.tree_remove("stages", "candidate", {}, timeout=120)
        receive.assert_called_once_with(timeout=95.0)
        self.assertEqual(session.timeout, 30.0)

    def test_invalid_timeout_is_rejected_before_dispatch_and_keeps_session_usable(self):
        session = self.session()
        invalid = [False, True, 0, -1, float("nan"), float("inf"), -float("inf"),
                   "120", {}, [], object(), 1j, 10 ** 1000,
                   1e300, -1e300, threading.TIMEOUT_MAX * 2]
        for value in invalid:
            with self.subTest(value=repr(value)):
                with self.assertRaises(ValueError):
                    session.tree_remove("stages", "candidate", {}, timeout=value)
        self.assertEqual(session._serial, 0)
        self.assertEqual(self.requests(), [])
        self.assertFalse(session._closed)
        self.assertEqual(session.timeout, 30.0)
        session.describe()
        self.assertEqual([request["op"] for request in self.requests()], ["describe"])

    def test_expired_override_is_unknown_kills_invocation_and_releases_kernel_lock_once(self):
        session = self.session("locked_timeout")
        expected = {"schema": 1, "identity": "retained pruning intent"}
        with self.assertRaises(Unknown) as lost:
            with session.lock("installation"):
                self.assertFalse(self.kernel_lock_available())
                response_get = session._responses.get

                def expired(*, timeout):
                    self.assertEqual(timeout, 120.0)
                    self.assertEqual(json.loads(response_get(timeout=5)), {"fixture": "tree_received"})
                    self.assertFalse(self.kernel_lock_available())
                    raise queue.Empty

                # Deterministic deadline fault after the child has consumed the
                # one removal request, while a real kernel lock is still held.
                with patch("constellation_native.client.time.monotonic", return_value=100.0), \
                     patch.object(session._responses, "get", side_effect=expired):
                    session.tree_remove("stages", "candidate", expected, timeout=120)
        self.assertEqual(lost.exception.code, "transport_timeout")
        self.assertTrue(session._closed)
        self.assertEqual(session._locks, set())
        self.assertIsNotNone(session._process.poll())
        self.assertTrue(self.kernel_lock_available())
        self.assertEqual(session.timeout, 30.0)
        with self.assertRaises(DriverError) as closed:
            session.tree_remove("stages", "candidate", expected, timeout=120)
        self.assertEqual(closed.exception.code, "closed_session")
        requests = self.requests()
        self.assertEqual([request["op"] for request in requests], ["lock.acquire", "tree.remove"])
        self.assertEqual(requests[1]["expected"], expected)


if __name__ == "__main__":
    unittest.main()
