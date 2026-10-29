# Protocol v1

Records are ASCII `key=value` lines, one trailing `crc=<8 hex>` line (FNV-1a
over every preceding byte, each content line followed by `\n`), then an `END`
sentinel. Max 1 KiB per record. Unknown keys, duplicates, wrong versions,
bad checksums and truncated records are rejected — a partial write is never
a command. Both sides (VScript and all three SDKs) enforce the same rules.

Byte-level framing:

1. At most 1024 bytes total.
2. ASCII only.
3. Lines split on `\n`; one trailing newline tolerated, one trailing `\r`
   per line tolerated (host writes).
4. Last line must be exactly `END`; second-to-last must be `crc=` plus 8
   hex digits.
5. `crc` covers every content byte: each content line plus `\n`.
6. Content lines are `key=value` with non-empty keys, no duplicates, no
   unknown keys, all required keys present.
7. `v` must be `1` (protocol v1). Anything else is `UnsupportedVersion`.

The complete golden records live in `tests/fixtures/` — copy those, do not
hand-write checksums. Every example below is the verbatim content of its
fixture file.

## command.txt (SDK -> game)

Verbatim `tests/fixtures/command_valid.txt`:

```text
v=1
seq=7
id=r_12_7
gen=g-9f2c
action=spawn_common
count=5
crc=f7d6eba6
END
```

- `v`: protocol version, currently `1`.
- `seq`: per-session sequence number. The game enforces ordering: an older
  `seq` never executes after a newer accepted one in the same generation.
- `id`: unique request id, `[A-Za-z0-9_-]{1,64}`. Must be unique per
  request — ids are the replay-protection key `(generation, id)`.
- `gen`: game generation pinned at VScript startup; commands from an old
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
- `test_connection` takes no `count`/`class`/`item`/`weapon`; every other
  action takes exactly its documented parameter set — no more, no less.

Other valid commands: `command_test_connection.txt`,
`command_spawn_item.txt`, `command_spawn_weapon.txt`,
`command_spawn_infected.txt` in `tests/fixtures/`.

## result.txt (game -> SDK)

Verbatim `tests/fixtures/result_applied.txt`:

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
crc=7f449c82
END
```

`status`: `applied | rejected | not_ready | partial | unknown`, echoing the
original `seq`/`id`/`gen`. `requested` is what was asked, `actual` is the
*verified* in-game count and never exceeds `requested`:

| Outcome | Status |
|---|---|
| all requested entities verified | `applied` |
| some verified | `partial` (+ `PARTIAL`) |
| none created, known reason | `rejected` |
| action unavailable | `rejected` + `CAPABILITY_UNAVAILABLE` |
| outcome cannot be determined | `unknown` |

`code`: machine refusal (`OK`, `PARTIAL`, `NOT_HOST`, `NOT_IN_ROUND`,
`INVALID_ITEM`, `INVALID_WEAPON`, `INVALID_COUNT`, `NO_ANCHOR`,
`CAPABILITY_UNAVAILABLE`, `ENTITY_CAP`, `COOLDOWN`, `SPAWN_FAILED`,
`OUT_OF_ORDER` semantics, `INTERRUPTED`, ...). `diag`: bounded (<=128)
printable ASCII.

A write is not an execution: the game acknowledges only after observing
the outcome. Timeouts never re-execute — the effect may already have
happened. A reply matches a wait only when `id`, `seq` **and** `gen` all
agree, so a stale reply from a recycled id can never satisfy a new wait.

## heartbeat.txt (game -> SDK)

Verbatim `tests/fixtures/heartbeat_ready.txt`:

```text
v=1
gen=g-9f2c
addon=0.1.0
map=c1m1_hotel
round=1
host=1
ready=1
caps=test_connection,spawn_common,spawn_item
tick=42
END
```

A heartbeat alone never means gameplay ready: `ready=1` additionally
requires host authority and an active round, and clients freshness-check
the file age plus `tick` progression — a valid but old heartbeat reads as
stale, never ready. `caps` advertises per-capability support so unverified
special spawns stay gated (`spawn_infected` is absent from `caps`
until proven). Poll conservatively (~250-500 ms in an active session); no
per-frame disk reads.

## Generation lifecycle

Each VScript startup (launch, map change) mints a new `gen` token from a
persisted counter. Clients pin the generation at handshake and reject any
reply or heartbeat from another generation. The game ignores commands from
old generations, executes each `(generation, id)` at most once, and rejects
out-of-order sequences. Repeated polling of the same `command.txt` never
re-executes: the original acknowledgement stands.

## Idempotency and timeouts

One outstanding mutation at a time per client, one writer per IPC directory
across processes (atomic `command.lock`, held for the whole transaction).
Timeouts distinguish: no heartbeat, stale heartbeat, game not ready,
unsupported capability, request timeout, generation change, filesystem
failure, confirmed rejection, ambiguous execution. Each maps to a typed
SDK error — see [SDK.md](SDK.md) and [API_REFERENCE.md](API_REFERENCE.md).
Ambiguous outcomes are reported as unknown/ambiguous and are never
automatically retried or re-executed.

## Compatibility

v1 is the only wire version. If a breaking change is ever required: explain
why v1 cannot support it, introduce a versioned protocol with explicit
negotiation, keep v1 decoding and fixtures, and never silently reinterpret
existing fields. All three SDKs and the VScript side must agree on
checksums and framing — enforced by the shared fixtures in
`tests/fixtures/` plus `scripts/gen_fixtures.py --check`.

## Cross-process coordination contract

- Exactly one active writer per IPC directory, enforced by creating
  `command.lock` atomically (`O_CREAT|O_EXCL` / `create_new`).
- The lock covers the full request/response transaction and is released on
  every path, including errors and timeouts.
- A lock untouched for longer than the stale window (default 30 s) may be
  reclaimed by exactly one waiter via token re-check + atomic recreate;
  a live holder is never removed.
- No mandatory external service: the lock is a file in the owned IPC dir.
