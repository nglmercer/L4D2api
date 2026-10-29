// spawns.nut -- survivor-safe spawn helpers with verification hooks.
//
// PROTOTYPE: ZSpawn / SpawnEntityFromTable semantics with sv_cheats 0 are
// UNVERIFIED. Capability-gate each spawn below separately during the live
// gate; never report an engine call without in-game confirmation as success.
//
// Survivor selection: nearest alive survivor with navigable space around
// them. Entity ceilings and per-round counters enforced via limits.nut.
// Cleanup: only entities THIS addon created are ever removed.

function TikTools_FindSurvivorAnchor() {
    // PROTOTYPE: survivor iteration + nav-position query chosen at live gate.
    return null;
}

function TikTools_SpawnCommon(count) {
    // Returns { ok, actual, code, diag }. Stub denies until the live gate
    // proves a spawn path with sv_cheats 0.
    return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = "common spawn unverified" };
}

function TikTools_SpawnSpecial(classname, count) {
    return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = "special spawn unverified" };
}

function TikTools_SpawnItem(item, count) {
    return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = "item spawn unverified" };
}

function TikTools_CleanupOwn() {
    // Removes only entities tagged as created by this addon in this round.
}
