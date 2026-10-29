"""Monitor heartbeat changes; show timeout and generation handling.

Watches the session health over time and distinguishes: no heartbeat,
stale heartbeat, connected-but-not-ready, and gameplay-ready. Any error
is reported with its typed name so callers can handle each case.

Usage: python examples/python/monitor.py [game_dir] [seconds]
"""

import sys
import time
from pathlib import Path

from l4d2api import L4D2Client
from l4d2api.errors import L4D2Error

game_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
    r"C:\Program Files (x86)\Steam"
    r"\steamapps\common\Left 4 Dead 2"
)
budget = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0

with L4D2Client(game_dir=game_dir, poll_interval=0.5) as client:
    try:
        hb = client.connect()
        print(f"connected: gen={hb.generation} tick={hb.tick}")
    except L4D2Error as exc:
        print(f"{type(exc).__name__}: {exc} (game closed or addon missing?)")
        raise SystemExit(1)

    last_tick = None
    deadline = time.monotonic() + budget
    while time.monotonic() < deadline:
        try:
            status = client.status()
        except L4D2Error as exc:
            print(f"{type(exc).__name__}: {exc}")
            time.sleep(0.5)
            continue
        hb = status.heartbeat
        tick = hb.tick if hb else None
        changed = " (tick advanced)" if last_tick is not None and tick != last_tick else ""
        print(f"health={status.health} tick={tick}{changed}")
        last_tick = tick
        try:
            result = client.test_connection()
            print(f"  ping: {result.status} {result.code}")
        except L4D2Error as exc:
            print(f"  ping: {type(exc).__name__}: {exc}")
        time.sleep(1.0)
