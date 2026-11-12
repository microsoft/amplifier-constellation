"""Actual enrolled Python -> native-driver service conformance, disposable CI only.

The Rust fixture creates and owns the unique sleep-only service. This script never
creates a service, changes enrollment, or substitutes another executable.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "support" / "python"))
from constellation_native import DriverError, Refused, Session  # noqa: E402


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def wait(native, active):
    deadline = time.monotonic() + 30
    while True:
        try:
            value = native.service_snapshot("body")
            if value["active"] is active and value["transitioning"] is False:
                return value
        except DriverError:
            if time.monotonic() >= deadline:
                raise
        require(time.monotonic() < deadline, "Service did not reach the observed state")
        time.sleep(0.1)


def refusal(native, action, expected, code):
    try:
        native.service_action("body", action, expected)
    except Refused as error:
        require(error.code == code, f"Unexpected refusal {error.code}: {error}")
    else:
        raise AssertionError("A stale or unavailable service action was accepted")


def main():
    require(os.environ.get("CONSTELLATION_INSTALLATION_SMOKE") == "1", "Disposable opt-in missing")
    driver, config, phase, receipt = sys.argv[1:]
    cases = []
    with Session(driver, config, timeout=30) as native, native.lock("installation"):
        require("service.snapshot" in native.describe()["operations"], "No service snapshot interface")
        before = wait(native, False)
        if phase == "cycle":
            refusal(native, "start", {"stale": True}, "stale_snapshot")
            cases.append("stale_start_refused")
            require(native.service_action("body", "start", before) == {"accepted": True}, "Start was not accepted")
            running = wait(native, True)
            require(bool(running["processes"]), "Running service has no process-generation evidence")
            for process in running["processes"]:
                require(native.process_matches(process["identity"]) is True, "Service generation is not current")
            cases.append("actual_start_and_process_generation")
            refusal(native, "stop", before, "stale_snapshot")
            require(wait(native, True)["active"] is True, "Stale stop changed the running service")
            cases.append("stale_stop_preserved_service")
            require(native.service_action("body", "stop", running) == {"accepted": True}, "Stop was not accepted")
            after = wait(native, False)
            require(before["definition"] == after["definition"], "Service cycle altered enrolled definition")
            cases.append("actual_stop_definition_preserved")
        elif phase == "definition_changed":
            expected = json.loads(Path(receipt).read_text(encoding="utf-8"))
            require(before["definition"] != expected["definition"] or before["effective_definition"] != expected["effective_definition"], "Fixture did not change the actual definition")
            refusal(native, "start", expected, "stale_snapshot")
            require(native.service_snapshot("body") == before, "Refused definition drift request changed service")
            cases.append("same_user_definition_change_refused")
        elif phase in ("disabled", "masked"):
            require(before["enabled"] is False, "Independent disable is absent")
            if phase == "masked":
                require(before["masked"] is True, "Independent mask is absent")
            refusal(native, "start", before, "disabled_or_unknown")
            require(native.service_snapshot("body") == before, "Refused start changed independent availability")
            cases.append(f"independent_{phase}_preserved")
        else:
            raise AssertionError(f"Unknown fixture phase: {phase}")
    print(json.dumps({"phase": phase, "cases": cases, "passed": len(cases)}, sort_keys=True))


if __name__ == "__main__":
    main()
