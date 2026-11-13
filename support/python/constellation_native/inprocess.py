"""Synchronous native operations whose kernel locks belong to the Python process.

The installation supplies a verified private library and matching fixed driver.
This module does not download them, create a supervisor, or implement transaction
recovery. ABI 1 has no independently interruptible transport timeout.
"""

from __future__ import annotations

import contextlib
import ctypes
import json
import os
from pathlib import Path
import threading
from typing import Any, Iterator, Mapping
import uuid

from .client import Busy, DriverError, Refused, Session, Unknown

_MAX_REQUEST = 16 * 1024 * 1024
_MAX_RESPONSE = 32 * 1024 * 1024
_OUTCOMES = {"ok", "busy", "refused", "unknown", "error"}
_POISON_CODES = {"native_panic", "abi_panic", "session_poisoned"}


class _Buffer(ctypes.Structure):
    _fields_ = [("data", ctypes.c_void_p), ("len", ctypes.c_size_t)]


class _Library:
    def __init__(self, dll: ctypes.CDLL):
        self.dll = dll
        self.version = dll.constellation_native_abi_version
        self.version.argtypes = []
        self.version.restype = ctypes.c_uint32
        if self.version() != 1:
            raise Refused("abi_version", "Unsupported native session ABI")
        self.open = dll.constellation_native_open
        self.open.argtypes = [
            ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_uint64),
            ctypes.POINTER(_Buffer),
        ]
        self.open.restype = ctypes.c_int32
        self.request = dll.constellation_native_request
        self.request.argtypes = [
            ctypes.c_uint64, ctypes.c_void_p, ctypes.c_size_t,
            ctypes.POINTER(_Buffer),
        ]
        self.request.restype = ctypes.c_int32
        self.close = dll.constellation_native_close
        self.close.argtypes = [ctypes.c_uint64, ctypes.POINTER(_Buffer)]
        self.close.restype = ctypes.c_int32
        self.free = dll.constellation_native_buffer_free
        self.free.argtypes = [_Buffer]
        self.free.restype = None
        for name in ("prepare", "parent", "child"):
            fn = getattr(dll, "constellation_native_fork_" + name)
            fn.argtypes = []
            fn.restype = None
            setattr(self, "fork_" + name, fn)

    def invoke(self, function: Any, *args: Any) -> bytes:
        out = _Buffer()
        try:
            status = function(*args, ctypes.byref(out))
            if status != 0 or not out.data or not (0 < out.len <= _MAX_RESPONSE):
                raise ValueError("Native ABI did not return a bounded outcome")
            return ctypes.string_at(out.data, out.len)
        finally:
            # Also runs when Python handles an interrupt immediately after C
            # returns. Native ownership is independent of this response buffer.
            if out.data:
                self.free(out)


# Strong process-lifetime references prevent unloading code which owns handles.
# Aliases that the loader maps to the same image share one callback registration.
_IMAGES: dict[int, _Library] = {}
_PATHS: dict[str, _Library] = {}
_LIBRARY_MUTEX = threading.RLock()
_FORKING = False
_FORK_STATE = threading.local()


class _ForkAttempt:
    def __init__(self):
        self.images: list[_Library] = []
        self.owns_preparation = False
        self.prepared = False


def _before_fork() -> None:
    global _FORKING
    # Python reports callback exceptions as unraisable and still forks. Keep
    # completion false until every native image is prepared; an interrupted
    # child must never return to copied critical-section code.
    attempt = _ForkAttempt()
    _FORK_STATE.current = attempt
    with _LIBRARY_MUTEX:
        if _FORKING:
            # Another thread owns the native preparation gates. Its state must
            # survive this attempt's parent callback; this child exits below.
            return
        attempt.owns_preparation = True
        _FORKING = True
        for library in _IMAGES.values():
            # Record before calling C: an interrupt may arrive after its gate
            # was acquired but before ctypes returns to this callback.
            attempt.images.append(library)
            library.fork_prepare()
        attempt.prepared = True
    # Native gates stay held. _load refuses cache changes until the matching
    # parent/child callback; no Python mutex ownership crosses the fork.


def _after_fork_parent() -> None:
    global _FORKING
    attempt = getattr(_FORK_STATE, "current", None)
    _FORK_STATE.current = None
    if attempt is None or not attempt.owns_preparation:
        return
    attempt.prepared = False
    failure = None
    try:
        for library in reversed(attempt.images):
            try:
                # Idempotent for an image whose prepare never completed.
                library.fork_parent()
            except BaseException as error:
                failure = error
    finally:
        attempt.images.clear()
        _FORKING = False
    if failure is not None:
        raise failure


def _after_fork_child() -> None:
    global _LIBRARY_MUTEX, _FORKING
    try:
        attempt = getattr(_FORK_STATE, "current", None)
        if attempt is None or not attempt.owns_preparation or not attempt.prepared:
            os._exit(125)
        attempt.prepared = False
        for library in attempt.images:
            # Native also exits 125 if any kernel lock was inherited.
            library.fork_child()
        _FORK_STATE.current = None
        _LIBRARY_MUTEX = threading.RLock()
        _FORKING = False
    except BaseException:
        os._exit(125)


