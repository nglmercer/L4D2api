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
        // Tank is exactly 1 with its own cooldown; others bounded 1-8.
        TikTools_Reject(cmd, "CAPABILITY_UNAVAILABLE", "special spawn unverified");
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
