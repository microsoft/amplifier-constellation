"""Actual enrolled tree removal; only reply loss uses a forwarding transport.

Successful fixture setup uses the production initialize entry points. All paths
are disposable, and outside sentinels prove links are not followed. Large-tree
qualification lives in run_tree_scale.py so repeated interpreter tests stay small.
"""

from __future__ import annotations

import errno
import json
import os
from pathlib import Path
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_driver import DRIVER, protect, windows_native_path

from constellation_native import (
    Busy, DriverError, Refused, Session, Unknown, initialize_config, initialize_root,
)


def directory_link(link: Path, target: Path) -> str:
    """Create a real disposable junction on Windows, symlink elsewhere."""
    if os.name != "nt":
        link.symlink_to(target, target_is_directory=True)
        return "symlink"
    # Test data creation only. Use the ordinary junction representation without
    # requiring developer mode or starting 1,064 shells in the scale fixture.
    import ctypes
    from ctypes import wintypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    create = kernel.CreateFileW
    create.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                       ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    create.restype = wintypes.HANDLE
    control = kernel.DeviceIoControl
    control.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.c_void_p,
                        wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD,
                        ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
    control.restype = wintypes.BOOL
    close = kernel.CloseHandle
    close.argtypes = [wintypes.HANDLE]
    close.restype = wintypes.BOOL
    native_target = windows_native_path(target)
    substitute = ("\\??\\" + native_target[4:]).encode("utf-16-le")
    display = str(target).encode("utf-16-le")
    paths = substitute + b"\0\0" + display + b"\0\0"
    data = struct.pack("<IHHHHHH", 0xA0000003, 8 + len(paths), 0,
                       0, len(substitute), len(substitute) + 2, len(display)) + paths
    link.mkdir(mode=0o700)
    handle = create(windows_native_path(link), 0x40000000, 7, None, 3,
                    0x02000000 | 0x00200000, None)
    if handle == ctypes.c_void_p(-1).value:
        link.rmdir()
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        buffer = ctypes.create_string_buffer(data)
        returned = wintypes.DWORD()
        if not control(handle, 0x000900A4, buffer, len(data), None, 0,
                       ctypes.byref(returned), None):
            raise ctypes.WinError(ctypes.get_last_error())
    finally:
        close(handle)
    if not link.lstat().st_file_attributes & 0x400:
        raise AssertionError("Fixture did not create a Windows reparse entry")
    return "junction"


def remove_directory_link(link: Path) -> None:
    if os.path.lexists(link):
        if os.name == "nt":
            link.rmdir()
        else:
            link.unlink()


class TreeFixture:
    def __init__(self, base: Path, driver: str, *, long_path: bool = False):
        self.driver = str(Path(driver).resolve(strict=True))
        # Prepare only the freshly allocated outer test directory. Installation
        # roots/configuration below it are created by the production initializer.
        protect(base)
        self.root = base / "installation"
        initialize_root(self.driver, self.root)
        self.state = self.root / "state"
        initialize_root(self.driver, self.state)
        parent = self.state
        if long_path:
            while len(str(parent).encode("utf-16-le")) // 2 <= 350:
                parent = parent / ("資料-é-" + "segment" * 4)
                initialize_root(self.driver, parent)
        self.stages = parent / "stages"
        initialize_root(self.driver, self.stages)
        self.config = self.root / "owner.json"
        initialize_config(self.driver, self.config, {
            "version": 1,
            "roots": {"state": str(self.state)},
            "locks": {"installation": str(self.state / "installation.lock")},
            "regenerable_trees": {"stages": {
                "root": "state", "path": self.stages.relative_to(self.state).as_posix(),
                "lock": "installation",
            }},
        })
        self.outside = base / "outside"
        self.outside.mkdir(mode=0o700)
        self.sentinel = self.outside / "retained.txt"
        self.sentinel.write_bytes(b"outside owner data: never remove or rewrite\n")
        self.sentinel_bytes = self.sentinel.read_bytes()

    def entry(self, name="candidate") -> Path:
        child = self.stages / name
        initialize_root(self.driver, child)
        return child

    def session(self, **kwargs):
        return Session(self.driver, self.config, **kwargs)

    def assert_outside(self):
        assert self.sentinel.read_bytes() == self.sentinel_bytes
        assert sorted(path.name for path in self.outside.iterdir()) == ["retained.txt"]


