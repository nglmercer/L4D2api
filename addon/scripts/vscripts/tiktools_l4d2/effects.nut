// effects.nut -- player effects (heal, temp health, damage, revive, give),
// director panic, special infected and hazards.
//
// This VScript runtime lacks several "standard" functions (seen live:
// Entities.CreateByClassname, EntFireByHandle, AcceptInput), so:
// - TikTools_ComputeCaps() advertises only actions whose engine functions
//   exist, and the SDKs refuse unadvertised actions before writing;
// - every engine call is guarded, and a missing function becomes
//   CAPABILITY_UNAVAILABLE instead of an interrupted dispatch.
//
// Per-target helpers only call methods on the player handle and root-table
// engine functions, so they are safe to call through a function reference.

// --- capability probing ------------------------------------------------------

function TikTools_HasMember(obj, name) {
    try {
        return obj != null && (name in obj);
    } catch (e) {
        return false;
    }
}

function TikTools_HasGlobal(name) {
    try {
        return name in getroottable();
    } catch (e) {
        return false;
    }
}

function TikTools_HasDirector(name) {
    try {
        return TikTools_HasMember(Director, name);
    } catch (e) {
        return false;
    }
}

function TikTools_PlayerHas(host, probe, name) {
    return !probe || TikTools_HasMember(host, name);
}

// Recomputed for every heartbeat: cheap membership checks only.
function TikTools_ComputeCaps() {
    local caps = ["test_connection", "spawn_common", "spawn_item", "spawn_weapon"];
    if (TikTools_HasGlobal("ZSpawn"))
        caps.append("spawn_infected");
    if (TikTools_HasDirector("ForcePanicEvent"))
        caps.append("panic");

    local host = null;
    try { host = GetListenServerHost(); } catch (e) { host = null; }
    // If membership checks do not see methods on this runtime (GetHealth is
    // known to work), stay optimistic: guarded calls still refuse cleanly.
    local probe = host != null && TikTools_HasMember(host, "GetHealth");
    if (TikTools_PlayerHas(host, probe, "SetHealth"))
        caps.append("heal");
    if (TikTools_PlayerHas(host, probe, "SetHealthBuffer")
        && TikTools_PlayerHas(host, probe, "GetHealthBuffer"))
        caps.append("temp_health");
    if (TikTools_PlayerHas(host, probe, "TakeDamage")
        || TikTools_PlayerHas(host, probe, "SetHealth"))
        caps.append("damage");
    if (TikTools_PlayerHas(host, probe, "ReviveFromIncap")
        && TikTools_PlayerHas(host, probe, "IsIncapacitated"))
        caps.append("revive");
    if (TikTools_PlayerHas(host, probe, "GiveItem"))
        caps.append("give_item");
    if (TikTools_PlayerHas(host, probe, "TakeDamage"))
        caps.append("spawn_hazard");

    local text = "";
    foreach (i, cap in caps)
        text += (i == 0 ? "" : ",") + cap;
    if (text != ::TikTools.CAPS) {
        ::TikTools.CAPS = text;
        printl("[TikTools] capacidades: " + text);
    }
}

// --- helpers -----------------------------------------------------------------

function TikTools_IsMissingApi(e) {
    return typeof e == "string" && e.find("does not exist") != null;
}

// Results carry diag text: printable ASCII only, bounded.
function TikTools_Diag(text) {
    local out = "";
    local s = text == null ? "" : text.tostring();
    for (local i = 0; i < s.len() && out.len() < 100; i++) {
        local c = s[i];
        out += (c >= 32 && c <= 126) ? c.tochar() : "_";
    }
    return out;
}

// Living survivors, including incapacitated ones (they still have health).
function TikTools_Survivors() {
    local out = [];
    try {
        local p = Entities.FindByClassname(null, "player");
        while (p != null) {
            if (TikTools_IsSurvivor(p))
                out.append(p);
            p = Entities.FindByClassname(p, "player");
        }
    } catch (e) { }
    return out;
}

function TikTools_RandomIndex(n) {
    try {
        return RandomInt(0, n - 1);
    } catch (e) {
        return (Time() * 1000).tointeger() % n;
    }
}

function TikTools_Targets(target) {
    local all = TikTools_Survivors();
    if (target == "all")
        return all;
    if (target == "host") {
        try {
            local h = GetListenServerHost();
            if (h != null && TikTools_IsSurvivor(h))
                return [h];
        } catch (e) { }
        return [];
    }
    if (target == "random" && all.len() > 0)
        return [all[TikTools_RandomIndex(all.len())]];
    return [];
}

