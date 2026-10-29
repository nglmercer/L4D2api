# Architecture

```text
TikTok LIVE event
  -> TikTools core / workflows / behavior
  -> l4d2.interactive (existing process plugin; same action IDs)
  -> L4D2api Rust IPC client (version-pinned crate)
  <-> command.txt / result.txt / heartbeat.txt  (atomic host writes)
  <-> left4dead2/ems/tiktools_l4d2/ (host installation)
  -> VScript addon loaded in the host's L4D2 session
  -> game action + observed response
```

## Why no in-game HTTP server

L4D2 runs Source Engine VScript, not Unity/BepInEx. VScript cannot open
HTTP sockets, so PEAKapi/REPOapi's `HttpListener` pattern does not transfer.
The transport is bounded command / acknowledgement / heartbeat files in the
game's `ems/` sandbox. An optional loopback REST facade may be added later
*outside* the game for other apps; it must never be required for TikTools.

## Crates

- `l4d2api-protocol` — codec, validation, shared fixtures. No I/O.
- `l4d2api-client` — session discovery, single-flight send, freshness
  checks, timeouts, typed responses, heartbeat polling.
- `l4d2api-installer` — Windows Steam discovery (registry-adjacent config,
  `libraryfolders.vdf`, `appmanifest_550.acf`), versioned
  install/upgrade/repair/uninstall of the `.vpk` plus owned `ems/` files.
- `l4d2api-pack` — deterministic VPK v1 writer + validator (lib and CLI).

## Addon modules (`addon/scripts/vscripts/tiktools_l4d2/`)

`bootstrap` (startup, generation, poll loop), `protocol` (framing, strict
parse, single-flight dispatch), `state` (host/round authority — fail closed),
`limits` (cooldowns, entity ceilings, per-round counters), `spawns`
(survivor-safe spawn helpers), `actions` (existing TikTools action IDs only).

## Legacy transport

SourceMod+RCON (`integrations/l4d2/adapter/tiktools_l4d2.sp` in TikTools-app)
stays as opt-in `transportMode = "legacy_rcon"` for known dedicated-server
users. Default is `local_ipc`. Nothing is removed until compatibility is
demonstrated on both paths.
