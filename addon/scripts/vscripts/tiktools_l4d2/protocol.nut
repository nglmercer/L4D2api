// protocol.nut -- strict v1 text-protocol framing for the EMS sandbox.
//
// Records are key=value lines ending in an END sentinel with a trailing
// integrity field. Both sides enforce the same contract:
//
// - ASCII only, at most 1024 bytes.
// - Exact END sentinel; crc line second-to-last with 8 hex digits.
// - FNV-1a checksum over every content byte (each line + "\n").
// - No duplicate keys, no unknown keys, all required keys present.
// - v must be "1"; seq/count are digit-only with overflow caps.
// - id/gen are [A-Za-z0-9_-]{1,64} tokens.
// - Per-action schemas: exact parameter sets, allowlisted values,
//   bounded counts. Anything else is IGNORED, never dispatched.
//
// Malformed records never reach TikTools_Dispatch(): parse failures,
// stale generations, duplicates and out-of-order sequences all return
// null (or are acknowledged without re-execution by the poll loop).

function TikTools_EmsPath(name) {
    return ::TikTools.EMS_DIR + "/" + name;
}

// Minimal FNV-1a over the body string (must match l4d2api-protocol).
function TikTools_Checksum(body) {
    local hash = 0x811c9dc5;
    for (local i = 0; i < body.len(); i++) {
        hash = hash ^ body[i];
        hash = (hash * 0x01000193) & 0xFFFFFFFF;
    }
    local digits = "0123456789abcdef";
    local out = "";
    for (local shift = 28; shift >= 0; shift -= 4)
        out += digits[(hash >> shift) & 0xF].tochar();
    return out;
}

function TikTools_FinishRecord(lines) {
    local body = "";
    foreach (line in lines)
        body += line + "\n";
    return body + "crc=" + TikTools_Checksum(body) + "\nEND";
}

// --- strict scalar validators (no unprotected tointeger() anywhere) ---

function TikTools_IsAscii(text) {
    for (local i = 0; i < text.len(); i++) {
        if (text[i] > 127)
            return false;
    }
    return true;
}

function TikTools_IsDigitString(text) {
    if (text == null || text.len() == 0)
        return false;
    for (local i = 0; i < text.len(); i++) {
        if (text[i] < 48 || text[i] > 57)
            return false;
    }
    return true;
}

// Squirrel integers are 32-bit signed: accept 0..2147483647 only.
function TikTools_ParseSeq(text) {
    if (!TikTools_IsDigitString(text) || text.len() > 10)
        return null;
    local digits = text;
    while (digits.len() > 1 && digits[0] == 48)
        digits = digits.slice(1);
    if (digits.len() > 10)
        return null;
    if (digits.len() == 10 && digits > "2147483647")
        return null;
    return digits.tointeger();
}

function TikTools_ParseCount(text, lo, hi) {
    local n = TikTools_ParseSeq(text);
    if (n == null || n < lo || n > hi)
        return null;
    return n;
}

function TikTools_IsToken(text, maxLen) {
    if (text == null || text.len() == 0 || text.len() > maxLen)
        return false;
    for (local i = 0; i < text.len(); i++) {
        local c = text[i];
        local ok = (c >= 48 && c <= 57) || (c >= 65 && c <= 90)
            || (c >= 97 && c <= 122) || c == 95 || c == 45;
        if (!ok)
            return false;
    }
    return true;
}

function TikTools_IsActionName(text) {
    return text == "test_connection" || text == "spawn_common"
        || text == "spawn_infected" || text == "spawn_item"
        || text == "spawn_weapon";
}

function TikTools_IsInfectedClass(text) {
    return text == "tank" || text == "witch" || text == "smoker"
        || text == "boomer" || text == "hunter" || text == "spitter"
        || text == "jockey" || text == "charger";
}

function TikTools_IsItemName(text) {
    return text == "first_aid_kit" || text == "pain_pills"
        || text == "adrenaline" || text == "pipe_bomb"
        || text == "molotov" || text == "defibrillator";
}

