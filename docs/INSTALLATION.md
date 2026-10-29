# Installation (Windows 10/11, Steam L4D2)

1. Install TikTools, open it, select **Left 4 Dead 2**.
2. Click **Install integration**. TikTools finds the Steam install
   (primary and secondary libraries via `libraryfolders.vdf`, AppID 550 via
   `appmanifest_550.acf`), validates `left4dead2/` plus the game executable,
   and installs the versioned `.vpk` under `left4dead2/addons/` — creating
   only the owned `left4dead2/ems/tiktools_l4d2/` files.
3. If you join someone else's stock dedicated server with workshop mods,
   TikTools scans your addons automatically and offers one-click quarantine
   for any that override stock scripts (they would kick you); your mods
   are restored afterwards. Mods that only change skins/sounds are never
   touched. Your own server needs `sv_consistency 0` once — then any mod
   joins with zero client action.
4. Launch L4D2 normally through Steam, create a local/listen-hosted campaign.
   TikTools shows **Connected — gameplay ready**.
5. Run an automation; TikTools reports the game-observed result.

No terminal, `.bat`/`.sh`, SourceMod, MetaMod, dedicated server, manual RCON
password, open firewall port, or extra background service. If discovery
fails, pick the game folder manually — unrelated folders are rejected.
Update/repair replaces only owned files; uninstall removes only owned files
and never touches other mods. A running game is asked to restart only when
required; admin rights are never required (permission failures explain the
specific blocker in the UI).
