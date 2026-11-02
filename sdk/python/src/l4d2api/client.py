"""File-IPC session client for the L4D2api game addon (stdlib only).

Same contract as the `l4d2api-client` Rust crate: atomic temp-file writes,
an atomic cross-process lock file held for the whole request/response
transaction, freshness-checked heartbeats, unique request ids, typed
errors, and no automatic gameplay retries.
"""

from __future__ import annotations

import os
import time
from pathlib import Path

from . import protocol
from .errors import (
    BusyError,
    LockedError,
    NoHeartbeatError,
    NotReadyError,
    StaleGenerationError,
    StaleHeartbeatError,
    TimeoutError,
    TransportError,
    UnsupportedCapabilityError,
)
from .models import ActionResult, SessionStatus
from .protocol import Heartbeat

COMMAND_FILE = "command.txt"
RESULT_FILE = "result.txt"
HEARTBEAT_FILE = "heartbeat.txt"
LOCK_FILE = "command.lock"

DEFAULT_HEARTBEAT_TIMEOUT = 5.0
DEFAULT_LOCK_STALE_AFTER = 30.0
# Total budget for retrying a rename blocked by a reader (Windows only).
_RENAME_RETRY_BUDGET_S = 2.0

_HEALTH_ORDER = ("disconnected", "connected", "stale", "not_ready", "gameplay_ready")


def _remove_quietly(path: Path) -> None:
    try:
        path.unlink()
    except OSError:
        pass


