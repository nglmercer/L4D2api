# Windows testing gate

Status: **evidence required — no recorded Windows run on this workspace.**
The reporter states the integration is now working in their local-host
session. That user-reported success is logged below; it does not replace
recorded evidence. Do not mark any capability `verified` until its row is
filled in with the required fields. Never fabricate Windows results.

## User-reported success (unrecorded)

- Reporter: integration user, local/listen-host session.
- Claim: L4D2 integration working (actions executing in-game).
- Missing (required before `verified`): Windows version, Steam/L4D2 build,
  addon version + hash, exact actions executed, expected vs observed
  behavior, host mode, `sv_cheats` state, repeated-invocation results, map
  transition and restart behavior.

## Feasibility checklist (tiny PoC `.vpk`, real Steam L4D2, local campaign)

- [ ] Per-addon `scripts/vscripts/director_base_addon.nut` loads (fallback:
      `mapspawn_addon.nut` if demonstrably more reliable), with other addons
      enabled, without editing base-game shared scripts.
- [ ] `FileToString("tiktools_l4d2/command.txt")` reads and
      `StringToFile(...)` writes under `left4dead2/ems/`; record path,
      encoding, 1 KiB limit behavior, caching, file locks.
- [ ] Poll loop persists across map transitions; measure disk reads and FPS
      impact (~250-500 ms cadence, no per-frame access).
- [ ] Harmless `ping` round-trips, then one item spawn and one common spawn
      with `sv_cheats 0` (investigate `ZSpawn`, `SpawnEntityFromTable`).
- [ ] Mutations run only on the authoritative host; menu / lobby / remote /
      non-host states refuse with the documented codes.

## Full matrix (before release)

Windows 10 + 11; Steam default and secondary libraries; game
launch/close/relaunch; menu to local campaign; map transition; several real
events in one round; another unrelated addon enabled; a player joining as a
client; host-only authority; `sv_cheats 0`; prolonged burst with FPS notes.

Record per run — all required:

- Windows version/build.
- Steam L4D2 version/build.
- Addon version and SHA-256 hash.
- Actual action executed (`test_connection`, `spawn_item`, ...).
- Expected and observed behavior.
- Host mode (single-player / listen host / client).
- `sv_cheats` state.
- Repeated invocation outcome (no duplicate execution on re-poll).
- Map transition behavior (generation change, no replay).
- Game restart behavior (stale heartbeat, handshake required).

Simulated Linux peer tests (this workspace's suites) prove protocol and
session logic only. They are not Windows gameplay verification and must
never be labeled as such.