function TikTools_IsDowned(p) {
    try {
        return p.IsIncapacitated();
    } catch (e) {
        return false;
    }
}

// Run `fn(player, args)` on each target; `fn` returns true when it changed
// something. Never throws.
function TikTools_ForTargets(target, label, fn, args) {
    local targets = TikTools_Targets(target);
    if (targets.len() == 0)
        return { ok = false, requested = 0, actual = 0, code = "NO_TARGET",
                 diag = "no living survivor for target " + target };
    local actual = 0;
    local missing = null;
    foreach (p in targets) {
        try {
            if (fn(p, args))
                actual++;
        } catch (e) {
            if (TikTools_IsMissingApi(e)) {
                missing = e;
                break;
            }
        }
    }
    local n = targets.len();
    if (actual == 0 && missing != null)
        return { ok = false, requested = n, actual = 0, code = "CAPABILITY_UNAVAILABLE",
                 diag = TikTools_Diag(missing) };
    if (actual == 0)
        return { ok = false, requested = n, actual = 0, code = "NO_EFFECT",
                 diag = label + ": no survivor changed" };
    return { ok = true, requested = n, actual = actual,
             code = actual < n ? "PARTIAL" : "OK",
             diag = label + " " + actual + " of " + n };
}

// --- per-target effects --------------------------------------------------------

function TikTools_FxHeal(p, args) {
    if (TikTools_IsDowned(p))
        return false;
    local max = p.GetMaxHealth();
    local hp = p.GetHealth();
    if (hp >= max)
        return false;
    local next = hp + args.amount;
    if (next > max)
        next = max;
    p.SetHealth(next);
    // Keep permanent + temporary health within the maximum.
    try {
        local buffer = p.GetHealthBuffer();
        if (next + buffer > max)
            p.SetHealthBuffer(max - next);
    } catch (e) { }
    return true;
}

function TikTools_FxTempHealth(p, args) {
    if (TikTools_IsDowned(p))
        return false;
    local max = p.GetMaxHealth();
    local hp = p.GetHealth();
    local buffer = p.GetHealthBuffer();
    local room = max - hp - buffer;
    if (room <= 0)
        return false;
    local add = args.amount < room ? args.amount : room;
    p.SetHealthBuffer(buffer + add);
    return true;
}

function TikTools_FxDamage(p, args) {
    try {
        local world = Entities.FindByClassname(null, "worldspawn");
        p.TakeDamage(args.amount.tofloat(), 0, world);
        return true;
    } catch (e) {
        if (!TikTools_IsMissingApi(e))
            throw e;
    }
    // Fallback without TakeDamage: lower health, never below 1.
    local hp = p.GetHealth();
    local next = hp - args.amount;
    if (next < 1)
        next = 1;
    if (next == hp)
        return false;
    p.SetHealth(next);
    return true;
}

function TikTools_FxRevive(p, args) {
    if (!p.IsIncapacitated())
        return false;
    p.ReviveFromIncap();
    return true;
}

function TikTools_FxGive(p, args) {
    p.GiveItem(args.item);
    return true;
}

// --- actions -----------------------------------------------------------------

function TikTools_Heal(target, amount) {
    return TikTools_ForTargets(target, "healed", TikTools_FxHeal, { amount = amount });
}

function TikTools_TempHealth(target, amount) {
    return TikTools_ForTargets(target, "temp health", TikTools_FxTempHealth, { amount = amount });
}

function TikTools_Damage(target, amount) {
    return TikTools_ForTargets(target, "damaged", TikTools_FxDamage, { amount = amount });
}

function TikTools_Revive(target) {
    return TikTools_ForTargets(target, "revived", TikTools_FxRevive, {});
}

// Protocol item names map to GiveItem names: weapons drop the `weapon_`
// prefix (the protocol already uses short names for give_item).
function TikTools_GiveItem(target, item) {
    return TikTools_ForTargets(target, "gave " + item + " to", TikTools_FxGive, { item = item });
}

function TikTools_Panic() {
    try {
        Director.ForcePanicEvent();
        return { ok = true, requested = 1, actual = 1, code = "OK", diag = "panic event started" };
    } catch (e) {
        return { ok = false, requested = 1, actual = 0,
                 code = TikTools_IsMissingApi(e) ? "CAPABILITY_UNAVAILABLE" : "SPAWN_FAILED",
                 diag = TikTools_Diag(e) };
    }
}

