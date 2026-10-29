"""Basic L4D2api session: connect, check capabilities, ping, spawn items.

Run from the repo root with the SDK installed (pip install -e ./sdk/python):

    python examples/python/basic.py

Against a real game, point game_dir at your Steam L4D2 install. Without
one, point it at a temp dir and run a mock peer (see monitor.py).
"""

from pathlib import Path

from l4d2api import L4D2Client

game_dir = Path(
    r"C:\Program Files (x86)\Steam"
    r"\steamapps\common\Left 4 Dead 2"
)

with L4D2Client(game_dir=game_dir) as client:
    client.connect()

    status = client.status()
    print("Status:", status.health)

    print("Capabilities:", client.capabilities())

    result = client.test_connection()
    print("Connection:", result.status, result.code)

    if "spawn_item" in client.capabilities():
        result = client.spawn_item("first_aid_kit", 2)
        print("Items:", result.actual, result.status)
