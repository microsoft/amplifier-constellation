"""Finite Windows ARM write diagnostics through the actual caller-owned ABI.

Two distinct owned processes use fresh roots: a new exact-source control build,
then the feature-instrumented build. This does not recreate the failed package.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import traceback

BASE = "8062eda22ea4fe9c333798be8397ce7fa4f6f2ee"
DEPTHS = (7, 8, 9, 10, 11, 14, 15)
SIZES = (0, 1024, 65536)
REPEATS = 3
FACADE = {
    "__init__.py": "a67847732b75eedd089a17d054698d986b34d11e5860106916bd555881b15cc0",
    "client.py": "d561744628f06a1cc90cf3955df475567ac56d408637a9bc542534d3795b1c81",
    "inprocess.py": "dcbdcbf3df9353966399226ca4eb362955acae648d07de8143549a35efc58a83",
}
ORIGINAL_HASHES = {
    "driver": "1a2e93693032fdf94f37a35e351d16cb7007606a248e4ae9b95bedd431a69619",
    "library": "2ad2c9343f15e8a3949619a2a4974c32cd5e036cc8d471d88f6a797a817a77ee",
}
ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    with Path(path).open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def git(source, *args):
    return subprocess.check_output(["git", "-C", str(source), *args], text=True).strip()


def fixture_parent():
    # Match the native fixture override, with an explicit user-profile default.
    # Runner scratch directories can have public-write ancestors. Never repair
    # the selected parent's policy; normal native enrollment still validates it.
    configured = os.environ.get("CONSTELLATION_NATIVE_TEST_ROOT")
    parent = Path(configured) if configured is not None else Path.home()
    if not parent.is_absolute():
        raise ValueError("Diagnostic fixture parent must be absolute")
    parent = parent.resolve(strict=True)
    if not parent.is_dir():
        raise ValueError("Diagnostic fixture parent must be an existing directory")
    return parent


def record_validation(result, name, context, observed, expected, passed):
    record = {
        "event": "validation",
        "check": name,
        "after_sequence": result["rows"][-1]["sequence"],
        "context": context,
        "observed": observed,
        "expected": expected,
        "passed": passed,
    }
    result.setdefault("validation", []).append(record)
    print(json.dumps(record), flush=True)
    assert passed, name


def child(spec_path):
    spec = json.loads(spec_path.read_text(encoding="utf-8"))
    facade = Path(spec["facade"])
    for name, expected in FACADE.items():
        assert sha(facade / "constellation_native" / name) == expected
    sys.path.insert(0, str(facade))
    sys.path.insert(0, str(ROOT / "tests/native_python"))
    from constellation_native import Busy, NativeSession, Refused
    from test_driver import (
        protect,
        windows_owner_sid,
        windows_policy_text,
        windows_user_sid,
    )

    assert (
        Path(sys.modules["constellation_native.inprocess"].__file__).resolve()
        == (facade / "constellation_native/inprocess.py").resolve()
    )
    home = Path(spec["root"])
    output = Path(spec["output"])
    assert home.is_dir() and not any(home.iterdir())
    result = {
        "status": "running",
        "role": spec["role"],
        "rows": [],
        "checks": {},
        "python": sys.version,
        "facade_source": BASE,
        "imported_facade_sha256": {
            name: sha(facade / "constellation_native" / name) for name in FACADE
        },
    }
    sessions = []
    try:
        protect(home)
        state = home / "state"
        state.mkdir()
        protect(state)
        native_dir = state / "native"
        native_dir.mkdir()
        protect(native_dir)
        artifacts = {}
        for key, name in (
            ("driver", "constellation-native-driver.exe"),
            ("library", "constellation_native_driver.dll"),
        ):
            destination = native_dir / name
            shutil.copyfile(spec[key], destination)
            protect(destination)
            assert sha(destination) == spec["hashes"][key]
            artifacts[key] = destination
        config = state / "native.json"
        config.write_text(
            json.dumps(
                {
                    "version": 1,
                    "roots": {"state": str(state)},
                    "locks": {"installation": str(state / "installation.lock")},
                    "services": {},
                    "commands": {},
                }
            ),
            encoding="utf-8",
        )
        protect(config)
        config_before = sha(config)
        parent_policy = windows_policy_text(home.parent)
        native = NativeSession(artifacts["library"], artifacts["driver"], config)
        sessions.append(native)
        contender = NativeSession(artifacts["library"], artifacts["driver"], config)
        sessions.append(contender)
        original_call = native._call
        last_call = {}

        def observed_call(operation, **fields):
            # Preserve the original facade and all outcomes; this outer clock
            # matches the consumer's _call observation, after bytes=list(data).
            if operation not in ("storage.write", "describe"):
                return original_call(operation, **fields)
            started = time.perf_counter_ns()
            try:
                return original_call(operation, **fields)
            finally:
                last_call.clear()
                last_call.update(
                    operation=operation, elapsed_ns=time.perf_counter_ns() - started
                )

        native._call = observed_call

        def contended():
            try:
                with contender.lock("installation"):
                    raise AssertionError(
                        "Contender entered the owner's critical section"
                    )
            except Busy as error:
                assert error.code == "lock_contended"

        def call(operation, depth, size, repetition, action, expected_refusal=False):
            sequence = len(result["rows"])
            print(
                json.dumps(
                    {
                        "event": "intent",
                        "sequence": sequence,
                        "operation": operation,
                        "depth": depth,
                        "size": size,
                        "repetition": repetition,
                    }
                ),
                flush=True,
            )
            last_call.clear()
            started = time.perf_counter_ns()
            try:
                value = action()
                elapsed = time.perf_counter_ns() - started
                assert not expected_refusal, "Existing destination was overwritten"
                outcome = "ok"
            except Refused as error:
                elapsed = time.perf_counter_ns() - started
                assert expected_refusal and error.code == "already_exists"
                value, outcome = None, "refused/already_exists"
            assert last_call["operation"] == operation
            row = {
                "sequence": sequence,
                "operation": operation,
                "relative_components": depth,
                "bytes": size,
                "repetition": repetition,
                "caller_inclusive_ns": elapsed,
                "native_session_call_ns": last_call["elapsed_ns"],
                "outside_native_session_call_ns": elapsed - last_call["elapsed_ns"],
                "outcome": outcome,
            }
            result["rows"].append(row)
            print(json.dumps({"event": "returned", **row}), flush=True)
            return value

        files = []
        with native.lock("installation"):
            contended()
            native.private_dir("state", "stages")
            # Equal-length arm roots and a fixed UUID-shaped staging component.
            stage = "stages/00000000-0000-4000-8000-000000000001"
            native.private_dir("state", stage)
            stage += "/package"
            native.private_dir("state", stage)
            for depth in DEPTHS:
                parent = stage
                for part in range(depth - 4):
                    parent += f"/d{depth:02d}-{part:02d}"
                    native.private_dir("state", parent)
                first = None
                for size in SIZES:
                    for repetition in range(REPEATS):
                        relative = parent + f"/payload-{size}-{repetition}.bin"
                        assert len(Path(relative).parts) == depth
                        data = bytes([size // 1024 % 251]) * size
                        value = call(
                            "storage.write",
                            depth,
                            size,
                            repetition,
                            lambda: native.private_write(
                                "state", relative, data, create_only=True
                            ),
                        )
                        context = {
                            "relative_path": relative,
                            "depth": depth,
                            "bytes": size,
                            "repetition": repetition,
                        }
                        record_validation(
                            result, "write_result", context, value, {}, value == {}
                        )
                        description = call(
                            "describe", 0, 0, repetition, native.describe
                        )
                        record_validation(
                            result,
                            "describe_contract",
                            context,
                            description,
                            {"protocol": 1, "required_operation": "storage.write"},
                            description["protocol"] == 1
                            and "storage.write" in description["operations"],
                        )
                        path = state / relative
                        content = path.read_bytes()
                        record_validation(
                            result,
                            "exact_content",
                            context,
                            {
                                "bytes": len(content),
                                "sha256": hashlib.sha256(content).hexdigest(),
                            },
                            {
                                "bytes": len(data),
                                "sha256": hashlib.sha256(data).hexdigest(),
                            },
                            content == data,
                        )
                        owner_sid, expected_sid = (
                            windows_owner_sid(path),
                            windows_user_sid(),
                        )
                        record_validation(
                            result,
                            "owner_sid",
                            context,
                            owner_sid,
                            expected_sid,
                            owner_sid == expected_sid,
                        )
                        policy = windows_policy_text(path)
                        expected_policy = (
                            f"D:P(A;;FA;;;SY)(A;;FA;;;{windows_user_sid()})"
                        )
                        record_validation(
                            result,
                            "exact_dacl_text",
                            context,
                            policy,
                            expected_policy,
                            policy == expected_policy,
                        )
                        files.append((relative, data, path.stat().st_ino))
                        if first is None:
                            first = (relative, data, path.stat().st_ino)
                relative, data, inode = first
                call(
                    "storage.write",
                    depth,
                    10,
                    0,
                    lambda: native.private_write(
                        "state", relative, b"no-replace", create_only=True
                    ),
                    True,
                )
                assert (state / relative).read_bytes() == data and (
                    state / relative
                ).stat().st_ino == inode
                contended()
            assert len(files) == 63
            assert sha(config) == config_before
            for key, path in artifacts.items():
                assert sha(path) == spec["hashes"][key]
            assert windows_policy_text(home.parent) == parent_policy
            for relative, data, inode in files:
                path = state / relative
                assert path.read_bytes() == data and path.stat().st_ino == inode
                native.durable_remove("state", relative)
                assert not path.exists()
            contended()
        with contender.lock("installation"):
            pass
        result["checks"] = {
            "creates": 63,
            "describes": 63,
            "duplicate_refusals": 7,
            "exact_content_and_owner": True,
            "original_identity_preserved_on_refusal": True,
            "whole_action_lock_contended": True,
            "lock_available_after_release": True,
            "config_and_artifacts_unchanged": True,
            "parent_policy_unchanged": True,
            "payloads_removed_by_native_owner": True,
        }
        result["path_metrics"] = {
            "state_absolute_components": len(state.parts),
            "state_characters": len(str(state)),
            "relative_components": list(DEPTHS),
        }
        for name, expected in FACADE.items():
            assert sha(facade / "constellation_native" / name) == expected
        result["status"] = "passed"
    except BaseException as error:
        result.update(
            status="failed_or_unknown",
            error={
                "type": type(error).__name__,
                "message": str(error),
                "traceback": traceback.format_exc(),
            },
        )
    finally:
        errors = []
        for session in reversed(sessions):
            try:
                session.close()
            except BaseException as error:
                errors.append(type(error).__name__ + ": " + str(error))
        result["close_errors"] = errors
        if errors:
            result["status"] = "failed_or_unknown"
        save(output / "child-result.json", result)
    return 0 if result["status"] == "passed" else 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--child", type=Path)
    parser.add_argument("--control-source", type=Path)
    parser.add_argument("--control-driver", type=Path)
    parser.add_argument("--control-library", type=Path)
    parser.add_argument("--instrumented-driver", type=Path)
    parser.add_argument("--instrumented-library", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    assert os.name == "nt" and platform.machine().lower() in ("arm64", "aarch64")
    if args.child:
        return child(args.child)
    source = args.control_source.resolve(strict=True)
    assert git(source, "rev-parse", "HEAD") == BASE
    assert not git(source, "status", "--porcelain", "--untracked-files=no")
    assert not git(ROOT, "status", "--porcelain", "--untracked-files=no")
    for name, expected in FACADE.items():
        assert sha(source / "support/python/constellation_native" / name) == expected
    output = args.output.resolve()
    output.mkdir(exist_ok=False)
    result = {
        "status": "running",
        "schema": 1,
        "source": git(ROOT, "rev-parse", "HEAD"),
        "control_source": BASE,
        "fixture_sha256": sha(Path(__file__)),
        "original_failed_artifact_hashes": ORIGINAL_HASHES,
        "original_artifacts_available": False,
        "arms": [],
        "scope": "Fresh uninstrumented control versus distinct timed build. Finite NativeSession workload only; not the original failed binaries or full package stage.",
    }
    parent = fixture_parent()
    result["fixture_parent"] = str(parent)
    temporary = Path(tempfile.mkdtemp(prefix="native-write-", dir=parent)).resolve()
    marker = temporary / "diagnostic-owner.json"
    owner = {"output": str(output), "pid": os.getpid(), "parent": str(parent)}
    save(marker, owner)
    owned_live = False
    try:
        from test_driver import protect

        protect(temporary)
        for role in ("control", "timings"):
            arm = output / role
            arm.mkdir()
            home = temporary / role
            home.mkdir()
            driver = (
                args.control_driver if role == "control" else args.instrumented_driver
            ).resolve(strict=True)
            library = (
                args.control_library if role == "control" else args.instrumented_library
            ).resolve(strict=True)
            hashes = {"driver": sha(driver), "library": sha(library)}
            spec = {
                "role": role,
                "root": str(home),
                "output": str(arm),
                "driver": str(driver),
                "library": str(library),
                "hashes": hashes,
                "facade": str(source / "support/python"),
            }
            save(arm / "inputs.json", spec)
            record = {
                "role": role,
                "source": BASE if role == "control" else result["source"],
                "hashes": hashes,
                "equals_original_failed_hashes": hashes == ORIGINAL_HASHES,
                "feature": None if role == "control" else "native-write-diagnostic",
            }
            result["arms"].append(record)
            with (
                (arm / "caller.jsonl").open("x") as out,
                (arm / "native-timings.jsonl").open("x") as err,
            ):
                process = subprocess.Popen(
                    [
                        sys.executable,
                        "-B",
                        str(Path(__file__).resolve()),
                        "--child",
                        str(arm / "inputs.json"),
                    ],
                    stdin=subprocess.DEVNULL,
                    stdout=out,
                    stderr=err,
                )
                owned_live = True
                try:
                    record["exit_code"] = process.wait(timeout=180)
                except subprocess.TimeoutExpired:
                    record["timeout_unknown"] = True
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=10)
                    raise RuntimeError(
                        "Owned diagnostic child exceeded its bound; no work is retried"
                    )
                finally:
                    owned_live = process.poll() is None
                    record["child_exited"] = not owned_live
            assert record["exit_code"] == 0
            child_result = json.loads((arm / "child-result.json").read_text())
            assert (
                child_result["status"] == "passed" and len(child_result["rows"]) == 133
            )
            timings = [
                json.loads(line)
                for line in (arm / "native-timings.jsonl").read_text().splitlines()
            ]
            assert len(timings) == (0 if role == "control" else 133)
            if role == "timings":
                for row, timing in zip(child_result["rows"], timings):
                    assert timing["native_write_diagnostic"] == 1
                    for field in ("operation", "relative_components", "bytes"):
                        assert timing[field] == row[field]
                    if row["operation"] == "storage.write" and row["outcome"] == "ok":
                        assert timing["phases"]["storage.publish"]["calls"] == 1
                    top_level = (
                        "driver.unchanged",
                        "enrollment.unchanged",
                        "enrollment.storage_path",
                        "storage.prepare",
                        "storage.create_private",
                        "storage.write_all",
                        "storage.sync_all",
                        "storage.close",
                        "storage.publish",
                    )
                    phase_sum = sum(
                        timing["phases"].get(name, {}).get("inclusive_ns", 0)
                        for name in top_level
                    )
                    row["rust_top_level_phase_sum_ns"] = phase_sum
                    row["rust_dispatch_residual_ns"] = (
                        timing["dispatch_inclusive_ns"] - phase_sum
                    )
                    row["native_call_outside_dispatch_ns"] = (
                        row["native_session_call_ns"] - timing["dispatch_inclusive_ns"]
                    )
                    # path.chain/windows_access are nested observations and are
                    # deliberately excluded from the nonoverlapping phase sum.
                save(
                    arm / "correlated-timings.json",
                    {
                        "rows": child_result["rows"],
                        "residual_scope": "Native call residual includes Python JSON, ABI framing/decoding and post-dispatch stderr logging. Rust residual includes untimed work and timer bookkeeping; nested phases are not additive.",
                    },
                )
            groups = {}
            for row in child_result["rows"]:
                key = f"{row['operation']}:{row['relative_components']}:{row['bytes']}:{row['outcome']}"
                groups.setdefault(key, []).append(row["caller_inclusive_ns"])
            record["caller_summary_ns"] = {
                key: {
                    "count": len(values),
                    "min": min(values),
                    "median": statistics.median(values),
                    "max": max(values),
                }
                for key, values in groups.items()
            }
            record["result_sha256"] = sha(arm / "child-result.json")
            record["caller_log_sha256"] = sha(arm / "caller.jsonl")
            record["native_timings_sha256"] = sha(arm / "native-timings.jsonl")
            assert sha(driver) == hashes["driver"] and sha(library) == hashes["library"]
        assert not git(source, "status", "--porcelain", "--untracked-files=no")
        assert not git(ROOT, "status", "--porcelain", "--untracked-files=no")
        result["tracked_source_unchanged_after"] = True
        result["status"] = "passed_bounded_diagnostic"
    except BaseException as error:
        result.update(
            status="failed_or_unknown",
            error={
                "type": type(error).__name__,
                "message": str(error),
                "traceback": traceback.format_exc(),
            },
        )
    finally:
        cleanup = {"owned_child_live": owned_live, "removed": False}
        try:
            assert not owned_live
            assert json.loads(marker.read_text()) == owner
            assert temporary.parent == parent
            shutil.rmtree(temporary)
            cleanup["removed"] = not temporary.exists()
        except BaseException as error:
            cleanup["error"] = type(error).__name__ + ": " + str(error)
            result["status"] = "failed_or_unknown"
        result["cleanup"] = cleanup
        save(output / "completion.json", result)
    return 0 if result["status"] == "passed_bounded_diagnostic" else 1


if __name__ == "__main__":
    raise SystemExit(main())
