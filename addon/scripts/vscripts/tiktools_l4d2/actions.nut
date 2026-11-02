// actions.nut -- bounded dispatcher: existing TikTools action IDs only.
//
// No arbitrary script/exec/convar/filesystem/URL handling. Unknown actions
// and non-host / not-in-round states get explicit refusal codes.
//
// Commands arrive pre-validated from TikTools_ParseCommand() (strict types,
// allowlists, exact parameter sets), so the checks below are
// defense-in-depth. Result semantics:
//
// - every requested entity verified -> "applied"
// - some verified                  -> "partial"
// - none created (known reason)    -> "rejected"
// - action unavailable             -> "rejected" + CAPABILITY_UNAVAILABLE
// - outcome cannot be determined   -> "unknown" (see protocol.nut guard)

function TikTools_ActionCount(cmd, lo, hi) {
    if (!("count" in cmd) || typeof cmd["count"] != "integer")
        return null;
    if (cmd["count"] < lo || cmd["count"] > hi)
        return null;
    return cmd["count"];
}

function TikTools_FinishSpawn(cmd, result, requested) {
    local actual = result.actual;
    if (actual < 0)
        actual = 0;
    if (actual > requested)
        actual = requested;
    if (!result.ok)
        TikTools_BuildResult(cmd, "rejected", result.code, requested, 0, result.diag);
    else if (actual < requested)
        TikTools_BuildResult(cmd, "partial", result.code, requested, actual, result.diag);
    else
        TikTools_BuildResult(cmd, "applied", result.code, requested, actual, result.diag);
}

// Player effects report how many survivors were targeted (`requested`) and
// how many actually changed (`actual`).
function TikTools_FinishEffect(cmd, result) {
    local requested = result.requested;
    local actual = result.actual;
    if (actual < 0)
        actual = 0;
    if (actual > requested)
        actual = requested;
    if (!result.ok)
        TikTools_BuildResult(cmd, "rejected", result.code, requested, 0, result.diag);
    else if (actual < requested)
        TikTools_BuildResult(cmd, "partial", result.code, requested, actual, result.diag);
    else
        TikTools_BuildResult(cmd, "applied", result.code, requested, actual, result.diag);
}

