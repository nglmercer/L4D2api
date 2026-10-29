# Protocol v1

Records are ASCII `key=value` lines, one trailing `crc=<8 hex>` line (FNV-1a
over every preceding byte, each content line followed by `\n`), then an `END`
sentinel. Max 1 KiB per record. Unknown keys, duplicates, wrong versions,
bad checksums and truncated records are ignored — a partial write is never
a command.

## command.txt (TikTools -> game)

```text
v=1
seq=7
id=r_12_7
gen=g-9f2c
action=spawn_common
count=5
crc=<hex>
END
```

- `v`: protocol version, currently `1`.
- `seq`: monotonically increasing host-side sequence per session.
- `id`: host-side request id, `[A-Za-z0-9_-]{1,64}`.
- `gen`: game generation emitted at VScript startup; commands from an old
  generation never execute after relaunch/map change.
- `action`: `test_connection | spawn_common | spawn_infected | spawn_item | spawn_weapon`.
- `count`: common 1-10; tank exactly 1; other specials 1-8; items 1-5;
  weapons 1-5.
- `class`: infected allowlist (`tank witch smoker boomer hunter spitter
  jockey charger`); `item`: item allowlist (`first_aid_kit pain_pills
  adrenaline pipe_bomb molotov defibrillator`); `weapon`: stock weapon
  classname allowlist (`weapon_pistol`, `weapon_pistol_magnum`, `weapon_smg`,
  `weapon_smg_silenced`, `weapon_smg_mp5`, `weapon_rifle`,
  `weapon_rifle_desert`, `weapon_rifle_ak47`, `weapon_rifle_sg552`,
  `weapon_shotgun_chrome`, `weapon_shotgun_spas`, `weapon_pumpshotgun`,
  `weapon_autoshotgun`, `weapon_hunting_rifle`, `weapon_sniper_military`,
  `weapon_sniper_awp`, `weapon_sniper_scout`, `weapon_grenade_launcher`,
  `weapon_chainsaw`, `weapon_melee_spawn`).
- `test_connection` takes no `count`/`class`/`item`/`weapon`.

## result.txt (game -> TikTools)

```text
v=1
seq=7
id=r_12_7
gen=g-9f2c
status=applied
code=OK
requested=5
actual=5
diag=spawned near survivor Ellis
END
```

`status`: `applied | rejected | not_ready | partial | unknown` with the
original `id`/`gen`. `code`: machine refusal (`NOT_HOST`, `NOT_IN_ROUND`,
`INVALID_ITEM`, `INVALID_WEAPON`, `NO_ANCHOR`, `CAPABILITY_UNAVAILABLE`,
`ENTITY_CAP`, `COOLDOWN`, `SPAWN_FAILED`, ...). `actual` is the *verified*
in-game count and never exceeds `requested`. `diag`: bounded (<=128)
printable ASCII.

A write is not an execution: the game acknowledges only after observing the
outcome. Timeouts never re-execute — the effect may already have happened.

## heartbeat.txt (game -> TikTools)

```text
v=1
gen=g-9f2c
addon=0.1.0
map=c1m1_hotel
round=1
host=1
ready=1
caps=test_connection,spawn_common,spawn_item,spawn_weapon
tick=42
END
```

A heartbeat alone never means gameplay ready: `ready=1` additionally
requires host authority and an active round. `caps` advertises per-capability
support so unverified spawns stay gated. Poll conservatively (~250-500 ms in
an active session); no per-frame disk reads.

Canonical golden fixtures live in `tests/fixtures/`.
