# Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `addon missing` | `.vpk` not installed / wrong `addons/` dir | Re-run Install; check Steam library path |
| `game closed` | L4D2 not running | Launch via Steam first |
| `game menu` | No active campaign | Create a local/listen-hosted campaign |
| `not host` | Joined someone else's server | Host the lobby yourself |
| `stale heartbeat` | Game restarted or map changed | Handshake again; old commands never replay |
| `protocol mismatch` | Addon/client versions differ | Update the integration |
| `permission denied` | Locked files / AV interference | Close the game, retry; repair |
| `legacy RCON server` | Dedicated SRCDS path in use | Keep `legacy_rcon` or migrate saves deliberately |

Failed or disconnected integrations never stall other TikTools automations:
reconnect happens on game restart / round / map change. A timeout is
reported as unconfirmed — never as success — and never auto-retried.