LOST_ACK_PROXY = r'''
import json, subprocess, sys
driver, config, capture = sys.argv[1:]
child = subprocess.Popen([driver, "--config", config, "serve"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=0)
try:
    for raw in sys.stdin.buffer:
        request = json.loads(raw)
        child.stdin.write(raw)
        child.stdin.flush()
        response = child.stdout.readline()
        if not response:
            raise RuntimeError("Real native driver closed before its reply")
        reply = json.loads(response)
        with open(capture, "a", encoding="utf-8") as log:
            log.write(json.dumps({"request": request, "reply": reply}) + "\n")
        if request["op"] == "tree.remove" and reply["outcome"] == "ok":
            # Commit is real. End the actual driver to release its invocation
            # lock, then deliberately lose only the already committed reply.
            child.stdin.close()
            child.wait(timeout=5)
            break
        sys.stdout.buffer.write(response)
        sys.stdout.buffer.flush()
finally:
    if child.poll() is None:
        child.kill()
    child.wait(timeout=5)
'''


def mount_probe(driver: str, parent_namespace: str) -> int:
    """Run only inside a new mount namespace, never modify host mounts."""
    if os.readlink("/proc/self/ns/mnt") == parent_namespace:
        raise RuntimeError("Mount fixture did not enter an isolated namespace")
    private = subprocess.run(["mount", "--make-rprivate", "/"], capture_output=True, timeout=10)
    if private.returncode:
        print("Mount namespace exists but cannot make its mount tree private", file=sys.stderr)
        return 77
    temporary = Path(tempfile.mkdtemp(prefix="native-tree-mount-")).resolve()
    mounts = []

    def bind(source, destination):
        subprocess.run(["mount", "--bind", str(source), str(destination)],
                       check=True, capture_output=True, timeout=10)
        mounts.append(destination)
        assert os.path.samefile(source, destination)

    def alias_fixture(name):
        base = temporary / name
        base.mkdir(mode=0o700)
        return TreeFixture(base, driver)

    def must_contend(session):
        try:
            with session.lock("installation"):
                raise AssertionError("Alias removal released or replaced the existing lock")
        except Busy:
            pass

    def alias_refusal(fixture, config, entry, physical_control, alias_control):
        physical_lock = physical_control / "installation.lock"
        alias_lock = alias_control / "installation.lock"
        sentinel = physical_control / "control-sentinel.txt"
        sentinel_bytes = sentinel.read_bytes()
        with Session(fixture.driver, config) as first, Session(fixture.driver, config) as second:
            with first.lock("installation"):
                assert os.path.samefile(physical_lock, alias_lock)
                lock_before = physical_lock.stat()
                lock_bytes = physical_lock.read_bytes()
                expected = first.tree_inspect("stages", entry)
                assert expected is not None
                must_contend(second)
                try:
                    first.tree_remove("stages", entry, expected)
                except Refused:
                    pass
                else:
                    raise AssertionError("An aliased installation control directory was removed")
                assert sentinel.read_bytes() == sentinel_bytes
                assert (alias_control / "control-sentinel.txt").read_bytes() == sentinel_bytes
                assert physical_lock.read_bytes() == lock_bytes
                assert os.path.samefile(physical_lock, alias_lock)
                after = physical_lock.stat()
                assert (after.st_dev, after.st_ino) == (lock_before.st_dev, lock_before.st_ino)
                must_contend(second)
            with second.lock("installation"):
                after = physical_lock.stat()
                assert (after.st_dev, after.st_ino) == (lock_before.st_dev, lock_before.st_ino)
        fixture.assert_outside()
        return {"refused": True, "control_bytes_preserved": True,
                "lock_identity_preserved": True, "second_session_busy_before_and_after": True}

    try:
        fixture = TreeFixture(temporary, driver)
        child = fixture.entry()
        mounted = child / "mounted"
        mounted.mkdir(mode=0o700)
        bind(fixture.outside, mounted)
        # This case deliberately shares st_dev; a device-only preflight fails it.
        assert mounted.stat().st_dev == child.stat().st_dev
        with fixture.session() as session, session.lock("installation"):
            expected = session.tree_inspect("stages", "candidate")
            try:
                session.tree_remove("stages", "candidate", expected)
            except Refused:
                pass
            else:
                raise AssertionError("A descendant bind mount was not refused before deletion")
        fixture.assert_outside()
        assert (mounted / "retained.txt").read_bytes() == fixture.sentinel_bytes

        # A: the fixed staged root itself is an alias of state. The selected
        # child is the real lock parent, although the configured lock is outside
        # the cleanup directory's lexical path and no mount is below that child.
        root_alias = alias_fixture("fixed-root-alias")
        control = root_alias.state / "control"
        initialize_root(driver, control)
        (control / "control-sentinel.txt").write_bytes(b"required installation control A")
        bind(root_alias.state, root_alias.stages)
        config = root_alias.root / "alias-owner.json"
        data = json.loads(root_alias.config.read_bytes())
        data["locks"]["installation"] = str(control / "installation.lock")
        initialize_config(driver, config, data)
        root_result = alias_refusal(root_alias, config, "control", control,
                                    root_alias.stages / "control")

        # B: the selected tree contains the actual control directory, while its
        # enrolled lock path uses a bind alias outside the lexical tree. There
        # is again no descendant mount point for mount-table checks to reject.
        lock_alias = alias_fixture("lock-parent-alias")
        selected = lock_alias.entry()
        control = selected / "control"
        initialize_root(driver, control)
        (control / "control-sentinel.txt").write_bytes(b"required installation control B")
        alias = lock_alias.state / "outside-control-alias"
        initialize_root(driver, alias)
        bind(control, alias)
        config = lock_alias.root / "alias-owner.json"
        data = json.loads(lock_alias.config.read_bytes())
        data["locks"]["installation"] = str(alias / "installation.lock")
        initialize_config(driver, config, data)
        lock_result = alias_refusal(lock_alias, config, "candidate", control, alias)

        print(json.dumps({"mount_probe": "passed", "same_device": True,
                          "outside_preserved": True, "namespace_isolated": True,
                          "aliases_protected": 2,
                          "fixed_root_alias": root_result, "lock_parent_alias": lock_result}))
        return 0
    finally:
        # ismount can miss same-device binds. If unmount fails, do not walk or
        # delete this fixture; namespace exit releases its mounts independently.
        for mounted in reversed(mounts):
            subprocess.run(["umount", str(mounted)], check=True, capture_output=True, timeout=10)
        shutil.rmtree(temporary)