class L4D2Client:
    """One session against the game's `ems/tiktools_l4d2/` directory."""

    def __init__(
        self,
        game_dir: str | Path | None = None,
        ipc_dir: str | Path | None = None,
        timeout: float = 5.0,
        poll_interval: float = 0.25,
        heartbeat_timeout: float = DEFAULT_HEARTBEAT_TIMEOUT,
        lock_stale_after: float = DEFAULT_LOCK_STALE_AFTER,
    ) -> None:
        if ipc_dir is None:
            if game_dir is None:
                raise ValueError("pass game_dir or ipc_dir")
            ipc_dir = Path(game_dir) / "left4dead2" / "ems" / "tiktools_l4d2"
        self._dir = Path(ipc_dir)
        self._timeout = timeout
        self._poll_interval = poll_interval
        self._heartbeat_timeout = heartbeat_timeout
        self._lock_stale_after = lock_stale_after
        self._seq = 0
        self._generation: str | None = None
        self._outstanding = False
        self._owner = f"pid{os.getpid()}-{time.time_ns()}"
        self._last_tick: tuple[str, int, float] | None = None

    # -- context management -------------------------------------------------
    def __enter__(self) -> L4D2Client:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def close(self) -> None:
        """Release per-instance state. The cross-process lock is always
        released per request; close() only clears the outstanding flag."""
        self._outstanding = False

    # -- low-level file access ----------------------------------------------
    def _read(self, name: str) -> str | None:
        try:
            text = (self._dir / name).read_text(encoding="ascii")
            return text.removesuffix("\0") if name in (HEARTBEAT_FILE, RESULT_FILE) else text
        except FileNotFoundError:
            return None
        except OSError as exc:
            raise TransportError(str(exc)) from exc

    def _age(self, name: str) -> float | None:
        try:
            return time.time() - (self._dir / name).stat().st_mtime
        except OSError:
            return None

    def heartbeat(self) -> Heartbeat | None:
        raw = self._read(HEARTBEAT_FILE)
        if raw is None:
            return None
        try:
            return protocol.decode_heartbeat(raw)
        except Exception:
            return None

    def heartbeat_fresh(self) -> tuple[Heartbeat, float] | None:
        hb = self.heartbeat()
        if hb is None:
            return None
        age = self._age(HEARTBEAT_FILE)
        return (hb, age if age is not None else float("inf"))

    # -- session lifecycle ----------------------------------------------------
    def connect(self) -> Heartbeat:
        """Pin the live generation. Clears stale per-instance state."""
        hb = self.heartbeat()
        if hb is None:
            raise NoHeartbeatError("no heartbeat yet: game addon is missing or closed")
        self._generation = hb.generation
        self._outstanding = False
        self._last_tick = (hb.generation, hb.tick, time.monotonic())
        return hb

    def capabilities(self) -> list[str]:
        hb = self.heartbeat()
        return list(hb.capabilities) if hb else []

    def status(self) -> SessionStatus:
        """Freshness-checked session health. A valid but old heartbeat is
        `stale`, never ready. Observes `tick` progression across calls."""
        fresh = self.heartbeat_fresh()
        if fresh is None:
            return SessionStatus("disconnected")
        hb, age = fresh
        if age > self._heartbeat_timeout:
            return SessionStatus("stale", hb)
        now = time.monotonic()
        stalled = (
            self._last_tick is not None
            and self._last_tick[0] == hb.generation
            and self._last_tick[1] == hb.tick
        )
        self._last_tick = (hb.generation, hb.tick, now)
        if stalled and age > self._poll_interval * 3:
            return SessionStatus("stale", hb)
        if not hb.addon_version:
            return SessionStatus("not_ready", hb)
        if not hb.host_authority:
            return SessionStatus("not_ready", hb)
        if not hb.gameplay_ready or not hb.round_active:
            return SessionStatus("connected", hb)
        return SessionStatus("gameplay_ready", hb)

    def ensure_ready_for(self, action: str) -> None:
        if self._generation is None:
            raise NoHeartbeatError("connect() first: no pinned generation")
        fresh = self.heartbeat_fresh()
        if fresh is None:
            raise NoHeartbeatError("no heartbeat: game addon missing or closed")
        hb, age = fresh
        if age > self._heartbeat_timeout:
            raise StaleHeartbeatError(age)
        if hb.generation != self._generation:
            raise StaleGenerationError(self._generation, hb.generation)
        if not hb.host_authority:
            raise NotReadyError("not_host")
        if action != "test_connection" and action not in hb.capabilities:
            raise UnsupportedCapabilityError(action)

    # -- cross-process single-writer lock --------------------------------------
    def _acquire_lock(self) -> None:
        path = self._dir / LOCK_FILE
        try:
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
        except FileExistsError:
            self._reclaim_or_refuse()
            return
        except OSError as exc:
            raise TransportError(str(exc)) from exc
        with os.fdopen(fd, "w", encoding="ascii") as fh:
            fh.write(self._owner + "\n")

    def _reclaim_or_refuse(self) -> None:
        path = self._dir / LOCK_FILE
        try:
            first = path.read_text(encoding="ascii")
        except OSError:
            first = ""
        age = self._age(LOCK_FILE)
        if age is None or age <= self._lock_stale_after:
            raise LockedError(f"another process holds the IPC write lock (age {age})")
        time.sleep(0.025)
        try:
            second = path.read_text(encoding="ascii")
        except OSError:
            second = ""
        if second != first:
            raise LockedError("lock changed during reclaim; another client is live")
        try:
            path.unlink()
        except OSError:
            pass
        try:
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
        except FileExistsError:
            raise LockedError("lost the reclaim race; another client is live")
        except OSError as exc:
            raise TransportError(str(exc)) from exc
        with os.fdopen(fd, "w", encoding="ascii") as fh:
            fh.write(self._owner + "\n")

    def _release_lock(self) -> None:
        path = self._dir / LOCK_FILE
        try:
            if path.read_text(encoding="ascii").strip() == self._owner:
                path.unlink()
        except OSError:
            pass

    def _write_command_atomic(self, text: str, request_id: str) -> None:
        safe = "".join(c if c.isalnum() or c in "_-" else "_" for c in request_id)[:48] or "id"
        target = self._dir / COMMAND_FILE
        tmp = self._dir / f"command.{os.getpid()}.{safe}.tmp"
        try:
            tmp.write_text(text, encoding="ascii")
        except OSError as exc:
            try:
                tmp.unlink()
            except OSError:
                pass
            raise TransportError(str(exc)) from exc
        # os.replace is atomic on both POSIX and Windows, including when
        # the destination already exists. On Windows it fails with
        # PermissionError while the game holds command.txt open; retry that
        # within a time budget, never anything else.
        deadline = time.monotonic() + _RENAME_RETRY_BUDGET_S
        while True:
            try:
                os.replace(tmp, target)
                return
            except PermissionError as exc:
                if time.monotonic() >= deadline:
                    _remove_quietly(tmp)
                    raise TransportError(str(exc)) from exc
                time.sleep(0.01)
            except OSError as exc:
                _remove_quietly(tmp)
                raise TransportError(str(exc)) from exc

    # -- requests ----------------------------------------------------------------
    def next_id(self) -> str:
        """Mint a unique request id. Never reuse ids: a stale result from
        an earlier request with the same id could satisfy a new wait."""
        self._seq += 1
        return f"r-{os.getpid()}-{time.time_ns()}-{self._seq}"

    def send(self, request_id: str, action: str, **params: str) -> ActionResult:
        """Send one action and wait for the game's typed reply.

        Replies must match id, sequence and generation together, so a stale
        result never satisfies a new wait. No automatic retries: a timeout
        leaves the outcome unknown.
        """
        if self._outstanding:
            raise BusyError("another mutation is already outstanding (single-flight)")
        if self._generation is None:
            raise NoHeartbeatError("connect() first: no pinned generation")
        generation = self._generation
        self._acquire_lock()
        self._outstanding = True
        try:
            return self._send_locked(request_id, action, generation, params)
        finally:
            self._outstanding = False
            self._release_lock()

    def _send_locked(
        self, request_id: str, action: str, generation: str, params: dict[str, str]
    ) -> ActionResult:
        self.ensure_ready_for(action)
        # The lock serializes all SDKs. Recover the last sequence even after
        # a client restart or a timeout with an unconfirmed game effect.
        for name, decode in ((COMMAND_FILE, protocol.decode_command), (RESULT_FILE, protocol.decode_result)):
            raw = self._read(name)
            if raw is not None:
                try:
                    previous = decode(raw)
                    if previous.generation == generation:
                        self._seq = max(self._seq, previous.seq)
                except protocol.ProtocolError:
                    pass
        self._seq += 1
        command = protocol.Command(
            seq=self._seq, id=request_id, generation=generation,
            action=action, params=params,
        )
        try:
            text = protocol.encode_command(command)
        except protocol.ProtocolError as exc:
            from .errors import ProtocolError as SdkProtocolError

            raise SdkProtocolError(str(exc)) from exc
        self._write_command_atomic(text, request_id)
        return self._wait_for_reply(request_id, self._seq, generation)

    def _wait_for_reply(self, request_id: str, seq: int, generation: str) -> ActionResult:
        deadline = time.monotonic() + self._timeout
        while True:
            raw = self._read(RESULT_FILE)
            if raw is not None:
                try:
                    record = protocol.decode_result(raw)
                except Exception:
                    record = None
                if record is not None and record.id == request_id and record.seq == seq:
                    if record.generation != generation:
                        raise StaleGenerationError(generation, record.generation)
                    return ActionResult(
                        request_id=record.id,
                        generation=record.generation,
                        status=record.status,
                        code=record.code,
                        requested=record.requested,
                        actual=record.actual,
                        diagnostic=record.diag,
                    )
            hb = self.heartbeat()
            if hb is not None and hb.generation != generation:
                raise StaleGenerationError(generation, hb.generation)
            if time.monotonic() >= deadline:
                raise TimeoutError(self._timeout)
            time.sleep(self._poll_interval)

    # -- ergonomic actions (unique ids, capability-checked) -----------------------
    def test_connection(self) -> ActionResult:
        return self.send(self.next_id(), "test_connection")

    def spawn_common(self, count: int) -> ActionResult:
        return self.send(self.next_id(), "spawn_common", count=str(count))

    def spawn_infected(self, class_name: str, count: int) -> ActionResult:
        return self.send(self.next_id(), "spawn_infected",
                         **{"class": class_name, "count": str(count)})

    def spawn_item(self, item: str, count: int) -> ActionResult:
        return self.send(self.next_id(), "spawn_item", item=item, count=str(count))

    def spawn_weapon(self, weapon: str, count: int) -> ActionResult:
        return self.send(self.next_id(), "spawn_weapon", weapon=weapon, count=str(count))
