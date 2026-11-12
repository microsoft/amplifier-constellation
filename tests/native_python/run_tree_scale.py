"""Separate actual-driver removal qualification; never part of unittest discovery."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import subprocess
import sys
import tempfile
import time
import traceback

from test_tree import TreeFixture, directory_link, remove_directory_link
from constellation_native import DriverError, Unknown

ROOT = Path(__file__).resolve().parents[2]
DIRECTORIES = 12
LINKS = 1064
FILES = 60000 - DIRECTORIES - LINKS
FILE_BYTES = b"regenerable package fixture\n".ljust(1024, b".")


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory(root):
    """Independent no-follow count, including Windows reparse directory leaves."""
    counts = {"files": 0, "directories": 0, "links": 0, "bytes": 0, "max_depth": 0}
    stack = [(root, 0)]
    while stack:
        parent, depth = stack.pop()
        with os.scandir(parent) as entries:
            for entry in entries:
                info = entry.stat(follow_symlinks=False)
                if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
                    counts["links"] += 1
                elif stat.S_ISDIR(info.st_mode):
                    counts["directories"] += 1
                    counts["max_depth"] = max(counts["max_depth"], depth + 1)
                    stack.append((Path(entry.path), depth + 1))
                elif stat.S_ISREG(info.st_mode):
                    counts["files"] += 1
                    counts["bytes"] += info.st_size
                else:
                    raise AssertionError("Unexpected object in scale fixture")
    counts["entries"] = counts["files"] + counts["directories"] + counts["links"]
    return counts


def run(output):
    record = {
        "schema": 1, "outcome": "error", "production_operations": False,
        "system": platform.system(), "machine": platform.machine(),
        "python_version": platform.python_version(), "python_executable": sys.executable,
        "python_sha256": sha256(sys.executable), "attempted_removals": 0,
        "qualification": "actual driver, default Session deadline; one representative disposable tree",
        "limits": ["Not a universal duration bound or Minecraft full-lifecycle proof",
                   "Cooperating protected-root writers and stable mount topology",
                   "Fixture cleanup after failure is separate from driver settlement"],
    }
    driver = None
    try:
        driver = Path(os.environ["CONSTELLATION_NATIVE_DRIVER"]).resolve(strict=True)
        record.update({
            "source": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()),
            "driver_path": str(driver), "driver_sha256": sha256(driver),
            "test_sha256": sha256(__file__),
            "fixture_sha256": sha256(Path(__file__).with_name("test_tree.py")),
            "python_facade_sha256": sha256(ROOT / "support/python/constellation_native/client.py"),
        })
        with tempfile.TemporaryDirectory(prefix="native-tree-scale-", dir=os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")) as temporary:
            base = Path(temporary).resolve()
            fixture = TreeFixture(base, str(driver), long_path=os.name == "nt")
            record["enrollment_sha256"] = sha256(fixture.config)
            root = fixture.entry()
            links = []
            try:
                started = time.monotonic()
                directories = []
                parent = root
                for index in range(DIRECTORIES):
                    parent = parent / f"package-{index:02d}"
                    parent.mkdir(mode=0o700)
                    directories.append(parent)
                for index in range(FILES):
                    target = directories[index % DIRECTORIES] / f"file-{index:05d}.bin"
                    fd = os.open(target, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
                    with os.fdopen(fd, "wb") as stream:
                        stream.write(FILE_BYTES)
                link_kind = None
                for index in range(LINKS):
                    link = directories[index % DIRECTORIES] / f"outside-{index:04d}"
                    link_kind = directory_link(link, fixture.outside)
                    links.append(link)
                record["creation_seconds"] = time.monotonic() - started
                record["inventory"] = inventory(root)
                record["link_kind"] = link_kind
                record["selected_path_utf16_units"] = len(str(root).encode("utf-16-le")) // 2
                assert record["inventory"] == {
                    "files": FILES, "directories": DIRECTORIES, "links": LINKS,
                    "bytes": FILES * len(FILE_BYTES), "max_depth": 12, "entries": 60000,
                }, record["inventory"]
                # Verify the constructed link really reaches outside before
                # asking the native no-follow walker to remove its leaf entry.
                assert (links[-1] / "retained.txt").read_bytes() == fixture.sentinel_bytes
                with fixture.session() as session, session.lock("installation"):
                    record["session_deadline_seconds"] = session.timeout
                    record["capabilities"] = session.describe()
                    assert record["capabilities"]["regenerable_tree_removal"] == 1
                    started = time.monotonic()
                    expected = session.tree_inspect("stages", "candidate")
                    record["inspect_seconds"] = time.monotonic() - started
                    assert expected is not None
                    record["expected_snapshot"] = expected
                    record["attempted_removals"] += 1
                    started = time.monotonic()
                    try:
                        record["result"] = session.tree_remove("stages", "candidate", expected)
                    finally:
                        record["remove_seconds"] = time.monotonic() - started
                    assert record["result"] == {"absent": True}
                    assert session.tree_inspect("stages", "candidate") is None
                fixture.assert_outside()
                record["outside_sentinel_preserved"] = True
                record["observed_absent"] = not os.path.lexists(root)
                assert record["observed_absent"]
                record["within_default_deadline"] = record["remove_seconds"] < record["session_deadline_seconds"]
                record["outcome"] = "passed"
            except DriverError as error:
                record["driver_error"] = {"type": type(error).__name__, "code": error.code, "message": str(error)}
                record["outcome"] = "unknown" if isinstance(error, Unknown) else "failed"
                # The failed critical section has unwound. A fresh invocation
                # only observes; it never dispatches another removal.
                with fixture.session() as recovery, recovery.lock("installation"):
                    record["reconciliation_snapshot"] = recovery.tree_inspect("stages", "candidate")
                fixture.assert_outside()
                record["outside_sentinel_preserved"] = True
                raise
            finally:
                # Remove fixture links as leaves before TemporaryDirectory's
                # cleanup if the actual driver left any tree behind.
                for link in reversed(links):
                    remove_directory_link(link)
    except BaseException as error:
        record["error"] = {"type": type(error).__name__, "message": str(error), "traceback": traceback.format_exc()}
    finally:
        if driver is not None:
            record["driver_sha256_after"] = sha256(driver)
            if record["driver_sha256_after"] != record.get("driver_sha256"):
                record["outcome"] = "error"
                record["artifact_changed"] = True
        output.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(json.dumps({"outcome": record["outcome"], "entries": record.get("inventory", {}).get("entries"),
                          "remove_seconds": record.get("remove_seconds"), "result": str(output)}))
    return 0 if record["outcome"] == "passed" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    raise SystemExit(run(parser.parse_args().output.resolve()))
