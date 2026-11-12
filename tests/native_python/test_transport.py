"""Transport fault injection only; these tests do not qualify an OS backend."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

SUPPORT = Path(__file__).resolve().parents[2] / "support" / "python"
sys.path.insert(0, str(SUPPORT))
from constellation_native import Busy, DriverError, Refused, Session, Unknown  # noqa: E402


FIXTURE = r"""
import json, os, sys, time
from pathlib import Path
mode, capture = sys.argv[1:]
held = None
if mode == "no_read":
    time.sleep(30)
for line in sys.stdin.buffer:
    request = json.loads(line)
    with open(capture, "a", encoding="utf-8") as out:
        out.write(json.dumps(request) + "\n")
    response = {"version": 1, "id": request["id"], "outcome": "ok", "result": request}
    if mode == "delayed_eof":
        import subprocess
        subprocess.Popen([sys.executable, "-c", "import time; time.sleep(1.5)"],
                         stdin=subprocess.DEVNULL, stdout=sys.stdout,
                         stderr=subprocess.DEVNULL)
    if mode == "locked_no_read" and request["op"] == "lock.acquire":
        held = open(Path(capture).with_suffix(".lock"), "a+b", buffering=0)
        held.write(b"x")
        held.seek(0)
        if os.name == "nt":
            import msvcrt
            msvcrt.locking(held.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(held.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    if mode == "lost" and request["op"] == "process.launch":
        sys.exit(0)
    if mode == "timeout":
        time.sleep(30)
    if mode == "wrong_id":
        response["id"] = "not-the-request"
    if mode == "bad_version":
        response["version"] = True
    if mode == "malformed":
        sys.stdout.write("not-json\n")
        sys.stdout.flush()
        continue
    if mode in ("busy", "refused", "unknown", "error"):
        response.pop("result")
        response.update(outcome=mode, error={"code": "fixture_" + mode, "message": mode})
    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()
    if mode == "locked_no_read":
        time.sleep(30)
"""


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.script = self.root / "fixture.py"
        self.script.write_text(FIXTURE, encoding="utf-8")
        self.capture = self.root / "requests.jsonl"
        self.config = self.root / "owner config.json"
        self.config.write_text("{}", encoding="utf-8")

    def session(self, mode="echo", timeout=1):
        real_popen = subprocess.Popen

        def spawn(argv, **kwargs):
            self.assertEqual(
                argv, [sys.executable, "--config", str(self.config), "serve"]
            )
            self.assertTrue(kwargs["close_fds"])
            self.assertNotIn("shell", kwargs)
            return real_popen(
                [sys.executable, str(self.script), mode, str(self.capture)], **kwargs
            )

        with patch("constellation_native.client.subprocess.Popen", side_effect=spawn):
            session = Session(sys.executable, self.config, timeout=timeout)
        self.addCleanup(session.close)
        return session

    def requests(self):
        if not self.capture.exists():
            return []
        return [
            json.loads(line)
            for line in self.capture.read_text(encoding="utf-8").splitlines()
        ]

    def test_lock_spans_critical_section_with_same_invocation(self):
        session = self.session()
        with session.lock("installation"):
            session.describe()
            self.assertEqual(
                [r["op"] for r in self.requests()], ["lock.acquire", "describe"]
            )
        self.assertEqual(
            [r["op"] for r in self.requests()],
            ["lock.acquire", "describe", "lock.release"],
        )
        self.assertEqual(len({r["id"] for r in self.requests()}), 3)

    def test_nested_lock_does_not_release_outer_ownership(self):
        session = self.session()
        with session.lock("installation"):
            with self.assertRaises(DriverError) as caught:
                with session.lock("installation"):
                    self.fail("Reentry cannot succeed")
            self.assertEqual(caught.exception.code, "lock_reentry")
        self.assertEqual(
            [r["op"] for r in self.requests()], ["lock.acquire", "lock.release"]
        )

    def test_lost_launch_ack_is_unknown_and_never_redispatched(self):
        session = self.session("lost")
        with self.assertRaises(Unknown):
            session.launch("original-session-probe", result="receipt.json")
        with self.assertRaises(DriverError) as caught:
            session.launch("original-session-probe", result="receipt.json")
        self.assertEqual(caught.exception.code, "closed_session")
        self.assertEqual([r["op"] for r in self.requests()], ["process.launch"])

    def test_protocol_corruption_after_mutation_is_unknown(self):
        for mode in ("malformed", "wrong_id", "bad_version"):
            with self.subTest(mode=mode):
                session = self.session(mode)
                with self.assertRaises(Unknown) as caught:
                    session.service_action("body", "stop", {"generation": "original"})
                self.assertEqual(caught.exception.code, "protocol_response")
                session.close()

    def test_read_failure_is_not_effect_uncertainty(self):
        session = self.session("malformed")
        with self.assertRaises(DriverError) as caught:
            session.service_snapshot("body")
        self.assertNotIsInstance(caught.exception, Unknown)

    def test_classifies_driver_outcomes_without_turning_storage_error_into_busy(self):
        for outcome, expected in (
            ("busy", Busy),
            ("refused", Refused),
            ("unknown", Unknown),
            ("error", DriverError),
        ):
            with self.subTest(outcome=outcome):
                session = self.session(outcome)
                with self.assertRaises(expected) as caught:
                    session.describe()
                self.assertIs(type(caught.exception), expected)
                self.assertEqual(caught.exception.code, "fixture_" + outcome)
                session.close()

    def test_response_timeout_is_bounded_and_unknown_for_mutation(self):
        session = self.session("timeout", timeout=0.25)
        start = time.monotonic()
        with self.assertRaises(Unknown):
            session.launch("probe")
        self.assertLess(time.monotonic() - start, 3)
        self.assertEqual(len(self.requests()), 1)

    def test_full_request_pipe_cannot_bypass_deadline(self):
        session = self.session("no_read", timeout=0.25)
        start = time.monotonic()
        with self.assertRaises(Unknown):
            session.private_write("state", "receipt.json", b"x" * 400000)
        self.assertLess(time.monotonic() - start, 3)

    def test_close_prevents_further_dispatch(self):
        session = self.session()
        session.close()
        with self.assertRaises(DriverError):
            session.describe()
        self.assertEqual(self.requests(), [])

    def test_reader_closes_pipe_after_delayed_descendant_eof(self):
        session = self.session("delayed_eof")
        session.describe()
        session.close()
        deadline = time.monotonic() + 5
        while session._reader.is_alive() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertFalse(session._reader.is_alive())
        self.assertTrue(session._process.stdout.closed)

    def test_timeout_releases_held_process_lock_before_returning(self):
        # Real kernel lock in a stalled transport fixture, not native lock proof.
        session = self.session("locked_no_read", timeout=0.25)
        start = time.monotonic()
        with session.lock("installation"):
            with self.assertRaises(Unknown):
                session.private_write("state", "receipt.json", b"x" * 400000)
        self.assertLess(time.monotonic() - start, 3)
        self.assertIsNotNone(session._process.poll())
        self.assertFalse(session._reader.is_alive())
        with self.capture.with_suffix(".lock").open("r+b", buffering=0) as lock:
            if sys.platform == "win32":
                import msvcrt

                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl

                fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    def test_serialized_bytes_remain_the_callers_bytes(self):
        session = self.session()
        data = b'{"authority":"unchanged"}\x00\xff'
        result = session.private_write("state", "receipt.json", data, create_only=True)
        self.assertEqual(bytes(result["bytes"]), data)
        self.assertIs(result["create_only"], True)
        self.assertNotIn("argv", result)

    def test_invalid_timeout_and_relative_setup_paths_refuse_before_spawn(self):
        for timeout in (0, -1, float("inf"), float("nan"), True):
            with self.assertRaises(ValueError):
                Session(sys.executable, self.config, timeout=timeout)
        with self.assertRaises(ValueError):
            Session("relative-driver", self.config)
        with self.assertRaises(ValueError):
            Session(sys.executable, "relative-config")

    def test_oversized_request_refuses_locally_before_dispatch(self):
        session = self.session()
        with patch("constellation_native.client._MAX_FRAME", 1024):
            with self.assertRaises(ValueError):
                session.private_write("state", "receipt.json", b"x" * 500)
        self.assertEqual(self.requests(), [])
        session.describe()
        self.assertEqual(len(self.requests()), 1)

    def test_private_directory_lost_response_is_unknown(self):
        session = self.session("malformed")
        with self.assertRaises(Unknown):
            session.private_dir("state", "transaction")
        self.assertEqual([r["op"] for r in self.requests()], ["storage.mkdir"])
        with self.assertRaises(DriverError) as caught:
            session.private_dir("state", "transaction")
        self.assertEqual(caught.exception.code, "closed_session")


if __name__ == "__main__":
    unittest.main()
