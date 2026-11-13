"""Library metadata fixtures never execute; actual ABI coverage is separate."""
import hashlib
import os
from pathlib import Path
import platform
import struct
import tempfile
import unittest
from unittest.mock import Mock, patch

import inspect_driver
import inspect_library
import run_packaged_python
from test_package import pe


def dll(architecture="x86_64", imported="KERNEL32.dll", delayed=False):
    data = pe(architecture, imported, delayed)
    struct.pack_into("<H", data, 0x80 + 22, 0x2002)
    return data


def elf(architecture="x86_64", needed=("libc.so.6",), *, kind=3, interpreter=False, extra=()):
    data = bytearray(0x1000)
    data[:7] = b"\x7fELF\x02\x01\x01"
    count = 3 if interpreter else 2
    struct.pack_into("<HHIQQQIHHHHHH", data, 16, kind,
                     {"x86_64": 62, "aarch64": 183}[architecture], 1, 0, 64, 0, 0,
                     64, 56, count, 0, 0, 0)
    strings = b"\0"
    entries = []
    for name in needed:
        entries.append((1, len(strings)))
        strings += name.encode("ascii") + b"\0"
    entries += [(5, 0x400), (10, len(strings)), *extra, (0, 0)]
    struct.pack_into("<IIQQQQQQ", data, 64, 1, 6, 0, 0, 0, len(data), len(data), 4096)
    struct.pack_into("<IIQQQQQQ", data, 120, 2, 6, 0x200, 0x200, 0x200,
                     len(entries) * 16, len(entries) * 16, 8)
    if interpreter:
        struct.pack_into("<IIQQQQQQ", data, 176, 3, 4, 0x600, 0x600, 0x600, 4, 4, 1)
    for index, (tag, value) in enumerate(entries):
        struct.pack_into("<qQ", data, 0x200 + index * 16, tag, value)
    data[0x400:0x400 + len(strings)] = strings
    return data


def dylib_command(command, name):
    payload = name.encode("ascii") + b"\0"
    size = (24 + len(payload) + 7) // 8 * 8
    data = bytearray(size)
    struct.pack_into("<IIIIII", data, 0, command, size, 24, 0, 0, 0)
    data[24:24 + len(payload)] = payload
    return data


def macho(architecture="x86_64", *, kind=6, dependency="/usr/lib/libSystem.B.dylib", extra=()):
    commands = [struct.pack("<6I", 0x32, 24, 1, 11 << 16, 15 << 16, 0),
                dylib_command(0xD, "@rpath/libconstellation_native_driver.dylib"),
                dylib_command(0xC, dependency), *extra]
    header = struct.pack("<8I", 0xFEEDFACF,
                         {"x86_64": 0x01000007, "aarch64": 0x0100000C}[architecture],
                         0, kind, len(commands), sum(map(len, commands)), 0, 0)
    return bytearray(header + b"".join(commands))


