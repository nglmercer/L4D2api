"""`l4d2api`: file-IPC client SDK for the L4D2api game addon.

Transport is the game's `ems/tiktools_l4d2/` files -- there are no HTTP
endpoints. Importing this package never touches the game or the disk.
"""

from .client import L4D2Client
from .errors import (
    BusyError,
    L4D2Error,
    LockedError,
    NoHeartbeatError,
    NotReadyError,
    ProtocolError,
    StaleGenerationError,
    StaleHeartbeatError,
    TimeoutError,
    TransportError,
    UnsupportedCapabilityError,
)
from .models import ActionResult, SessionStatus
from .protocol import Command, Heartbeat, ResultRecord

__all__ = [
    "L4D2Client",
    "ActionResult",
    "SessionStatus",
    "Command",
    "Heartbeat",
    "ResultRecord",
    "L4D2Error",
    "ProtocolError",
    "TransportError",
    "BusyError",
    "LockedError",
    "TimeoutError",
    "StaleGenerationError",
    "NoHeartbeatError",
    "StaleHeartbeatError",
    "NotReadyError",
    "UnsupportedCapabilityError",
]

__version__ = "0.1.0"
