# l4d2api (Python)

File-IPC client SDK for the L4D2api game addon. There are no HTTP
endpoints: the client talks to the game through the
`left4dead2/ems/tiktools_l4d2/` files (`command.txt`, `result.txt`,
`heartbeat.txt`, plus the `command.lock` single-writer lock).

## Install

```bash
pip install -e ./sdk/python
```

Requires Python 3.10+, stdlib only (no dependencies).

## Use

```python
from pathlib import Path
from l4d2api import L4D2Client

with L4D2Client(game_dir=Path(r"C:\Program Files (x86)\Steam\steamapps\common\Left 4 Dead 2")) as client:
    client.connect()
    print(client.status().health)
    print(client.capabilities())
    if "spawn_item" in client.capabilities():
        result = client.spawn_item("first_aid_kit", 2)
        print(result.status, result.actual)
```

Runnable examples live in `examples/python/`: `basic.py`,
`spawn_items.py`, `monitor.py`.

## Rules

- Mutations check the advertised capability first; unadvertised actions
  raise `UnsupportedCapabilityError` without writing anything.
- Every request mints a unique id; stale results never satisfy a new wait.
- Timeouts (`TimeoutError`) leave the outcome unknown: never retry a
  timed-out mutation automatically.
- Only one writer per IPC directory: a second process gets `LockedError`.
- Heartbeats older than `heartbeat_timeout` (default 5 s) read as stale.

## Tests

```bash
cd sdk/python && PYTHONPATH=src python3 -m unittest discover -s tests
```

`tests/test_protocol.py` pins the shared fixtures in `tests/fixtures/`
against the Rust reference codec.
