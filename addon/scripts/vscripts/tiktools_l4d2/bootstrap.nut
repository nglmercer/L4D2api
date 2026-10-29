// bootstrap.nut -- addon startup, session generation, heartbeat loop.
//
// PROTOTYPE (live gate BLOCKED): every engine behavior assumed here must be
// proven in a real Windows Steam local campaign before any claim of
// compatibility. Fail closed: no host authority or no round means no
// mutations, only heartbeat/status reads.

::TikTools <- {
    VERSION = "0.1.0",
    API_VERSION = 1,
    EMS_DIR = "tiktools_l4d2",
    GENERATION = null,
    TICK = 0,
    LAST_POLL = 0.0,
    POLL_INTERVAL = 0.35,
    BUSY_ID = null,
    LAST_ID = null,
    SEEN_GEN = null,
    SEEN_IDS = [],
    MAX_SEQ = -1,
    OWNED = [],
    CAPS = "test_connection,spawn_item,spawn_weapon",
};

IncludeScript("tiktools_l4d2/state.nut");
IncludeScript("tiktools_l4d2/protocol.nut");
IncludeScript("tiktools_l4d2/limits.nut");
IncludeScript("tiktools_l4d2/spawns.nut");
IncludeScript("tiktools_l4d2/actions.nut");

// New generation per VScript startup; old commands from a previous game
// generation must never execute after a relaunch or map change. Replay
// state, per-round counters, cooldowns and owned-entity tracking all reset
// deterministically here.
function TikTools_NewGeneration() {
    // A monotonically increasing counter persisted to disk so a relaunch
    // or map change never reuses a previous generation token.
    local counter = 0;
    try {
        local raw = FileToString(TikTools_EmsPath("generation.txt"));
        if (raw != null) {
            local digits = "";
            for (local i = 0; i < raw.len(); i++) {
                local c = raw[i];
                if (c >= 48 && c <= 57)
                    digits += c.tochar();
            }
            if (digits != "")
                counter = digits.tointeger();
        }
    } catch (e) { }
    counter++;
    StringToFile(TikTools_EmsPath("generation.txt"), counter.tostring());
    ::TikTools.GENERATION = "g-" + counter.tostring();
    ::TikTools.TICK = 0;
    ::TikTools.BUSY_ID = null;
    ::TikTools.LAST_ID = null;
    TikTools_ResetReplay();
    TikTools_ResetRoundCounters();
    TikTools_ResetOwned();
    TikTools_WriteHeartbeat();
}

// Called by the director once the map is live. Kept tiny: heartbeat first,
// then at most one bounded command poll per POLL_INTERVAL seconds -- never
// per-frame disk access.
function OnGameEvent_round_start(params) {
    TikTools_NewGeneration();
}

function TikTools_Think() {
    if (::TikTools.GENERATION == null)
        TikTools_NewGeneration();
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
