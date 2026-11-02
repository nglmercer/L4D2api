// director_base_addon.nut -- per-addon entry point (Last Stand and later).
//
// The engine loads this file for the addon when present, without touching
// the base game's shared scripts. It hands off to the TikTools bootstrap.
// Live auto-load verified in a Windows Steam local-host session (L4D2 build
// 10097); see docs/WINDOWS_TESTING.md for the recorded run.

IncludeScript("tiktools_l4d2/bootstrap.nut");
