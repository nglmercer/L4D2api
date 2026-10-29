# L4D2api

Standalone, installable Left 4 Dead 2 VScript addon (`.vpk`) plus a reusable
Rust IPC client / installer SDK, for driving a **normal Windows Steam local /
listen-hosted L4D2 session** from TikTools — no SourceMod, no MetaMod, no
dedicated server, no RCON password, no terminal.

> **Status: feasibility prototype, `0.1.0`. NOT production ready.**
> The Windows live gate (real Steam L4D2 local campaign: addon auto-load +
> bidirectional file IPC + one visible gameplay action) has **not** been run —
> this workspace is Linux. See [docs/WINDOWS_TESTING.md](docs/WINDOWS_TESTING.md)
> and [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md). Nothing in this repo
> claims a verified Windows gameplay result.

## Layout

```text
addon/                  VScript addon source (packed into the .vpk)
crates/
  l4d2api-protocol/     v1 text-protocol codec, validation, fixtures
  l4d2api-client/       file-IPC session client (single-flight, freshness)
  l4d2api-installer/    Windows Steam discovery + versioned install/repair/uninstall + workshop conflict quarantine
  l4d2api-pack/         deterministic VPK v1 packer (lib + CLI)
tests/fixtures/         canonical valid/malformed/truncated protocol records
docs/                   architecture, protocol, install, security, test plan
```

## Quick start (developers)

```bash
cargo test --locked --workspace
cargo run -p l4d2api-pack -- --addon addon --out dist/tiktools-l4d2api-0.1.0.vpk --version 0.1.0
bash scripts/lint_nut.sh
```

On Windows the `cargo` commands are identical (PowerShell: use
`dist\...` paths); `scripts/lint_nut.sh` runs in Git Bash, which ships
with Git for Windows. End users skip all of this — see Releases below.

## Windows (10/11, Steam L4D2 AppID 550)

**End users: no Rust, no build.** Every tagged release attaches ready-made
artifacts: the versioned `.vpk` (+ SHA-256 + manifest) and the Windows
CLIs `l4d2api-installer.exe` / `l4d2api-pack.exe` (+ checksums), built on
`windows-latest`. Download, verify the hash, and run the commands below
with `l4d2api-installer.exe` instead of `cargo run -p l4d2api-installer --`.

### What the release contains (and what it doesn't)

Included in every tagged release:

- `tiktools-l4d2api-<version>.vpk` (+ SHA-256 + manifest) — the game addon.
- `l4d2api-installer[.exe]` — Steam discovery, versioned
  install/repair/uninstall, workshop conflict check/quarantine/restore.
- `l4d2api-pack[.exe]` — rebuild or verify the `.vpk` yourself.

You provide: Steam + Left 4 Dead 2 (AppID 550) installed, and TikTools.
There is deliberately NO game server in the release and none is needed:
in the local-host flow *your own L4D2 session is the server* — you launch
the game normally and host a local/listen campaign. No SRCDS download, no
RCON password, no port forwarding, no background service, no SourceMod.
(A dedicated SRCDS only belongs to the separate opt-in `legacy_rcon`
path, which lives in TikTools-app and is configured there — not here.)

Developers (Rust + Git): every command below is identical on Windows and
Linux — the CLI auto-discovers `Program Files (x86)\Steam`; pass
`--game-dir` only for secondary libraries. Quote paths with spaces.

```powershell
cargo test --locked --workspace
cargo run -p l4d2api-pack -- --addon addon --out dist\tiktools-l4d2api-0.1.0.vpk --version 0.1.0
cargo run -p l4d2api-installer -- check
cargo run -p l4d2api-installer -- check --game-dir "D:\SteamLibrary\steamapps\common\Left 4 Dead 2"
```

What works on Windows today (verified logic, same binaries):

- **Pack** the addon into a deterministic, validated `.vpk`.
- **Check** your installed addons for stock-script overrides that get you
  kicked from stock servers (`[KICK]`: measured weapon scripts;
  `[SUSPICIOUS]`: other stock `scripts/*.txt`). Exit code `1` when a
  kick-level conflict exists, `0` when clean — scriptable.
- **Quarantine / restore** a colliding addon with one command each
  (moves only that file to an owned backup dir and back; skins/sounds
  mods are never flagged):
  `l4d2api-installer quarantine --game-dir <dir> left4dead2/addons/workshop/2221439970.vpk`
  then `l4d2api-installer restore --game-dir <dir>`.
- **Install** the versioned `.vpk` into the game's `addons/`:
  `l4d2api-installer install --game-dir <dir> --vpk dist\tiktools-l4d2api-0.1.0.vpk --version 0.1.0`.

What does NOT work yet (do not expect gameplay): `spawn_common` and
`spawn_infected` still refuse with `CAPABILITY_UNAVAILABLE` until each
VScript path is proven with `sv_cheats 0`. `spawn_item` and `spawn_weapon`
are implemented against stock world-pickup entities and are pending the
live-gate record in [docs/WINDOWS_TESTING.md](docs/WINDOWS_TESTING.md).
TikTools' `local_ipc` default transport is a separate pending integration.
For real confirmed spawns today, use the opt-in `legacy_rcon`
dedicated-server path documented in TikTools.

## Protocol (v1, summary)

`command.txt` / `result.txt` / `heartbeat.txt` live in the game's
`left4dead2/ems/tiktools_l4d2/` sandbox. Records are `key=value` lines ending
in an `END` sentinel plus an FNV-1a integrity field; max 1 KiB per record.
Full format: [docs/PROTOCOL.md](docs/PROTOCOL.md).

Responses: `applied`, `rejected`, `not_ready`, `partial`, `unknown`.
One outstanding mutation at a time; never auto re-execute after a timeout.

## TikTools side

TikTools (`l4d2.interactive`, same action IDs: `l4d2.test_connection`,
`l4d2.spawn_common`, `l4d2.spawn_infected`, `l4d2.spawn_item`,
`l4d2.spawn_weapon`) consumes the version-pinned `l4d2api-client` crate as
its default `local_ipc` transport.
Legacy SourceMod+RCON stays as opt-in `legacy_rcon`. Integration lands in a
separate TikTools-app PR — this repo never touches TikTools core.

## Limits

- Windows 10/11, Steam L4D2 AppID 550, single-player or locally hosted co-op.
- Remote/community dedicated servers and non-host clients are not controllable.
- `sv_cheats 0` always. No injection, DLL hooks, exe patching, cheats.
