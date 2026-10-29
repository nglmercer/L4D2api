// bootstrap.nut -- addon startup, session generation, heartbeat loop.
//
// PROTOTYPE (live gate BLOCKED): every engine behavior assumed here must be
// proven in a real Windows Steam local campaign before any claim of
// compatibility. Fail closed: no host authority or no round means no
// mutations, only heartbeat/status reads.

//::TikTools <- {
//    VERSION = "0.1.0",
//    API_VERSION = 1,
//    EMS_DIR = "ems/tiktools_l4d2",
//    GENERATION = null,
//    TICK = 0,
//    LAST_POLL = 0.0,
//    POLL_INTERVAL = 0.35,
//    BUSY_ID = null,
//    CAPS = "test_connection,spawn_common,spawn_item",
//};

IncludeScript("tiktools_l4d2/state.nut");
IncludeScript("tiktools_l4d2/protocol.nut");
IncludeScript("tiktools_l4d2/limits.nut");
IncludeScript("tiktools_l4d2/spawns.nut");
IncludeScript("tiktools_l4d2/actions.nut");

// New random generation per VScript startup; old commands from a previous
// game generation must never execute after a relaunch or map change.
function TikTools_NewGeneration() {
    ::TikTools.GENERATION = "g-" + rand().tostring() + "-" + Time().tostring();
    ::TikTools.TICK = 0;
    ::TikTools.BUSY_ID = null;
    TikTools_WriteHeartbeat();
}

// Called by the director once the map is live. Kept tiny: heartbeat first,
// then at most one bounded command poll per POLL_INTERVAL seconds -- never
// per-frame disk access.
function OnGameEvent_round_start(params) {
    TikTools_NewGeneration();
}

function TikTools_Think() {
    local now = Time();
    if (now - ::TikTools.LAST_POLL < ::TikTools.POLL_INTERVAL)
        return;
    ::TikTools.LAST_POLL = now;
    ::TikTools.TICK++;
    TikTools_WriteHeartbeat();
    TikTools_PollCommand();
}

// The host polls this from the director think path once wired; until the
// live gate proves the hook, this file documents intent, not behavior.
function Update() {
    TikTools_Think();
}
