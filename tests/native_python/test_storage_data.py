"""Actual protected data publication; Windows readers are fixture-only handles."""

from __future__ import annotations

from contextlib import contextmanager
import os
from pathlib import Path
import stat
import sys
import tempfile
import threading
import unittest

SUPPORT = Path(__file__).resolve().parents[2] / "support" / "python"
sys.path.insert(0, str(SUPPORT))
from constellation_native import (  # noqa: E402
    Busy, Refused, Session, Unknown, initialize_config, initialize_root,
)
from test_setup import policy_snapshot  # noqa: E402

DRIVER = os.environ.get("CONSTELLATION_NATIVE_DRIVER")


@contextmanager
def windows_reader(path: Path, *, delete_share: bool = True):
    """Open one explicit native read handle, never changing production I/O."""
    import ctypes
    from ctypes import wintypes
    import msvcrt
    from test_driver import windows_native_path

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    create = kernel.CreateFileW
    create.argtypes = [
        wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p,
        wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE,
    ]
    create.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    # GENERIC_READ, FILE_SHARE_READ/WRITE[/DELETE], OPEN_EXISTING, NORMAL.
    handle = create(windows_native_path(path), 0x80000000,
                    0x1 | 0x2 | (0x4 if delete_share else 0), None, 3, 0x80, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        descriptor = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    except BaseException:
        kernel.CloseHandle(handle)
        raise
    # open_osfhandle transfers ownership: closing this stream closes the handle.
    with os.fdopen(descriptor, "rb", buffering=0) as reader:
        yield reader


@unittest.skipUnless(DRIVER, "Set CONSTELLATION_NATIVE_DRIVER to test native data publication")
class ActualDataPublicationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="native-data-", dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
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
            "version": 1, "roots": {"state": str(self.state)},
            "locks": {"installation": str(self.state / "installation.lock")},
            "services": {}, "commands": {},
        })

    def session(self):
        session = Session(DRIVER, self.config, timeout=10)
        self.addCleanup(session.close)
        return session

    def assert_private_owner(self, path):
        if os.name == "nt":
            from test_driver import windows_owner_sid, windows_user_sid
            self.assertEqual(windows_owner_sid(path), windows_user_sid())
        else:
            self.assertEqual(path.stat().st_uid, os.geteuid())
            self.assertEqual(stat.S_IMODE(path.stat().st_mode) & 0o077, 0)

    def test_descriptor_and_nested_bytes_preserve_owner_lock_and_strict_create(self):
        native, contender = self.session(), self.session()
        description = native.describe()
        self.assertEqual(description["private_data_publication"], 1)
        self.assertIn("storage.write_data", description["operations"])
        config_before = self.config.read_bytes()
        with native.lock("installation"):
            native.private_dir("state", "transactions")
            native.private_dir("state", "transactions/original")
            relative = "transactions/original/journal.json"
            path = self.state / relative
            original = b'{"phase":"prepared","authority":"original"}\x00\xff'
            replacement = b'{"phase":"committed","authority":"original"}\n'
            self.assertEqual(native.private_write_data("state", relative, original), {})
            self.assertEqual(path.read_bytes(), original)
            self.assert_private_owner(path)
            self.assertEqual(native.private_write_data("state", relative, replacement), {})
            self.assertEqual(path.read_bytes(), replacement)
            self.assert_private_owner(path)
            self.assert_private_owner(path.parent)
            with self.assertRaises(Refused) as refused:
                native.private_write("state", relative, b"must not replace", create_only=True)
            self.assertEqual(refused.exception.code, "already_exists")
            self.assertEqual(path.read_bytes(), replacement)
            with self.assertRaises(Busy):
                with contender.lock("installation"):
                    self.fail("Successful data publication released its owner's lock")
            native.durable_remove("state", relative)
            self.assertFalse(path.exists())
        with contender.lock("installation"):
            pass
        self.assertEqual(self.config.read_bytes(), config_before)
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    def test_data_publication_refuses_extra_fields_and_invalid_wire_bytes(self):
        native = self.session()
        invalid = [
            {"bytes": [1], "create_only": True},
            {"bytes": [1], "expected": None},
            {"bytes": [1], "timeout": 1},
            {"bytes": [-1]}, {"bytes": [256]}, {"bytes": [True]},
            {"bytes": "serialized elsewhere"}, {"bytes": None}, {},
        ]
        for fields in invalid:
            with self.subTest(fields=fields), self.assertRaises(Refused) as refused:
                native._call("storage.write_data", mutating=True, root="state",
                             path="must-not-exist", **fields)
            self.assertEqual(refused.exception.code, "invalid_request")
        self.assertFalse((self.state / "must-not-exist").exists())

    def test_data_publication_refuses_unsafe_paths_without_mutation(self):
        native = self.session()
        native.private_dir("state", "directory")
        native.private_write("state", "retained", b"original", create_only=True)
        before = (self.state / "retained").read_bytes()
        for root, path in [
            ("missing-root", "receipt"), ("state", "../outside"),
            ("state", str(self.root / "outside")), ("state", ""),
            ("state", "missing-parent/receipt"), ("state", "retained/child"),
            ("state", "directory"),
        ]:
            with self.subTest(root=root, path=path), self.assertRaises(Refused):
                native.private_write_data(root, path, b"must not publish")
        if os.name == "nt":
            with self.assertRaises(Refused):
                native.private_write_data("state", "retained:stream", b"not a file")
        self.assertEqual((self.state / "retained").read_bytes(), before)
        self.assertFalse((self.root / "outside").exists())
        self.assertEqual(list((self.state / "directory").iterdir()), [])
        self.assertEqual(sorted(p.name for p in self.state.iterdir()), ["directory", "retained"])

    def test_data_publication_refuses_existing_policy_drift_without_repair(self):
        native = self.session()
        native.private_write_data("state", "journal.json", b"retained")
        path = self.state / "journal.json"
        if os.name == "nt":
            from test_driver import windows_policy
            windows_policy(path, world_read=True)
        else:
            path.chmod(0o644)
        before = (path.read_bytes(), policy_snapshot(path), path.stat().st_ino)
        with self.assertRaises(Refused):
            native.private_write_data("state", "journal.json", b"must not repair")
        self.assertEqual((path.read_bytes(), policy_snapshot(path), path.stat().st_ino), before)

    @unittest.skipUnless(os.name == "nt", "Windows delete-sharing semantics")
    def test_delete_sharing_reader_retains_old_snapshot_and_strict_write_stays_strict(self):
        native = self.session()
        with native.lock("installation"):
            native.private_dir("state", "transactions")
            relative = "transactions/journal.json"
            path = self.state / relative
            native.private_write_data("state", relative, b"original")
            with windows_reader(path) as old:
                with self.assertRaises(Unknown) as unknown:
                    native.private_write("state", relative, b"strict must refuse")
                self.assertEqual(unknown.exception.code, "write_unconfirmed")
                self.assertEqual(path.read_bytes(), b"original")
                native.private_write_data("state", relative, b"replacement")
                self.assertEqual(old.read(), b"original")
                with windows_reader(path) as current:
                    self.assertEqual(current.read(), b"replacement")
                self.assert_private_owner(path)
            self.assertEqual(sorted(p.name for p in path.parent.iterdir()), ["journal.json"])

    @unittest.skipUnless(os.name == "nt", "Windows sharing and read-only refusal")
    def test_denied_delete_sharing_and_readonly_remain_unknown_without_retry(self):
        native = self.session()
        native.private_write_data("state", "journal.json", b"original")
        path = self.state / "journal.json"
        before = (path.read_bytes(), policy_snapshot(path), path.stat().st_ino)
        with windows_reader(path, delete_share=False):
            with self.assertRaises(Unknown) as unknown:
                native.private_write_data("state", "journal.json", b"sharing denied")
            self.assertEqual(unknown.exception.code, "write_unconfirmed")
            self.assertEqual((path.read_bytes(), policy_snapshot(path), path.stat().st_ino), before)
        mode = path.stat().st_mode
        try:
            path.chmod(stat.S_IREAD)
            with self.assertRaises(Unknown) as unknown:
                native.private_write_data("state", "journal.json", b"readonly denied")
            self.assertEqual(unknown.exception.code, "write_unconfirmed")
            self.assertEqual(path.read_bytes(), b"original")
            self.assertFalse(path.stat().st_mode & stat.S_IWRITE)
        finally:
            path.chmod(mode)
        self.assertEqual((path.read_bytes(), policy_snapshot(path), path.stat().st_ino), before)
        self.assertEqual(sorted(p.name for p in self.state.iterdir()), ["journal.json"])

    @unittest.skipUnless(os.name == "nt", "Windows atomic publication under concurrent readers")
    def test_concurrent_readers_never_observe_missing_or_partial_data(self):
        native = self.session()
        native.private_write_data("state", "journal.json", bytes(16384))
        path = self.state / "journal.json"
        stop, ready = threading.Event(), threading.Barrier(3)
        failures, counts = [], []

        def read_snapshots():
            count = 0
            try:
                ready.wait(timeout=5)
                while not stop.is_set():
                    with windows_reader(path) as reader:
                        data = reader.read()
                    if len(data) != 16384 or data != data[:1] * 16384:
                        raise AssertionError("A concurrent reader observed a partial snapshot")
                    count += 1
            except BaseException as error:
                failures.append(error)
            finally:
                counts.append(count)

        readers = [threading.Thread(target=read_snapshots, daemon=True) for _ in range(2)]
        for reader in readers:
            reader.start()
        try:
            ready.wait(timeout=5)
            for value in range(1, 26):
                native.private_write_data("state", "journal.json", bytes([value]) * 16384)
        finally:
            stop.set()
            for reader in readers:
                reader.join(timeout=5)
        self.assertTrue(all(not reader.is_alive() for reader in readers))
        self.assertEqual(failures, [])
        self.assertEqual(len(counts), 2)
        self.assertTrue(all(count > 0 for count in counts))
        self.assertEqual(path.read_bytes(), bytes([25]) * 16384)


if __name__ == "__main__":
    unittest.main()
