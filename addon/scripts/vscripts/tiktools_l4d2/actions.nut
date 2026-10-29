// actions.nut -- bounded dispatcher: existing TikTools action IDs only.
//
// No arbitrary script/exec/convar/filesystem/URL handling. Unknown actions
// and non-host / not-in-round states get explicit refusal codes.

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
        local count = ("count" in cmd) ? cmd["count"].tointeger() : -1;
        if (count < 1 || count > 10) {
            TikTools_Reject(cmd, "INVALID_COUNT", "count must be 1-10");
            return;
        }
        if (!TikTools_CheckLimit("common", ::TikToolsLimits.COMMON_COOLDOWN)) {
            TikTools_Reject(cmd, "COOLDOWN", "common spawn cooling down");
            return;
        }
        local result = TikTools_SpawnCommon(count);
        TikTools_NoteSpawn("common", result.actual);
        TikTools_BuildResult(cmd, result.ok ? "applied" : "rejected",
            result.code, count, result.actual, result.diag);
        return;
    }

    if (action == "spawn_infected") {
        // Tank is exactly 1 with its own cooldown; others bounded 1-8.
        TikTools_Reject(cmd, "CAPABILITY_UNAVAILABLE", "special spawn unverified");
        return;
    }

    if (action == "spawn_item") {
        local count = ("count" in cmd) ? cmd["count"].tointeger() : -1;
        if (count < 1 || count > 5) {
            TikTools_Reject(cmd, "INVALID_COUNT", "count must be 1-5");
            return;
        }
        if (!TikTools_CheckLimit("item", ::TikToolsLimits.ITEM_COOLDOWN)) {
            TikTools_Reject(cmd, "COOLDOWN", "item spawn cooling down");
            return;
        }
        local result = TikTools_SpawnItem(("item" in cmd) ? cmd["item"] : "", count);
        TikTools_NoteSpawn("item", result.actual);
        TikTools_BuildResult(cmd, result.ok ? "applied" : "rejected",
            result.code, count, result.actual, result.diag);
        return;
    }

    TikTools_Reject(cmd, "UNKNOWN_ACTION", "unknown action");
}
