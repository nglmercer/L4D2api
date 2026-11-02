# Compatibility

Supported: Windows 10/11, Steam L4D2 (AppID 550), single-player or locally
hosted co-op where the TikTools user is the authoritative host.
`sv_cheats 0` throughout; no injection, DLL hooks, exe patching, or cheats.

Not supported (documented limits, not bugs):

- Remote / Valve / community dedicated servers and non-host clients: the
  addon only acts on the authoritative host; anything else is refused with
  `NOT_HOST` / `not_host`. Dedicated-server users keep opt-in `legacy_rcon`.
- One outstanding mutation at a time per client; one active writer per IPC
  directory across processes (extra writers get `Busy` / `Locked`, never
  an interleaved write).
- Old commands never cross a game generation (relaunch / map change):
  outstanding state is cleared and a fresh handshake is required.

## Per-capability matrix

Labels: `verified` (reproducible evidence on record), `experimental`
(implemented, live-game verification pending), `unsupported` (refused by
design), `blocked` (cannot work by construction).

| Capability | Label | Notes |
|---|---|---|
| `test_connection` | `verified` | readiness-gated |
| `spawn_item` | `verified` | stock world pickups, incl. ammo pile and both upgrade packs |
| `spawn_weapon` | `verified` | all 20 allowlisted weapons |
| `spawn_common` | `verified` | 10 per request |
| `spawn_infected` | `verified` | `ZSpawn` near the survivors: hunter, witch, tank seen live; smoker, boomer, spitter, jockey, charger use the same path but were not run |
| `panic` | `verified` | no `Director.ForcePanicEvent` in this runtime; a `ZSpawn` mob near the survivors |
| `heal` / `temp_health` / `damage` | `verified` | `target` all, host or random |
| `revive` | `verified` | downed survivors only |
| `give_item` | `verified` | molotov to all seen live; other names use the same `GiveItem` call |
| `spawn_hazard` | `verified` | fire and explosion |

`verified` means the action returned the expected result in the recorded
single-player local-host run in [WINDOWS_TESTING.md](WINDOWS_TESTING.md)
(Windows 11, L4D2 build 10097, `sv_cheats 0`), which also covered a
`changelevel` map transition (new generation, no replay of old commands).
It does not yet cover a second human player or long bursts; those rows in
the test plan are still open. Capabilities are probed in-game, and SDKs only
advertise what the live heartbeat lists.

## Workshop mods and consistency kicks

Measured live 2026-10-29: a client with a workshop weapon mod (SG553
replacing stock `scripts/weapon_rifle_sg552.txt`) is kicked from a stock
SRCDS with `Server is enforcing consistency for this file`. No RCON
password, TikTools version, or network setting avoids it — the engine
compares the client's stock files against the server's.

Three durable answers, no manual file juggling:

1. **Your own server:** set `sv_consistency 0` (operator opt-in, private
   servers). Any mod joins, zero client action. Verified live on the test
   server the same day.
2. **Someone else's stock server:** `l4d2api-installer` detects colliding
   addons (`scan_addon_conflicts`: any installed VPK overriding a stock
   `scripts/*.txt`) and quarantines them with one click; `restore_quarantined`
   brings them back afterwards. Weapon scripts (`scripts/weapon_*.txt`)
   are a measured kick trigger; other stock script overrides are reported
   as suspicious for the user to decide. Skins/sounds/models never kick.
3. **Local-host path (this repo's default):** the file-IPC transport has
   no client/server file comparison at all, so it is inherently
   mod-agnostic. Pending the Windows live evidence gate.

Hard limit, stated plainly: no code in this repo can force a
third-party stock server to accept modded stock files. Detection +
quarantine is the complete client-side answer; `sv_consistency 0` is the
complete operator-side answer.
