# l4d2api (JavaScript/TypeScript)

File-IPC client SDK for the L4D2api game addon. There are no HTTP
endpoints: the client talks to the game through the
`left4dead2/ems/tiktools_l4d2/` files (`command.txt`, `result.txt`,
`heartbeat.txt`, plus the `command.lock` single-writer lock).

A browser-only integration cannot work: the browser has no access to the
game's files. If a browser UI is ever added, it needs an explicitly
enabled, trusted local bridge -- that bridge does not exist yet and is
not part of this package.

## Install / build

```bash
cd sdk/javascript
npm install
npm run build
npm test
```

Requires modern Node.js (>= 18) with native filesystem support.

## Use

```javascript
import { L4D2Client } from "l4d2api";

const client = new L4D2Client({
  gameDir: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Left 4 Dead 2",
  timeoutMs: 5000,
});

try {
  await client.connect();
  console.log("Status:", (await client.status()).health);
  const capabilities = await client.capabilities();
  if (capabilities.includes("spawn_item")) {
    const result = await client.spawnItem("first_aid_kit", 2);
    console.log(result.status, result.actual);
  }
} finally {
  await client.close();
}
```

Runnable examples live in `examples/javascript/`: `basic.mjs`,
`spawn_items.mjs`, `monitor.mjs` (run against `sdk/javascript/dist/`).

## Rules

- Mutations check the advertised capability first; unadvertised actions
  throw `UnsupportedCapabilityError` without writing anything.
- Every request mints a unique id; stale results never satisfy a new wait.
- Timeouts (`TimeoutError`) leave the outcome unknown: never retry a
  timed-out mutation automatically. Passing an `AbortSignal` cancels the
  wait only, never the game effect.
- Only one writer per IPC directory: a second process gets `LockedError`.
- Heartbeats older than `heartbeatTimeoutMs` (default 5000 ms) read as stale.

## Tests

`npm test` builds with `tsc` and runs `node:test` suites in `tests/`:
record encoding/decoding, FNV-1a parity, duplicate prevention,
cross-generation rejection, stale heartbeats, lock contention, atomic
replacement and timeout behavior -- all pinned to the shared fixtures in
`tests/fixtures/`.
