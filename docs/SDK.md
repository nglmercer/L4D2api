# SDK guide

Three SDKs, one contract: the file-IPC protocol in [PROTOCOL.md](PROTOCOL.md).
`l4d2api-protocol` (Rust) is the reference codec; the Python and
JavaScript/TypeScript codecs are twins pinned to the same fixtures.
There are no HTTP endpoints anywhere — do not write integrations against
imaginary REST URLs.

| SDK | Package | Requires | Install |
|---|---|---|---|
| Rust | crates `l4d2api-client` + `l4d2api-protocol` | Rust 1.85+ | workspace / crates.io pin |
| Python | `l4d2api` | Python 3.10+, stdlib only | `pip install -e ./sdk/python` |
| JavaScript/TypeScript | `l4d2api` | Node.js >= 18 | `cd sdk/javascript && npm install && npm run build` |

## Shared lifecycle (all SDKs)

1. Point the client at the game (`gameDir`) or directly at the IPC dir.
2. `connect()` — pins the live generation.
3. `status()` — `disconnected / connected / stale / not_ready /
   gameplay_ready`. Only `gameplay_ready` means playable.
4. `capabilities()` — act only on advertised actions.
5. Mutations (`spawn_*`) — unique request ids, capability-checked, typed
   `ActionResult` (`requestId`, `generation`, `status`, `code`,
   `requested`, `actual`, `diagnostic`). `requestId` is the SDK name for
   the wire field `id`.
6. Errors are typed per failure: no heartbeat, stale heartbeat, not
   ready, unsupported capability, timeout, generation change, filesystem
   failure, confirmed rejection, ambiguous execution. Handle each case;
   never retry a timed-out mutation automatically.
7. `close()` — releases per-instance state (the write lock is always
   released per request).

Tuning: `timeout` (how long to wait for the game's reply),
`pollInterval` (reply/heartbeat poll cadence), `heartbeatTimeout`
(freshness window, default 5 s), `lockStaleAfter` (dead-holder reclaim
window, default 30 s).

## Rules every SDK follows

- Mutations check the advertised capability *before* writing anything.
- One writer per IPC directory (`command.lock`, atomic create, held for
  the whole transaction, released on every path).
- Atomic `command.txt` replacement (temp file in the same directory +
  rename); no partially visible host-written commands.
- Heartbeats older than the freshness window never read as ready.
- A stale reply (wrong `id`/`seq`/`gen`) never satisfies a new wait.
- Generation changes invalidate the session — handshake again.
- No mandatory server, no background process, no gameplay auto-retry.

## Which SDK to use

Use the Rust crates inside TikTools (version-pinned `local_ipc`
transport). Use Python for scripting and test harnesses, JavaScript for
Node-based tooling. A browser-only integration cannot work — browsers
cannot reach the game's files; a trusted local bridge would be needed
first, and none exists yet.
