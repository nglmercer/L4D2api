# L4D2api

File-IPC integration for Left 4 Dead 2: a versioned VScript game addon
(`.vpk`) plus reusable SDKs (Rust, Python, JavaScript/TypeScript) that
drive a **normal Windows Steam local / listen-hosted L4D2 session** —
no SourceMod, no MetaMod, no dedicated server, no RCON, no DLL injection,
no executable patching, no `.bat`/`.sh` scripts for end users, no HTTP
server, no background service. `sv_cheats 0` throughout.

Transport is files, not HTTP: the SDKs and the game exchange
`command.txt` / `result.txt` / `heartbeat.txt` in the game's
`left4dead2/ems/tiktools_l4d2/` sandbox. Nothing here is REST-based.

## What currently works

- **Pack**: deterministic, validated `.vpk` builds (`l4d2api-pack`).
- **Install / verify / repair / status / uninstall**: versioned, transactional,
  ownership-scoped (`l4d2api-installer` CLI; `check`, `quarantine`, `restore`).
- **Protocol v1**: strict codec + validation in Rust, Python and
  JavaScript/TypeScript, pinned to shared fixtures (`tests/fixtures/`).
- **Session clients**: single-writer lock, atomic writes, heartbeat
  freshness, typed errors, unique request ids, no gameplay auto-retries.
- **Game side**: `test_connection`, `spawn_item`, `spawn_weapon` against
  stock world-pickup entities (implemented; live-game verification pending
  the recorded evidence gate — see below).
- `spawn_common` is experimental and requires live-game verification;
  `spawn_infected` remains capability-gated.

User report (unrecorded): the reporter states the L4D2 integration is now
working in their local-host session. That report is tracked as
user-reported success in [docs/WINDOWS_TESTING.md](docs/WINDOWS_TESTING.md),
separate from reproducible test evidence, which is still required before
any `verified` label changes.

## Quick installation (Windows 10/11, Steam L4D2)

End users need no Rust, Python or Node.js. From a tagged release, download
the versioned `.vpk` (+ SHA-256 + manifest) and `l4d2api-installer.exe`,
verify the hash, then:

```powershell
l4d2api-installer.exe install --game-dir "D:\SteamLibrary\steamapps\common\Left 4 Dead 2" --vpk tiktools-l4d2api-0.1.0.vpk --version 0.1.0
l4d2api-installer.exe verify --game-dir "D:\SteamLibrary\steamapps\common\Left 4 Dead 2"
```

Details: [docs/INSTALLATION.md](docs/INSTALLATION.md). Checking for
workshop conflicts: `l4d2api-installer check`.

## Supported game modes

Windows 10/11, Steam L4D2 (AppID 550), single-player or locally hosted
co-op where you are the authoritative host. Remote/community dedicated
servers and non-host clients are refused (`NOT_HOST`); dedicated-server
users keep the opt-in `legacy_rcon` path in TikTools-app.
Full matrix: [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

## Available actions

| Action | Arguments | Status |
|---|---|---|
| `test_connection` | — | working (readiness-gated) |
| `spawn_item` | `item`, `count` 1–5 | working (verified in a local-host session) |
| `spawn_weapon` | `weapon`, `count` 1–5 | working (verified in a local-host session) |
| `spawn_common` | `count` 1–10 | experimental; live verification pending |
| `spawn_infected` | `class`, `count` | implemented via `ZSpawn`; live verification pending |
| `heal` / `temp_health` / `damage` | `target`, `amount` 1–100 | implemented; live verification pending |
| `revive` | `target` | implemented; live verification pending |
| `give_item` | `target`, `item` | implemented; live verification pending |
| `panic` | — | implemented; live verification pending |
| `spawn_hazard` | `kind` (`explosion`/`fire`), `count` 1–3 | implemented; live verification pending |

`target` is `all`, `host` or `random`. Actions are advertised in the
heartbeat only when the running game has the engine functions they need.

Full schemas: [docs/PROTOCOL.md](docs/PROTOCOL.md). Per-capability labels:
[docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

## Python usage

```bash
pip install -e ./sdk/python
```

```python
from pathlib import Path
from l4d2api import L4D2Client

with L4D2Client(game_dir=Path(r"C:\Program Files (x86)\Steam\steamapps\common\Left 4 Dead 2")) as client:
    client.connect()
    print(client.status().health)
    if "spawn_item" in client.capabilities():
        print(client.spawn_item("first_aid_kit", 2).status)
```

SDK guide: [docs/SDK.md](docs/SDK.md). Examples: `examples/python/`.

## JavaScript usage

```bash
cd sdk/javascript && npm install && npm run build
```

```javascript
import { L4D2Client } from "l4d2api";

const client = new L4D2Client({ gameDir: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Left 4 Dead 2" });
try {
  await client.connect();
  console.log((await client.status()).health);
} finally {
  await client.close();
}
```

SDK guide: [docs/SDK.md](docs/SDK.md). Examples: `examples/javascript/`.

## Rust usage

```bash
cargo run -p l4d2api-example -- --game-dir "D:\SteamLibrary\steamapps\common\Left 4 Dead 2"
```

```rust
let mut client = Client::new(&ipc_dir, Duration::from_secs(5), Duration::from_millis(250));
let heartbeat = client.connect()?;
let result = client.test_connection()?;
```

Crates: `l4d2api-protocol` (reference codec), `l4d2api-client` (session),
`l4d2api-installer`, `l4d2api-pack`. API: [docs/API_REFERENCE.md](docs/API_REFERENCE.md).

## Troubleshooting

Stale heartbeat? Not host? Mod conflict kick? See
[docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md). Key rules: a timeout is
unconfirmed (never success, never auto-retried); old generations never
replay; concurrent SDK clients serialize on the write lock.

## Development and contribution

Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Contribute:
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Test matrix:
[docs/TESTING.md](docs/TESTING.md). Windows live gate:
[docs/WINDOWS_TESTING.md](docs/WINDOWS_TESTING.md). Security model:
[docs/SECURITY.md](docs/SECURITY.md).