function TikTools_Dispatch(cmd) {
    local action = cmd["action"];

    if (action == "test_connection") {
        local ready = TikTools_GameplayReady();
        TikTools_BuildResult(cmd, ready ? "applied" : "not_ready",
            ready ? "OK" : "NOT_IN_ROUND", 0, 0,
            ready ? "gameplay ready" : "game not ready");
        return;
    }

    // All mutating actions require host authority and an active round.
    if (!TikTools_IsHost()) {
        TikTools_Reject(cmd, "NOT_HOST", "not the authoritative host");
        return;
    }
    if (!TikTools_InRound()) {
        TikTools_Reject(cmd, "NOT_IN_ROUND", "no active round");
        return;
    }

    if (action == "spawn_common") {
        local count = TikTools_ActionCount(cmd, 1, 10);
        if (count == null) {
            TikTools_Reject(cmd, "INVALID_COUNT", "count must be 1-10");
            return;
        }
        local limit = TikTools_CheckLimit("common", ::TikToolsLimits.COMMON_COOLDOWN, count);
        if (limit != null) {
            TikTools_Reject(cmd, limit, limit == "ENTITY_CAP" ? "round entity cap reached" : "common spawn cooling down");
            return;
        }
        local result = TikTools_SpawnCommon(count);
        TikTools_NoteSpawn("common", result.actual);
        TikTools_FinishSpawn(cmd, result, count);
        return;
    }

    if (action == "spawn_infected") {
        // Tank is exactly 1 with its own cooldown; others bounded 1-8
        // (counts already enforced by the parser).
        local cls = cmd["class"];
        local count = cmd["count"];
        local kind = cls == "tank" ? "tank" : "special";
        local cooldown = cls == "tank" ? ::TikToolsLimits.TANK_COOLDOWN : ::TikToolsLimits.SPECIAL_COOLDOWN;
        local limit = TikTools_CheckLimit(kind, cooldown, count);
        if (limit != null) {
            TikTools_Reject(cmd, limit, limit == "ENTITY_CAP" ? "round entity cap reached" : kind + " spawn cooling down");
            return;
        }
        local result = TikTools_SpawnSpecial(cls, count);
        TikTools_NoteSpawn(kind, result.actual);
        TikTools_FinishSpawn(cmd, result, count);
        return;
    }

    if (action == "heal" || action == "temp_health" || action == "damage"
        || action == "revive" || action == "give_item") {
        local kind = action == "temp_health" ? "heal" : (action == "give_item" ? "give" : action);
        local cooldown = ::TikToolsLimits[kind.toupper() + "_COOLDOWN"];
        local limit = TikTools_CheckLimit(kind, cooldown, 0);
        if (limit != null) {
            TikTools_Reject(cmd, limit, action + " cooling down");
            return;
        }
        local result = null;
        if (action == "heal")
            result = TikTools_Heal(cmd["target"], cmd["amount"]);
        else if (action == "temp_health")
            result = TikTools_TempHealth(cmd["target"], cmd["amount"]);
        else if (action == "damage") {
            if (cmd["amount"] > ::TikToolsLimits.DAMAGE_MAX) {
                TikTools_Reject(cmd, "AMOUNT_LIMIT", "damage above damage_max " + ::TikToolsLimits.DAMAGE_MAX);
                return;
            }
            result = TikTools_Damage(cmd["target"], cmd["amount"]);
        } else if (action == "revive")
            result = TikTools_Revive(cmd["target"]);
        else
            result = TikTools_GiveItem(cmd["target"], cmd["item"]);
        if (result.ok)
            TikTools_NoteSpawn(kind, 0);
        TikTools_FinishEffect(cmd, result);
        return;
    }

    if (action == "panic") {
        local limit = TikTools_CheckLimit("panic", ::TikToolsLimits.PANIC_COOLDOWN, 0);
        if (limit != null) {
            TikTools_Reject(cmd, limit, "panic cooling down");
            return;
        }
        local result = TikTools_Panic();
        if (result.ok)
            TikTools_NoteSpawn("panic", 0);
        TikTools_FinishEffect(cmd, result);
        return;
    }

    if (action == "spawn_hazard") {
        local count = cmd["count"];
        local limit = TikTools_CheckLimit("hazard", ::TikToolsLimits.HAZARD_COOLDOWN, count);
        if (limit != null) {
            TikTools_Reject(cmd, limit, limit == "ENTITY_CAP" ? "round entity cap reached" : "hazard cooling down");
            return;
        }
        local result = TikTools_SpawnHazard(cmd["kind"], count);
        TikTools_NoteSpawn("hazard", result.actual);
        TikTools_FinishSpawn(cmd, result, count);
        return;
    }

    if (action == "spawn_item") {
        local count = TikTools_ActionCount(cmd, 1, 5);
        if (count == null) {
            TikTools_Reject(cmd, "INVALID_COUNT", "count must be 1-5");
            return;
        }
        local limit = TikTools_CheckLimit("item", ::TikToolsLimits.ITEM_COOLDOWN, count);
        if (limit != null) {
            TikTools_Reject(cmd, limit, limit == "ENTITY_CAP" ? "round entity cap reached" : "item spawn cooling down");
            return;
        }
        local result = TikTools_SpawnItem(cmd["item"], count);
        TikTools_NoteSpawn("item", result.actual);
        TikTools_FinishSpawn(cmd, result, count);
        return;
    }

    if (action == "spawn_weapon") {
        local count = TikTools_ActionCount(cmd, 1, 5);
        if (count == null) {
            TikTools_Reject(cmd, "INVALID_COUNT", "count must be 1-5");
            return;
        }
        local limit = TikTools_CheckLimit("weapon", ::TikToolsLimits.WEAPON_COOLDOWN, count);
        if (limit != null) {
            TikTools_Reject(cmd, limit, limit == "ENTITY_CAP" ? "round entity cap reached" : "weapon spawn cooling down");
            return;
        }
        local result = TikTools_SpawnWeapon(cmd["weapon"], count);
        TikTools_NoteSpawn("weapon", result.actual);
        TikTools_FinishSpawn(cmd, result, count);
        return;
    }

    TikTools_Reject(cmd, "UNKNOWN_ACTION", "unknown action");
}
