// limits.nut -- per-action cooldowns, entity ceilings, per-round counters.
//
// Defaults are conservative; the Rust side enforces its own bounds on top
// and the game reports the verified actual count. Fail closed.

//::TikToolsLimits <- {
//    COMMON_MAX = 10,
//    SPECIAL_MAX = 8,
//    ITEM_MAX = 5,
//    COMMON_COOLDOWN = 5.0,
//    SPECIAL_COOLDOWN = 30.0,
//    TANK_COOLDOWN = 120.0,
//    ITEM_COOLDOWN = 5.0,
//    LAST = { common = -9999.0, special = -9999.0, tank = -9999.0, item = -9999.0 },
//    ROUND_SPAWNED = 0,
//    ROUND_CAP = 60,
//};

function TikTools_CheckLimit(kind, cooldown) {
    local now = Time();
    if (now - ::TikToolsLimits.LAST[kind] < cooldown)
        return false;
    if (::TikToolsLimits.ROUND_SPAWNED >= ::TikToolsLimits.ROUND_CAP)
        return false;
    return true;
}

function TikTools_NoteSpawn(kind, count) {
    ::TikToolsLimits.LAST[kind] = Time();
    ::TikToolsLimits.ROUND_SPAWNED += count;
}

function TikTools_ResetRoundCounters() {
    ::TikToolsLimits.ROUND_SPAWNED = 0;
}
