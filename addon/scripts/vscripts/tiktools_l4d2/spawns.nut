// spawns.nut -- survivor-safe spawn helpers with verification hooks.
//
// Stock L4D2 world pickups are ordinary entities: weapons use their
// `weapon_*` classnames, items use `weapon_first_aid_kit` and friends, and
// melee uses the `weapon_melee_spawn` pickup entity. We spawn a bounded
// number next to a living survivor, then verify that the engine returned a
// valid handle before reporting `actual`.
//
// Survivor selection: the listen-server host when alive on the survivor
// team, otherwise the first living survivor. Infected players are never
// valid anchors. Entity ceilings and per-round counters are enforced via
// limits.nut. Only entities this addon creates are ever considered for
// cleanup.

function TikTools_FindSurvivorAnchor() {
    try {
        local host = GetListenServerHost();
        if (host != null && TikTools_IsSurvivor(host))
            return host;
        local player = Entities.FindByClassname(null, "player");
        while (player != null) {
            if (TikTools_IsSurvivor(player))
                return player;
            player = Entities.FindByClassname(player, "player");
        }
    } catch (e) {
        return null;
    }
    return null;
}

function TikTools_ItemClassname(item) {
    switch (item) {
        case "first_aid_kit": return "weapon_first_aid_kit";
        case "pain_pills":    return "weapon_pain_pills";
        case "adrenaline":    return "weapon_adrenaline";
        case "pipe_bomb":     return "weapon_pipe_bomb";
        case "molotov":       return "weapon_molotov";
        case "defibrillator": return "weapon_defibrillator";
        default:              return null;
    }
}

function TikTools_IsWeaponClassname(weapon) {
    local allowed = [
        "weapon_pistol",
        "weapon_pistol_magnum",
        "weapon_smg",
        "weapon_smg_silenced",
        "weapon_smg_mp5",
        "weapon_rifle",
        "weapon_rifle_desert",
        "weapon_rifle_ak47",
        "weapon_rifle_sg552",
        "weapon_shotgun_chrome",
        "weapon_shotgun_spas",
        "weapon_pumpshotgun",
        "weapon_autoshotgun",
        "weapon_hunting_rifle",
        "weapon_sniper_military",
        "weapon_sniper_awp",
        "weapon_sniper_scout",
        "weapon_grenade_launcher",
        "weapon_chainsaw",
        "weapon_melee_spawn",
    ];
    return allowed.find(weapon) != null;
}

// --- bounded bookkeeping for addon-owned entities ---
//
// OWNED holds handles created by this addon in the current round so the
// game can account, inspect and (only when asked) clean up exactly those.
// Handles are re-checked on every pass; map- or player-owned entities are
// never tracked and never touched. The list is bounded (128 entries) and
// cleared on every generation/round reset.

function TikTools_ResetOwned() {
    ::TikTools.OWNED = [];
}

function TikTools_TrackSpawn(entity) {
    try {
        if (::TikTools.OWNED.len() >= 128)
            TikTools_PruneOwned();
        ::TikTools.OWNED.append(entity);
        if (::TikTools.OWNED.len() > 128)
            ::TikTools.OWNED.remove(0);
    } catch (e) { }
}

function TikTools_PruneOwned() {
    try {
        local live = [];
        foreach (entity in ::TikTools.OWNED) {
            try {
                if (entity != null && entity.IsValid())
                    live.append(entity);
            } catch (e) { }
        }
        ::TikTools.OWNED = live;
    } catch (e) {
        ::TikTools.OWNED = [];
    }
}

function TikTools_OwnedCount() {
    TikTools_PruneOwned();
    return ::TikTools.OWNED.len();
}

function TikTools_CleanupOwn() {
    // Removes only entities tagged as created by this addon in this round
    // that are still valid. Never aggressive: entities already picked up or
    // used by players are simply gone from the list, and anything this
    // addon did not create is never considered.
    TikTools_PruneOwned();
    foreach (entity in ::TikTools.OWNED) {
        try {
            if (entity != null && entity.IsValid())
                entity.Kill();
        } catch (e) { }
    }
    ::TikTools.OWNED = [];
}

// Spawn `count` copies of `classname` in a small ring around the anchor.
// Result semantics: all verified -> ok/PARTIAL-free "OK"; some verified ->
// ok with PARTIAL; none -> rejected. `actual` counts only handles the
// engine reported as valid and never exceeds `requested`. One entity's
// failure never stops the remaining spawns and never poisons later calls.
function TikTools_SpawnBunch(classname, count) {
    local anchor = TikTools_FindSurvivorAnchor();
    if (anchor == null)
        return { ok = false, actual = 0, code = "NO_ANCHOR", diag = "no living survivor" };

    local anchorOrigin = null;
    try {
        anchorOrigin = anchor.GetOrigin();
    } catch (e) {
        return { ok = false, actual = 0, code = "NO_ANCHOR", diag = "anchor has no position" };
    }
    local offsets = [
        Vector(48, 0, 0),
        Vector(-48, 0, 0),
        Vector(0, 48, 0),
        Vector(0, -48, 0),
        Vector(48, 48, 0),
    ];

    local actual = 0;
    for (local i = 0; i < count; i++) {
        try {
            local table = {
                origin = anchorOrigin + offsets[i % offsets.len()] + Vector(0, 0, 8),
                angles = Vector(0, 0, 0),
                solid = "6", // Vphysics: pickup behaves like a map-placed item.
            };
            if (classname == "weapon_melee_spawn") {
                table.melee_weapon <- "any";
                table.spawnflags <- 3;
            }
            local entity = SpawnEntityFromTable(classname, table);
            if (entity != null && entity.IsValid() && entity.GetClassname() == classname) {
                actual++;
                TikTools_TrackSpawn(entity);
            }
        } catch (e) { }
        if (actual > count)
            actual = count;
    }

    if (actual == 0)
        return { ok = false, actual = 0, code = "SPAWN_FAILED", diag = "engine returned no entity" };
    if (actual < count)
        return { ok = true, actual = actual, code = "PARTIAL", diag = "spawned " + actual + " of " + count + " " + classname };
    return { ok = true, actual = actual, code = "OK", diag = "spawned " + actual + " " + classname };
}

function TikTools_SpawnCommon(count) {
    // ZSpawn semantics with sv_cheats 0 are still unproven; keep gated.
    return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = "common spawn unverified" };
}

function TikTools_SpawnSpecial(classname, count) {
    return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = "special spawn unverified" };
}

function TikTools_SpawnItem(item, count) {
    local classname = TikTools_ItemClassname(item);
    if (classname == null)
        return { ok = false, actual = 0, code = "INVALID_ITEM", diag = "unknown item" };
    return TikTools_SpawnBunch(classname, count);
}

function TikTools_SpawnWeapon(weapon, count) {
    if (!TikTools_IsWeaponClassname(weapon))
        return { ok = false, actual = 0, code = "INVALID_WEAPON", diag = "unknown weapon" };
    return TikTools_SpawnBunch(weapon, count);
}
