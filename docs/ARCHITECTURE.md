# Architecture

```text
TikTok LIVE event
  -> TikTools core / workflows / behavior
  -> l4d2.interactive (existing process plugin; same action IDs)
  -> L4D2api SDK (Rust crate, Python or JS package; version-pinned)
  <-> command.txt / result.txt / heartbeat.txt (+ command.lock)  (atomic host writes)
  <-> left4dead2/ems/tiktools_l4d2/ (host installation)
  -> VScript addon loaded in the host's L4D2 session
  -> game action + observed response
```

The three language SDKs speak the same file-IPC protocol — there is no
server, no REST layer, no background process. `l4d2api-protocol` (Rust) is
the reference codec; the Python (`sdk/python`) and JavaScript/TypeScript
(`sdk/javascript`) codecs are twins pinned to the same fixtures in
`tests/fixtures/`. All SDK clients implement the same session lifecycle:

1. `connect()` / handshake pins the live generation.
2. Every request mints a unique id, acquires the cross-process write lock,
   replaces `command.txt` atomically, and waits for the matching reply
   (`id` + `seq` + `gen`), then releases the lock on every path.
3. Heartbeats are freshness-checked (`stale` beats `ready`); capabilities
   gate every mutation; timeouts are ambiguous and never retried.
4. A generation change invalidates the session; the caller handshakes again.

Session health everywhere: `disconnected / connected / stale / not_ready /
gameplay_ready`. Single-writer model: one outstanding mutation per client,
one active writer per IPC directory across processes. An optional loopback
REST facade may be added later *outside* the game for other apps; it must
never be required for TikTools.

## Why no in-game HTTP server

L4D2 runs Source Engine VScript, not Unity/BepInEx. VScript cannot open
HTTP sockets, so an `HttpListener` pattern does not transfer. The transport
is bounded command / acknowledgement / heartbeat files in the game's `ems/`
sandbox — see [PROTOCOL.md](PROTOCOL.md).

## Crates

- `l4d2api-protocol` — codec, validation, shared fixtures. No I/O.
- `l4d2api-client` — session discovery, single-flight + cross-process
  send, freshness checks, typed readiness, typed errors, ergonomic actions.
- `l4d2api-installer` — Windows Steam discovery (registry-adjacent config,
  `libraryfolders.vdf`, `appmanifest_550.acf`), validated, transactional,
  ownership-scoped install/upgrade/repair/uninstall of the `.vpk` plus
  owned `ems/` metadata; workshop conflict scan/quarantine/restore.
- `l4d2api-pack` — deterministic VPK v1 writer + validator (lib and CLI).

## SDKs

- `sdk/python` — `l4d2api` package (stdlib only, 3.10+), same lifecycle.
- `sdk/javascript` — `l4d2api` package (Node >= 18, TypeScript-first),
  Promise-based, `AbortSignal` cancels the wait only.
- `examples/rust` — runnable Rust session against the real crates.

Guide: [SDK.md](SDK.md). API: [API_REFERENCE.md](API_REFERENCE.md).

## Addon modules (`addon/scripts/vscripts/tiktools_l4d2/`)

`bootstrap` (startup, generation, poll loop), `protocol` (strict framing,
bounded `(generation, id)` replay protection, guarded dispatch), `state`
(host/round authority — fail closed), `limits` (cooldowns, entity ceilings
with requested-count accounting, per-round counters), `spawns`
(survivor-team-safe spawn helpers, verified counts, bounded owned-entity
tracking), `actions` (existing TikTools action IDs only, accurate
applied/partial/rejected/unknown reporting).

## Legacy transport

SourceMod+RCON (`integrations/l4d2/adapter/tiktools_l4d2.sp` in TikTools-app)
stays as opt-in `transportMode = "legacy_rcon"` for known dedicated-server
users. Default is `local_ipc`. Nothing is removed until compatibility is
demonstrated on both paths.

## On-disk layout

Game VScript sandbox (runtime): `left4dead2/ems/tiktools_l4d2/` —
`command.txt`, `result.txt`, `heartbeat.txt`, `generation.txt`,
`command.lock`. Installer-owned metadata (same directory):
`installed.manifest`, `quarantine.manifest`, `quarantine/`, plus
`install.journal` / `installed.manifest.bak` crash-recovery temporaries.
Addon payload: `left4dead2/addons/tiktools-l4d2api-<version>.vpk`. Reads
accept the pre-0.2 manifest/quarantine locations; installs migrate forward.
