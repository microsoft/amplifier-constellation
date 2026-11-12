"""CI-only inspection of the exact tested driver; never execute the binary."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[2]
VENDOR = Path(__file__).resolve().parent / "vendor" / "native-binary"


def load_inspector():
    provenance = json.loads((VENDOR / "vendor.json").read_text(encoding="utf-8"))
    source = VENDOR / "native_binary.py"
    if hashlib.sha256(source.read_bytes()).hexdigest() != provenance["sha256"]:
        raise ValueError("Vendored native-binary inspector hash mismatch")
    spec = importlib.util.spec_from_file_location("driver_native_binary", source)
    inspector = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(inspector)
    return inspector, provenance


def native_platform(system: str, machine: str) -> str:
    family = {"Windows": "windows", "Linux": "linux", "Darwin": "macos"}
    architecture = {"amd64": "x86_64", "x86_64": "x86_64",
                    "arm64": "aarch64", "aarch64": "aarch64"}
    try:
        return family[system] + "-" + architecture[machine.lower()]
    except KeyError as error:
        raise ValueError(f"Unsupported native host: {system}/{machine}") from error


def inspect_driver(path: Path, *, system: str, machine: str) -> dict:
    inspector, provenance = load_inspector()
    data = path.read_bytes()
    target = native_platform(system, machine)
    inspection = inspector.inspect(data, target)
    return {
        "os": system,
        "architecture": machine,
        "native_platform": target,
        "driver_path": str(path.resolve()),
        "driver_bytes": len(data),
        "driver_sha256": inspection["binary_sha256"],
        "driver_inspection": inspection,
        "inspector": provenance,
        "production_operations": False,
    }


def record(path: Path, output: Path, *, verify: bool = False) -> None:
    receipt = inspect_driver(path, system=platform.system(), machine=platform.machine())
    receipt["source"] = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    if verify:
        if json.loads(output.read_text(encoding="utf-8")) != receipt:
            raise ValueError("Native driver or inspection changed after qualification")
    else:
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--driver", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    record(args.driver, args.output, verify=args.verify)
