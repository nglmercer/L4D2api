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
- **Game side**: every action below returned its expected result in a
  recorded live run (Windows 11, L4D2 build 10097, single-player local
  host, `sv_cheats 0`): item, weapon, ammo and upgrade-pack spawns; common
  and special infected (incl. witch and tank); panic; heal, temp health,
  damage, revive and give item; fire and explosion hazards.
- **Config with hot reload**: per-action cooldowns, `round_cap` and
  `damage_max` in `ems/tiktools_l4d2/config.txt`, applied in about a second.

A `changelevel` map transition was also checked live: the loop restarts
under a new generation and old commands never run again. Evidence, builds
and what is still untested (a second human player, long bursts):
[docs/WINDOWS_TESTING.md](docs/WINDOWS_TESTING.md).

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
| `test_connection` | — | verified live (readiness-gated) |
| `spawn_item` | `item`, `count` 1–5 | verified live (incl. `ammo` pile and upgrade packs) |
| `spawn_weapon` | `weapon`, `count` 1–5 | verified live (all 20 weapons) |
| `spawn_common` | `count` 1–10 | verified live |
| `spawn_infected` | `class`, `count` (tank exactly 1) | verified live: hunter, witch, tank |
| `heal` / `temp_health` / `damage` | `target`, `amount` 1–100 | verified live |
| `revive` | `target` | verified live |
| `give_item` | `target`, `item` | verified live (molotov to all) |
| `panic` | — | verified live (`ZSpawn` horde; this game has no `ForcePanicEvent`) |
| `spawn_hazard` | `kind` (`explosion`/`fire`), `count` 1–3 | verified live |

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
