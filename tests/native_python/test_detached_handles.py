"""Actual detached launch/capture ownership; no services, shell, or live homes.

The endpoint waits for an explicit fixture release. EOF must arrive before that
release while both recorded native generations remain alive. Windows additionally
compares a deliberately inheritable event by kernel object identity, not by a
possibly reused handle number.
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
import uuid

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parents[1] / "support" / "python"))
from constellation_native import NativeSession  # noqa: E402
from test_driver import protect  # noqa: E402


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


class _WindowsEvent:
    """Test-only Win32 handles; duplicate only from the exact observed process."""

    def __init__(self, name: str, *, inherited: bool = False):
        import ctypes
        from ctypes import wintypes

        self.ctypes = ctypes
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.kernelbase = ctypes.WinDLL("kernelbase", use_last_error=True)
        api = self.kernel
        api.CreateEventW.argtypes = [ctypes.c_void_p, wintypes.BOOL,
                                     wintypes.BOOL, wintypes.LPCWSTR]
        api.CreateEventW.restype = wintypes.HANDLE
        api.OpenEventW.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.LPCWSTR]
        api.OpenEventW.restype = wintypes.HANDLE
        api.GetHandleInformation.argtypes = [wintypes.HANDLE,
                                             ctypes.POINTER(wintypes.DWORD)]
        api.GetHandleInformation.restype = wintypes.BOOL
        api.GetCurrentProcess.argtypes = []
        api.GetCurrentProcess.restype = wintypes.HANDLE
        api.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        api.OpenProcess.restype = wintypes.HANDLE
        api.DuplicateHandle.argtypes = [wintypes.HANDLE, wintypes.HANDLE,
                                       wintypes.HANDLE, ctypes.POINTER(wintypes.HANDLE),
                                       wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        api.DuplicateHandle.restype = wintypes.BOOL
        self.kernelbase.CompareObjectHandles.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        self.kernelbase.CompareObjectHandles.restype = wintypes.BOOL
        api.CloseHandle.argtypes = [wintypes.HANDLE]
        api.CloseHandle.restype = wintypes.BOOL
        self.handle = (api.OpenEventW(0x1F0003, True, name) if inherited
                       else api.CreateEventW(None, True, False, name))
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        flags = wintypes.DWORD()
        if not api.GetHandleInformation(self.handle, ctypes.byref(flags)):
            self.close()
            raise ctypes.WinError(ctypes.get_last_error())
        if bool(flags.value & 1) != inherited:
            self.close()
            raise AssertionError("Fixture event inheritance flag differs")
        self.inherited = inherited

    def close(self) -> None:
        if self.handle:
            handle, self.handle = self.handle, None
            if not self.kernel.CloseHandle(handle):
                raise self.ctypes.WinError(self.ctypes.get_last_error())

    def same_object_in(self, identity: dict, candidate: int, native: NativeSession) -> bool:
        """A reused numeric handle is harmless unless it is this exact event."""
        from ctypes import wintypes

        api, ctypes = self.kernel, self.ctypes
        if not native.process_matches(identity):
            raise AssertionError("Witness process generation ended before inspection")
        process = api.OpenProcess(0x0040, False, identity["pid"])  # PROCESS_DUP_HANDLE
        if not process:
            raise ctypes.WinError(ctypes.get_last_error())
        duplicate = wintypes.HANDLE()
        try:
            copied = api.DuplicateHandle(process, candidate, api.GetCurrentProcess(),
                                         ctypes.byref(duplicate), 0, False, 0x2)
            if not copied:
                error = ctypes.get_last_error()
                if error != 6:  # ERROR_INVALID_HANDLE, not access/observation failure.
                    raise ctypes.WinError(error)
                result = False
            else:
                # Compare against the parent's live reference to the GUID-named
                # event. Other inherited or reused handles cannot be mistaken
                # for this witness merely because the numeric value matches.
                result = bool(self.kernelbase.CompareObjectHandles(duplicate, self.handle))
        finally:
            if duplicate.value:
                api.CloseHandle(duplicate)
            api.CloseHandle(process)
        if not native.process_matches(identity):
            raise AssertionError("Witness process generation changed during inspection")
        return result


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
    event = _WindowsEvent(plan["event"], inherited=True) if plan.get("event") else None
    try:
        with NativeSession(plan["library"], plan["driver"], plan["config"]) as native:
            caller = native.process_identity(os.getpid())
            _save(root / "caller-opened.json", {
                "identity": caller, "event_handle": int(event.handle) if event else None,
                "event_inheritable": bool(event and event.inherited),
            })
            with native.lock("installation"):
                supervisor = native.launch("endpoint")
            lock_released = time.monotonic()
        _save(root / "caller-finished.json", {
            "identity": caller, "supervisor": supervisor,
            "lock_released": lock_released, "native_closed": time.monotonic(),
        })
        print("short-caller-complete", flush=True)
    finally:
        if event:
            event.close()
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

    def _exercise(self, *, event_witness: bool = False):
        event_name = "Local\\ConstellationNativeFixture-" + uuid.uuid4().hex
        event = _WindowsEvent(event_name) if event_witness else None
        if event:
            self.addCleanup(event.close)
        self.plan_path.write_text(json.dumps({
            "root": str(self.root), "library": self.library, "driver": str(self.driver),
            "config": str(self.config), "event": event_name if event else None,
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
                # wait() observes process exit only. Reader completion separately
                # proves EOF rather than hiding it in subprocess.communicate().
                code = process.wait(timeout=20)
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
                if event:
                    opened = _read_until(self.root / "caller-opened.json")
                    self.assertTrue(opened["event_inheritable"])
                    for identity in identities:
                        self.assertFalse(event.same_object_in(identity, opened["event_handle"],
                                                             observation),
                                         "Unrelated inheritable event reached detached process")
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

    @unittest.skipUnless(os.name == "nt", "Windows inheritable kernel object handles")
    def test_unrelated_inheritable_event_does_not_reach_detached_processes(self):
        self._exercise(event_witness=True)


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "--fixture":
        raise SystemExit(_child(sys.argv[2], Path(sys.argv[3]), sys.argv[4:]))
    unittest.main()
