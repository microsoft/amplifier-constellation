"""Inspect the exact native-session library without loading or executing it.

Library checks are separate from the unchanged Runtime executable inspector.
ELF fields follow https://gabi.xinuos.com/elf/09-dynamic.html; Mach-O commands
follow Apple's mach-o/loader.h; PE imports reuse the pinned Runtime parser.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import struct
import subprocess

import inspect_driver

MAX_LIBRARY_BYTES = 80 * 1024 * 1024
LINUX_SYSTEM_LIBRARIES = frozenset({
    "libc.so.6", "libgcc_s.so.1", "libm.so.6", "libpthread.so.0",
    "libdl.so.2", "librt.so.1", "ld-linux-x86-64.so.2", "ld-linux-aarch64.so.1",
})


def unpack(data, fmt, offset):
    if offset < 0 or offset + struct.calcsize(fmt) > len(data):
        raise ValueError("Truncated native library metadata")
    return struct.unpack_from(fmt, data, offset)


def text_at(data, offset, end):
    if not 0 <= offset < end <= len(data):
        raise ValueError("Invalid library dependency string offset")
    stop = data.find(b"\0", offset, min(end, offset + 4096))
    if stop <= offset:
        raise ValueError("Missing or unterminated library dependency")
    value = data[offset:stop].decode("ascii")
    if any(ord(char) < 32 or ord(char) == 127 for char in value):
        raise ValueError("Invalid library dependency text")
    return value


def inspect_elf(data, architecture, inspector):
    result = inspector.inspect(data, "linux-" + architecture)
    if unpack(data, "<H", 16)[0] != 3:
        raise ValueError("Expected an ELF shared library, not an executable")
    header_size, entry_size, count = unpack(data, "<HHH", 52)
    offset = unpack(data, "<Q", 32)[0]
    if header_size != 64 or entry_size != 56 or not 0 < count <= 1024 or offset < 64:
        raise ValueError("Invalid ELF library program headers")
    if offset + entry_size * count > len(data):
        raise ValueError("Truncated ELF library program headers")
    loads, dynamic = [], []
    for index in range(count):
        kind, _flags, start, address, _physical, size, memory_size, _alignment = unpack(
            data, "<IIQQQQQQ", offset + index * entry_size)
        if start + size > len(data) or size > memory_size:
            raise ValueError("Invalid ELF library segment")
        if kind == 3:
            raise ValueError("ELF interpreter belongs to an executable, not this library")
        if kind == 1:
            loads.append((address, start, size))
        elif kind == 2:
            dynamic.append((start, size))
    if len(dynamic) != 1 or not loads:
        raise ValueError("Missing or ambiguous ELF dynamic metadata")
    start, size = dynamic[0]
    if size < 16 or size % 16 or size > 4096 * 16:
        raise ValueError("Invalid ELF dynamic table bound")
    entries = []
    for at in range(start, start + size, 16):
        tag, value = unpack(data, "<qQ", at)
        if tag == 0:
            break
        entries.append((tag, value))
    else:
        raise ValueError("Unterminated ELF dynamic table")
    if any(tag in (15, 29, 0x7FFFFFFD, 0x7FFFFFFF) for tag, _ in entries):
        raise ValueError("ELF library has an extra search path or dependency filter")
    if any(tag == 0x6FFFFFFB and value & 0x08000000 for tag, value in entries):
        raise ValueError("ELF PIE executable is not a native-session library")

    def single(tag):
        values = [value for candidate, value in entries if candidate == tag]
        if len(values) != 1:
            raise ValueError("Missing or ambiguous ELF string table")
        return values[0]

    address, size = single(5), single(10)
    if not 0 < size <= 1024 * 1024:
        raise ValueError("Invalid ELF string table bound")
    locations = [start + address - base for base, start, length in loads
                 if base <= address and address + size <= base + length]
    if len(locations) != 1:
        raise ValueError("ELF string table is outside stored load segments")
    start = locations[0]

    def dependency(value):
        if value >= size:
            raise ValueError("ELF dependency is outside its string table")
        return text_at(data, start + value, start + size)

    needed = [dependency(value) for tag, value in entries if tag == 1]
    if any(name not in LINUX_SYSTEM_LIBRARIES for name in needed):
        raise ValueError("ELF library requires a dependency outside the system baseline")
    names = [dependency(value) for tag, value in entries if tag == 14]
    if len(names) > 1 or any("/" in name or "\\" in name for name in names):
        raise ValueError("Invalid ELF library SONAME")
    return {**result, "kind": "shared_library", "needed": needed,
            "soname": names[0] if names else None, "search_paths": [],
            "dependency_baseline": "glibc-system-libraries"}


def inspect_macho(data, architecture):
    if len(data) < 32 or data[:4] != b"\xcf\xfa\xed\xfe":
        raise ValueError("Expected a native little-endian Mach-O64 library")
    cpu, _subtype, kind, count, command_bytes = unpack(data, "<IIIII", 4)
    if cpu != {"x86_64": 0x01000007, "aarch64": 0x0100000C}[architecture]:
        raise ValueError("Wrong Mach-O library architecture")
    if kind != 6:
        raise ValueError("Expected MH_DYLIB, not a Mach-O executable or bundle")
    if not 0 < count <= 4096 or 32 + command_bytes > len(data):
        raise ValueError("Invalid Mach-O library command bounds")
    position, minimum, install_name = 32, None, None
    dependencies = []
    dependency_commands = {0xC, 0x80000018, 0x8000001F, 0x20, 0x80000023}
    for _ in range(count):
        if position + 8 > 32 + command_bytes:
            raise ValueError("Truncated Mach-O library load command")
        command, size = unpack(data, "<II", position)
        if size < 8 or size % 8 or position + size > 32 + command_bytes:
            raise ValueError("Invalid Mach-O library load command")
        if command in (0x32, 0x24):
            if minimum is not None:
                raise ValueError("Ambiguous Mach-O deployment metadata")
            if command == 0x32:
                if size < 24 or unpack(data, "<I", position + 8)[0] != 1:
                    raise ValueError("Library build target is not macOS")
                minimum = unpack(data, "<I", position + 12)[0]
                tools = unpack(data, "<I", position + 20)[0]
                if 24 + tools * 8 > size:
                    raise ValueError("Truncated Mach-O build tools")
            else:
                if size < 16:
                    raise ValueError("Invalid Mach-O minimum version")
                minimum = unpack(data, "<I", position + 8)[0]
        if command in dependency_commands | {0xD}:
            if size < 24:
                raise ValueError("Invalid Mach-O dylib command")
            name_offset = unpack(data, "<I", position + 8)[0]
            if name_offset < 24:
                raise ValueError("Invalid Mach-O dylib name offset")
            name = text_at(data, position + name_offset, position + size)
            if command == 0xD:
                if install_name is not None:
                    raise ValueError("Duplicate Mach-O library identity")
                install_name = name
            else:
                path = PurePosixPath(name)
                if ".." in path.parts or not name.startswith(("/usr/lib/", "/System/Library/Frameworks/")):
                    raise ValueError("Mach-O library requires a non-system dependency")
                dependencies.append({"command": command, "path": name})
        if command == 0x8000001C:
            raise ValueError("Mach-O library has an extra runtime search path")
        position += size
    if position != 32 + command_bytes or minimum is None or install_name is None:
        raise ValueError("Missing or inconsistent Mach-O library metadata")
    return {"format": "Mach-O64", "kind": "shared_library", "architecture": architecture,
            "binary_sha256": hashlib.sha256(data).hexdigest(),
            "deployment_target": f"{minimum >> 16}.{(minimum >> 8) & 255}.{minimum & 255}",
            "install_name": install_name, "dependencies": dependencies, "search_paths": [],
            "dependency_baseline": "macOS-system-libraries-and-frameworks"}


def inspect_bytes(data, target, inspector):
    if len(data) > MAX_LIBRARY_BYTES:
        raise ValueError("Native library exceeds the inspection byte bound")
    family, architecture = target.split("-", 1)
    if architecture not in ("x86_64", "aarch64"):
        raise ValueError("Unsupported library architecture")
    if family == "linux":
        return inspect_elf(data, architecture, inspector)
    if family == "macos":
        return inspect_macho(data, architecture)
    if family == "windows":
        result = inspector.inspect(data, target)
        pe = unpack(data, "<I", 0x3C)[0]
        characteristics = unpack(data, "<H", pe + 22)[0]
        if characteristics & 0x2002 != 0x2002:
            raise ValueError("Expected an executable PE DLL image, not a program")
        return {**result, "format": "PE32+", "kind": "shared_library",
                "dependency_baseline": "existing-executable-ordinary-and-delay-CRT-import-guard"}
    raise ValueError("Unsupported native library platform")


def inspect_library(path: Path, *, system: str, machine: str) -> dict:
    if path.stat().st_size > MAX_LIBRARY_BYTES:
        raise ValueError("Native library exceeds the inspection byte bound")
    inspector, provenance = inspect_driver.load_inspector()
    target = inspect_driver.native_platform(system, machine)
    data = path.read_bytes()
    inspection = inspect_bytes(data, target, inspector)
    return {"os": system, "architecture": machine, "native_platform": target,
            "library_path": str(path.resolve()), "library_bytes": len(data),
            "library_sha256": inspection["binary_sha256"], "library_inspection": inspection,
            "inspector": provenance, "library_loaded": False, "abi_verified": False,
            "production_operations": False}


def record(path: Path, output: Path, *, verify=False):
    receipt = inspect_library(path, system=platform.system(), machine=platform.machine())
    receipt["source"] = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=inspect_driver.ROOT, text=True).strip()
    if verify:
        if json.loads(output.read_text(encoding="utf-8")) != receipt:
            raise ValueError("Native library or inspection changed after qualification")
    else:
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    record(args.library, args.output, verify=args.verify)
