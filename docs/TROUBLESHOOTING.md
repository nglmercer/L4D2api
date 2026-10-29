# Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `Server is enforcing consistency for this file: scripts/...` + kick | Workshop mod overrides a stock script | Own server: `sv_consistency 0`. Others' server: `l4d2api-installer check`, quarantine the listed addon, restore after |
| VPK missing | `.vpk` not installed / wrong `addons/` dir | `l4d2api-installer install --vpk <file> --version <x.y.z>`; check Steam library path |
| Invalid installation | Corrupted or foreign bytes installed | `l4d2api-installer verify`; `repair` with a validated payload (arbitrary bytes are rejected, prior install preserved) |
| Failed upgrade | New payload invalid or crash mid-upgrade | Previous version kept automatically; next `install` finishes or rolls back via `install.journal` |
| Stale heartbeat | Game closed, frozen, or restarted; map changed | Handshake again (`connect()`); old commands never replay |
| Incorrect game directory | Discovery picked nothing / secondary library | Pass `--game-dir`; unrelated folders are rejected |
| Game not ready | Menu, loading, no active round | Create a local/listen-hosted campaign; `status()` reads `connected` until the round is live |
| Non-host session | Joined someone else's server | Host the lobby yourself (`NOT_HOST` otherwise) |
| Unsupported action | Gated capability (`spawn_common`, `spawn_infected`) | Check `capabilities()` first; unadvertised actions raise before any write |
| Timeout without confirmed execution | Ambiguous: effect may have happened | Report as unconfirmed; never auto-retry a mutation |
| Concurrent SDK clients | Second writer while one holds the lock | `Busy` (same process) / `Locked` (another process); serialize, don't retry blindly |
| Filesystem permissions | Locked files / antivirus interference | Close the game, retry; `repair`; Windows retries transient sharing violations |
| Protocol mismatch | Addon/client versions differ | Update the integration; `v != 1` records are rejected on both sides |
| Corrupted `command.txt` / `result.txt` | Partial write, manual edit | Ignored safely and overwritten on the next valid write; `verify` the install if persistent |
| Quarantine restoration conflict | A new file landed at the original path | Existing destination preserved; entry stays recorded for manual resolution |
| Duplicate execution fear | Re-polled `command.txt` | Replay protection: each `(generation, id)` executes at most once; out-of-order sequences rejected |

Failed or disconnected integrations never stall other TikTools automations:
reconnect happens on game restart / round / map change. A timeout is
reported as unconfirmed — never as success — and never auto-retried.
