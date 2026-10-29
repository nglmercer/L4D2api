# Compatibility

Supported: Windows 10/11, Steam L4D2 (AppID 550), single-player or locally
hosted co-op where the TikTools user is the authoritative host.
`sv_cheats 0` throughout; no injection, DLL hooks, exe patching, or cheats.

Not supported (documented limits, not bugs):

- Remote / Valve / community dedicated servers and non-host clients: the
  addon only acts on the authoritative host; anything else is refused with
  `NOT_HOST` / `not_host`. Dedicated-server users keep opt-in `legacy_rcon`.
- Special-infected spawns are capability-gated per entry and stay
  `CAPABILITY_UNAVAILABLE` until each is proven via VScript with `sv_cheats 0`.
- Old commands never cross a game generation (relaunch / map change):
  outstanding state is cleared and a fresh handshake is required.
- One outstanding mutation at a time; later parallelism needs a newly
  versioned, proven-safe protocol.

Features are labeled `verified`, `experimental`, `unsupported`, or `blocked`
based on evidence — never aspiration. Current label for the whole Windows
local path: **blocked** (live gate not yet run on this Linux workspace).

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
3. **Local-host path (this repo's future default):** the file-IPC transport
   has no client/server file comparison at all, so it is inherently
   mod-agnostic. Pending the Windows live gate.

Hard limit, stated plainly: no code in this repo can force a
third-party stock server to accept modded stock files. Detection +
quarantine is the complete client-side answer; `sv_consistency 0` is the
complete operator-side answer.
