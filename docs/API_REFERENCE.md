# API reference

Wire fields are in [PROTOCOL.md](PROTOCOL.md). This page maps them to the
three SDK surfaces. `requestId` is the SDK name for the wire field `id`
everywhere.

## Session (all SDKs)

| Operation | Rust (`Client`) | Python (`L4D2Client`) | JS (`L4D2Client`) |
|---|---|---|---|
| Create | `Client::new(&ipc_dir, timeout, poll_interval)` | `L4D2Client(game_dir=..., timeout=...)` | `new L4D2Client({ gameDir, timeoutMs })` |
| Handshake | `handshake()` / `connect()` | `connect()` | `connect()` |
| Health | `status()` → `Health` | `status()` → `SessionStatus` | `status()` → `SessionStatus` |
| Heartbeat | `heartbeat()` / `heartbeat_fresh()` | `heartbeat()` / `heartbeat_fresh()` | `heartbeat()` / `heartbeatFresh()` |
| Capabilities | `capabilities()` / `advertised_capabilities()` | `capabilities()` | `capabilities()` |
| Readiness gate | `ensure_ready_for(action)` | `ensure_ready_for(action)` | `ensureReadyFor(action)` |
| Unique id | `next_id()` | `next_id()` | `nextId()` |
| Send | `send(id, action)` / `send_auto(action)` | `send(request_id, action, **params)` | `send(requestId, action, params?, signal?)` |
| Close | (no-op; per-request cleanup) | `close()` / context manager | `close()` |
| Options | `set_heartbeat_timeout`, `set_lock_stale_after` | constructor kwargs | `ClientOptions` fields |

Health values: `Disconnected / Connected / Stale / NotReady /
GameplayReady` (Rust `Health`; Python/JS `disconnected / connected /
stale / not_ready / gameplay_ready`).

## Actions (all SDKs)

| Operation | Rust | Python | JS |
|---|---|---|---|
| ping | `test_connection()` | `test_connection()` | `testConnection()` |
| item | `spawn_item(item, count)` | `spawn_item(item, count)` | `spawnItem(item, count)` |
| weapon | `spawn_weapon(weapon, count)` | `spawn_weapon(weapon, count)` | `spawnWeapon(weapon, count)` |
| common | `spawn_common(count)` | `spawn_common(count)` | `spawnCommon(count)` |
| special | `spawn_infected(class, count)` | `spawn_infected(class_name, count)` | `spawnInfected(className, count)` |

Every action mints a unique request id and checks the advertised
capability first (`test_connection` is always allowed; the rest require
their capability token). Counts are validated against the protocol maxima
before any write.

## Results

`TypedResponse` (Rust) / `ActionResult` (Python/JS): `status`
(`applied | rejected | not_ready | partial | unknown`), `code`, `requested`,
`actual` (verified, never exceeds `requested`), `diag`/`diagnostic`,
`generation`, plus `request_id`/`requestId`. Rejections arrive as values,
not throws — only transport/session failures throw.

## Errors

| Case | Rust | Python | JS |
|---|---|---|---|
| protocol | `Error::Protocol` | `ProtocolError` | `ProtocolError` |
| filesystem | `Error::Io` | `TransportError` | `TransportError` |
| in-flight | `Error::Busy` | `BusyError` | `BusyError` |
| foreign writer | `Error::Locked` | `LockedError` | `LockedError` |
| no heartbeat | `Error::NoHeartbeat` | `NoHeartbeatError` | `NoHeartbeatError` |
| stale heartbeat | `Error::StaleHeartbeat` | `StaleHeartbeatError` | `StaleHeartbeatError` |
| not ready | `Error::NotReady` | `NotReadyError` | `NotReadyError` |
| gated action | `Error::UnsupportedCapability` | `UnsupportedCapabilityError` | `UnsupportedCapabilityError` |
| timeout | `Error::Timeout` | `TimeoutError` | `TimeoutError` |
| generation drift | `Error::StaleGeneration` | `StaleGenerationError` | `StaleGenerationError` |
| game said no | `TypedResponse{rejected,…}` | `ActionResult(status="rejected")` | `ActionResult(status="rejected")` |

A timeout leaves the outcome unknown: the game may already have acted.
Never retry a timed-out mutation automatically. (`GameRejected` exists in
Rust only for explicit fail-fast call sites; the normal path returns the
typed rejection above.)

## Installer library (`l4d2api-installer`)

`validate_game_dir`, `find_l4d2_installs`, `install` (validated,
transactional), `repair`, `verify`, `uninstall`, `status`,
`scan_addon_conflicts`, `quarantine_addon`, `restore_quarantined`,
`quarantined_addons`, `join_owned`, `sha256_hex`. CLI:
`l4d2api-installer check|status|verify|install|repair|uninstall|quarantine|restore`
(`--json` for `check`/`status`/`verify`).

## Pack library (`l4d2api-pack`)

`build_vpk`, `validate_vpk`, `read_tree` / `read_tree_paths`, `collect_files`,
`write_release`, `crc32`, `sha256_hex`.
