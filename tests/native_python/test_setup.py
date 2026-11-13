"""Owner setup fixtures; successful setup uses no Python permission helpers.

The transport peer is simulated. ActualDriverSetupTests uses the supplied native
executable in fresh disposable paths, never live services or game state.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

SUPPORT = Path(__file__).resolve().parents[2] / "support" / "python"
sys.path.insert(0, str(SUPPORT))
from constellation_native import (  # noqa: E402
    Busy,
    DriverError,
    Refused,
    Session,
    Unknown,
    initialize_config,
    initialize_root,
)

DRIVER = os.environ.get("CONSTELLATION_NATIVE_DRIVER")

PEER = r"""
import json, sys, time
from pathlib import Path
mode, capture = sys.argv[1:]
if mode == "no_read":
    time.sleep(30)
request = json.loads(sys.stdin.buffer.readline())
with open(capture, "a", encoding="utf-8") as out:
    out.write(json.dumps(request) + "\n")
if mode == "lost":
    sys.exit(0)
if mode == "timeout":
    time.sleep(30)
result = {"path": request["path"], "created": True}
if request["op"] == "setup.config":
    result["sha256"] = "a" * 64
reply = {"version": 1, "id": request["id"], "outcome": "ok", "result": result}
if mode == "wrong_id":
    reply["id"] = "another-request"
if mode == "malformed":
    print("not-json", flush=True)
    sys.exit(0)
if mode == "bad_version":
    reply["version"] = True
if mode == "bad_result":
    result["created"] = 1
if mode == "bad_path":
    result["path"] = str(Path(request["path"]).with_name("other"))
if mode == "bad_digest":
    result["sha256"] = "not-a-digest"
if mode in ("busy", "refused", "unknown", "error"):
    reply.pop("result")
    reply.update(outcome=mode, error={"code": "fixture_" + mode, "message": mode})
