"""Actual disposable service registration -> Python driver -> application identity."""

from __future__ import annotations

from contextlib import contextmanager
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

from test_driver import DRIVER, Refused, Session, protect


@contextmanager
def fixture_directory():
    temp = tempfile.TemporaryDirectory(
        prefix="native-service-app-",
        dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT"),
    )
    try:
        yield temp.name
    finally:
        failure = sys.exc_info()[1]
        try:
            temp.cleanup()
        except Exception as error:
            if failure is None:
                raise
            failure.add_note(f"Disposable directory cleanup also failed: {error!r}")


@unittest.skipUnless(
    DRIVER and os.environ.get("CONSTELLATION_INSTALLATION_SMOKE") == "1",
    "Requires an opted-in disposable native service runner",
)
class NativeServiceApplicationTests(unittest.TestCase):
    def test_fresh_no_start_exact_controller_binding_and_preserved_pause(self):
        with fixture_directory() as temp:
            root = Path(temp).resolve()
            protect(root)
            config = root / "owner.json"
            name = "constellation-driver-test-" + uuid.uuid4().hex
            definition = None
            if sys.platform == "linux":
                name += ".service"
                definition = Path.home() / ".config/systemd/user" / name
                manager = "systemd_user"
            elif sys.platform == "darwin":
                definition = Path.home() / "Library/LaunchAgents" / (name + ".plist")
                manager = "launch_agent"
            else:
                manager = "scheduled_task"
            if definition:
                definition.parent.mkdir(parents=True, exist_ok=True)
            actual = root / "actual.json"
            receipt = root / "identity.json"
            program = root / "application.py"
            program.write_text(
                "import os,json,pathlib,time\n"
                "pathlib.Path("
                + repr(str(actual))
                + ').write_text(json.dumps({"pid":os.getpid()}))\n'
                "time.sleep(120)\n"
            )
            protect(program)
            data = {
                "version": 1,
                "roots": {"state": str(root)},
                "locks": {"installation": str(root / "installation.lock")},
                "services": {
                    "controller": {
                        "manager": manager,
                        "name": name,
                        "definition_path": str(definition) if definition else None,
                    }
                },
                "registrations": {
                    "controller": {
                        "executable": str(Path(DRIVER).resolve()),
                        "args": [
                            "--config",
                            str(config),
                            "service-run",
                            "--service",
                            "controller",
                        ],
                        "cwd": str(root),
                        "description": "Disposable Unicode service α",
                        "autostart": False,
                    }
                },
                "service_applications": {
                    "controller": {
                        "executable": sys.executable,
                        "args": [str(program)],
                        "cwd": str(root),
                        "identity_path": str(receipt),
                    }
                },
            }
            config.write_text(json.dumps(data))
            protect(config)
            application = None
            try:
                with Session(DRIVER, config) as session, session.lock("installation"):
                    first = session.service_register("controller")
                    self.assertTrue(first["created"])
                    self.assertFalse(
                        actual.exists(), "Registration must not start the application"
                    )
                    stopped = self.wait(session, False)
                    second = session.service_register("controller")
                    self.assertFalse(second["created"])
                    self.assertEqual(
                        stopped,
                        self.wait(session, False),
                        "Repeat setup may not reload or enable",
                    )
                    session.service_action("controller", "start", stopped)
                    running = self.wait(session, True)
                    application = running["application"]["identity"]
                    self.assertEqual(
                        application["pid"], json.loads(actual.read_text())["pid"]
                    )
                    self.assertTrue(session.process_matches(application))
                    self.assertNotEqual(
                        application, running["application"]["supervisor"]
                    )
                    self.assertIn(
                        running["application"]["manager_process"], running["processes"]
                    )
                    original = receipt.read_bytes()
                    forged = json.loads(original)
                    forged["application"] = session.process_identity(os.getpid())
                    session.private_write(
                        "state", "identity.json", json.dumps(forged).encode()
                    )
                    unconfirmed = session.service_snapshot("controller")
                    self.assertEqual(
                        unconfirmed["application"]["status"], "unconfirmed"
                    )
                    with self.assertRaises(Refused):
                        session.service_action("controller", "stop", unconfirmed)
                    self.assertTrue(session.process_matches(application))
                    session.private_write("state", "identity.json", original)
                    running = self.wait(session, True)
                    session.service_action("controller", "stop", running)
                    stopped = self.wait(session, False)
                    self.assertFalse(session.process_matches(application))
                    session.service_action("controller", "start", stopped)
                    restarted = self.wait(session, True)
                    self.assertNotEqual(
                        restarted["application"]["identity"], application
                    )
                    with self.assertRaises(Refused):
                        session.service_action("controller", "stop", running)
                    session.service_action("controller", "stop", restarted)
                    self.wait(session, False)
                # An independently created pause must survive the same setup request.
                self.pause(name, definition)
                with Session(DRIVER, config) as session, session.lock("installation"):
                    before = session.service_snapshot("controller")
                    try:
                        session.service_register("controller")
                    except Refused:
                        pass
                    after = session.service_snapshot("controller")
                    self.assertEqual(after, before)
                    with self.assertRaises(Refused):
                        session.service_action("controller", "start", after)
                print(
                    json.dumps(
                        {
                            "native_service_application": "passed",
                            "platform": sys.platform,
                            "actual_application": application,
                            "no_start": True,
                            "generation_binding": True,
                            "independent_pause_preserved": True,
                        }
                    ),
                    flush=True,
                )
            finally:
                failure = sys.exc_info()[1]
                try:
                    self.cleanup(name, definition, config, receipt)
                except Exception as error:
                    if failure is None:
                        raise
                    failure.add_note(f"Disposable service cleanup also failed: {error!r}")

    @staticmethod
    def wait(session, active):
        until = time.monotonic() + 25
        latest = None
        while time.monotonic() < until:
            try:
                latest = session.service_snapshot("controller")
                if (
                    latest["active"] is active
                    and latest["transitioning"] is False
                    and latest["application"]["status"]
                    == ("running" if active else "absent")
                ):
                    return latest
            except Exception as error:
                latest = repr(error)
            time.sleep(0.1)
        raise AssertionError(f"Native application did not settle: {latest}")

    @staticmethod
    def powershell(script, name, **environment):
        exe = (
            Path(os.environ["SystemRoot"])
            / "System32/WindowsPowerShell/v1.0/powershell.exe"
        )
        subprocess.run(
            [str(exe), "-NoProfile", "-NonInteractive", "-Command", script],
            env=dict(os.environ, DISPOSABLE_SERVICE_NAME=name, **environment),
            check=True,
            timeout=30,
        )

    def pause(self, name, definition):
        if sys.platform == "linux":
            # A static manual-only unit has no enablement to disable: create an
            # actual independent mask while retaining its exact original bytes.
            definition.rename(definition.with_suffix(".saved"))
            definition.symlink_to("/dev/null")
            subprocess.run(
                ["systemctl", "--user", "daemon-reload"], check=True, timeout=10
            )
        elif sys.platform == "darwin":
            subprocess.run(
                ["launchctl", "disable", f"gui/{os.getuid()}/{name}"],
                check=True,
                timeout=10,
            )
        else:
            self.powershell(
                "$s=New-Object -ComObject 'Schedule.Service';$s.Connect();$t=$s.GetFolder('\\').GetTask($env:DISPOSABLE_SERVICE_NAME);$t.Enabled=$false",
                name,
            )

    def cleanup(self, name, definition, config, receipt):
        if sys.platform == "linux":
            subprocess.run(
                ["systemctl", "--user", "stop", name], capture_output=True, timeout=10
            )
            definition.unlink(missing_ok=True)
            definition.with_suffix(".saved").unlink(missing_ok=True)
            subprocess.run(
                ["systemctl", "--user", "daemon-reload"],
                capture_output=True,
                timeout=10,
            )
        elif sys.platform == "darwin":
            subprocess.run(
                ["launchctl", "bootout", f"gui/{os.getuid()}/{name}"],
                capture_output=True,
                timeout=10,
            )
            subprocess.run(
                ["launchctl", "enable", f"gui/{os.getuid()}/{name}"],
                capture_output=True,
                timeout=10,
            )
            definition.unlink(missing_ok=True)
        else:
            # The ownership failure is diagnostic evidence, not a reason to
            # skip stopping this unique disposable task. Never repair its ACL.
            try:
                self.powershell(
                    "$ErrorActionPreference='Stop';"
                    "$rows=@();foreach($p in @($env:DISPOSABLE_ROOT,$env:DISPOSABLE_RECEIPT,$env:DISPOSABLE_LOCK)){"
                    "if(Test-Path -LiteralPath $p){$a=Get-Acl -LiteralPath $p;"
                    "$rows+=@{path=$p;owner_sid=$a.GetOwner([System.Security.Principal.SecurityIdentifier]).Value}}};"
                    "@{fixture_owners=$rows;current_sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value}|ConvertTo-Json -Compress -Depth 4",
                    name,
                    DISPOSABLE_ROOT=str(config.parent),
                    DISPOSABLE_RECEIPT=str(receipt),
                    DISPOSABLE_LOCK=str(receipt.with_suffix(".invocation.lock")),
                )
            except Exception as error:
                print(f"Disposable owner diagnostics failed: {error!r}", flush=True)
            identities = []
            try:
                identities = self.receipt_identities(receipt)
            except Exception as error:
                # An unreadable receipt must not prevent stopping our fixture.
                # The post-stop observation will still fail if it stays unknown.
                print(f"Disposable pre-stop receipt read failed: {error!r}", flush=True)
            self.powershell(
                "$ErrorActionPreference='Stop';$s=New-Object -ComObject 'Schedule.Service';$s.Connect();"
                "$f=$s.GetFolder('\\');try{$t=$f.GetTask($env:DISPOSABLE_SERVICE_NAME)}"
                "catch{if($_.Exception.HResult -eq -2147024894){exit 0}else{throw}};"
                "$t.Stop(0);$w=[Diagnostics.Stopwatch]::StartNew();"
                "while($t.GetInstances(0).Count -ne 0){"
                "if($w.Elapsed.TotalSeconds -ge 25){throw 'Disposable task did not stop'};Start-Sleep -Milliseconds 100}",
                name,
            )
            # TaskScheduler Stop acknowledges the request before its wrapper's
            # job descendants and non-inherited invocation lock have settled.
            # Observe exact generations; never kill a PID from a stale receipt.
            self.wait_windows_release(config, receipt, identities)
            self.powershell(
                "$s=New-Object -ComObject 'Schedule.Service';$s.Connect();$f=$s.GetFolder('\\');"
                "try{$f.DeleteTask($env:DISPOSABLE_SERVICE_NAME,0)}"
                "catch{if($_.Exception.HResult -ne -2147024894){throw}}",
                name,
            )

    @staticmethod
    def receipt_identities(receipt):
        if not receipt.exists():
            return []
        value = json.loads(receipt.read_text())
        if value.get("schema") != 1 or value.get("service") != "controller":
            raise AssertionError("Unrecognized disposable application receipt")
        return [value[key] for key in ("supervisor", "application")]

    @staticmethod
    def windows_lock_released(path):
        """Read-only exclusive open: no delete, repair, or lock-file rewrite."""
        import ctypes
        from ctypes import wintypes

        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        create = kernel.CreateFileW
        create.argtypes = [
            wintypes.LPCWSTR,
            wintypes.DWORD,
            wintypes.DWORD,
            ctypes.c_void_p,
            wintypes.DWORD,
            wintypes.DWORD,
            wintypes.HANDLE,
        ]
        create.restype = wintypes.HANDLE
        close = kernel.CloseHandle
        close.argtypes = [wintypes.HANDLE]
        close.restype = wintypes.BOOL
        handle = create(str(path), 0x80000000, 0, None, 3, 0x80, None)
        if handle == ctypes.c_void_p(-1).value:
            error = ctypes.get_last_error()
            if error == 2:  # No invocation was started / lock absent.
                return True
            if error in (32, 33):  # Sharing / lock violation: still owned.
                return False
            raise ctypes.WinError(error)
        if not close(handle):
            raise ctypes.WinError(ctypes.get_last_error())
        return True

    def wait_windows_release(self, config, receipt, identities):
        until = time.monotonic() + 25
        latest = None
        with Session(DRIVER, config, timeout=5) as session:
            while time.monotonic() < until:
                for identity in self.receipt_identities(receipt):
                    if identity not in identities:
                        identities.append(identity)
                live = [
                    identity
                    for identity in identities
                    if session.process_matches(identity)
                ]
                released = self.windows_lock_released(
                    receipt.with_suffix(".invocation.lock")
                )
                latest = {"live_generations": live, "invocation_lock_released": released}
                if not live and released:
                    print(json.dumps({"disposable_service_cleanup": latest}), flush=True)
                    return
                time.sleep(0.1)
        raise AssertionError(f"Disposable service descendants did not settle: {latest}")
