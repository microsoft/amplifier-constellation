"""Bounded production data-reader and isolated JSONL transport qualification.

ActualDriverDataReadTests uses the supplied native executable and production
reader Sessions. The transport peer below only injects protocol faults.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

SUPPORT = Path(__file__).resolve().parents[2] / "support" / "python"
sys.path.insert(0, str(SUPPORT))
from constellation_native import (  # noqa: E402
    Busy, DriverError, Refused, Session, Unknown, initialize_config, initialize_root,
)
from test_setup import policy_snapshot  # noqa: E402

DRIVER = os.environ.get("CONSTELLATION_NATIVE_DRIVER")
LIMIT = 1048576

PEER = r'''
import json, os, sys, time
from pathlib import Path
mode, payload, capture = sys.argv[1:]
held = None
for line in sys.stdin.buffer:
    request = json.loads(line)
    with open(capture, "a", encoding="utf-8") as out:
        out.write(json.dumps(request) + "\n")
    if request["op"] == "lock.acquire":
        held = open(Path(capture).with_suffix(".lock"), "a+b", buffering=0)
        held.write(b"x"); held.seek(0)
        if os.name == "nt":
            import msvcrt
            msvcrt.locking(held.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(held.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    result = {}
    response = {"version":1, "id":request["id"], "outcome":"ok", "result":result}
    if request["op"] == "storage.read_data":
        if mode == "lost": sys.exit(0)
        if mode == "timeout": time.sleep(30)
        if mode == "refused":
            response = {"version":1, "id":request["id"], "outcome":"refused",
                        "error":{"code":"data_too_large", "message":"bounded refusal"}}
        else:
            response["result"] = json.loads(Path(payload).read_text(encoding="utf-8"))
    if request["op"] == "lock.release":
        held.close(); held = None
    print(json.dumps(response), flush=True)
'''


class DataReadTransportTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.script = self.root / "peer.py"
        self.script.write_text(PEER, encoding="utf-8")
        self.config = self.root / "owner.json"
        self.config.write_text("{}", encoding="utf-8")
        self.serial = 0

    def session(self, result=None, *, mode="result", timeout=2):
        self.serial += 1
        capture = self.root / f"requests-{self.serial}.jsonl"
        payload = self.root / f"payload-{self.serial}.json"
        payload.write_text(json.dumps(result), encoding="utf-8")
        real_popen = subprocess.Popen

        def spawn(argv, **kwargs):
            self.assertEqual(argv, [sys.executable, "--config", str(self.config), "serve"])
            self.assertTrue(kwargs["close_fds"])
            self.assertNotIn("shell", kwargs)
            return real_popen(
                [sys.executable, str(self.script), mode, str(payload), str(capture)], **kwargs
            )

        with patch("constellation_native.client.subprocess.Popen", side_effect=spawn):
            session = Session(sys.executable, self.config, timeout=timeout)
        self.addCleanup(session.close)
        return session, capture

    @staticmethod
    def requests(capture):
        return [json.loads(line) for line in capture.read_text().splitlines()] if capture.exists() else []

    def assert_kernel_lock_released(self, capture):
        with capture.with_suffix(".lock").open("r+b", buffering=0) as lock:
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    def test_absence_empty_and_binary_results_preserve_exact_request_shape(self):
        for result, expected in [
            ({"present":False}, None),
            ({"present":True, "bytes":[]}, b""),
            ({"present":True, "bytes":[0, 255, 128, 10]}, b"\x00\xff\x80\n"),
        ]:
            with self.subTest(result=result):
                native, capture = self.session(result)
                self.assertEqual(native.private_read_data("state", "receipt", max_bytes=4), expected)
                request, = self.requests(capture)
                self.assertEqual(set(request), {"version", "id", "op", "root", "path", "max_bytes"})
                self.assertEqual(request["op"], "storage.read_data")
                self.assertEqual((request["root"], request["path"], request["max_bytes"]), ("state", "receipt", 4))
        native, capture = self.session({"present":False})
        self.assertIsNone(native.private_read_data("state", "missing"))
        self.assertEqual(self.requests(capture)[0]["max_bytes"], LIMIT)

    def test_invalid_limits_refuse_locally_without_dispatch_or_closing_session(self):
        native, capture = self.session({"present":False})
        for value in (True, False, 0, -1, LIMIT + 1, 1.0, "1", None, [], {}, float("nan")):
            with self.subTest(value=repr(value)), self.assertRaises(ValueError):
                native.private_read_data("state", "receipt", max_bytes=value)
        self.assertEqual(self.requests(capture), [])
        self.assertIsNone(native._process.poll())
        self.assertIsNone(native.private_read_data("state", "receipt", max_bytes=1))
        self.assertEqual(len(self.requests(capture)), 1)

    def test_malformed_or_oversized_results_invalidate_owned_session_without_retry(self):
        malformed = [None, [], {}, {"present":0}, {"present":1, "bytes":[]},
                     {"present":False, "bytes":[]}, {"present":False, "extra":None},
                     {"present":True}, {"present":True, "bytes":"secret-marker"},
                     {"present":True, "bytes":[True]}, {"present":True, "bytes":[-1]},
                     {"present":True, "bytes":[256]}, {"present":True, "bytes":[1.0]},
                     {"present":True, "bytes":[0, 1, 2, 3]},
                     {"present":True, "bytes":[], "extra":None}]
        for result in malformed:
            with self.subTest(result=result):
                native, capture = self.session(result)
                with native.lock("installation"):
                    with self.assertRaises(DriverError) as caught:
                        native.private_read_data("state", "receipt", max_bytes=3)
                self.assertIs(type(caught.exception), DriverError)
                self.assertEqual(caught.exception.code, "invalid_response")
                self.assertNotIn("secret-marker", str(caught.exception))
                self.assertEqual(native._locks, set())
                self.assertIsNotNone(native._process.poll())
                self.assert_kernel_lock_released(capture)
                with self.assertRaises(DriverError) as closed:
                    native.private_read_data("state", "receipt")
                self.assertEqual(closed.exception.code, "closed_session")
                self.assertEqual([r["op"] for r in self.requests(capture)],
                                 ["lock.acquire", "storage.read_data"])

    def test_read_transport_loss_invalidates_locks_without_mutation_or_redispatch(self):
        for mode, code in (("lost", "transport_closed"), ("timeout", "transport_timeout")):
            with self.subTest(mode=mode):
                native, capture = self.session(mode=mode)
                with native.lock("installation"):
                    if mode == "timeout":native.timeout = 0.2
                    with self.assertRaises(DriverError) as caught:
                        native.private_read_data("state", "receipt")
                self.assertIs(type(caught.exception), DriverError)
                self.assertNotIsInstance(caught.exception, Unknown)
                self.assertEqual(caught.exception.code, code)
                self.assertEqual(native._locks, set())
                self.assertIsNotNone(native._process.poll())
                self.assert_kernel_lock_released(capture)
                self.assertEqual([r["op"] for r in self.requests(capture)],
                                 ["lock.acquire", "storage.read_data"])

    def test_correlated_size_refusal_preserves_session_and_lock(self):
        native, capture = self.session(mode="refused")
        with native.lock("installation"):
            with self.assertRaises(Refused) as caught:
                native.private_read_data("state", "receipt", max_bytes=1)
            self.assertEqual(caught.exception.code, "data_too_large")
            self.assertEqual(native._locks, {"installation"})
            self.assertIsNone(native._process.poll())
        self.assertEqual([r["op"] for r in self.requests(capture)],
                         ["lock.acquire", "storage.read_data", "lock.release"])
        self.assert_kernel_lock_released(capture)


@unittest.skipUnless(DRIVER, "Set CONSTELLATION_NATIVE_DRIVER to test native data reads")
class ActualDriverDataReadTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="native-read-", dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
        )
        self.addCleanup(temporary.cleanup)
        self.parent = Path(temporary.name).resolve()
        self.parent_policy = policy_snapshot(self.parent)
        self.root = self.parent / "owner-café-雪"
        self.state = self.root / "state"
        initialize_root(DRIVER, self.root)
        initialize_root(DRIVER, self.state)
        self.config = self.root / "owner.json"
        initialize_config(DRIVER, self.config, {
            "version":1, "roots":{"state":str(self.state)},
            "locks":{"installation":str(self.state/"installation.lock")},
            "services":{}, "commands":{},
        })

    def session(self):
        native = Session(DRIVER, self.config)
        self.addCleanup(native.close)
        return native

    def test_absence_empty_binary_and_read_only_state(self):
        native = self.session()
        description = native.describe()
        self.assertEqual(description["private_data_read"], 1)
        self.assertIn("storage.read_data", description["operations"])
        native.private_dir("state", "nested")
        original_config = self.config.read_bytes()
        self.assertIsNone(native.private_read_data("state", "nested/absent"))
        self.assertFalse((self.state/"nested/absent").exists())
        for data in (b"", b"\x00\xff\x80 binary\n"):
            native.private_write_data("state", "nested/receipt", data)
            path = self.state/"nested/receipt"
            before = (path.stat().st_ino, path.stat().st_mtime_ns, policy_snapshot(path), path.read_bytes())
            self.assertEqual(native.private_read_data("state", "nested/receipt"), data)
            self.assertEqual((path.stat().st_ino, path.stat().st_mtime_ns, policy_snapshot(path), path.read_bytes()), before)
        self.assertEqual(sorted(p.name for p in (self.state/"nested").iterdir()), ["receipt"])
        self.assertEqual(self.config.read_bytes(), original_config)
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    def test_size_bound_refuses_without_truncation_or_modification(self):
        native = self.session()
        data = bytes(range(256)) * 4096
        self.assertEqual(len(data), LIMIT)
        native.private_write_data("state", "bounded", data)
        self.assertEqual(native.private_read_data("state", "bounded", max_bytes=LIMIT), data)
        before = ((self.state/"bounded").stat().st_ino, (self.state/"bounded").read_bytes())
        with self.assertRaises(Refused) as refused:
            native.private_read_data("state", "bounded", max_bytes=LIMIT-1)
        self.assertEqual(refused.exception.code, "data_too_large")
        self.assertEqual(((self.state/"bounded").stat().st_ino, (self.state/"bounded").read_bytes()), before)
        native.private_write_data("state", "bounded", data+b"x")
        with self.assertRaises(Refused) as refused:
            native.private_read_data("state", "bounded")
        self.assertEqual(refused.exception.code, "data_too_large")
        self.assertEqual((self.state/"bounded").read_bytes(), data+b"x")

    def test_wire_limits_unknown_fields_roots_paths_and_directories_refuse(self):
        native = self.session()
        native.private_dir("state", "directory")
        native.private_write_data("state", "retained", b"original")
        before = sorted(p.name for p in self.state.iterdir())
        for fields in ({"max_bytes":0}, {"max_bytes":LIMIT+1}, {"max_bytes":True},
                       {"max_bytes":1.5}, {"max_bytes":None}, {"max_bytes":1, "extra":True}):
            with self.subTest(fields=fields), self.assertRaises(Refused):
                native._call("storage.read_data", root="state", path="retained", **fields)
        for root, relative in (("unknown", "retained"), ("state", "../outside"),
                               ("state", str(self.root/"outside")), ("state", ""),
                               ("state", "directory"), ("state", "retained/child"),
                               ("state", "missing-parent/child")):
            with self.subTest(root=root, path=relative), self.assertRaises(Refused):
                native.private_read_data(root, relative)
        if os.name == "nt":
            with self.assertRaises(Refused):native.private_read_data("state", "retained:stream")
        self.assertEqual(sorted(p.name for p in self.state.iterdir()), before)
        self.assertEqual((self.state/"retained").read_bytes(), b"original")

    def test_existing_policy_drift_is_refused_without_repair(self):
        native = self.session()
        native.private_write_data("state", "receipt", b"private")
        path = self.state/"receipt"
        if os.name == "nt":
            from test_driver import windows_policy
            windows_policy(path, world_read=True)
        else:path.chmod(0o644)
        before = (path.stat().st_ino, policy_snapshot(path), path.read_bytes())
        with self.assertRaises(Refused):native.private_read_data("state", "receipt")
        self.assertEqual((path.stat().st_ino, policy_snapshot(path), path.read_bytes()), before)

    def test_symlink_leaf_and_ancestor_refuse_without_following(self):
        native = self.session()
        native.private_dir("state", "real")
        native.private_write_data("state", "real/receipt", b"preserve")
        try:
            (self.state/"alias").symlink_to(self.state/"real", target_is_directory=True)
            (self.state/"leaf").symlink_to(self.state/"real/receipt")
        except OSError as error:
            if os.name == "nt" and getattr(error, "winerror", None) == 1314:
                self.skipTest("Windows symbolic-link fixture requires symlink privilege")
            raise
        for path in ("alias/receipt", "leaf"):
            with self.subTest(path=path), self.assertRaises(Refused):
                native.private_read_data("state", path)
        self.assertEqual((self.state/"real/receipt").read_bytes(), b"preserve")

    def test_foreign_owner_is_refused_without_repair(self):
        if os.name == "nt":
            from test_fixture_owner import require_elevated_owner_fixture
            require_elevated_owner_fixture(self)
        elif os.geteuid() != 0:
            self.skipTest("Changing a disposable file to another POSIX owner requires privilege")
        native = self.session()
        native.private_write_data("state", "foreign", b"preserve")
        path = self.state/"foreign"
        if os.name == "nt":
            from test_driver import windows_owner_sid, windows_policy
            windows_policy(path, owner_sid="S-1-5-32-544")
            owner = lambda: windows_owner_sid(path)
            self.assertEqual(owner(), "S-1-5-32-544")
        else:
            os.chown(path, 1, -1)
            owner = lambda: path.stat().st_uid
            self.assertEqual(owner(), 1)
        before = (owner(), policy_snapshot(path), path.stat().st_ino, path.read_bytes())
        with self.assertRaises(Refused):native.private_read_data("state", "foreign")
        self.assertEqual((owner(), policy_snapshot(path), path.stat().st_ino, path.read_bytes()), before)

    def test_production_reader_sessions_observe_complete_snapshots_during_publication(self):
        writer, first, second, contender = (self.session() for _ in range(4))
        count_bytes = 65536
        writer.private_write_data("state", "journal", bytes(count_bytes))
        original_config = self.config.read_bytes()
        stop = threading.Event()
        ready = threading.Barrier(3)
        failures, counts = [], []

        def reads(native):
            count = 0
            try:
                ready.wait(timeout=10)
                while not stop.is_set():
                    data = native.private_read_data("state", "journal", max_bytes=count_bytes)
                    if data is None or len(data) != count_bytes or data != data[:1] * count_bytes:
                        raise AssertionError("Production reader saw missing or partial data")
                    count += 1
            except BaseException as error:
                failures.append(error)
                stop.set()
            finally:counts.append(count)

        threads = [threading.Thread(target=reads, args=(reader,), daemon=True)
                   for reader in (first, second)]
        with writer.lock("installation"):
            for thread in threads:thread.start()
            try:
                ready.wait(timeout=10)
                for value in range(1, 21):
                    if failures:break
                    writer.private_write_data("state", "journal", bytes([value])*count_bytes)
                with self.assertRaises(Busy):
                    with contender.lock("installation"):self.fail("Reader released writer's lock")
            finally:
                stop.set()
                for thread in threads:thread.join(timeout=30)
            self.assertTrue(all(not thread.is_alive() for thread in threads))
            self.assertEqual(failures, [])
            self.assertEqual(len(counts), 2)
            self.assertTrue(all(count > 0 for count in counts))
        self.assertEqual(first.private_read_data("state", "journal"), bytes([20])*count_bytes)
        with contender.lock("installation"):pass
        self.assertEqual(self.config.read_bytes(), original_config)
        self.assertEqual(sorted(p.name for p in self.state.iterdir()), ["installation.lock", "journal"])


if __name__ == "__main__":
    unittest.main()