print(json.dumps(reply), flush=True)
"""


class SetupTransportTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.script = self.root / "peer.py"
        self.script.write_text(PEER, encoding="utf-8")
        self.capture = self.root / "requests.jsonl"
        self.target = self.root / "owner"
        self.children = []

    def invoke(self, operation, mode="ok", *, config=None, timeout=2, serialized_pipe=False):
        real_popen = subprocess.Popen

        def spawn(argv, **kwargs):
            self.assertEqual(argv, [sys.executable, "initialize"])
            self.assertTrue(kwargs["close_fds"])
            self.assertEqual(kwargs["stderr"], subprocess.DEVNULL)
            self.assertNotIn("shell", kwargs)
            child = real_popen(
                [sys.executable, str(self.script), mode, str(self.capture)], **kwargs
            )
            self.children.append(child)
            self.addCleanup(self.reap_child, child)
            if serialized_pipe:
                child.stdin = SerializedWritePipe(child.stdin)
            return child

        with patch("constellation_native.client.subprocess.Popen", side_effect=spawn):
            if operation == "setup.root":
                return initialize_root(sys.executable, self.target, timeout=timeout)
            return initialize_config(
                sys.executable, self.target, config or {"version": 1}, timeout=timeout
            )

    @staticmethod
    def reap_child(child):
        if child.poll() is None:
            child.kill()
        child.wait(timeout=5)
        if isinstance(child.stdin, SerializedWritePipe):
            child.stdin.write_finished.wait(timeout=1)
        if child.stdin is not None:
            child.stdin.close()

    def assert_children_reaped(self):
        self.assertTrue(self.children)
        for child in self.children:
            self.assertIsNotNone(child.poll(), "Setup returned with its driver still running")

    def requests(self):
        if not self.capture.exists():
            return []
        return [json.loads(line) for line in self.capture.read_text().splitlines()]

    def test_two_fixed_setup_requests_and_no_session_capability(self):
        self.assertEqual(
            self.invoke("setup.root"), {"path": str(self.target), "created": True}
        )
        config = {"version": 1, "roots": {"state": str(self.target)}}
        reply = self.invoke("setup.config", config=config)
        self.assertEqual(reply["sha256"], "a" * 64)
        requests = self.requests()
        self.assertEqual([request["op"] for request in requests], ["setup.root", "setup.config"])
        self.assertEqual(requests[1]["config"], config)
        self.assertNotEqual(requests[0]["id"], requests[1]["id"])
        self.assertEqual(set(requests[0]), {"version", "id", "op", "path"})
        self.assertFalse(hasattr(Session, "initialize_root"))
        self.assertFalse(hasattr(Session, "initialize_config"))

    def test_lost_ack_is_unknown_with_exactly_one_dispatch(self):
        with self.assertRaises(Unknown) as caught:
            self.invoke("setup.root", "lost")
        self.assertEqual(caught.exception.code, "transport_closed")
        self.assertEqual(len(self.requests()), 1)

    def test_uncorrelated_or_malformed_setup_reply_stays_unknown(self):
        for mode in ("wrong_id", "malformed", "bad_version", "bad_result", "bad_path", "bad_digest"):
            with self.subTest(mode=mode), self.assertRaises(Unknown) as caught:
                self.invoke("setup.config", mode, config={"version": 1, "label": "inert-private-marker"})
            self.assertEqual(caught.exception.code, "protocol_response")
            self.assertNotIn("inert-private-marker", str(caught.exception))
        self.assertEqual(len(self.requests()), 6)

    def test_setup_timeout_never_retries(self):
        started = time.monotonic()
        with self.assertRaises(Unknown) as caught:
            self.invoke("setup.root", "timeout", timeout=0.2)
        self.assertLess(time.monotonic() - started, 5.0)
        self.assertEqual(caught.exception.code, "transport_timeout")
        self.assertEqual(len(self.requests()), 1)
        self.assert_children_reaped()

    def test_setup_write_is_also_bounded(self):
        started = time.monotonic()
        with self.assertRaises(Unknown) as caught:
            self.invoke(
                "setup.config", "no_read", config={"padding": "x" * (1536 * 1024)}, timeout=0.2
            )
        self.assertLess(time.monotonic() - started, 5.0)
        self.assertEqual(caught.exception.code, "transport_timeout")
        self.assertEqual(self.requests(), [])
        self.assert_children_reaped()

    def test_stalled_writer_is_terminated_before_serialized_pipe_close(self):
        # FileIO close can serialize behind an in-flight write on Windows. The
        # wrapper rejects that ordering immediately on every platform rather
        # than allowing an old implementation to hang until the child wakes.
        # The child and full OS pipe are real; only close serialization is modeled.
        started = time.monotonic()
        with self.assertRaises(Unknown) as caught:
            self.invoke(
                "setup.config", "no_read", config={"padding": "x" * (1536 * 1024)},
                timeout=0.2, serialized_pipe=True,
            )
        self.assertLess(time.monotonic() - started, 5.0)
        self.assertEqual(caught.exception.code, "transport_timeout")
        self.assertEqual(self.requests(), [])
        self.assert_children_reaped()
        self.assertTrue(self.children[0].stdin.write_started.is_set())
        self.assertTrue(self.children[0].stdin.closed)

    def test_typed_outcomes_are_preserved(self):
        for outcome, expected in (("busy", Busy), ("refused", Refused), ("unknown", Unknown), ("error", DriverError)):
            with self.subTest(outcome=outcome), self.assertRaises(expected) as caught:
                self.invoke("setup.root", outcome)
            self.assertEqual(caught.exception.code, "fixture_" + outcome)

    def test_spawn_failure_is_observed_error_before_dispatch(self):
        with patch("constellation_native.client.subprocess.Popen", side_effect=OSError("missing")):
            with self.assertRaises(DriverError) as caught:
                initialize_root(sys.executable, self.target)
        self.assertNotIsInstance(caught.exception, Unknown)
        self.assertEqual(caught.exception.code, "driver_start")

    def test_invalid_python_inputs_do_not_start_driver(self):
        with patch(
            "constellation_native.client.subprocess.Popen",
            side_effect=AssertionError("Invalid input reached process start"),
        ) as spawn:
            invalid = (
                False,
                True,
                0,
                -1,
                float("nan"),
                float("inf"),
                -float("inf"),
                "30",
                10**1000,
                1e300,
                threading.TIMEOUT_MAX * 2,
            )
            for timeout in invalid:
                for operation in ("root", "config"):
                    with (
                        self.subTest(timeout=repr(timeout), operation=operation),
                        self.assertRaises(ValueError),
                    ):
                        if operation == "root":
                            initialize_root(
                                sys.executable, self.target, timeout=timeout
                            )
                        else:
                            initialize_config(
                                sys.executable,
                                self.target,
                                {"version": 1},
                                timeout=timeout,
                            )
            with self.assertRaises(ValueError):
                initialize_root("relative-driver", self.target)
            with self.assertRaises(ValueError):
                initialize_root(sys.executable, "relative-target")
            with self.assertRaises(TypeError):
                initialize_config(sys.executable, self.target, b"{}")
            with self.assertRaises(ValueError):
                initialize_config(
                    sys.executable, self.target, {"version": float("nan")}
                )
            with self.assertRaises(ValueError):
                initialize_config(
                    sys.executable, self.target, {"padding": "x" * (2 * 1024 * 1024)}
                )
            spawn.assert_not_called()


class SerializedWritePipe:
    """Test-only close/write serialization over an actual anonymous OS pipe."""

    def __init__(self, stream):
        self.stream = stream
        self.mutex = threading.Lock()
        self.write_started = threading.Event()
        self.write_finished = threading.Event()
        self.writer_thread = None

    def write(self, data):
        with self.mutex:
            self.writer_thread = threading.get_ident()
            self.write_finished.clear()
            self.write_started.set()
            try:
                return self.stream.write(data)
            finally:
                self.write_finished.set()

    def close(self):
        if (
            self.write_started.is_set()
            and not self.write_finished.is_set()
            and threading.get_ident() != self.writer_thread
        ):
            raise AssertionError("Setup closed stdin while another thread was blocked writing")
        with self.mutex:
            self.stream.close()

    @property
    def closed(self):
        return self.stream.closed


def policy_snapshot(path):
    if os.name == "nt":
        # Inspection only. Successful setup never imports or calls fixture ACL
        # creation; the native driver is the sole private-storage implementation.
        from test_driver import windows_policy_text
        return windows_policy_text(path)
    observed = path.stat()
    return (observed.st_uid, observed.st_gid, stat.S_IMODE(observed.st_mode))


def identity_and_policy(path):
    observed = path.stat()
    if os.name == "nt":
        from test_driver import windows_owner_sid
        owner = windows_owner_sid(path)
    else:
        owner = observed.st_uid
    return (observed.st_dev, observed.st_ino, owner, policy_snapshot(path))


@unittest.skipUnless(DRIVER, "Set CONSTELLATION_NATIVE_DRIVER to test native setup")
class ActualDriverSetupTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(
            prefix="native-setup-", dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
        )
        self.addCleanup(temporary.cleanup)
        # Resolve only the test runner's existing temporary location (/var on
        # macOS). Requested installation paths are passed unchanged to the CLI.
        self.parent = Path(temporary.name).resolve()
        self.parent_policy = policy_snapshot(self.parent)
        self.root = self.parent / "owner-café-雪"
        self.assertEqual(
            initialize_root(DRIVER, self.root), {"path": str(self.root), "created": True}
        )
        self.state = self.root / "state"
        initialize_root(DRIVER, self.state)
        self.config = self.root / "owner.json"
        self.data = {
            "version": 1,
            "roots": {"state": str(self.state)},
            "locks": {"installation": str(self.state / "installation.lock")},
            "services": {},
            "commands": {},
        }

    def test_fresh_root_config_session_lock_and_write_without_permission_helpers(self):
        root_policy = policy_snapshot(self.root)
        repeated = initialize_root(DRIVER, self.root)
        self.assertEqual(repeated, {"path": str(self.root), "created": False})
        self.assertEqual(policy_snapshot(self.root), root_policy)
        result = initialize_config(DRIVER, self.config, self.data)
        self.assertEqual(result["path"], str(self.config))
        self.assertTrue(result["created"])
        before = self.config.read_bytes()
        self.assertEqual(result["sha256"], hashlib.sha256(before).hexdigest())
        # Object ordering is not meaningful in the setup request. The native
        # driver alone chooses the exact canonical bytes used for repeat checks.
        repeated = initialize_config(DRIVER, self.config, dict(reversed(list(self.data.items()))))
        self.assertFalse(repeated["created"])
        self.assertEqual(repeated["sha256"], result["sha256"])
        with Session(DRIVER, self.config) as native, Session(DRIVER, self.config) as contender:
            self.assertNotIn("setup.root", native.describe()["operations"])
            self.assertNotIn("setup.config", native.describe()["operations"])
            with native.lock("installation"):
                with self.assertRaises(Busy):
                    with contender.lock("installation"):
                        self.fail("Setup did not preserve actual lock contention")
                native.private_write("state", "receipt.json", b"same installation")
        self.assertEqual((self.state / "receipt.json").read_bytes(), b"same installation")
        self.assertEqual(self.config.read_bytes(), before)
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    def test_changed_existing_config_is_not_overwritten(self):
        initialize_config(DRIVER, self.config, self.data)
        before = self.config.read_bytes()
        policy = policy_snapshot(self.config)
        changed = {**self.data, "roots": {"different": str(self.state)}}
        with self.assertRaises(Refused):
            initialize_config(DRIVER, self.config, changed)
        self.assertEqual(self.config.read_bytes(), before)
        self.assertEqual(policy_snapshot(self.config), policy)

    def test_nested_storage_directory_then_write_uses_production_setup(self):
        initialize_config(DRIVER, self.config, self.data)
        preserved = {path: (path.read_bytes() if path.is_file() else None,
                            identity_and_policy(path))
                     for path in (self.parent, self.root, self.state, self.config)}
        with Session(DRIVER, self.config) as native:
            with native.lock("installation"):
                self.assertEqual(native.private_dir("state", "transactions"), {})
                self.assertEqual(native.private_dir("state", "transactions/original"), {})
                relative = "transactions/original/participant.json"
                content = b'{"task":"original","phase":"prepared","unknowns":[]}'
                native.private_write("state", relative, content, create_only=True)
                paths = [self.state / "transactions", self.state / "transactions/original"]
                identities = [identity_and_policy(path) for path in paths]
                for name in ("transactions", "transactions/original"):
                    self.assertEqual(native.private_dir("state", name), {})
                self.assertEqual([identity_and_policy(path) for path in paths], identities)
                self.assertEqual((self.state / relative).read_bytes(), content)
                if os.name == "nt":
                    from test_driver import windows_owner_sid, windows_user_sid
                    user = windows_user_sid()
                    for path in (*paths, self.state / relative):
                        self.assertEqual(windows_owner_sid(path), user)
                    print(json.dumps({"nested_storage_owner": "passed",
                                      "owner": user, "production_setup": True}))
                replacement = b'{"task":"original","phase":"restored","unknowns":[]}'
                native.private_write("state", relative, replacement)
                self.assertEqual((self.state / relative).read_bytes(), replacement)
                if os.name == "nt":
                    self.assertEqual(windows_owner_sid(self.state / relative), user)
                with self.assertRaises(Refused):
                    native.private_write("state", relative, b"must-not-replace", create_only=True)
                self.assertEqual((self.state / relative).read_bytes(), replacement)
                native.durable_remove("state", relative)
                self.assertFalse((self.state / relative).exists())
                self.assertEqual([identity_and_policy(path) for path in paths], identities)
                print(json.dumps({"nested_storage_create_rewrite_remove": "passed",
                                  "production_setup": True}), flush=True)
        for path, (content, identity) in preserved.items():
            self.assertEqual(identity_and_policy(path), identity)
            if content is not None:
                self.assertEqual(path.read_bytes(), content)
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    @unittest.skipUnless(os.name == "nt", "Windows token-default-owner regression")
    def test_nested_storage_with_administrators_default_owner_in_child_only(self):
        from test_fixture_owner import require_elevated_owner_fixture
        require_elevated_owner_fixture(self)
        # Alter only this short-lived child token, never the test runner's token
        # or an existing filesystem policy. New driver children inherit it.
        code = r'''
import ctypes as c, json, sys, unittest
from ctypes import wintypes as w
sys.path.insert(0, sys.argv[1])
import test_setup
kernel=c.WinDLL("kernel32",use_last_error=True)
advapi=c.WinDLL("advapi32",use_last_error=True)
kernel.GetCurrentProcess.restype=w.HANDLE
kernel.CloseHandle.argtypes=[w.HANDLE]
kernel.LocalFree.argtypes=[c.c_void_p]
advapi.OpenProcessToken.argtypes=[w.HANDLE,w.DWORD,c.POINTER(w.HANDLE)]
advapi.SetTokenInformation.argtypes=[w.HANDLE,c.c_int,c.c_void_p,w.DWORD]
advapi.GetTokenInformation.argtypes=[w.HANDLE,c.c_int,c.c_void_p,w.DWORD,c.POINTER(w.DWORD)]
advapi.ConvertStringSidToSidW.argtypes=[w.LPCWSTR,c.POINTER(c.c_void_p)]
advapi.ConvertSidToStringSidW.argtypes=[c.c_void_p,c.POINTER(w.LPWSTR)]
token=w.HANDLE();sid=c.c_void_p()
class Owner(c.Structure): _fields_=[("sid",c.c_void_p)]
if not advapi.OpenProcessToken(kernel.GetCurrentProcess(),0x8|0x80,c.byref(token)):raise c.WinError(c.get_last_error())
try:
    if not advapi.ConvertStringSidToSidW("S-1-5-32-544",c.byref(sid)):raise c.WinError(c.get_last_error())
    owner=Owner(sid)
    if not advapi.SetTokenInformation(token,4,c.byref(owner),c.sizeof(owner)):raise c.WinError(c.get_last_error())
    needed=w.DWORD();advapi.GetTokenInformation(token,4,None,0,c.byref(needed))
    data=c.create_string_buffer(needed.value)
    if not advapi.GetTokenInformation(token,4,data,len(data),c.byref(needed)):raise c.WinError(c.get_last_error())
    value=w.LPWSTR()
    if not advapi.ConvertSidToStringSidW(c.cast(data,c.POINTER(Owner)).contents.sid,c.byref(value)):raise c.WinError(c.get_last_error())
    try:
        assert value.value=="S-1-5-32-544",value.value
        print(json.dumps({"child_token_default_owner":value.value}),flush=True)
    finally:kernel.LocalFree(c.cast(value,c.c_void_p))
finally:
    if sid.value:kernel.LocalFree(sid)
    kernel.CloseHandle(token)
suite=unittest.defaultTestLoader.loadTestsFromNames([
    "ActualDriverSetupTests.test_nested_storage_directory_then_write_uses_production_setup",
    "ActualDriverSetupTests.test_nested_storage_refuses_foreign_owner_without_repair",
    "ActualDriverSetupTests.test_nested_storage_refuses_foreign_file_owner_without_repair",
],test_setup)
sys.exit(0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1)
'''
        result = subprocess.run([sys.executable, "-I", "-B", "-c", code,
                                 str(Path(__file__).resolve().parent)],
                                capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('"child_token_default_owner": "S-1-5-32-544"', result.stdout)
        self.assertIn('"nested_storage_owner": "passed"', result.stdout)
        print(result.stdout, end="")

    @unittest.skipUnless(os.name == "nt", "Windows existing-owner preservation")
    def test_nested_storage_refuses_foreign_owner_without_repair(self):
        from test_fixture_owner import require_elevated_owner_fixture
        require_elevated_owner_fixture(self)
        from test_driver import windows_owner_sid, windows_policy
        initialize_config(DRIVER, self.config, self.data)
        with Session(DRIVER, self.config) as native:
            native.private_dir("state", "foreign")
            path = self.state / "foreign"
            # Fault injection in an existing disposable directory; no repair.
            windows_policy(path, owner_sid="S-1-5-32-544")
            before = (windows_owner_sid(path), policy_snapshot(path), path.stat().st_ino)
            self.assertEqual(before[0], "S-1-5-32-544")
            for relative in ("foreign", "foreign/child"):
                with self.assertRaises(Refused):
                    native.private_dir("state", relative)
            self.assertEqual((windows_owner_sid(path), policy_snapshot(path), path.stat().st_ino), before)
            self.assertEqual(list(path.iterdir()), [])

    @unittest.skipUnless(os.name == "nt", "Windows existing file-owner preservation")
    def test_nested_storage_refuses_foreign_file_owner_without_repair(self):
        from test_fixture_owner import require_elevated_owner_fixture
        require_elevated_owner_fixture(self)
        from test_driver import windows_owner_sid, windows_policy
        initialize_config(DRIVER, self.config, self.data)
        with Session(DRIVER, self.config) as native:
            with native.lock("installation"):
                native.private_dir("state", "retained")
                relative = "retained/participant.json"
                content = b'{"task":"original","unknowns":["retained"]}'
                native.private_write("state", relative, content, create_only=True)
                path = self.state / relative
                windows_policy(path, owner_sid="S-1-5-32-544")
                before = (identity_and_policy(path), path.read_bytes())
                self.assertEqual(windows_owner_sid(path), "S-1-5-32-544")
                with self.assertRaises(Refused):
                    native.private_write("state", relative, b"must-not-replace")
                self.assertEqual((identity_and_policy(path), path.read_bytes()), before)
                with self.assertRaises(Refused):
                    native.durable_remove("state", relative)
                self.assertEqual((identity_and_policy(path), path.read_bytes()), before)

    def test_nested_storage_creation_race_preserves_one_directory(self):
        from concurrent.futures import ThreadPoolExecutor
        initialize_config(DRIVER, self.config, self.data)
        with Session(DRIVER, self.config) as left, Session(DRIVER, self.config) as right:
            for index in range(8):
                name = f"transaction-{index}"
                barrier = threading.Barrier(2)
                def create(session):
                    barrier.wait(timeout=5)
                    return session.private_dir("state", name)
                with ThreadPoolExecutor(max_workers=2) as executor:
                    a, b = executor.submit(create, left), executor.submit(create, right)
                    self.assertEqual(a.result(timeout=10), {})
                    self.assertEqual(b.result(timeout=10), {})
                path = self.state / name
                before = (path.stat().st_dev, path.stat().st_ino, policy_snapshot(path))
                left.private_write("state", name + "/receipt", b"retained", create_only=True)
                right.private_dir("state", name)
                self.assertEqual((path.stat().st_dev, path.stat().st_ino, policy_snapshot(path)), before)
                self.assertEqual((path / "receipt").read_bytes(), b"retained")

    def test_nested_storage_refuses_unsafe_existing_paths_without_repair(self):
        initialize_config(DRIVER, self.config, self.data)
        with Session(DRIVER, self.config) as native:
            native.private_write("state", "file", b"retained", create_only=True)
            for relative in ("absent/child", "../escape", "file"):
                with self.subTest(relative=relative), self.assertRaises(Refused):
                    native.private_dir("state", relative)
            self.assertFalse((self.state / "absent").exists())
            self.assertFalse((self.root / "escape").exists())
            self.assertEqual((self.state / "file").read_bytes(), b"retained")
            native.private_dir("state", "exposed")
            exposed = self.state / "exposed"
            if os.name == "nt":
                subprocess.run(["icacls.exe", str(exposed), "/grant", "*S-1-1-0:(OI)(CI)(R)"],
                               check=True, capture_output=True, timeout=10)
            else:
                exposed.chmod(0o755)
            before = (policy_snapshot(exposed), exposed.stat().st_ino)
            with self.assertRaises(Refused):
                native.private_dir("state", "exposed")
            self.assertEqual((policy_snapshot(exposed), exposed.stat().st_ino), before)
            self.assertEqual(list(exposed.iterdir()), [])
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    def test_config_requires_valid_existing_prerequisites(self):
        for data in (
            {**self.data, "version": 2},
            {**self.data, "unknown": True},
            {**self.data, "roots": {"missing": str(self.root / "missing")}},
        ):
            with self.subTest(data=data), self.assertRaises(Refused):
                initialize_config(DRIVER, self.config, data)
            self.assertFalse(self.config.exists())
        self.assertFalse((self.root / "missing").exists())

    def test_missing_parent_traversal_and_existing_file_refuse_without_changes(self):
        target = self.root / "absent" / "child"
        with self.assertRaises(Refused):
            initialize_root(DRIVER, target)
        self.assertFalse(target.parent.exists())
        traversal = self.root / ".." / "escaped"
        with self.assertRaises(Refused):
            initialize_root(DRIVER, traversal)
        self.assertFalse((self.parent / "escaped").exists())
        existing = self.root / "existing-file"
        existing.write_bytes(b"preserved")
        with self.assertRaises(Refused):
            initialize_root(DRIVER, existing)
        self.assertEqual(existing.read_bytes(), b"preserved")
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    def test_existing_nonprivate_root_is_refused_without_policy_repair(self):
        exposed = self.root / "exposed"
        initialize_root(DRIVER, exposed)
        # Fault injection only: deliberately weaken a disposable existing root.
        # This is not part of successful setup or a domain ACL implementation.
        if os.name == "nt":
            subprocess.run(
                ["icacls.exe", str(exposed), "/grant", "*S-1-1-0:(OI)(CI)(R)"],
                check=True, capture_output=True, timeout=10,
            )
        else:
            exposed.chmod(0o755)
        before = policy_snapshot(exposed)
        with self.assertRaises(Refused):
            initialize_root(DRIVER, exposed)
        self.assertEqual(policy_snapshot(exposed), before)
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)

    def test_link_root_and_ancestor_refuse_without_touching_target(self):
        outside = self.root / "outside"
        initialize_root(DRIVER, outside)
        sentinel = outside / "sentinel"
        sentinel.write_bytes(b"preserved")
        link = self.root / "link"
        if os.name == "nt":
            subprocess.run(
                ["cmd.exe", "/d", "/c", "mklink", "/J", str(link), str(outside)],
                check=True, capture_output=True, timeout=10,
            )
            self.addCleanup(lambda: link.rmdir() if link.exists() else None)
        else:
            link.symlink_to(outside, target_is_directory=True)
        for target in (link, link / "must-not-exist"):
            with self.subTest(target=target), self.assertRaises(Refused):
                initialize_root(DRIVER, target)
        self.assertEqual(sentinel.read_bytes(), b"preserved")
        self.assertEqual(sorted(path.name for path in outside.iterdir()), ["sentinel"])

    @unittest.skipUnless(os.name == "nt", "Actual Windows long Unicode setup")
    def test_deep_unicode_roots_and_config_use_setup_at_every_level(self):
        ordinary = str(self.root)
        if ordinary.startswith("\\\\?\\UNC\\"):
            ordinary = "\\\\" + ordinary[8:]
        elif ordinary.startswith("\\\\?\\"):
            ordinary = ordinary[4:]
        home = Path(ordinary)
        while len(str(home).encode("utf-16-le")) // 2 <= 362:
            home /= "install-café-雪-🧭-0123456789"
            result = initialize_root(DRIVER, home)
            self.assertTrue(result["created"])
        state = home / "state"
        initialize_root(DRIVER, state)
        config = home / "owner-雪-🧭.json"
        data = {
            "version": 1,
            "roots": {"state": str(state)},
            "locks": {"installation": str(state / "installation.lock")},
        }
        self.assertFalse(str(config).startswith("\\\\?\\"))
        self.assertGreater(len(str(config).encode("utf-16-le")) // 2, 362)
        result = initialize_config(DRIVER, config, data)
        self.assertEqual(result["sha256"], hashlib.sha256(config.read_bytes()).hexdigest())
        with Session(DRIVER, config) as native:
            with native.lock("installation"):
                native.private_write("state", "receipt.json", b"long path confirmed")
        self.assertEqual((state / "receipt.json").read_bytes(), b"long path confirmed")
        self.assertEqual(policy_snapshot(self.parent), self.parent_policy)
        print(json.dumps({"setup_owner_config_utf16_units": len(str(config).encode("utf-16-le")) // 2,
                          "setup_without_permission_helpers": True}))


if __name__ == "__main__":
    unittest.main()
