"""Actual disposable service registration -> Python driver -> application identity."""

from __future__ import annotations

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


@unittest.skipUnless(
    DRIVER and os.environ.get("CONSTELLATION_INSTALLATION_SMOKE") == "1",
    "Requires an opted-in disposable native service runner",
)
class NativeServiceApplicationTests(unittest.TestCase):
    def test_fresh_no_start_exact_controller_binding_and_preserved_pause(self):
        with tempfile.TemporaryDirectory(
            prefix="native-service-app-",
            dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT"),
        ) as temp:
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
                self.cleanup(name, definition)

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
    def powershell(script, name):
        exe = (
            Path(os.environ["SystemRoot"])
            / "System32/WindowsPowerShell/v1.0/powershell.exe"
        )
        subprocess.run(
            [str(exe), "-NoProfile", "-NonInteractive", "-Command", script],
            env=dict(os.environ, DISPOSABLE_SERVICE_NAME=name),
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

    def cleanup(self, name, definition):
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
            self.powershell(
                "$s=New-Object -ComObject 'Schedule.Service';$s.Connect();$f=$s.GetFolder('\\');try{$t=$f.GetTask($env:DISPOSABLE_SERVICE_NAME);$t.Stop(0);$f.DeleteTask($env:DISPOSABLE_SERVICE_NAME,0)}catch{if($_.Exception.HResult -ne -2147024894){throw}}",
                name,
            )
