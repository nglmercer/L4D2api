# Examples

All examples use file IPC against `left4dead2/ems/tiktools_l4d2/` — no
network, no server to start. Against a real game, point them at your Steam
L4D2 install; without one, they report `NoHeartbeat`/`Disconnected`
instead of fabricating success.

## Rust

```bash
cargo run -p l4d2api-example -- --game-dir "<game-dir>"
```

Source: `examples/rust/src/main.rs`. Connects, prints the map and
capabilities, pings, and spawns two first-aid kits behind the
`spawn_item` capability gate (skipped with an explanation otherwise).

## Python

```bash
pip install -e ./sdk/python
python examples/python/basic.py
python examples/python/spawn_items.py "<game-dir>"
python examples/python/monitor.py "<game-dir>" 10
```

- `basic.py` — connect, capabilities, ping, one gated item spawn.
- `spawn_items.py` — item + weapon spawns with typed outcomes and
  per-case error handling.
- `monitor.py` — heartbeat/health watch: distinguishes no heartbeat,
  stale heartbeat, connected-but-not-ready, and gameplay-ready; shows
  timeout and generation error names.

## JavaScript

```bash
cd sdk/javascript && npm install && npm run build
node examples/javascript/basic.mjs
node examples/javascript/spawn_items.mjs "<game-dir>"
node examples/javascript/monitor.mjs "<game-dir>" 10
```

- `basic.mjs` — the runnable session from the task brief.
- `spawn_items.mjs` — capability-gated item + weapon spawns.
- `monitor.mjs` — health watch with typed error names.

The examples import the built `sdk/javascript/dist/` output, so rebuild
after changing `sdk/javascript/src/`.
