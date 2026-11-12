"""Executable packaging guards, including the actual CI driver when supplied."""

import hashlib
import os
from pathlib import Path
import platform
import struct
import tempfile
import unittest
from unittest.mock import patch

import inspect_driver


# Fixture layout from Runtime 3896baaf tests/package_release.py. The inspector
# itself is vendored byte-for-byte; this creates metadata, not an executable.
def pe(architecture="x86_64", imported="KERNEL32.dll", delayed=False):
    data = bytearray(0x800)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", data, 0x84, 0x8664 if architecture == "x86_64" else 0xAA64, 1)
    struct.pack_into("<H", data, 0x94, 240)
    optional = 0x98
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<I", data, optional + 60, 0x200)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<IIII", data, optional + 240 + 8, 0x600, 0x1000, 0x600, 0x200)
    directory, size = (13, 64) if delayed else (1, 40)
    struct.pack_into("<II", data, optional + 112 + directory * 8, 0x1000, size)
    if delayed:
        struct.pack_into("<8I", data, 0x200, 1, 0x1100, 0, 0, 0, 0, 0, 0)
    else:
        struct.pack_into("<5I", data, 0x200, 0, 0, 0, 0x1100, 0)
    data[0x300:0x300 + len(imported) + 1] = imported.encode() + b"\0"
    return data


class DriverPackage(unittest.TestCase):
    def test_accepted_runtime_inspector_is_byte_exact(self):
        _, provenance = inspect_driver.load_inspector()
        self.assertEqual(provenance, {
            "repository": "https://github.com/microsoft/amplifier-constellation-runtime",
            "commit": "3896baaf8bc5e160dd3fd7fab3d8da68480bdbf3",
            "path": "scripts/native_binary.py",
            "sha256": "d6dc8cdce7c752b10a7e4a239d53b66b40370a6f69434a4272ad7abca5137dc1",
        })

    def test_both_windows_architectures_retain_import_evidence(self):
        for machine, architecture in (("AMD64", "x86_64"), ("ARM64", "aarch64")):
            for delayed in (False, True):
                with self.subTest(machine=machine, delayed=delayed), tempfile.TemporaryDirectory() as temp:
                    path = Path(temp) / "driver.exe"
                    path.write_bytes(pe(architecture, "KERNEL32.dll", delayed))
                    receipt = inspect_driver.inspect_driver(path, system="Windows", machine=machine)
                    self.assertEqual(receipt["driver_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
                    result = receipt["driver_inspection"]
                    self.assertEqual(result["architecture"], architecture)
                    self.assertTrue(result["standalone_vc_runtime"])
                    self.assertEqual(result["external_vc_runtime"], [])
                    self.assertEqual(result["delay_imports" if delayed else "imports"], ["KERNEL32.dll"])

    def test_external_crt_ordinary_or_delayed_import_refuses_actual_receipt(self):
        for machine, architecture in (("AMD64", "x86_64"), ("ARM64", "aarch64")):
            for imported, delayed in (("VCRUNTIME140.dll", False), ("mSvCp140_2.DLL", True)):
                with self.subTest(machine=machine, imported=imported), tempfile.TemporaryDirectory() as temp:
                    path = Path(temp) / "driver.exe"
                    path.write_bytes(pe(architecture, imported, delayed))
                    with self.assertRaisesRegex(ValueError, "external Visual C"):
                        inspect_driver.inspect_driver(path, system="Windows", machine=machine)

    def test_wrong_architecture_and_truncated_binary_are_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "driver.exe"
            path.write_bytes(pe("aarch64"))
            with self.assertRaisesRegex(ValueError, "architecture"):
                inspect_driver.inspect_driver(path, system="Windows", machine="AMD64")
            path.write_bytes(pe()[:200])
            with self.assertRaises(ValueError):
                inspect_driver.inspect_driver(path, system="Windows", machine="AMD64")

    def test_preflight_record_rejects_changed_artifact_after_tests(self):
        with tempfile.TemporaryDirectory() as temp:
            path, output = Path(temp) / "driver.exe", Path(temp) / "result.json"
            path.write_bytes(pe())
            with patch.object(inspect_driver.platform, "system", return_value="Windows"), \
                    patch.object(inspect_driver.platform, "machine", return_value="AMD64"):
                inspect_driver.record(path, output)
                before = output.read_bytes()
                inspect_driver.record(path, output, verify=True)
                path.write_bytes(pe(imported="ADVAPI32.dll"))
                with self.assertRaisesRegex(ValueError, "changed after qualification"):
                    inspect_driver.record(path, output, verify=True)
                self.assertEqual(output.read_bytes(), before)

    @unittest.skipUnless(os.environ.get("CONSTELLATION_NATIVE_DRIVER"), "actual CI driver not supplied")
    def test_actual_executed_driver_matches_host_and_has_no_external_vc_runtime(self):
        path = Path(os.environ["CONSTELLATION_NATIVE_DRIVER"])
        receipt = inspect_driver.inspect_driver(path, system=platform.system(), machine=platform.machine())
        self.assertEqual(receipt["driver_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        if os.name == "nt":
            self.assertTrue(receipt["driver_inspection"]["standalone_vc_runtime"])


if __name__ == "__main__":
    unittest.main()
