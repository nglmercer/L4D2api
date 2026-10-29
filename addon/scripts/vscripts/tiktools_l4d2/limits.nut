// limits.nut -- per-action cooldowns, entity ceilings, per-round counters.
//
// Defaults are conservative; the Rust side enforces its own bounds on top
// and the game reports the verified actual count. Fail closed.

::TikToolsLimits <- {
    COMMON_MAX = 10,
    SPECIAL_MAX = 8,
    ITEM_MAX = 5,
    COMMON_COOLDOWN = 5.0,
    SPECIAL_COOLDOWN = 30.0,
    TANK_COOLDOWN = 120.0,
    ITEM_COOLDOWN = 5.0,
    WEAPON_COOLDOWN = 5.0,
    LAST = { common = -9999.0, special = -9999.0, tank = -9999.0, item = -9999.0, weapon = -9999.0 },
    ROUND_SPAWNED = 0,
    ROUND_CAP = 60,
};

// Returns null when the spawn may proceed, else a refusal code.
// Accounts the requested count against the remaining round capacity so a
// request can never push the round total past ROUND_CAP.
function TikTools_CheckLimit(kind, cooldown, requested) {
    local now = Time();
    if (now - ::TikToolsLimits.LAST[kind] < cooldown)
        return "COOLDOWN";
    if (::TikToolsLimits.ROUND_SPAWNED + requested > ::TikToolsLimits.ROUND_CAP)
        return "ENTITY_CAP";
    return null;
}

function TikTools_NoteSpawn(kind, count) {
    ::TikToolsLimits.LAST[kind] = Time();
    ::TikToolsLimits.ROUND_SPAWNED += count;
    if (::TikToolsLimits.ROUND_SPAWNED > ::TikToolsLimits.ROUND_CAP)
        ::TikToolsLimits.ROUND_SPAWNED = ::TikToolsLimits.ROUND_CAP;
}

function TikTools_ResetRoundCounters() {
    ::TikToolsLimits.ROUND_SPAWNED = 0;
    ::TikToolsLimits.LAST.common = -9999.0;
    ::TikToolsLimits.LAST.special = -9999.0;
    ::TikToolsLimits.LAST.tank = -9999.0;
    ::TikToolsLimits.LAST.item = -9999.0;
    ::TikToolsLimits.LAST.weapon = -9999.0;
}