function TikTools_IsWeaponName(text) {
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
    return allowed.find(text) != null;
}

// --- bounded replay protection: (generation, request_id) + seq order ---
//
// SEEN_IDS holds the ids accepted under SEEN_GEN (bounded at 64 entries);
// MAX_SEQ is the highest sequence executed under that generation. A new
// generation resets both, so old commands can never run after a relaunch
// or map change. Duplicates never execute twice; older sequences never
// execute after newer accepted ones.

function TikTools_ResetReplay() {
    ::TikTools.SEEN_GEN = ::TikTools.GENERATION;
    ::TikTools.SEEN_IDS = [];
    ::TikTools.MAX_SEQ = -1;
}

function TikTools_ReplayCheck(cmd) {
    if (::TikTools.SEEN_GEN != ::TikTools.GENERATION)
        TikTools_ResetReplay();
    if (cmd["gen"] != ::TikTools.GENERATION)
        return "stale";
    if (::TikTools.SEEN_IDS.find(cmd["id"]) != null)
        return "duplicate";
    if (cmd["seq"] <= ::TikTools.MAX_SEQ)
        return "out_of_order";
    return null;
}

function TikTools_ReplayNote(cmd) {
    if (::TikTools.SEEN_GEN != ::TikTools.GENERATION)
        TikTools_ResetReplay();
    ::TikTools.SEEN_IDS.append(cmd["id"]);
    if (::TikTools.SEEN_IDS.len() > 64)
        ::TikTools.SEEN_IDS.remove(0);
    if (cmd["seq"] > ::TikTools.MAX_SEQ)
        ::TikTools.MAX_SEQ = cmd["seq"];
}

// Parse a raw command record. Returns a validated table with integer
// seq/count fields, or null when the record must be ignored.
function TikTools_ParseCommand(raw) {
    if (raw == null || raw == "" || raw.len() > 1024)
        return null;
    if (!TikTools_IsAscii(raw))
        return null;
    local lines = split(raw, "\n");
    // Tolerate one trailing newline; tolerate CR per line (host writes).
    if (lines.len() > 0 && lines[lines.len() - 1] == "")
        lines.remove(lines.len() - 1);
    for (local i = 0; i < lines.len(); i++) {
        if (lines[i].len() > 0 && lines[i][lines[i].len() - 1] == 13)
            lines[i] = lines[i].slice(0, lines[i].len() - 1);
    }
    if (lines.len() < 3 || lines[lines.len() - 1] != "END")
        return null;
    local crcLine = lines[lines.len() - 2];
    if (crcLine.len() != 12 || crcLine.slice(0, 4) != "crc=")
        return null;
    local hex = crcLine.slice(4).tolower();
    for (local i = 0; i < hex.len(); i++) {
        local c = hex[i];
        local ok = (c >= 48 && c <= 57) || (c >= 97 && c <= 102);
        if (!ok)
            return null;
    }
    local body = "";
    for (local i = 0; i < lines.len() - 2; i++)
        body += lines[i] + "\n";
    if (TikTools_Checksum(body) != hex)
        return null;
    local fields = {};
    for (local i = 0; i < lines.len() - 2; i++) {
        local eq = lines[i].find("=");
        if (eq == null || eq == 0)
            return null;
        local key = lines[i].slice(0, eq);
        local value = lines[i].slice(eq + 1);
        if (!TikTools_IsToken(key, 16))
            return null;
        if (key in fields)
            return null;
        fields[key] <- value;
    }
    if (!("v" in fields) || fields["v"] != "1")
        return null;
    foreach (key in ["seq", "id", "gen", "action"]) {
        if (!(key in fields))
            return null;
    }
    local seq = TikTools_ParseSeq(fields["seq"]);
    if (seq == null)
        return null;
    if (!TikTools_IsToken(fields["id"], 64))
        return null;
    if (!TikTools_IsToken(fields["gen"], 64))
        return null;
    local action = fields["action"];
    if (!TikTools_IsActionName(action))
        return null;
    // Exact parameter sets per action: no unexpected keys allowed.
    local allowed = ["v", "seq", "id", "gen", "action"];
    local count = 0;
    if (action == "test_connection") {
        // No parameters at all.
    } else if (action == "spawn_common") {
        allowed.append("count");
        if (!("count" in fields))
            return null;
        count = TikTools_ParseCount(fields["count"], 1, 10);
        if (count == null)
            return null;
    } else if (action == "spawn_infected") {
        allowed.append("class");
        allowed.append("count");
        if (!("class" in fields) || !("count" in fields))
            return null;
        if (!TikTools_IsInfectedClass(fields["class"]))
            return null;
        local hi = fields["class"] == "tank" ? 1 : 8;
        count = TikTools_ParseCount(fields["count"], 1, hi);
        if (count == null)
            return null;
    } else if (action == "spawn_item") {
        allowed.append("item");
        allowed.append("count");
        if (!("item" in fields) || !("count" in fields))
            return null;
        if (!TikTools_IsItemName(fields["item"]))
            return null;
        count = TikTools_ParseCount(fields["count"], 1, 5);
        if (count == null)
            return null;
    } else if (action == "spawn_weapon") {
        allowed.append("weapon");
        allowed.append("count");
        if (!("weapon" in fields) || !("count" in fields))
            return null;
        if (!TikTools_IsWeaponName(fields["weapon"]))
            return null;
        count = TikTools_ParseCount(fields["count"], 1, 5);
        if (count == null)
            return null;
    }
    foreach (key, _ in fields) {
        if (allowed.find(key) == null)
            return null;
    }
    fields["seq"] = seq;
    fields["count"] <- count;
    // Freshness: commands from an old generation never execute.
    if (fields["gen"] != ::TikTools.GENERATION)
        return null;
    return fields;
}