if hasattr(os, "register_at_fork"):
    # Exactly one Python registration coordinates every distinct loaded image.
    os.register_at_fork(
        before=_before_fork,
        after_in_parent=_after_fork_parent,
        after_in_child=_after_fork_child,
    )


def _absolute(value: os.PathLike[str] | str) -> Path:
    path = Path(value)
    if not path.is_absolute() or "\0" in os.fspath(path):
        raise ValueError("Native library, driver and configuration paths must be absolute")
    return path


def _load(path: Path) -> _Library:
    # Installation/package verification establishes executable trust before this
    # call. This regular-file check is not a substitute for owner/ACL enrollment.
    if path.is_symlink() or not path.is_file():
        raise Refused("library_path", "Use the installation's ordinary native library")
    key = os.fspath(path.resolve(strict=True))
    with _LIBRARY_MUTEX:
        if _FORKING:
            raise Busy("fork_in_progress", "Native library loading overlaps fork preparation")
        if key in _PATHS:
            return _PATHS[key]
        try:
            if os.name == "nt":
                # Resolve only beside this exact library or in the OS directory.
                dll = ctypes.CDLL(key, winmode=0x100 | 0x800)
            else:
                dll = ctypes.CDLL(key, mode=ctypes.RTLD_LOCAL)
            library = _IMAGES.get(dll._handle)
            if library is None:
                library = _Library(dll)
                _IMAGES[dll._handle] = library
        except (OSError, AttributeError) as error:
            raise DriverError("library_load", "Cannot load the installed native ABI") from error
        _PATHS[key] = library
        return library


def _encode(value: Mapping[str, Any]) -> bytes:
    raw = json.dumps(
        value, allow_nan=False, ensure_ascii=False, separators=(",", ":")
    ).encode("utf-8")
    if len(raw) > _MAX_REQUEST:
        raise ValueError("Native request exceeds the ABI framing limit")
    return raw


