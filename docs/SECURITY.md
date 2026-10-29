# Security

- No process injection, DLL hooks, anti-cheat bypasses, exe patching, or
  `sv_cheats` shortcuts. VScript only, `sv_cheats 0`.
- No arbitrary `script` / `exec` / convar / console-command / filesystem-path
  / URL fields: commands are strictly typed against allowlists on both sides.
- Request IDs and sequence IDs are freshness/idempotency tooling, not a
  claim of local-process authentication; the threat model assumes the local
  user owns both ends.
- Installer writes only owned files (versioned `.vpk` + owned `ems/` dir),
  validates `left4dead2/` + executable, refuses symlink/path escapes, never
  overwrites shared scripts, configs, DLLs, or other mods. No admin rights,
  no open ports, no background service, no bundled RCON credentials.
- Release provenance: tagged GitHub releases carry the `.vpk`, SHA-256 and
  changelog; update paths verify provenance and checksums before installing.
- Never redistribute proprietary Valve tools or game assets; the packer and
  all addon sources in this repo are original.