// --- special infected (ZSpawn) -------------------------------------------------

function TikTools_ZombieType(cls) {
    switch (cls) {
        case "smoker":  return 1;
        case "boomer":  return 2;
        case "hunter":  return 3;
        case "spitter": return 4;
        case "jockey":  return 5;
        case "charger": return 6;
        case "witch":   return 7;
        case "tank":    return 8;
        default:        return null;
    }
}

function TikTools_SpawnSpecial(cls, count) {
    local type = TikTools_ZombieType(cls);
    if (type == null)
        return { ok = false, actual = 0, code = "INVALID_CLASS", diag = "unknown class" };
    if (!TikTools_HasGlobal("ZSpawn"))
        return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = "ZSpawn not available" };
    local anchor = TikTools_FindSurvivorAnchor();
    if (anchor == null)
        return { ok = false, actual = 0, code = "NO_ANCHOR", diag = "no living survivor" };
    local origin = null;
    try { origin = anchor.GetOrigin(); } catch (e) { origin = null; }
    local offsets = [Vector(300, 0, 0), Vector(-300, 0, 0), Vector(0, 300, 0),
                     Vector(0, -300, 0), Vector(220, 220, 0), Vector(-220, -220, 0),
                     Vector(220, -220, 0), Vector(-220, 220, 0)];
    local actual = 0;
    for (local i = 0; i < count; i++) {
        local ok = false;
        try {
            if (origin != null)
                ok = ZSpawn({ type = type, pos = origin + offsets[i % offsets.len()] + Vector(0, 0, 16) });
            // Position rejected (wall, void): let the director choose.
            if (!ok)
                ok = ZSpawn({ type = type });
        } catch (e) {
            ok = false;
        }
        if (ok == true)
            actual++;
    }
    if (actual == 0)
        return { ok = false, actual = 0, code = "SPAWN_FAILED", diag = "ZSpawn refused " + cls };
    if (actual < count)
        return { ok = true, actual = actual, code = "PARTIAL", diag = "spawned " + actual + " of " + count + " " + cls };
    return { ok = true, actual = actual, code = "OK", diag = "spawned " + actual + " " + cls };
}

// --- hazards: shoot a spawned propane tank (explosion) or gas can (fire) -------

function TikTools_SpawnHazard(kind, count) {
    local model = kind == "explosion" ? "models/props_junk/propanecanister001a.mdl"
                                      : "models/props_junk/gascan001a.mdl";
    local anchor = TikTools_FindSurvivorAnchor();
    if (anchor == null)
        return { ok = false, actual = 0, code = "NO_ANCHOR", diag = "no living survivor" };
    local origin = null;
    try { origin = anchor.GetOrigin(); } catch (e) {
        return { ok = false, actual = 0, code = "NO_ANCHOR", diag = "anchor has no position" };
    }
    try { PrecacheEntityFromTable({ classname = "prop_physics", model = model }); } catch (e) { }
    local world = null;
    try { world = Entities.FindByClassname(null, "worldspawn"); } catch (e) { world = null; }
    local offsets = [Vector(200, 0, 0), Vector(-200, 0, 0), Vector(0, 200, 0)];
    local actual = 0;
    local missing = null;
    for (local i = 0; i < count; i++) {
        try {
            local ent = SpawnEntityFromTable("prop_physics", {
                model = model,
                origin = origin + offsets[i % offsets.len()] + Vector(0, 0, 16),
                angles = Vector(0, 0, 0),
            });
            if (ent == null || !ent.IsValid())
                continue;
            TikTools_TrackSpawn(ent);
            // Bullet damage ignites a gas can and detonates a propane tank.
            ent.TakeDamage(200.0, 2, world);
            actual++;
        } catch (e) {
            if (TikTools_IsMissingApi(e)) {
                missing = e;
                break;
            }
        }
    }
    if (actual == 0 && missing != null)
        return { ok = false, actual = 0, code = "CAPABILITY_UNAVAILABLE", diag = TikTools_Diag(missing) };
    if (actual == 0)
        return { ok = false, actual = 0, code = "SPAWN_FAILED", diag = "no " + kind + " created" };
    if (actual < count)
        return { ok = true, actual = actual, code = "PARTIAL", diag = actual + " of " + count + " " + kind };
    return { ok = true, actual = actual, code = "OK", diag = actual + " " + kind };
}