function TikTools_SafeRequested(cmd) {
    if ("count" in cmd && typeof cmd["count"] == "integer")
        return cmd["count"];
    return 0;
}

function TikTools_BuildResult(cmd, status, code, requested, actual, diag) {
    local lines = [
        "v=1",
        "seq=" + cmd["seq"].tostring(),
        "id=" + cmd["id"],
        "gen=" + ::TikTools.GENERATION,
        "status=" + status,
        "code=" + code,
        "requested=" + requested.tostring(),
        "actual=" + actual.tostring(),
        "diag=" + diag,
    ];
    StringToFile(TikTools_EmsPath("result.txt"), TikTools_FinishRecord(lines));
}

function TikTools_Reject(cmd, code, diag) {
    TikTools_BuildResult(cmd, "rejected", code, TikTools_SafeRequested(cmd), 0, diag);
}

// Guarded dispatch: parse, freshness check, mark processing, execute in a
// protected block, always clear transient state. An interrupted execution
// is reported as unknown, never applied.
function TikTools_PollCommand() {
    if (::TikTools.BUSY_ID != null)
        return;
    local raw = null;
    try {
        raw = FileToString(TikTools_EmsPath("command.txt"));
    } catch (e) {
        return;
    }
    if (raw == null || raw == "")
        return;
    local cmd = null;
    try {
        cmd = TikTools_ParseCommand(raw);
    } catch (e) {
        cmd = null;
    }
    if (cmd == null)
        return; // Malformed, partial or cross-generation: ignore safely.
    local replay = null;
    try {
        replay = TikTools_ReplayCheck(cmd);
    } catch (e) {
        return;
    }
    if (replay != null)
        return; // Duplicate or out-of-order: original ack stands, never re-run.
    ::TikTools.LAST_ID = cmd["id"];
    ::TikTools.BUSY_ID = cmd["id"];
    try {
        TikTools_ReplayNote(cmd);
    } catch (e) {
        ::TikTools.BUSY_ID = null;
        return;
    }
    try {
        TikTools_Dispatch(cmd);
    } catch (e) {
        try {
            TikTools_BuildResult(cmd, "unknown", "INTERRUPTED",
                TikTools_SafeRequested(cmd), 0, "dispatch interrupted");
        } catch (inner) { }
    }
    ::TikTools.BUSY_ID = null;
}
