"""Thin, synchronous binding for the native driver's fixed JSONL interface.

The domain transaction journal remains with its caller. In particular, an
uncertain launch or service request is never retried by this module.
"""

from __future__ import annotations

import contextlib
import json
import math
import os
from pathlib import Path
import queue
import subprocess
import threading
import time
from typing import Any, Iterator, Mapping
import uuid
import weakref


class DriverError(RuntimeError):
    """An observed driver error, or a failed read-only observation."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


class Busy(DriverError):
    """The existing kernel lock is held by another invocation."""


class Refused(DriverError):
    """The driver confirmed refusal before dispatching the requested effect."""


class Unknown(DriverError):
    """An effect may have happened. Reconcile the domain's durable operation."""


_OUTCOMES = {"ok", "busy", "refused", "unknown", "error"}
_MAX_FRAME = 16 * 1024 * 1024
_SESSIONS: weakref.WeakSet[Session] = weakref.WeakSet()


def _after_fork() -> None:
    # Keep a forked child from extending its parent's invocation/lock lifetime.
    for session in list(_SESSIONS):
        for stream in (session._process.stdin, session._process.stdout):
            if stream is not None:
                with contextlib.suppress(OSError):
                    stream.close()
        session._closed = True


if hasattr(os, "register_at_fork"):
    os.register_at_fork(after_in_child=_after_fork)


def _read_responses(stream: Any, responses: queue.Queue[Any]) -> None:
    """Read with a bound without blocking the caller's operation deadline."""

    def deliver(value: Any) -> bool:
        try:
            responses.put(value, timeout=1)
            return True
        except queue.Full:
            # An unsolicited-response flood cannot grow Python memory forever.
            return False

    try:
        while True:
            raw = stream.readline(_MAX_FRAME + 1)
            if not raw:
                deliver(EOFError("Native driver closed its response stream"))
                return
            if len(raw) > _MAX_FRAME or not raw.endswith(b"\n"):
                deliver(ValueError("Native driver response exceeds its framing limit"))
                return
            if not deliver(raw):
                return
    except (OSError, ValueError) as error:
        deliver(error)


