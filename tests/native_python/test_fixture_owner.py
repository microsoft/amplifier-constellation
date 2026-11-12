"""Explicit Windows owner preparation for fresh disposable fixtures only."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from test_driver import (
    DRIVER,
    Session,
    protect,
    windows_owner_sid,
    windows_policy,
    windows_policy_text,
    windows_user_sid,
)


@unittest.skipUnless(os.name == "nt" and DRIVER, "Actual Windows driver required")
class WindowsFixtureOwnerTests(unittest.TestCase):
    def test_explicit_fixture_protection_changes_owner_but_driver_never_does(self):
        with tempfile.TemporaryDirectory(
            prefix="native-fixture-owner-",
            dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT"),
        ) as temporary:
            parent = Path(temporary).resolve()
            protect(parent)
            parent_before = (windows_owner_sid(parent), windows_policy_text(parent))
            root = parent / "fresh-root"
            root.mkdir()
            config = root / "owner.json"
            data = {
                "version": 1,
                "roots": {"state": str(root)},
                "locks": {"installation": str(root / "installation.lock")},
            }
            config.write_text(json.dumps(data), encoding="utf-8")
            contents = config.read_bytes()
            config_identity = (config.stat().st_dev, config.stat().st_ino)
            root_identity = (root.stat().st_dev, root.stat().st_ino)
            user = windows_user_sid()
            initial_owner = windows_owner_sid(root)
            administrator = "S-1-5-32-544"
            # The test must establish the exact faulty fixture condition even
            # when this Python version already makes current-user-owned folders.
            # An inability to create it is a failed qualification, never a skip.
            try:
                windows_policy(root, owner_sid=administrator)
            except OSError as error:
                self.fail(
                    "Cannot establish the disposable Administrators-owned fixture; "
                    f"initial owner={initial_owner}, current user={user}: {error}"
                )
            windows_policy(config, owner_sid=user)
            self.assertEqual(windows_owner_sid(root), administrator)
            self.assertEqual(windows_owner_sid(config), user)
            private_dacl = windows_policy_text(root)
            self.assertNotIn(";;;BA)", private_dacl)
            self.assertNotIn(";;;WD)", private_dacl)

            def rejected_without_repair():
                before = {
                    path: (windows_owner_sid(path), windows_policy_text(path))
                    for path in (root, config)
                }
                result = subprocess.run(
                    [str(Path(DRIVER).resolve()), "--config", str(config), "serve"],
                    input=b'{"version":1,"id":"owner-fixture","op":"describe"}\n',
                    capture_output=True,
                    timeout=10,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertIn(b"Installation must belong to this user", result.stderr)
                self.assertIn(administrator.encode(), result.stderr)
                self.assertEqual(config.read_bytes(), contents)
                self.assertFalse((root / "installation.lock").exists())
                for path, policy in before.items():
                    self.assertEqual(
                        (windows_owner_sid(path), windows_policy_text(path)), policy
                    )

            rejected_without_repair()
            protect(root)
            self.assertEqual(windows_owner_sid(root), user)
            self.assertEqual(windows_policy_text(root), private_dacl)
            self.assertEqual((root.stat().st_dev, root.stat().st_ino), root_identity)
            with Session(DRIVER, config) as session:
                self.assertEqual(session.describe()["protocol"], 1)

            windows_policy(config, owner_sid=administrator)
            self.assertEqual(windows_owner_sid(config), administrator)
            rejected_without_repair()
            protect(config)
            self.assertEqual(windows_owner_sid(config), user)
            self.assertEqual(config.read_bytes(), contents)
            self.assertEqual(
                (config.stat().st_dev, config.stat().st_ino), config_identity
            )
            self.assertTrue(config.is_file())
            with Session(DRIVER, config) as session:
                self.assertEqual(session.describe()["protocol"], 1)
                with session.lock("installation"):
                    session.private_write("state", "receipt.json", b"fixture proof")
            self.assertEqual((root / "receipt.json").read_bytes(), b"fixture proof")
            self.assertEqual(
                (windows_owner_sid(parent), windows_policy_text(parent)), parent_before
            )
            print(
                json.dumps(
                    {
                        "windows_fixture_owner": "passed",
                        "python": sys.version,
                        "initial_root_owner": initial_owner,
                        "explicit_group_owner": administrator,
                        "verified_current_user_owner": user,
                        "root_and_config_refused_before_fixture_repair": True,
                        "production_policy_unchanged": True,
                    }
                )
            )


if __name__ == "__main__":
    unittest.main()
