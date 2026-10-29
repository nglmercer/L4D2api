# Windows testing gate (BLOCKED)

Status: **BLOCKED — no Windows game session available on this workspace.**
Nothing below has been run. Do not mark the integration production ready
until it is.

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

Record per run: game version, Windows build, addon hash, TikTools commit,
test script, expected/actual result, FPS observation, logs, screenshot/video
if available, pass/fail/blocker. Never fabricate Windows results.
