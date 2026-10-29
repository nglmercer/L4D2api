"""Typed result and session models for the `l4d2api` Python SDK."""

from __future__ import annotations

from dataclasses import dataclass

from .protocol import Heartbeat


@dataclass
class ActionResult:
    """Outcome of one request, mirrored from the game's `result.txt`.

    `request_id` is the public SDK name for the wire field `id`.
    A written `command.txt` is never reported as success: success is only
    the game's acknowledgement decoded here.
    """

    request_id: str
    generation: str
    status: str
    code: str
    requested: int
    actual: int
    diagnostic: str = ""

    @property
    def ok(self) -> bool:
        return self.status in ("applied", "partial")


@dataclass
class SessionStatus:
    """Point-in-time session health plus the heartbeat it was derived from."""

    health: str
    heartbeat: Heartbeat | None = None

    @property
    def ready(self) -> bool:
        return self.health == "gameplay_ready"
