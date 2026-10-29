"""`l4d2api` errors: one typed error per failure mode.

Timeouts never imply the game acted or did not act; callers must treat an
ambiguous mutation as unknown and never retry it automatically.
"""

from __future__ import annotations


class L4D2Error(Exception):
    """Base class for every error raised by this package."""


class ProtocolError(L4D2Error):
    """A record failed framing or schema validation."""


class TransportError(L4D2Error):
    """The IPC directory could not be read or written."""


class BusyError(L4D2Error):
    """This client already has a request outstanding (single-flight)."""


class LockedError(L4D2Error):
    """Another process holds the IPC write lock."""


class TimeoutError(L4D2Error):
    """No matching reply arrived before the deadline.

    The game may still have acted: never retry a timed-out mutation
    automatically.
    """

    def __init__(self, timeout: float) -> None:
        super().__init__(f"timed out waiting for the game reply after {timeout:.2f}s")
        self.timeout = timeout


class StaleGenerationError(L4D2Error):
    """The game generation changed mid-request; handshake again."""

    def __init__(self, expected: str, found: str) -> None:
        super().__init__(
            f"game generation changed (was {expected}, now {found}); handshake again"
        )
        self.expected = expected
        self.found = found


class NoHeartbeatError(L4D2Error):
    """No heartbeat yet: game addon missing or game closed."""


class StaleHeartbeatError(L4D2Error):
    """Heartbeat too old to trust; the game may be closed or frozen."""

    def __init__(self, age: float) -> None:
        super().__init__(f"heartbeat is stale (last update {age:.2f}s ago)")
        self.age = age


class NotReadyError(L4D2Error):
    """The game is not ready for gameplay (`reason`: not_host, ...)."""

    def __init__(self, reason: str) -> None:
        super().__init__(f"game is not ready: {reason}")
        self.reason = reason


class UnsupportedCapabilityError(L4D2Error):
    """The game does not advertise the requested action."""

    def __init__(self, action: str) -> None:
        super().__init__(f"action `{action}` is not advertised by the game")
        self.action = action
