# Installation (Windows 10/11, Steam L4D2)

Scope: this document covers what **this repository** installs (the game
addon + SDKs). The TikTools-app UI button/integration is a separate,
pending integration — nothing here claims a TikTools UI element exists.

## What gets installed

- `left4dead2/addons/tiktools-l4d2api-<version>.vpk` — the game addon.
- `left4dead2/ems/tiktools_l4d2/installed.manifest` — ownership manifest.
- The `left4dead2/ems/tiktools_l4d2/` runtime sandbox is created for the
  game's heartbeat/command/result files.

Only files owned by this integration are ever written or removed.
Unrelated mods are never touched.

## Installing the VPK (release artifacts)

End users need no Rust, Python or Node.js. From a tagged release, download
the versioned `.vpk` (+ SHA-256 + manifest) and `l4d2api-installer.exe`,
verify the hash, then:

```powershell
l4d2api-installer.exe install --game-dir "<game-dir>" --vpk tiktools-l4d2api-0.1.0.vpk --version 0.1.0
l4d2api-installer.exe verify --game-dir "<game-dir>"
```

The payload is validated before it touches the installation (VPK
signature, format, tree, entry ranges, per-entry CRCs, required files,
safe paths). An invalid file is rejected and the previous installation is
left intact. Upgrades are transactional: the new payload is staged and
verified first; the old payload is removed only after the new manifest is
in place; a crash mid-upgrade is finished or rolled back on the next run.

## Configuring cooldowns (hot reload)

On the first round after install, the addon creates
`left4dead2/ems/tiktools_l4d2/config.txt`:

```text
item_cooldown=0
weapon_cooldown=0
common_cooldown=0
special_cooldown=30
tank_cooldown=120
round_cap=60
heal_cooldown=0
damage_cooldown=0
revive_cooldown=0
give_cooldown=0
panic_cooldown=0
hazard_cooldown=0
damage_max=100
```

Cooldowns are seconds between uses of that kind (`0` = none, max `600`);
`heal_cooldown` covers `heal` and `temp_health`. `damage_max` (1-100) caps
the `amount` a single `damage` request may use. Special infected, tanks and
hazards count toward `round_cap`; player effects do not. A config file
created by an older build simply lacks the newer keys, which then use
their defaults; add the lines to change them.
`round_cap` (1-1000) is the total entities this integration may spawn per
round. Edit and save the file while the game is running: the addon re-reads
it about once per second and prints `[TikTools] config cargada: ...` to the
console when the new values apply. A missing or out-of-range key falls back
to its default (and logs `valor invalido`). The file survives upgrades and
uninstall. Refusals still arrive as `COOLDOWN` / `ENTITY_CAP` results.

## Checking installation status

```powershell
l4d2api-installer.exe status --game-dir "<game-dir>"
l4d2api-installer.exe check --game-dir "<game-dir>"
```

`status` reports the installed version, payload validity, quarantine count
and conflict count. `check` scans installed addons for stock-script
overrides that trigger consistency kicks (`[KICK]` vs `[SUSPICIOUS]`; exit
code `1` on kick-level conflicts). Append `--json` for machine-readable
output on `check`, `status` and `verify`.

## Repairing or upgrading

```powershell
l4d2api-installer.exe repair --game-dir "<game-dir>" --vpk tiktools-l4d2api-0.1.0.vpk --version 0.1.0
l4d2api-installer.exe install --game-dir "<game-dir>" --vpk tiktools-l4d2api-0.2.0.vpk --version 0.2.0
```

Reinstalling the same version repairs in place. Upgrading keeps the old
version working until the new payload verifies.

## Uninstalling

```powershell
l4d2api-installer.exe uninstall --game-dir "<game-dir>"
```

Removes only the owned payload and metadata. Foreign mods, configs and
other `.vpk` files survive.

## Quarantine / restore (workshop conflicts, opt-in)

If you join someone else's stock dedicated server with workshop mods that
override stock scripts, the engine kicks you (`Server is enforcing
consistency for this file`). Detection is automatic; quarantine is always
your explicit choice — nothing quarantines automatically:

```powershell
l4d2api-installer.exe quarantine --game-dir "<game-dir>" left4dead2/addons/workshop/2221439970.vpk
l4d2api-installer.exe restore --game-dir "<game-dir>"
```

Restoring never overwrites an existing destination: conflicts stay
recorded for you to resolve. The integration's own `.vpk` can never be
quarantined. Your own server needs `sv_consistency 0` once instead — then
any mod joins with zero client action.

## Manual developer installation

Developers (Rust + Git): every command is identical on Windows and Linux —
the CLI auto-discovers `Program Files (x86)\Steam`; pass `--game-dir`
only for secondary libraries. Quote paths with spaces.

```powershell
cargo run -p l4d2api-pack -- --addon addon --out dist\tiktools-l4d2api-0.1.0.vpk --version 0.1.0
cargo run -p l4d2api-installer -- install --game-dir "<game-dir>" --vpk dist\tiktools-l4d2api-0.1.0.vpk --version 0.1.0
```

No terminal tricks, `.bat`/`.sh`, SourceMod, MetaMod, dedicated server,
manual RCON password, open firewall port, or extra background service. If
discovery fails, pick the game folder manually — unrelated folders are
rejected. Admin rights are never required.
