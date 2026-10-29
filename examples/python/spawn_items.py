"""Spawn items and weapons, capability-gated, with typed outcomes.

Usage: python examples/python/spawn_items.py [game_dir]
"""

import sys
from pathlib import Path

from l4d2api import L4D2Client
from l4d2api.errors import L4D2Error

game_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
    r"C:\Program Files (x86)\Steam"
    r"\steamapps\common\Left 4 Dead 2"
)

with L4D2Client(game_dir=game_dir, timeout=10.0) as client:
    client.connect()
    caps = client.capabilities()
    print("Capabilities:", caps)

    if "spawn_item" in caps:
        try:
            result = client.spawn_item("pain_pills", 2)
            print(f"spawn_item: status={result.status} code={result.code} "
                  f"actual={result.actual}/{result.requested}")
        except L4D2Error as exc:
            print(f"spawn_item failed without ambiguity: {exc}")
    else:
        print("spawn_item not advertised; skipping (no blind execution)")

    if "spawn_weapon" in caps:
        try:
            result = client.spawn_weapon("weapon_rifle_ak47", 1)
            print(f"spawn_weapon: status={result.status} code={result.code} "
                  f"actual={result.actual}/{result.requested}")
        except L4D2Error as exc:
            print(f"spawn_weapon failed without ambiguity: {exc}")
    else:
        print("spawn_weapon not advertised; skipping (no blind execution)")
