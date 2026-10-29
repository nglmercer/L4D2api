// protocol.nut -- v1 text-protocol framing for the EMS sandbox.
//
// Records are key=value lines ending in an END sentinel with a trailing
// integrity field. This Squirrel side only builds results/heartbeats and
// parses commands with strict checks; any malformed, truncated or
// cross-generation record is IGNORED, never executed.
//
// PROTOTYPE: FileToString/StringToFile paths, encoding, size limits and
// caching behavior are UNVERIFIED -- see docs/WINDOWS_TESTING.md.

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

// Parse a raw command record. Returns a table or null when the record must
// be ignored (missing sentinel, bad checksum, wrong version or generation,
// unknown action, out-of-range values).
function TikTools_ParseCommand(raw) {
    if (raw == null || raw.len() > 1024)
        return null;
    local lines = split(raw, "\n");
    // Strip trailing empty line from a final newline.
    if (lines.len() > 0 && lines[lines.len() - 1] == "")
        lines.remove(lines.len() - 1);
    if (lines.len() < 3 || lines[lines.len() - 1] != "END")
        return null;
    local crcLine = lines[lines.len() - 2];
    if (crcLine.len() != 12 || crcLine.slice(0, 4) != "crc=")
        return null;
    local body = "";
    for (local i = 0; i < lines.len() - 2; i++)
        body += lines[i] + "\n";
    if (TikTools_Checksum(body) != crcLine.slice(4).tolower())
        return null;
    local fields = {};
    for (local i = 0; i < lines.len() - 2; i++) {
        local eq = lines[i].find("=");
        if (eq == null)
            return null;
        local key = lines[i].slice(0, eq);
        local value = lines[i].slice(eq + 1);
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
    // Freshness: commands from an old generation never execute.
    if (fields["gen"] != ::TikTools.GENERATION)
        return null;
    return fields;
}

function TikTools_BuildResult(cmd, status, code, requested, actual, diag) {
    local lines = [
        "v=1",
        "seq=" + cmd["seq"],
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
    local requested = 0;
    if ("count" in cmd)
        requested = cmd["count"].tointeger();
    TikTools_BuildResult(cmd, "rejected", code, requested, 0, diag);
}

// At most one outstanding mutation: while BUSY_ID is set, new commands wait.
function TikTools_PollCommand() {
    if (::TikTools.BUSY_ID != null)
        return;
    local raw = FileToString(TikTools_EmsPath("command.txt"));
    if (raw == null || raw == "")
        return;
    local cmd = TikTools_ParseCommand(raw);
    if (cmd == null)
        return; // Ignore partial/malformed/stale snapshots silently.
    ::TikTools.BUSY_ID = cmd["id"];
    TikTools_Dispatch(cmd);
    ::TikTools.BUSY_ID = null;
}
