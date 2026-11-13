"""Actual detached launch/capture ownership; no services, shell, or live homes.

The endpoint waits for an explicit fixture release. EOF must arrive before that
release while both recorded native generations remain alive. Windows additionally
holds a deliberately inheritable exclusive file handle. A control open must fail
while the caller holds it, then succeed on the same unchanged file after caller
exit while both detached generations still live.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "support" / "python"))
from constellation_native import NativeSession  # noqa: E402
from test_driver import protect, windows_native_path  # noqa: E402


def _save(path: Path, value: dict) -> None:
    temporary = path.with_suffix(".pending")
    temporary.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
    os.replace(temporary, path)


def _read_until(path: Path, timeout: float = 10) -> dict:
    until = time.monotonic() + timeout
    while time.monotonic() < until:
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            time.sleep(0.01)
    raise AssertionError("Missing disposable launch receipt: " + path.name)


class _WindowsExclusiveFile:
    """Test-only sharing witness; never infer inheritance from a handle number."""

    def __init__(self, path: Path, *, inherited: bool = False):
        import ctypes
        from ctypes import wintypes

        self.ctypes = ctypes
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        api = self.kernel
        class SecurityAttributes(ctypes.Structure):
            _fields_ = [("length", wintypes.DWORD), ("descriptor", ctypes.c_void_p),
                        ("inherit", wintypes.BOOL)]
        class FileInformation(ctypes.Structure):
            _fields_ = [("attributes", wintypes.DWORD), ("created", wintypes.FILETIME),
                        ("accessed", wintypes.FILETIME), ("written", wintypes.FILETIME),
                        ("volume", wintypes.DWORD), ("size_high", wintypes.DWORD),
                        ("size_low", wintypes.DWORD), ("links", wintypes.DWORD),
                        ("index_high", wintypes.DWORD), ("index_low", wintypes.DWORD)]
        self.information_type = FileInformation
        api.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                   ctypes.POINTER(SecurityAttributes), wintypes.DWORD,
                                   wintypes.DWORD, wintypes.HANDLE]
        api.CreateFileW.restype = wintypes.HANDLE
        api.GetHandleInformation.argtypes = [wintypes.HANDLE,
                                             ctypes.POINTER(wintypes.DWORD)]
        api.GetHandleInformation.restype = wintypes.BOOL
        api.GetFileInformationByHandle.argtypes = [wintypes.HANDLE,
                                                    ctypes.POINTER(FileInformation)]
        api.GetFileInformationByHandle.restype = wintypes.BOOL
        api.ReadFile.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD,
                                 ctypes.POINTER(wintypes.DWORD), ctypes.c_void_p]
        api.ReadFile.restype = wintypes.BOOL
        api.CloseHandle.argtypes = [wintypes.HANDLE]
        api.CloseHandle.restype = wintypes.BOOL
        security = SecurityAttributes(ctypes.sizeof(SecurityAttributes), None, inherited)
        # GENERIC_READ, no sharing, OPEN_EXISTING, OPEN_REPARSE_POINT. Creating
        # the file and its private policy belongs to existing fixture setup.
        self.handle = api.CreateFileW(windows_native_path(path), 0x80000000, 0,
                                      ctypes.byref(security), 3, 0x00200080, None)
        if self.handle == ctypes.c_void_p(-1).value:
            self.handle = None
            raise ctypes.WinError(ctypes.get_last_error())
        flags = wintypes.DWORD()
        if not api.GetHandleInformation(self.handle, ctypes.byref(flags)):
            error = ctypes.get_last_error()
            self.close()
            raise ctypes.WinError(error)
        if bool(flags.value & 1) != inherited:
            self.close()
            raise AssertionError("Fixture file inheritance flag differs")
        self.inherited = inherited

    def close(self) -> None:
        if self.handle:
            handle, self.handle = self.handle, None
            if not self.kernel.CloseHandle(handle):
                raise self.ctypes.WinError(self.ctypes.get_last_error())

    def snapshot(self) -> dict:
        """Observe file identity and bytes through this exact exclusive handle."""
        from ctypes import wintypes

        api, ctypes = self.kernel, self.ctypes
        information = self.information_type()
        if not api.GetFileInformationByHandle(self.handle, ctypes.byref(information)):
            raise ctypes.WinError(ctypes.get_last_error())
        size = (information.size_high << 32) | information.size_low
        if information.attributes & 0x400 or size > 4096:
            raise AssertionError("Unexpected witness reparse point or fixture size")
        buffer = ctypes.create_string_buffer(4097)
        read = wintypes.DWORD()
        if not api.ReadFile(self.handle, buffer, len(buffer), ctypes.byref(read), None):
            raise ctypes.WinError(ctypes.get_last_error())
        if read.value != size:
            raise AssertionError("Incomplete exclusive-file witness read")
        return {"volume": information.volume, "index_high": information.index_high,
                "index_low": information.index_low, "size": size,
                "bytes": buffer.raw[:read.value].hex()}


def _child(mode: str, plan_path: Path, arguments: list[str]) -> int:
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    root = Path(plan["root"])
    if mode == "endpoint":
        with NativeSession(plan["library"], plan["driver"], plan["config"]) as native:
            identity = native.process_identity(os.getpid())
        _save(root / "endpoint.json", {
            "identity": identity, "argv": arguments, "cwd": str(Path.cwd().resolve()),
        })
        until = time.monotonic() + 30
        while not (root / "release").exists():
            if time.monotonic() >= until:
                raise TimeoutError("Disposable endpoint was not explicitly released")
            time.sleep(0.01)
        _save(root / "endpoint-finished.json", {"released": True, "identity": identity})
        return 0
    if mode != "caller":
        raise ValueError("Unknown fixture mode")
    witness = (_WindowsExclusiveFile(Path(plan["exclusive_file"]), inherited=True)
               if plan.get("exclusive_file") else None)
    try:
        with NativeSession(plan["library"], plan["driver"], plan["config"]) as native:
            caller = native.process_identity(os.getpid())
            _save(root / "caller-opened.json", {
                "identity": caller, "file_inheritable": bool(witness and witness.inherited),
                "file": witness.snapshot() if witness else None,
            })
            if witness:
                until = time.monotonic() + 10
                while not (root / "launch-release").exists():
                    if time.monotonic() >= until:
                        raise TimeoutError("Exclusive-file prelaunch control was not released")
                    time.sleep(0.01)
            with native.lock("installation"):
                supervisor = native.launch("endpoint")
            lock_released = time.monotonic()
        _save(root / "caller-finished.json", {
            "identity": caller, "supervisor": supervisor,
            "lock_released": lock_released, "native_closed": time.monotonic(),
        })
        print("short-caller-complete", flush=True)
    finally:
        if witness:
            witness.close()
    return 0


class DetachedHandleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        required = ("CONSTELLATION_NATIVE_LIBRARY", "CONSTELLATION_NATIVE_DRIVER")
        missing = [name for name in required if not os.environ.get(name)]
        if missing:
            message = "Detached handle conformance requires " + ", ".join(missing)
            if os.environ.get("CI", "").lower() in ("1", "true"):
                raise AssertionError(message)
            raise unittest.SkipTest(message)
        cls.library = str(Path(os.environ[required[0]]).resolve(strict=True))
        cls.input_driver = Path(os.environ[required[1]]).resolve(strict=True)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="native handles 雪 ",
                                               dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT"))
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        protect(self.root)
        self.state = self.root / "state journal"
        self.cwd = self.root / "working directory café"
        for directory in (self.state, self.cwd):
            directory.mkdir()
            protect(directory)
        # Keep the Unix basename inside the kernel's comm byte limit while the
        # full executable/config/cwd/argv paths still exercise Unicode/spaces.
        self.driver = self.root / ("雪 driver.exe" if os.name == "nt" else "雪 driver")
        shutil.copyfile(self.input_driver, self.driver)
        protect(self.driver)
        if os.name != "nt":
            self.driver.chmod(0o700)
        self.config = self.root / "owner config 雪.json"
        self.plan_path = self.root / "fixture plan 雪.json"
        self.arguments = ["argument with spaces", 'embedded"quote', "", "tail\\", "机器人 café"]
        self.config.write_text(json.dumps({
            "version": 1, "roots": {"state": str(self.state)},
            "locks": {"installation": str(self.state / "installation.lock")},
            "services": {}, "commands": {"endpoint": {
                "executable": str(Path(sys.executable).resolve()),
                "args": ["-I", str(Path(__file__).resolve()), "--fixture", "endpoint",
                         str(self.plan_path), *self.arguments],
                "cwd": str(self.cwd), "timeout_seconds": 30, "result_root": None,
            }},
        }), encoding="utf-8")
        protect(self.config)

    def _exercise(self, *, file_witness: bool = False):
        witness_path = self.root / "exclusive witness 雪.bin"
        expected_file = None
        if file_witness:
            witness_path.write_bytes(b"exclusive native fixture\0" + bytes(range(16)))
            protect(witness_path)
            reference = _WindowsExclusiveFile(witness_path)
            try:
                expected_file = reference.snapshot()
            finally:
                reference.close()
        self.plan_path.write_text(json.dumps({
            "root": str(self.root), "library": self.library, "driver": str(self.driver),
            "config": str(self.config), "exclusive_file": str(witness_path) if file_witness else None,
        }), encoding="utf-8")
        protect(self.plan_path)
        stdout, stderr = [], []
        eof = [threading.Event(), threading.Event()]
        def drain(stream, output, done):
            try:
                output.append(stream.read())
            finally:
                done.set()
        readers = []
        identities = []
        # Observation setup can refuse. Do it before creating any owned child,
        # and arm cleanup immediately after Popen succeeds, including failures
        # while constructing or starting the capture readers.
        with NativeSession(self.library, self.driver, self.config) as observation:
            caller_deadline = time.monotonic() + 20
            process = subprocess.Popen([
                sys.executable, "-I", str(Path(__file__).resolve()), "--fixture", "caller",
                str(self.plan_path),
            ], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                close_fds=True, cwd=self.cwd)
            try:
                readers = [threading.Thread(target=drain, args=(stream, output, done), daemon=True)
                           for stream, output, done in zip((process.stdout, process.stderr),
                                                          (stdout, stderr), eof)]
                for thread in readers:
                    thread.start()
                if file_witness:
                    opened = _read_until(self.root / "caller-opened.json", timeout=10)
                    self.assertTrue(opened["file_inheritable"])
                    self.assertEqual(opened["file"], expected_file)
                    try:
                        unexpected = _WindowsExclusiveFile(witness_path)
                    except OSError as error:
                        self.assertEqual(error.winerror, 32, "Expected sharing violation, not refusal")
                    else:
                        unexpected.close()
                        self.fail("Control open succeeded while caller held its exclusive handle")
                    self.assertTrue(observation.process_matches(opened["identity"]))
                    (self.root / "launch-release").touch()
                # wait() observes process exit only. Reader completion separately
                # proves EOF rather than hiding it in subprocess.communicate().
                code = process.wait(timeout=max(0, caller_deadline - time.monotonic()))
                if code:
                    eof[1].wait(1)
                self.assertEqual(code, 0, b"".join(stderr).decode("utf-8", errors="replace"))
                caller = _read_until(self.root / "caller-finished.json")
                endpoint = _read_until(self.root / "endpoint.json")
                identities = [caller["supervisor"], endpoint["identity"]]
                self.assertFalse(observation.process_matches(caller["identity"]))
                self.assertEqual(endpoint["argv"], self.arguments)
                self.assertEqual(endpoint["cwd"], str(self.cwd.resolve()))
                self.assertTrue(all(observation.process_matches(i) for i in identities))
                if file_witness:
                    # The caller is gone but these same detached generations are
                    # alive. Any retained inherited exclusive handle would make
                    # this open fail with ERROR_SHARING_VIOLATION.
                    reopened = _WindowsExclusiveFile(witness_path)
                    try:
                        self.assertEqual(reopened.snapshot(), expected_file)
                    finally:
                        reopened.close()
                self.assertTrue(eof[0].wait(3), "stdout remained open after caller exit")
                self.assertTrue(eof[1].wait(3), "stderr remained open after caller exit")
                self.assertEqual(b"".join(stdout), b"short-caller-complete\r\n" if os.name == "nt"
                                 else b"short-caller-complete\n")
                self.assertEqual(b"".join(stderr), b"")
                # The same processes must still be alive after EOF; waiting for
                # endpoint expiry/cleanup cannot accidentally satisfy this case.
                self.assertTrue(all(observation.process_matches(i) for i in identities))
                with observation.lock("installation"):
                    self.assertFalse((self.root / "release").exists())
                    self.assertTrue(all(observation.process_matches(i) for i in identities))
                self.assertFalse((self.root / "endpoint-finished.json").exists())
                (self.root / "release").touch()
                self.assertTrue(_read_until(self.root / "endpoint-finished.json")["released"])
            finally:
                # Even a red inheritance witness releases the same endpoint and
                # observes exact generations; no PID-only kill or retry launch.
                primary = sys.exc_info()[1]
                cleanup_errors = []
                def attempt(action):
                    try:
                        action()
                    except BaseException as error:
                        cleanup_errors.append(error)
                attempt(lambda: (self.root / "release").touch(exist_ok=True))
                attempt(lambda: (self.root / "launch-release").touch(exist_ok=True))
                def settle_caller():
                    if process.poll() is None:
                        # Give this short caller time to publish the generation
                        # receipt after an early reader-start failure. Its endpoint
                        # is already released, so this cannot wait on that gate.
                        try:
                            process.wait(timeout=20)
                        except subprocess.TimeoutExpired:
                            process.kill()  # Exact still-owned Popen caller only.
                            process.wait(timeout=5)
                attempt(settle_caller)
                def settle_generations():
                    for name, key in (("caller-finished.json", "supervisor"),
                                      ("endpoint.json", "identity")):
                        if (self.root / name).exists():
                            identity = json.loads((self.root / name).read_text(encoding="utf-8"))[key]
                            if identity not in identities:
                                identities.append(identity)
                    until = time.monotonic() + 35
                    while any(observation.process_matches(i) for i in identities):
                        if time.monotonic() >= until:
                            self.fail("Owned detached generations did not settle")
                        time.sleep(0.02)
                attempt(settle_generations)
                for thread in readers:
                    if thread.ident is not None:
                        attempt(lambda thread=thread: thread.join(timeout=5))
                        if thread.is_alive():
                            cleanup_errors.append(AssertionError("Capture cleanup unconfirmed"))
                for index, stream in enumerate((process.stdout, process.stderr)):
                    if index >= len(readers) or not readers[index].is_alive():
                        attempt(stream.close)
                if cleanup_errors:
                    if primary is None:
                        raise cleanup_errors[0]
                    for error in cleanup_errors:
                        primary.add_note("Fixture cleanup also failed: " + repr(error))

    def test_captured_caller_eof_precedes_detached_endpoint_release(self):
        self._exercise()

    @unittest.skipUnless(os.name == "nt", "Windows inheritable exclusive file handles")
    def test_unrelated_inheritable_exclusive_file_is_released_with_caller(self):
        self._exercise(file_witness=True)


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "--fixture":
        raise SystemExit(_child(sys.argv[2], Path(sys.argv[3]), sys.argv[4:]))
    unittest.main()
