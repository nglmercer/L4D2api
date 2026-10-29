// spawns.nut -- survivor-safe spawn helpers with verification hooks.
//
// Stock L4D2 world pickups are ordinary entities: weapons use their
// `weapon_*` classnames, items use `weapon_first_aid_kit` and friends, and
// melee uses the `weapon_melee_spawn` pickup entity. We spawn a bounded
// number next to a living survivor, then verify that the engine returned a
// valid handle before reporting `actual`.
//
// Survivor selection: the listen-server host when alive, otherwise the
// first living survivor. Entity ceilings and per-round counters are
// enforced via limits.nut. Only entities this addon creates are ever
// considered for cleanup.

function TikTools_FindSurvivorAnchor() {
    try {
        local host = GetListenServerHost();
        if (host != null && host.IsValid() && host.GetHealth() > 0)
            return host;
        local player = Entities.FindByClassname(null, "player");
        while (player != null) {
            if (player.IsValid() && player.GetHealth() > 0)
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

// Spawn `count` copies of `classname` in a small ring around the anchor.
// Returns { ok, actual, code, diag }; `actual` counts only handles the
// engine reported as valid, never the requested count.
function TikTools_SpawnBunch(classname, count) {
    local anchor = TikTools_FindSurvivorAnchor();
    if (anchor == null)
        return { ok = false, actual = 0, code = "NO_ANCHOR", diag = "no living survivor" };

    local anchorOrigin = anchor.GetOrigin();
    local offsets = [
        Vector(48, 0, 0),
        Vector(-48, 0, 0),
        Vector(0, 48, 0),
        Vector(0, -48, 0),
        Vector(48, 48, 0),
    ];

    local actual = 0;
    for (local i = 0; i < count; i++) {
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
        if (entity != null && entity.IsValid() && entity.GetClassname() == classname)
            actual++;
    }

    if (actual == 0)
        return { ok = false, actual = 0, code = "SPAWN_FAILED", diag = "engine returned no entity" };
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

function TikTools_CleanupOwn() {
    // Removes only entities tagged as created by this addon in this round.
    // Nothing is tagged yet, so this is intentionally a no-op.
}
