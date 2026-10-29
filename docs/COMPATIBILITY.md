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
| `test_connection` | `experimental` | readiness-gated; user-reported working, evidence still required |
| `spawn_item` | `experimental` | stock world pickups; user-reported working, evidence still required |
| `spawn_weapon` | `experimental` | stock world pickups; user-reported working, evidence still required |
| `spawn_common` | `experimental` | engine entity spawning; live verification pending |
| `spawn_infected` | `unsupported` | gated `CAPABILITY_UNAVAILABLE` until proven with `sv_cheats 0` |

The user reports the integration is now working in their local-host
session. That report is recorded (not dismissed) in
[WINDOWS_TESTING.md](WINDOWS_TESTING.md), but per-capability labels stay
`experimental` until reproducible evidence (versions, hashes, observed
behavior) is on record. Nothing here is marked `verified` on the basis of
an unrecorded session, and nothing in the SDKs advertises a gated action
as available: `capabilities()` only lists what the game's live heartbeat
advertises.

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