class LibraryPackage(unittest.TestCase):
    def inspect(self, data, system="Linux", machine="x86_64"):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "native-library"
            path.write_bytes(data)
            return inspect_library.inspect_library(path, system=system, machine=machine)

    def test_elf_libraries_of_both_architectures_record_dependencies(self):
        for machine, architecture in [("x86_64", "x86_64"), ("aarch64", "aarch64")]:
            data = elf(architecture, ("libgcc_s.so.1", "libc.so.6"))
            receipt = self.inspect(data, machine=machine)
            self.assertEqual(receipt["library_sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual(receipt["library_inspection"]["needed"], ["libgcc_s.so.1", "libc.so.6"])
            self.assertFalse(receipt["library_loaded"])
            self.assertFalse(receipt["abi_verified"])

    def test_elf_executable_and_pie_refused(self):
        for data in [elf(kind=2), elf(interpreter=True), elf(extra=((0x6FFFFFFB, 0x08000000),))]:
            with self.subTest(), self.assertRaisesRegex(ValueError, "executable"):
                self.inspect(data)

    def test_elf_extra_dependencies_search_paths_and_filters_refused(self):
        for data in [elf(needed=("libpython3.13.so",)), elf(needed=("/tmp/libc.so.6",)),
                     elf(extra=((15, 1),)), elf(extra=((29, 1),)), elf(extra=((0x7FFFFFFF, 1),))]:
            with self.subTest(), self.assertRaises(ValueError):
                self.inspect(data)

    def test_elf_malformed_dynamic_and_string_bounds_refused(self):
        candidates = []
        data = elf(); struct.pack_into("<Q", data, 0x200 + 16 + 8, 0x10000); candidates.append(data)
        data = elf(); struct.pack_into("<H", data, 54, 8); candidates.append(data)
        data = elf(); struct.pack_into("<Q", data, 0x200 + 32 + 8, 2 * 1024 * 1024); candidates.append(data)
        candidates += [elf()[:150], elf("aarch64")]
        for data in candidates:
            with self.subTest(), self.assertRaises(ValueError):
                self.inspect(data)

    def test_macho_requires_dylib_and_keeps_executable_inspector_unchanged(self):
        inspector, _ = inspect_driver.load_inspector()
        for architecture, machine in [("x86_64", "x86_64"), ("aarch64", "arm64")]:
            data = macho(architecture)
            receipt = self.inspect(data, "Darwin", machine)
            self.assertEqual(receipt["library_inspection"]["deployment_target"], "11.0.0")
            self.assertEqual(receipt["library_inspection"]["kind"], "shared_library")
            with self.assertRaisesRegex(ValueError, "executable architecture"):
                inspector.inspect(data, "macos-" + architecture)
            for kind in (2, 8):
                with self.assertRaisesRegex(ValueError, "MH_DYLIB"):
                    self.inspect(macho(architecture, kind=kind), "Darwin", machine)

    def test_macho_only_system_dependencies_in_all_load_forms(self):
        for command in (0xC, 0x80000018, 0x8000001F, 0x20, 0x80000023):
            receipt = self.inspect(macho(extra=[dylib_command(command, "/System/Library/Frameworks/Security.framework/Versions/A/Security")]), "Darwin")
            self.assertEqual(len(receipt["library_inspection"]["dependencies"]), 2)
            for name in ("@rpath/libextra.dylib", "/usr/local/lib/custom.dylib", "/usr/lib/../../tmp/custom.dylib"):
                with self.subTest(command=command, name=name), self.assertRaisesRegex(ValueError, "non-system"):
                    self.inspect(macho(extra=[dylib_command(command, name)]), "Darwin")

    def test_macho_wrong_architecture_platform_and_search_path_refused(self):
        data = macho(); struct.pack_into("<I", data, 32 + 8, 2)
        rpath = struct.pack("<IIII", 0x8000001C, 16, 12, 0)
        candidates = [data, macho("aarch64"), macho(extra=[rpath]), macho()[:-1]]
        for data in candidates:
            with self.subTest(), self.assertRaises(ValueError):
                self.inspect(data, "Darwin")

    def test_windows_both_architectures_and_delayed_import_evidence(self):
        for machine, architecture in [("AMD64", "x86_64"), ("ARM64", "aarch64")]:
            for delayed in (False, True):
                receipt = self.inspect(dll(architecture, delayed=delayed), "Windows", machine)
                inspection = receipt["library_inspection"]
                self.assertEqual(inspection["delay_imports" if delayed else "imports"], ["KERNEL32.dll"])
                self.assertTrue(inspection["standalone_vc_runtime"])
                self.assertEqual(inspection["kind"], "shared_library")

    def test_windows_executable_wrong_architecture_and_external_crt_refused(self):
        candidates = [pe(), dll("aarch64"), dll(imported="VCRUNTIME140.dll"),
                      dll(imported="mSvCp140_2.dll", delayed=True), dll()[:160]]
        for data in candidates:
            with self.subTest(), self.assertRaises(ValueError):
                self.inspect(data, "Windows", "AMD64")

    def test_artifact_bound_and_posttest_identity_refuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            path, output = Path(temporary) / "native.so", Path(temporary) / "inspection.json"
            path.write_bytes(elf())
            with patch.object(inspect_library.platform, "system", return_value="Linux"), \
                 patch.object(inspect_library.platform, "machine", return_value="x86_64"):
                inspect_library.record(path, output)
                before = output.read_bytes()
                inspect_library.record(path, output, verify=True)
                changed = elf(); changed[-1] = 1; path.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, "changed after qualification"):
                    inspect_library.record(path, output, verify=True)
                self.assertEqual(output.read_bytes(), before)
                with patch.object(inspect_library, "MAX_LIBRARY_BYTES", 1024):
                    with self.assertRaisesRegex(ValueError, "byte bound"):
                        inspect_library.inspect_library(path, system="Linux", machine="x86_64")

    @unittest.skipUnless(os.environ.get("CONSTELLATION_NATIVE_LIBRARY"), "actual native library not supplied")
    def test_actual_library_metadata_matches_host_without_loading(self):
        path = Path(os.environ["CONSTELLATION_NATIVE_LIBRARY"])
        receipt = inspect_library.inspect_library(path, system=platform.system(), machine=platform.machine())
        self.assertEqual(receipt["library_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertFalse(receipt["library_loaded"])


class PackagedLibraryQualification(unittest.TestCase):
    def outcomes(self):
        return {"passed": ["test_inprocess_lock." + name for name in run_packaged_python.REQUIRED_INPROCESS_TESTS],
                "skipped": [], "errors": [], "failures": [], "expected_failures": []}

    def test_mandatory_actual_abi_and_helper_death_proof_cannot_be_missing_or_skipped(self):
        outcomes = self.outcomes()
        run_packaged_python.require_inprocess_passes(outcomes)
        for category in ("skipped", "errors", "failures", "expected_failures"):
            value = self.outcomes()
            missing = value["passed"].pop()
            value[category].append({"test": missing, "reason": "fixture unavailable"})
            with self.subTest(category=category), self.assertRaises(ValueError):
                run_packaged_python.require_inprocess_passes(value)
        with self.assertRaises(ValueError):
            run_packaged_python.require_inprocess_passes({"passed": []})

    def test_duplicate_or_conflicting_test_outcomes_cannot_qualify(self):
        value = self.outcomes(); value["passed"].append(value["passed"][0])
        with self.assertRaises(ValueError):
            run_packaged_python.require_inprocess_passes(value)
        value = self.outcomes(); value["skipped"].append({"test": value["passed"][0]})
        with self.assertRaises(ValueError):
            run_packaged_python.require_inprocess_passes(value)

    def test_changed_library_refuses_before_loading(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "untrusted-library"
            path.write_bytes(b"not code")
            with patch.dict(os.environ, {"CONSTELLATION_NATIVE_LIBRARY": str(path),
                                         "CONSTELLATION_NATIVE_LIBRARY_SHA256": "0" * 64}), \
                 patch.object(run_packaged_python.ctypes, "CDLL") as loader:
                with self.assertRaisesRegex(ValueError, "qualification binding"):
                    run_packaged_python.probe_library()
                loader.assert_not_called()

    def test_relative_library_refuses_before_loading(self):
        with patch.dict(os.environ, {"CONSTELLATION_NATIVE_LIBRARY": "relative-library",
                                     "CONSTELLATION_NATIVE_LIBRARY_SHA256": "0" * 64}), \
             patch.object(run_packaged_python.ctypes, "CDLL") as loader:
            with self.assertRaisesRegex(ValueError, "absolute"):
                run_packaged_python.probe_library()
            loader.assert_not_called()

    def test_wrong_loaded_abi_refuses_qualification(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "fixture-only-library"
            data = elf(); path.write_bytes(data)
            digest = hashlib.sha256(data).hexdigest()
            library = Mock()
            library.constellation_native_abi_version.return_value = 2
            with patch.dict(os.environ, {"CONSTELLATION_NATIVE_LIBRARY": str(path),
                                         "CONSTELLATION_NATIVE_LIBRARY_SHA256": digest}), \
                 patch.object(inspect_library, "inspect_library", return_value={"library_sha256": digest}), \
                 patch.object(run_packaged_python.ctypes, "CDLL", return_value=library):
                with self.assertRaisesRegex(ValueError, "ABI differs"):
                    run_packaged_python.probe_library()


if __name__ == "__main__":
    unittest.main()
