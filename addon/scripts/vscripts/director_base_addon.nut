// director_base_addon.nut -- per-addon entry point (Last Stand and later).
//
// The engine loads this file for the addon when present, without touching
// the base game's shared scripts. It hands off to the TikTools bootstrap.
// PROTOTYPE: live auto-load in a Windows Steam session is UNVERIFIED --
// see docs/WINDOWS_TESTING.md. If mapspawn_addon.nut proves more reliable
// during the live gate, this file will change and the gate will record why.

IncludeScript("tiktools_l4d2/bootstrap.nut");
