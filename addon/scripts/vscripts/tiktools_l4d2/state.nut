// state.nut -- authoritative host/round readiness signals.
//
// A heartbeat alone never means gameplay is ready. Mutations additionally
// require IsHost() and an active round. Everything here fails closed.

function TikTools_IsHost() {
    // PROTOTYPE: exact host detection (netprop vs. director query) must be
    // chosen during the live gate. Default deny until proven.
    return false;
}

function TikTools_InRound() {
    // PROTOTYPE: round-state source (director table / game event) must be
    // chosen during the live gate. Default deny until proven.
    return false;
}

function TikTools_CurrentMap() {
    // PROTOTYPE: map-name source must be chosen during the live gate.
    return "";
}

function TikTools_GameplayReady() {
    return TikTools_IsHost() && TikTools_InRound();
}

function TikTools_WriteHeartbeat() {
    local lines = [
        "v=1",
        "gen=" + ::TikTools.GENERATION,
        "addon=" + ::TikTools.VERSION,
        "map=" + TikTools_CurrentMap(),
        "round=" + (TikTools_InRound() ? "1" : "0"),
        "host=" + (TikTools_IsHost() ? "1" : "0"),
        "ready=" + (TikTools_GameplayReady() ? "1" : "0"),
        "caps=" + ::TikTools.CAPS,
        "tick=" + ::TikTools.TICK.tostring(),
    ];
    StringToFile(TikTools_EmsPath("heartbeat.txt"), TikTools_FinishRecord(lines));
}