class NativeSession:
    """An explicit, process-owned native session over a fixed protected enrollment.

    Calls finish synchronously. The external operation worker retains its own
    deadline and uncertainty policy. Do not use preexec_fn or uncoordinated
    Python-returning foreign forks inside a native critical section.
    """

    def __init__(
        self,
        library_path: os.PathLike[str] | str,
        driver_path: os.PathLike[str] | str,
        config_path: os.PathLike[str] | str,
    ):
        library, driver, config = map(_absolute, (library_path, driver_path, config_path))
        self._pid = os.getpid()
        self._mutex = threading.RLock()
        self._closed = False
        self._poisoned = False
        self._depth = 0
        self._attempted: set[str] = set()
        self._serial = 0
        self._prefix = uuid.uuid4().hex
        self._handle = ctypes.c_uint64()
        self._library = _load(library)
        options = _encode({"version": 1, "config": str(config), "driver": str(driver)})
        try:
            raw = self._library.invoke(
                self._library.open, options, len(options), ctypes.byref(self._handle)
            )
            self._decode(raw, "native.open", mutating=False)
            if not self._handle.value:
                raise self._lost("protocol_handle", "Native open returned no handle", False)
        except BaseException:
            if self._handle.value:
                # Open cannot hold an enrolled lock. Do not leak a successfully
                # allocated handle if Python was interrupted while receiving it.
                with contextlib.suppress(Exception):
                    self._library.invoke(self._library.close, self._handle.value)
            self._closed = True
            raise

    def __enter__(self) -> NativeSession:
        self._ensure_current()
        return self

    def __exit__(self, _kind: Any, error: BaseException | None, _traceback: Any) -> None:
        try:
            self.close()
        except DriverError:
            if error is None:
                raise
            if hasattr(error, "add_note"):
                error.add_note("Native session close was not confirmed; original error retained.")

    def _ensure_current(self) -> None:
        # Always precedes acquiring an inherited Python mutex.
        if os.getpid() != self._pid:
            raise DriverError("forked_session", "Open a new native session after fork")
        if self._closed:
            raise DriverError("closed_session", "Native session is closed")

    def _lost(self, code: str, message: str, mutating: bool) -> DriverError:
        self._poisoned = True
        # Never close a native handle or clear ownership here.
        return (Unknown if mutating else DriverError)(code, message)

    def _decode(self, raw: bytes, request_id: str, *, mutating: bool) -> Any:
        try:
            response = json.loads(raw)
            if (
                not isinstance(response, dict)
                or type(response.get("version")) is not int
                or response["version"] != 1
                or response.get("id") != request_id
                or response.get("outcome") not in _OUTCOMES
            ):
                raise ValueError("Invalid native response envelope")
            if response["outcome"] == "ok":
                if "result" not in response:
                    raise ValueError("Missing native result")
                return response["result"]
            detail = response.get("error")
            if (
                not isinstance(detail, dict)
                or not isinstance(detail.get("code"), str)
                or not isinstance(detail.get("message"), str)
            ):
                raise ValueError("Invalid native error envelope")
        except (ValueError, TypeError, UnicodeError) as error:
            raise self._lost(
                "protocol_response", "Native response could not be correlated", mutating
            ) from error
        if detail["code"] in _POISON_CODES:
            self._poisoned = True
        error_type = {
            "busy": Busy, "refused": Refused, "unknown": Unknown, "error": DriverError,
        }[response["outcome"]]
        raise error_type(detail["code"], detail["message"])

    def _guarded_decode(self, raw: bytes, request_id: str, *, mutating: bool) -> Any:
        try:
            return self._decode(raw, request_id, mutating=mutating)
        except DriverError:
            # Correlated native outcomes keep their classification. Malformed
            # envelopes already poison dispatch inside _decode.
            raise
        except Exception as error:
            raise self._lost(
                "protocol_response", "Native response could not be decoded", mutating
            ) from error
        except BaseException:
            # The effect may have completed before decoding was interrupted.
            # Preserve the original interrupt and ownership through unwind.
            self._poisoned = True
            raise

    def _call(self, op: str, *, mutating: bool = False, **fields: Any) -> Any:
        self._ensure_current()
        if "_timeout" in fields or "timeout" in fields:
            raise TypeError("Native ABI 1 does not accept a transport timeout")
        with self._mutex:
            self._ensure_current()
            if self._poisoned and op != "lock.release":
                raise Unknown("session_poisoned", "Only release and close remain available")
            self._serial += 1
            request_id = f"{self._prefix}:{self._serial}"
            encoded = _encode({"version": 1, "id": request_id, "op": op, **fields})
            try:
                raw = self._library.invoke(
                    self._library.request, self._handle.value, encoded, len(encoded)
                )
                return self._guarded_decode(raw, request_id, mutating=mutating)
            except DriverError:
                raise
            except Exception as error:
                raise self._lost(
                    "abi_response", "Native call outcome is unavailable", mutating
                ) from error
            except BaseException:
                self._poisoned = True
                raise

    @contextlib.contextmanager
    def lock(self, name: str) -> Iterator[None]:
        self._ensure_current()
        if not isinstance(name, str) or not name:
            raise ValueError("An enrolled lock name is required")
        with self._mutex:
            self._ensure_current()
            if name in self._attempted:
                raise DriverError("lock_reentry", "This session already owns or attempted this lock")
            self._attempted.add(name)
            self._depth += 1
            primary: BaseException | None = None
            try:
                try:
                    self._call("lock.acquire", mutating=True, name=name)
                except (Busy, Refused):
                    self._attempted.remove(name)
                    raise
                yield
            except BaseException as error:
                primary = error
                raise
            finally:
                try:
                    if name in self._attempted:
                        try:
                            self._call("lock.release", mutating=True, name=name)
                        except Refused as error:
                            if error.code != "not_held":
                                raise
                        self._attempted.remove(name)
                except BaseException as error:
                    self._poisoned = True
                    if primary is None:
                        raise
                    if hasattr(primary, "add_note"):
                        primary.add_note(
                            "Native release was not confirmed; original error retained. "
                            "The session may still own its lock."
                        )
                finally:
                    self._depth -= 1

    def close(self) -> None:
        if os.getpid() != self._pid:
            raise DriverError("forked_session", "Inherited sessions cannot be closed in a child")
        with self._mutex:
            if self._closed:
                return
            if self._depth:
                raise Busy("locks_held", "Cannot close inside a native critical section")
            try:
                raw = self._library.invoke(self._library.close, self._handle.value)
                self._guarded_decode(raw, "native.close", mutating=True)
                self._closed = True
                self._attempted.clear()
                self._handle.value = 0
            except DriverError:
                raise
            except Exception as error:
                raise self._lost("abi_close", "Native close outcome is unavailable", True) from error
            except BaseException:
                self._poisoned = True
                raise

    # Reuse only pure named request builders. The executable transport's process,
    # shutdown, timeout and lost-reply behavior are deliberately not inherited.
    describe = Session.describe
    process_identity = Session.process_identity
    process_matches = Session.process_matches
    process_status = Session.process_status
    service_register = Session.service_register
    service_snapshot = Session.service_snapshot
    service_action = Session.service_action
    tree_inspect = Session.tree_inspect
    exchange_inspect = Session.exchange_inspect
    exchange_prepare_pointer = Session.exchange_prepare_pointer
    exchange_apply = Session.exchange_apply
    private_write = Session.private_write
    private_dir = Session.private_dir
    durable_remove = Session.durable_remove
    launch = Session.launch

    def tree_remove(self, name: str, entry: str, expected: Mapping[str, Any]) -> Any:
        """Synchronous removal; the external action worker owns its deadline."""
        return self._call("tree.remove", mutating=True, name=name, entry=entry, expected=expected)
