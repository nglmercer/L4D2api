# Research notes (pre-implementation audit, 2026-10-29)

Sources: TikTools-app (`remake` branch) `plugins/l4d2/*`
(`plugin.json` schemaVersion 3, id `l4d2.interactive`, action IDs
`l4d2.test_connection|spawn_common|spawn_infected|spawn_item`),
`plugins/l4d2/backend/src/{main,command,config,ratelimit,rcon}.rs`,
`integrations/l4d2/{README.md,adapter/tiktools_l4d2.sp}`,
`examples/{peak-game,repo-game}/`. Reference repos PEAKapi / REPOapi were
named in the directive but not vendored here — reuse their *principles*
(state, capabilities, validation, idempotency), not their Unity/BepInEx
implementation.

## Confirmed from the existing tree

- Action surface to preserve: same four action IDs, same infected allowlist
  (8) and item allowlist (6), same `schemaVersion: 3` manifest conventions.
- Existing backend validates against strict allowlists, single RCON attempt
  per action, no retry after timeout, honest `Unknown` outcomes — keep all
  of that in the IPC client.
- Linux SRCDS + SourceMod path is verified live (2026-09-28); Windows local
  path is not. Legacy RCON stays opt-in.

## Open uncertainties (must be settled at the live gate)

1. EMS sandbox path/encoding/size/caching/locks for VScript file I/O.
2. Per-addon `director_base_addon.nut` auto-load vs `mapspawn_addon.nut`.
3. `ZSpawn` / `SpawnEntityFromTable` semantics with `sv_cheats 0` (do NOT
   assume `NoLimit_CreateInfected` / Left4DHooks equivalence).
4. Host-detection and round-state query sources in VScript.
5. Poll cadence cost (disk + FPS) on real hardware.
6. VPK root-directory tree encoding accepted by the L4D2 client.

Nothing in this repo claims these are resolved.