class Session:
    """One local driver invocation, retaining locks until released or closed.

    Both executable and configuration paths come from installation-owned setup.
    Requests select names in that protected configuration; they do not carry
    arbitrary host commands. This object does not reconnect after a lost reply.
    """

    def __init__(
        self,
        driver_path: os.PathLike[str] | str,
        config_path: os.PathLike[str] | str,
        *,
        timeout: float = 30.0,
    ):
        if not isinstance(timeout, (int, float)) or isinstance(timeout, bool):
            raise ValueError("Native driver timeout must be a positive finite number")
        if not math.isfinite(timeout) or timeout <= 0:
            raise ValueError("Native driver timeout must be a positive finite number")
        driver = Path(driver_path)
        config = Path(config_path)
        if not driver.is_absolute() or not config.is_absolute():
            raise ValueError(
                "Native driver and owner configuration paths must be absolute"
            )
        self.timeout = float(timeout)
        self._mutex = threading.RLock()
        self._responses: queue.Queue[Any] = queue.Queue(maxsize=2)
        self._serial = 0
        self._prefix = uuid.uuid4().hex
        self._pid = os.getpid()
        self._closed = False
        self._locks: set[str] = set()
        try:
            self._process = subprocess.Popen(
                [str(driver), "--config", str(config), "serve"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                close_fds=True,
                bufsize=0,
            )
        except OSError as error:
            raise DriverError(
                "driver_start", "Unable to start the enrolled native driver"
            ) from error
        self._reader = threading.Thread(
            target=_read_responses,
            args=(self._process.stdout, self._responses),
            name="constellation-native-response",
            daemon=True,
        )
        self._reader.start()
        _SESSIONS.add(self)

    def __enter__(self) -> Session:
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def _ensure_current(self) -> None:
        if os.getpid() != self._pid:
            raise DriverError(
                "forked_session", "Create a new native Session after a process fork"
            )
        if self._closed:
            raise DriverError(
                "closed_session", "Native Session is closed; requests are not replayed"
            )

    def _stop(self, *, failed: bool = False) -> None:
        """Close only this invocation; detached enrolled work owns its lifetime."""
        if self._closed:
            return
        self._closed = True
        self._locks.clear()
        process = self._process
        if failed and process.poll() is None:
            with contextlib.suppress(OSError):
                process.kill()
        if process.stdin is not None:
            with contextlib.suppress(OSError):
                process.stdin.close()
        try:
            process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            with contextlib.suppress(OSError):
                process.kill()
            with contextlib.suppress(subprocess.TimeoutExpired):
                process.wait(timeout=4)
        self._reader.join(timeout=1)
        if not self._reader.is_alive() and process.stdout is not None:
            process.stdout.close()

    def close(self) -> None:
        """Release invocation-owned locks. Never stop separately launched probes."""
        if os.getpid() != self._pid:
            # A forked copy must not close or signal its parent's driver.
            return
        with self._mutex:
            self._stop()

    def _lost(self, code: str, message: str, mutating: bool) -> DriverError:
        self._stop(failed=True)
        error_type = Unknown if mutating else DriverError
        return error_type(code, message)

    def _call(self, op: str, *, mutating: bool = False, **fields: Any) -> Any:
        with self._mutex:
            self._ensure_current()
            self._serial += 1
            request_id = f"{self._prefix}-{self._serial}"
            encoded = (
                json.dumps(
                    {"version": 1, "id": request_id, "op": op, **fields},
                    allow_nan=False,
                    separators=(",", ":"),
                ).encode("utf-8")
                + b"\n"
            )
            if len(encoded) > _MAX_FRAME:
                raise ValueError("Native driver request exceeds its framing limit")
            deadline = time.monotonic() + self.timeout
            written: queue.Queue[Any] = queue.Queue()

            def send() -> None:
                try:
                    assert self._process.stdin is not None
                    remaining = memoryview(encoded)
                    while remaining:
                        count = self._process.stdin.write(remaining)
                        if not count:
                            raise BrokenPipeError("Native driver request pipe closed")
                        remaining = remaining[count:]
                    written.put(None)
                except (OSError, ValueError) as error:
                    written.put(error)

            writer = threading.Thread(
                target=send, name="constellation-native-request", daemon=True
            )
            writer.start()
            try:
                error = written.get(timeout=max(0, deadline - time.monotonic()))
            except queue.Empty as error:
                raise self._lost(
                    "transport_timeout",
                    "Native driver did not read this request within its deadline",
                    mutating,
                ) from error
            if isinstance(error, Exception):
                raise self._lost(
                    "transport_write",
                    "Native driver request outcome is unavailable",
                    mutating,
                ) from error
            try:
                raw = self._responses.get(timeout=max(0, deadline - time.monotonic()))
            except queue.Empty as error:
                raise self._lost(
                    "transport_timeout",
                    "Native driver did not acknowledge this request within its deadline",
                    mutating,
                ) from error
            if isinstance(raw, Exception):
                raise self._lost(
                    "transport_closed", "Native driver response was lost", mutating
                ) from raw
            try:
                response = json.loads(raw)
                if (
                    not isinstance(response, dict)
                    or type(response.get("version")) is not int
                    or response["version"] != 1
                    or response.get("id") != request_id
                    or response.get("outcome") not in _OUTCOMES
                ):
                    raise ValueError("Invalid native driver response envelope")
                if response["outcome"] == "ok":
                    return response.get("result")
                detail = response.get("error")
                if (
                    not isinstance(detail, dict)
                    or not isinstance(detail.get("code"), str)
                    or not isinstance(detail.get("message"), str)
                ):
                    raise ValueError("Invalid native driver error envelope")
            except (ValueError, TypeError, UnicodeError) as error:
                raise self._lost(
                    "protocol_response",
                    "Native driver response could not be correlated",
                    mutating,
                ) from error
            error_type = {
                "busy": Busy,
                "refused": Refused,
                "unknown": Unknown,
                "error": DriverError,
            }[response["outcome"]]
            raise error_type(detail["code"], detail["message"])

    def describe(self) -> Any:
        return self._call("describe")

    @contextlib.contextmanager
    def lock(self, name: str) -> Iterator[None]:
        """Hold the enrolled lock across the whole caller critical section."""
        with self._mutex:
            if name in self._locks:
                raise DriverError(
                    "lock_reentry", "This Session already holds the named lock"
                )
            self._call("lock.acquire", mutating=True, name=name)
            self._locks.add(name)
        try:
            yield
        finally:
            with self._mutex:
                if name in self._locks:
                    self._call("lock.release", mutating=True, name=name)
                    self._locks.remove(name)

    def process_identity(self, pid: int) -> Any:
        return self._call("process.identity", pid=pid)

    def process_matches(self, identity: Mapping[str, Any]) -> bool:
        result = self._call("process.matches", identity=identity)
        if not isinstance(result, dict) or type(result.get("matches")) is not bool:
            raise self._lost(
                "protocol_result", "Native process match result is not a boolean", False
            )
        return result["matches"]

    def service_snapshot(self, name: str) -> Any:
        return self._call("service.snapshot", name=name)

    def service_action(
        self, name: str, action: str, expected: Mapping[str, Any]
    ) -> Any:
        return self._call(
            "service.action", mutating=True, name=name, action=action, expected=expected
        )

    def private_write(
        self, root: str, path: str, data: bytes, *, create_only: bool = False
    ) -> Any:
        if not isinstance(data, bytes):
            raise TypeError("private_write expects already serialized bytes")
        return self._call(
            "storage.write",
            mutating=True,
            root=root,
            path=path,
            bytes=list(data),
            create_only=create_only,
        )

    def durable_remove(self, root: str, path: str) -> Any:
        return self._call("storage.remove", mutating=True, root=root, path=path)

    def launch(self, name: str, *, result: str | None = None) -> Any:
        fields: dict[str, Any] = {"name": name}
        if result is not None:
            fields["result"] = result
        return self._call("process.launch", mutating=True, **fields)

    def process_status(self, identity: Mapping[str, Any]) -> Any:
        return self._call("process.status", identity=identity)
