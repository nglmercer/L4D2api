// state.nut -- authoritative host/round readiness signals.
//
// A heartbeat alone never means gameplay is ready. Mutations additionally
// require a listen-server host and an active round. Everything here fails
// closed: unknown or unavailable engine state reads as "not ready".

function TikTools_GetMapName() {
    // Try the Director method first (used by stock director scripts), then
    // the global helper. Either may be unavailable depending on scope.
    try {
        local name = Director.GetMapName();
        if (name != null && name != "")
            return name;
    } catch (e) { }
    try {
        local name = GetMapName();
        if (name != null && name != "")
            return name;
    } catch (e) { }
    return "";
}

function TikTools_IsHost() {
    // On a listen server (including single player) this returns the host
    // player; on a dedicated server it returns null.
    try {
        local host = GetListenServerHost();
        return host != null && host.IsValid();
    } catch (e) {
        return false;
    }
}

// True when the handle is a living survivor-team player. Team lookup may
// be unavailable in some scopes; then any living player counts (preserves
// the previously working behavior instead of failing closed here -- the
// host/round gates still guard every mutation).
function TikTools_IsSurvivor(player) {
    try {
        if (player == null || !player.IsValid() || player.GetHealth() <= 0)
            return false;
    } catch (e) {
        return false;
    }
    try {
        return player.GetTeam() == 2;
    } catch (e) {
        return true;
    }
}

function TikTools_InRound() {
    try {
        if (!TikTools_IsHost())
            return false;
        local map = TikTools_GetMapName();
        if (map == null || map == "")
            return false;
        // A live round has at least one living survivor player entity. Bots
        // count, so single player and local co-op both qualify.
        local player = Entities.FindByClassname(null, "player");
        while (player != null) {
            if (TikTools_IsSurvivor(player))
                return true;
            player = Entities.FindByClassname(player, "player");
        }
    } catch (e) {
        return false;
    }
    return false;
}

function TikTools_CurrentMap() {
    try {
        local name = TikTools_GetMapName();
        if (name == null)
            return "";
        // The heartbeat map field must be a [A-Za-z0-9_-] token.
        local out = "";
        for (local i = 0; i < name.len(); i++) {
            local c = name[i];
            // Keep a-z, A-Z, 0-9, underscore and hyphen; map the rest to '_'.
            local keep = (c >= 97 && c <= 122) || (c >= 65 && c <= 90) || (c >= 48 && c <= 57) || c == 95 || c == 45;
            out += keep ? c.tochar() : "_";
        }
        if (out.len() > 64)
            out = out.slice(0, 64);
        return out;
    } catch (e) {
        return "";
    }
}

function TikTools_GameplayReady() {
    return TikTools_IsHost() && TikTools_InRound();
}

function TikTools_WriteHeartbeat() {
    // Re-probe capabilities about every 7 s (every 21 ticks, a multiple of
    // the heartbeat cadence, including the first heartbeat of a generation)
    // instead of on every heartbeat, to keep the timer cheap.
    if (::TikTools.TICK % 21 == 0) {
        try { TikTools_ComputeCaps(); } catch (e) { }
    }
    // Each check walks players / queries the host: compute once.
    local host = TikTools_IsHost();
    local round = TikTools_InRound();
    local lines = [
        "v=1",
        "gen=" + ::TikTools.GENERATION,
        "addon=" + ::TikTools.VERSION,
        "map=" + TikTools_CurrentMap(),
        "round=" + (round ? "1" : "0"),
        "host=" + (host ? "1" : "0"),
        "ready=" + (host && round ? "1" : "0"),
        "caps=" + ::TikTools.CAPS,
        "tick=" + ::TikTools.TICK.tostring(),
    ];
    StringToFile(TikTools_EmsPath("heartbeat.txt"), TikTools_FinishRecord(lines));
}
