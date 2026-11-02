# Windows testing gate

Status: **one recorded Windows run (2026-11-01), below.** It covers every
action in a single-player local-host session. Map transitions, a second
human player and long bursts are still unrecorded. Do not mark any
capability `verified` without a row here. Never fabricate Windows results.

## Recorded run: 2026-11-01

- Windows: Windows 11 Enterprise LTSC 2024, 10.0.26100.
- Steam L4D2: build 10097 (console `Build: 10097`), default Steam library.
- Launch: `-steam -condebug +map c1m1_hotel`. Host mode: single-player
  listen host (1 human + 3 survivor bots), heartbeat `host=1 ready=1`.
- `sv_cheats`: 0 (console refuses cheat cvars throughout).
- Other addons: Steam Workshop items present (installer `check`: 6
  `[SUSPICIOUS]`, 0 `[KICK]`).
- Client: Python SDK from this repo, one request per action, no retries.
- Results come from the game's `result.txt`. Visual confirmation was by the
  player for the spawned items, weapons and infected.

Addon builds (packing is deterministic, so each SHA-256 maps to a commit):

| Build SHA-256 (prefix) | Commit | Used for |
|---|---|---|
| `bfe42ed7` | `a0ff692` | first working loop: `test_connection`, items, weapons |
| `b3d0a426` | `e343a0e` | config hot reload, all 20 weapons |
| `04c2fd37` | `9dfa947` | ammo pile, common/hunter, heal, capability probing |
| `bcdad006` | `c63d07a` | panic, damage, temp_health, revive, give_item, hazards, upgrade packs, witch, tank |

| Action executed | Expected | Observed (game result) |
|---|---|---|
| `test_connection` | `applied` when ready | `applied OK` "gameplay ready" |
| `spawn_item` first_aid_kit, pain_pills x2, ammo, upgradepack_incendiary, upgradepack_explosive | pickups near survivors | each `applied`, actual = requested |
| `spawn_weapon` all 20 allowlisted weapons | 1 each | 20/20 `applied` |
| `spawn_common 10` | 10 common infected | `applied` 10/10 |
| `spawn_infected` hunter x2, witch x1, tank x1 | specials near survivors | each `applied`, actual = requested |
| `panic` | horde | `applied` "horde spawned (ZSpawn mob)" |
| `heal all 100` | full-health and downed survivors skipped | `partial` 4 of 5 |
| `damage random 100` then `revive all` | one survivor downed, then revived | `applied` 1/1, then revived 1 (of 4 targeted) |
| `damage all 40` then `temp_health all 40` | everyone hurt, then temp health | `applied` 4/4, `applied` 4/4 |
| `give_item all molotov` | molotov in each throwable slot | `applied` 4/4 |
| `spawn_hazard` fire x1, explosion x1 | gas can burns, propane explodes | each `applied` 1/1 |
| Cooldown refusal | refused within cooldown | `rejected COOLDOWN` (items/weapons at 5 s; later `item_cooldown=10` via hot reload) |
| Config hot reload | new values within ~1 s, no restart | console `[TikTools] config cargada: item=10 ...`, next item refused |

Restart behavior: the game was relaunched about ten times. Every launch
started a new generation (`g-1` to `g-9`), and the SDK connected to the new
one each time. No stale command ran. A deliberate stale-command replay
across a restart was not attempted.

Findings fixed during the run (all in this repo's history):
- The poll loop never ran: `AddThinkToEnt`, `Entities.CreateByClassname`,
  `EntFireByHandle`, `AcceptInput` and `Director.ForcePanicEvent` do not
  exist in this runtime, and entity output callbacks resolve in the
  entity's own script scope.
- `ZSpawn` refuses the mob type without a position and accepts it near
  the survivors.
- An SDK read could catch `heartbeat.txt` mid-write and report "no
  heartbeat"; the SDKs now re-read briefly.
- The poll timer logged `SCRIPT PERF WARNING` at 3-10 ms per run. A build
  that writes the heartbeat once per second is pending a live check.

Not covered by this run: repeated invocation of the same request id,
map transition, a second human player, prolonged bursts with FPS notes,
Windows 10, and a secondary Steam library.

## Earlier user report

Before the recorded run above, the integration user reported it working in
a local-host session without the required fields. The recorded run now
supersedes that report; it also found that the loop had not been running
until the fixes listed above.

## Feasibility checklist (real Steam L4D2, local campaign)

- [x] Per-addon `scripts/vscripts/director_base_addon.nut` loads with other
      addons enabled, without editing base-game shared scripts (console:
      `Loading addon script ... director_base_addon.nut`).
- [x] `FileToString` reads and `StringToFile` writes under
      `left4dead2/ems/tiktools_l4d2/` (`command.txt`, `result.txt`,
      `heartbeat.txt`, `generation.txt`, `config.txt`). Records may carry a
      trailing NUL, which the SDKs strip. 1 KiB limit and file-lock
      behavior beyond the rename retry are not separately measured.
- [ ] Poll loop persists across map transitions: not yet tested. Cadence is
      0.35 s via a `logic_timer`; per-run cost measured at 3-10 ms
      (`SCRIPT PERF WARNING`), FPS impact not measured.
- [x] `test_connection` round-trips, then item, weapon and common spawns
      with `sv_cheats 0` (`SpawnEntityFromTable`; specials via `ZSpawn`).
- [ ] Mutations run only on the authoritative host: host path verified
      (`host=1`). Menu, lobby, remote and non-host refusals not yet
      exercised live.

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