@unittest.skipUnless(DRIVER, "Actual native driver required")
class ActualTreeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="native-tree-", dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
        )
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name).resolve()
        self.fixture = TreeFixture(self.base, DRIVER)

    def assert_preflight_error(self, operation):
        with self.assertRaises(DriverError) as caught:
            operation()
        self.assertNotIsInstance(caught.exception, Unknown)

    def test_only_enrolled_child_is_removed_under_same_session_lock(self):
        fixture = self.fixture
        child = fixture.entry()
        (child / "package.txt").write_bytes(b"regenerable")
        sibling = fixture.entry("retained")
        (sibling / "keep.txt").write_bytes(b"current or rollback")
        with fixture.session() as session:
            self.assertEqual(session.describe()["regenerable_tree_removal"], 1)
            self.assertIsNone(session.tree_inspect("stages", "absent"))
            expected = session.tree_inspect("stages", "candidate")
            self.assertIsInstance(expected, dict)
            with self.assertRaises(Refused):
                session.tree_remove("stages", "candidate", expected)
            self.assertTrue((child / "package.txt").is_file())
            with session.lock("installation"):
                self.assertEqual(session.tree_remove("stages", "candidate", expected), {"absent": True})
                self.assertIsNone(session.tree_inspect("stages", "candidate"))
            self.assertEqual((sibling / "keep.txt").read_bytes(), b"current or rollback")
        fixture.assert_outside()

    def test_invalid_names_and_unenrolled_capability_never_escape(self):
        fixture = self.fixture
        with fixture.session() as session:
            for entry in ("", ".", "..", "../outside", "a/b", "a\\b", "a:stream", "/absolute"):
                with self.subTest(entry=entry):
                    self.assert_preflight_error(lambda: session.tree_inspect("stages", entry))
            self.assert_preflight_error(lambda: session.tree_inspect("unregistered", "candidate"))
        self.assertEqual(list(fixture.stages.iterdir()), [])
        fixture.assert_outside()

    def test_enrollment_cannot_include_its_config_or_any_configured_lock(self):
        fixture = self.fixture
        original = fixture.config.read_bytes()
        data = json.loads(original)
        attempted_config = fixture.stages / "recursive-owner.json"
        with self.assertRaises(Refused):
            initialize_config(fixture.driver, attempted_config, data)
        self.assertFalse(attempted_config.exists())
        data["locks"]["other-owner-lock"] = str(fixture.stages / "other.lock")
        attempted_config = fixture.root / "unsafe-lock-owner.json"
        with self.assertRaises(Refused):
            initialize_config(fixture.driver, attempted_config, data)
        self.assertFalse(attempted_config.exists())
        self.assertFalse((fixture.stages / "other.lock").exists())
        self.assertEqual(fixture.config.read_bytes(), original)
        with fixture.session() as session:
            self.assertIsNone(session.tree_inspect("stages", "absent"))
        fixture.assert_outside()

    def test_stale_child_and_fixed_root_identities_preserve_replacements(self):
        fixture = self.fixture
        child = fixture.entry()
        (child / "old.txt").write_bytes(b"retained original")
        with fixture.session() as session, session.lock("installation"):
            old = session.tree_inspect("stages", "candidate")
            child.rename(fixture.stages / "old-candidate")
            replacement = fixture.entry()
            (replacement / "new.txt").write_bytes(b"replacement")
            with self.assertRaises(Refused):
                session.tree_remove("stages", "candidate", old)
            fresh = session.tree_inspect("stages", "candidate")
            self.assertNotEqual(old, fresh)
            fixture.stages.rename(fixture.stages.with_name("retained-stages"))
            initialize_root(fixture.driver, fixture.stages)
            replacement = fixture.entry()
            (replacement / "new-root.txt").write_bytes(b"new fixed root")
            with self.assertRaises(Refused):
                session.tree_remove("stages", "candidate", fresh)
            self.assertEqual((replacement / "new-root.txt").read_bytes(), b"new fixed root")
            retained = fixture.stages.with_name("retained-stages")
            self.assertEqual((retained / "candidate/new.txt").read_bytes(), b"replacement")
            self.assertEqual((retained / "old-candidate/old.txt").read_bytes(), b"retained original")
        fixture.assert_outside()

    def test_root_and_selected_child_links_refuse_without_following(self):
        fixture = self.fixture
        link = fixture.stages / "candidate"
        directory_link(link, fixture.outside)
        self.addCleanup(remove_directory_link, link)
        with fixture.session() as session:
            self.assert_preflight_error(lambda: session.tree_inspect("stages", "candidate"))
        remove_directory_link(link)
        fixture.stages.rename(fixture.stages.with_name("retained-stages"))
        directory_link(fixture.stages, fixture.outside)
        self.addCleanup(remove_directory_link, fixture.stages)
        with fixture.session() as session:
            self.assert_preflight_error(lambda: session.tree_inspect("stages", "candidate"))
        fixture.assert_outside()

    def test_descendant_links_are_removed_as_leaves(self):
        fixture = self.fixture
        child = fixture.entry()
        nested = child / "package"
        nested.mkdir(mode=0o700)
        link = nested / "outside"
        directory_link(link, fixture.outside)
        self.addCleanup(remove_directory_link, link)
        self.assertEqual((link / "retained.txt").read_bytes(), fixture.sentinel_bytes)
        if os.name != "nt":
            (child / "outside-file").symlink_to(fixture.sentinel)
        with fixture.session() as session, session.lock("installation"):
            expected = session.tree_inspect("stages", "candidate")
            self.assertEqual(session.tree_remove("stages", "candidate", expected), {"absent": True})
        self.assertFalse(child.exists())
        fixture.assert_outside()

    def test_outside_readonly_hardlink_keeps_bytes_and_permissions(self):
        fixture = self.fixture
        child = fixture.entry()
        linked = child / "shared-file"
        try:
            os.link(fixture.sentinel, linked)
        except OSError as error:
            unsupported = {errno.ENOSYS, errno.ENOTSUP, errno.EOPNOTSUPP}
            if error.errno in unsupported or getattr(error, "winerror", None) in (1, 50):
                self.skipTest("Fixture filesystem does not support hard links: " + str(error))
            raise
        self.assertTrue(os.path.samefile(linked, fixture.sentinel))
        self.assertGreaterEqual(fixture.sentinel.stat().st_nlink, 2)
        original = fixture.sentinel.stat()
        if os.name == "nt":
            import ctypes
            from ctypes import wintypes

            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            set_attributes = kernel.SetFileAttributesW
            set_attributes.argtypes = [wintypes.LPCWSTR, wintypes.DWORD]
            set_attributes.restype = wintypes.BOOL

            def attributes(value):
                if not set_attributes(windows_native_path(fixture.sentinel), value):
                    raise ctypes.WinError(ctypes.get_last_error())

            attributes(original.st_file_attributes | 1)  # FILE_ATTRIBUTE_READONLY.
            restore = lambda: attributes(original.st_file_attributes)
        else:
            fixture.sentinel.chmod(0o400)
            restore = lambda: fixture.sentinel.chmod(stat.S_IMODE(original.st_mode))
        try:
            protected = fixture.sentinel.stat()
            with fixture.session() as session, session.lock("installation"):
                expected = session.tree_inspect("stages", "candidate")
                self.assertEqual(session.tree_remove("stages", "candidate", expected), {"absent": True})
            self.assertFalse(child.exists())
            fixture.assert_outside()
            after = fixture.sentinel.stat()
            self.assertEqual(stat.S_IMODE(after.st_mode), stat.S_IMODE(protected.st_mode))
            if os.name == "nt":
                self.assertTrue(protected.st_file_attributes & 1)
                self.assertEqual(after.st_file_attributes, protected.st_file_attributes)
            # Link count may decrease. Do not mistake removal of one directory
            # entry for authority to rewrite the shared file or its permissions.
        finally:
            restore()  # Disposable-fixture cleanup only, after preservation checks.

    def test_two_real_invocations_cannot_share_removal_ownership(self):
        fixture = self.fixture
        child = fixture.entry()
        (child / "file").write_bytes(b"unchanged until owner removes")
        with fixture.session() as first, fixture.session() as second:
            expected = first.tree_inspect("stages", "candidate")
            with first.lock("installation"):
                with self.assertRaises(Busy):
                    with second.lock("installation"):
                        self.fail("Contending native invocation acquired owner's lock")
                with self.assertRaises(Refused):
                    second.tree_remove("stages", "candidate", expected)
                self.assertTrue((child / "file").is_file())
                identity = (fixture.state / "installation.lock").stat()
            with second.lock("installation"):
                current = (fixture.state / "installation.lock").stat()
                self.assertEqual((identity.st_dev, identity.st_ino), (current.st_dev, current.st_ino))
                self.assertEqual(second.tree_remove("stages", "candidate", expected), {"absent": True})

    def test_actual_committed_lost_ack_reconciles_without_repeating_removal(self):
        fixture = self.fixture
        child = fixture.entry()
        (child / "file").write_bytes(b"remove once")
        # Allocate while the original still exists so this replacement has a
        # distinct filesystem identity without assuming inode IDs never recycle.
        replacement = fixture.entry("incoming-replacement")
        (replacement / "keep").write_bytes(b"new allocation")
        proxy = self.base / "lost_ack.py"
        proxy.write_text(LOST_ACK_PROXY, encoding="utf-8")
        capture = self.base / "proxy.jsonl"
        real_popen = subprocess.Popen

        def start_proxy(argv, **kwargs):
            self.assertEqual(argv, [fixture.driver, "--config", str(fixture.config), "serve"])
            return real_popen([sys.executable, str(proxy), fixture.driver,
                               str(fixture.config), str(capture)], **kwargs)

        with patch("constellation_native.client.subprocess.Popen", side_effect=start_proxy):
            session = fixture.session()
        self.addCleanup(session.close)
        expected = session.tree_inspect("stages", "candidate")
        with self.assertRaises(Unknown), session.lock("installation"):
            session.tree_remove("stages", "candidate", expected)
        self.assertIsNotNone(session._process.poll())
        with self.assertRaises(DriverError):
            session.tree_inspect("stages", "candidate")
        frames = [json.loads(line) for line in capture.read_text(encoding="utf-8").splitlines()]
        removes = [frame for frame in frames if frame["request"]["op"] == "tree.remove"]
        self.assertEqual(len(removes), 1)
        self.assertEqual(removes[0]["reply"]["result"], {"absent": True})
        with fixture.session() as recovery, recovery.lock("installation"):
            # This read settles the already committed operation. A replacement
            # is later observed as a new identity, never removed by recovery.
            self.assertIsNone(recovery.tree_inspect("stages", "candidate"))
            replacement.rename(child)
            self.assertNotEqual(recovery.tree_inspect("stages", "candidate"), expected)
            self.assertEqual((child / "keep").read_bytes(), b"new allocation")
        self.assertEqual(len([line for line in capture.read_text().splitlines()
                              if json.loads(line)["request"]["op"] == "tree.remove"]), 1)
        fixture.assert_outside()

    @unittest.skipUnless(os.name == "nt", "Actual ordinary long Windows paths")
    def test_unicode_path_over_340_utf16_units(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")) as temporary:
            fixture = TreeFixture(Path(temporary).resolve(), DRIVER, long_path=True)
            child = fixture.entry()
            self.assertGreater(len(str(child).encode("utf-16-le")) // 2, 340)
            (child / "資料.txt").write_bytes(b"long path data")
            link = child / "outside-junction"
            directory_link(link, fixture.outside)
            try:
                with fixture.session() as session, session.lock("installation"):
                    snapshot = session.tree_inspect("stages", "candidate")
                    self.assertEqual(session.tree_remove("stages", "candidate", snapshot), {"absent": True})
                fixture.assert_outside()
            finally:
                remove_directory_link(link)

    @unittest.skipUnless(sys.platform.startswith("linux"), "Linux isolated bind-mount fixture")
    def test_same_device_bind_mount_refuses_before_deletion(self):
        if not shutil.which("unshare") or not shutil.which("mount"):
            self.skipTest("Mount namespace tools unavailable; mount protection not measured")
        child = [sys.executable, str(Path(__file__).resolve()), "--mount-probe",
                 self.fixture.driver, os.readlink("/proc/self/ns/mnt")]
        attempts = [["unshare", "--user", "--map-root-user", "--mount", "--fork", "--", *child]]
        if shutil.which("sudo"):
            attempts.append(["sudo", "-n", "unshare", "--mount", "--fork", "--", *child])
        unavailable = []
        for command in attempts:
            run = subprocess.run(command, text=True, capture_output=True, timeout=45)
            if run.returncode == 0:
                result = json.loads(run.stdout.strip().splitlines()[-1])
                self.assertEqual(result["mount_probe"], "passed")
                self.assertIs(result["same_device"], True)
                self.assertEqual(result["aliases_protected"], 2)
                for name in ("fixed_root_alias", "lock_parent_alias"):
                    self.assertTrue(all(result[name].values()), (name, result[name]))
                print(json.dumps(result), flush=True)
                return
            # Do not turn a native fixture failure into a skipped namespace row.
            if "Traceback (most recent call last)" in run.stderr:
                self.fail("Isolated bind-mount fixture failed: " + run.stderr[-6000:])
            unavailable.append(run.stderr.strip()[-500:])
        self.skipTest("Unprivileged and noninteractive privileged namespaces unavailable: " + " | ".join(unavailable))


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--mount-probe":
        raise SystemExit(mount_probe(sys.argv[2], sys.argv[3]))
    unittest.main()
