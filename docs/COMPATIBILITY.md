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
