"""Disposable caller/adapter for the real source-pinned Instance worker.

The domain adapter intentionally performs only a lock-protected local write.
It is not Minecraft, a service installation, or a simulated update success.
"""
from __future__ import annotations

import argparse
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

FIXTURES = Path(__file__).resolve().parent
sys.path.insert(0, str(FIXTURES.parents[2] / "support" / "python"))


def worker_module(source: Path):
    pin = json.loads((FIXTURES / "worker_source.json").read_text())
    path = source / pin["path"]
    if hashlib.sha256(path.read_bytes()).hexdigest() != pin["sha256"]:
        raise ValueError("Instance worker source does not match the immutable pin")
    spec = importlib.util.spec_from_file_location("actual_instance_worker", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def save(path: str, value: dict):
    # Test observations live outside the domain journal; never overwrite them.
    with Path(path).open("x", encoding="utf-8") as stream:
        json.dump(value, stream)
        stream.flush()
        os.fsync(stream.fileno())


def native(plan):
    from constellation_native import NativeSession
    return NativeSession(plan["library"], plan["driver"], plan["native_config"])


def until_release(path: str):
    deadline = time.monotonic() + 12
    while not Path(path).exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("Disposable fixture release was not received")
        time.sleep(0.02)


def worker_busy(worker, plan):
    try:
        with worker.single_worker(Path(plan["worker_root"])):
            return False
    except OSError as exc:
        if exc.errno not in (errno.EACCES, errno.EAGAIN):
            raise
        return True


def domain(plan, role):
    from constellation_native import Busy
    worker = worker_module(Path(plan["instance_source"]))
    if not worker_busy(worker, plan):
        raise AssertionError("Worker must own its lock before domain dispatch")
    if role == "provider":
        assert Path(os.environ["CONSTELLATION_UPDATE_PLAN"]).read_bytes() == b"{}"
        assert os.environ["CONSTELLATION_UPDATE_TRANSACTION"] == plan["transaction"]
    elif role == "participant":
        assert Path(os.environ["CONSTELLATION_UPDATE_PLAN"]).read_bytes() == b"{}"
        assert os.environ["CONSTELLATION_UPDATE_OUTCOME"] == "unchanged"
        assert os.environ["CONSTELLATION_UPDATE_TRANSACTION"] == plan["transaction"]
    elif role == "connection":
        incoming = json.loads(sys.stdin.buffer.read())
        assert incoming == {"schema": 1, "connection_id": "fixture", "revision": "fixed",
                            "action": "status", "request_id": plan["transaction"]}
    else:
        raise AssertionError(role)
    before = Path(plan["journal"]).read_bytes()
    try:
        with native(plan) as session:
            with session.lock("installation"):
                save(plan["entered"], {"role": role, "pid": os.getpid(),
                                      "worker_lock_held_before_domain": True})
                until_release(plan["release"])
                # This is direct caller-side work while NativeSession owns the
                # actual kernel lock, not a helper's storage command.
                Path(plan["journal"]).write_bytes(before + b"\nfixture " + role.encode())
    except Busy:
        assert Path(plan["journal"]).read_bytes() == before
        save(plan["refused"], {"status": "busy", "role": role,
                              "worker_lock_held_before_domain": True})
        return 4
    if role == "participant":
        reply = {"ready": True}
    elif role == "connection":
        reply = {"schema": 1, "connection_id": "fixture", "state": "connected",
                 "request_id": plan["transaction"], "settled": True,
                 "message": "Disposable lock fixture", "context_ready": True}
    else:
        reply = {"fixture": True}
    print(json.dumps(reply), flush=True)
    return 0


def dispatch(worker, plan, plan_path: Path, role: str):
    # Only the actual methods below are under test. Avoid constructing a real
    # network/TLS-enabled Worker or claiming a complete update transaction.
    caller = worker.Worker.__new__(worker.Worker)
    caller.state = {"transaction": {"id": plan["transaction"]}}
    argv = [sys.executable, str(Path(__file__).resolve()), "domain", str(plan_path), "--role", role]
    directory = Path(plan["envelopes"])
    if role == "provider":
        component = {"id": "fixture", "protocol": 2, "commands": {"status": argv}}
        return json.loads(caller.provider_call(component, "status", directory))
    if role == "participant":
        file = directory / "fixture.json"
        context = {"id": plan["transaction"], "directory": str(directory),
                   "contexts": {"fixture": worker.digest(file)}}
        return caller.participant_call({"id": "fixture", "commands": {"prepare": argv}},
                                       "prepare", context, "unchanged")
    entry = {"id": "fixture", "revision": "fixed", "commands": {"status": argv}}
    return worker.connection_call(entry, "status", plan["transaction"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("worker-probe", "worker-hold", "worker-dispatch", "domain"))
    parser.add_argument("plan", type=Path)
    parser.add_argument("--role", choices=("provider", "participant", "connection"))
    args = parser.parse_args()
    plan = json.loads(args.plan.read_text())
    worker = worker_module(Path(plan["instance_source"]))
    if args.mode == "domain":
        return domain(plan, args.role)
    try:
        with worker.single_worker(Path(plan["worker_root"])):
            if args.mode == "worker-hold":
                save(plan["entered"], {"status": "worker_held", "pid": os.getpid()})
                until_release(plan["release"])
            elif args.mode == "worker-dispatch":
                try:
                    result = dispatch(worker, plan, args.plan, args.role)
                except (RuntimeError, ValueError):
                    # The real worker maps a nonzero adapter exit to failure.
                    # Only an independently persisted Busy observation explains
                    # this specific expected test refusal.
                    refusal = json.loads(Path(plan["refused"]).read_text())
                    assert refusal == {"status": "busy", "role": args.role,
                                       "worker_lock_held_before_domain": True}
                    print(json.dumps({"status": "domain_busy", "role": args.role}), flush=True)
                    return 0
                print(json.dumps({"status": "dispatched", "role": args.role, "result": result}), flush=True)
                return 0
    except OSError as exc:
        if (args.mode not in ("worker-probe", "worker-dispatch")
                or exc.errno not in (errno.EACCES, errno.EAGAIN)):
            raise
        print(json.dumps({"status": "worker_busy", "errno": exc.errno}), flush=True)
        return 0
    print(json.dumps({"status": "worker_acquired"}), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
