// limits.nut -- per-action cooldowns, entity ceilings, per-round counters.
//
// Defaults are conservative; the Rust side enforces its own bounds on top
// and the game reports the verified actual count. Fail closed.

::TikToolsLimits <- {
    COMMON_MAX = 10,
    SPECIAL_MAX = 8,
    ITEM_MAX = 5,
    // Cooldowns (seconds) and ROUND_CAP are overwritten from config.txt by
    // TikTools_ReloadConfig; these are the built-in defaults.
    COMMON_COOLDOWN = 0.0,
    SPECIAL_COOLDOWN = 30.0,
    TANK_COOLDOWN = 120.0,
    ITEM_COOLDOWN = 0.0,
    WEAPON_COOLDOWN = 0.0,
    LAST = { common = -9999.0, special = -9999.0, tank = -9999.0, item = -9999.0, weapon = -9999.0 },
    ROUND_SPAWNED = 0,
    ROUND_CAP = 60,
};

// Returns null when the spawn may proceed, else a refusal code.
// Accounts the requested count against the remaining round capacity so a
// request can never push the round total past ROUND_CAP.
function TikTools_CheckLimit(kind, cooldown, requested) {
    local now = Time();
    if (now - ::TikToolsLimits.LAST[kind] < cooldown)
        return "COOLDOWN";
    if (::TikToolsLimits.ROUND_SPAWNED + requested > ::TikToolsLimits.ROUND_CAP)
        return "ENTITY_CAP";
    return null;
}

function TikTools_NoteSpawn(kind, count) {
    ::TikToolsLimits.LAST[kind] = Time();
    ::TikToolsLimits.ROUND_SPAWNED += count;
    if (::TikToolsLimits.ROUND_SPAWNED > ::TikToolsLimits.ROUND_CAP)
        ::TikToolsLimits.ROUND_SPAWNED = ::TikToolsLimits.ROUND_CAP;
}

function TikTools_ResetRoundCounters() {
    ::TikToolsLimits.ROUND_SPAWNED = 0;
    ::TikToolsLimits.LAST.common = -9999.0;
    ::TikToolsLimits.LAST.special = -9999.0;
    ::TikToolsLimits.LAST.tank = -9999.0;
    ::TikToolsLimits.LAST.item = -9999.0;
    ::TikToolsLimits.LAST.weapon = -9999.0;
}

// ---------------------------------------------------------------------------
// User config with hot reload.
//
// ems/tiktools_l4d2/config.txt holds `key=value` lines (`#` starts a
// comment). The poll loop re-reads it at most once per CHECK_INTERVAL and
// applies it only when the text changed, so edits take effect within about a
// second without restarting the game. Every key is range-checked; a missing
// or invalid key falls back to its default rather than keeping a stale value.
// ---------------------------------------------------------------------------

::TikToolsConfig <- {
    FILE = "config.txt",
    CHECK_INTERVAL = 1.0,
    LAST_CHECK = -9999.0,
    LAST_TEXT = null,
};

// key -> [limits field, min, max, default, is_integer]
::TikToolsConfigSpec <- {
    item_cooldown = ["ITEM_COOLDOWN", 0.0, 600.0, 0.0, false],
    weapon_cooldown = ["WEAPON_COOLDOWN", 0.0, 600.0, 0.0, false],
    common_cooldown = ["COMMON_COOLDOWN", 0.0, 600.0, 0.0, false],
    special_cooldown = ["SPECIAL_COOLDOWN", 0.0, 600.0, 30.0, false],
    tank_cooldown = ["TANK_COOLDOWN", 0.0, 600.0, 120.0, false],
    round_cap = ["ROUND_CAP", 1.0, 1000.0, 60.0, true],
};

function TikTools_DefaultConfigText() {
    return "# TikTools L4D2 config. Saved edits apply within ~1 s (hot reload).\n"
        + "# Cooldowns in seconds (0 = none, max 600). round_cap: 1-1000.\n"
        + "item_cooldown=0\n"
        + "weapon_cooldown=0\n"
        + "common_cooldown=0\n"
        + "special_cooldown=30\n"
        + "tank_cooldown=120\n"
        + "round_cap=60\n";
}

// Parse one `key=value` line into `out`, ignoring blanks, spaces and comments.
function TikTools_ConfigLine(line, out) {
    local key = "";
    local val = "";
    local seenEq = false;
    for (local i = 0; i < line.len(); i++) {
        local c = line[i];
        if (c == 32 || c == 9)
            continue;
        if (!seenEq && key == "" && c == 35)
            return; // '#' comment
        if (!seenEq && c == 61) {
            seenEq = true;
            continue;
        }
        if (seenEq)
            val += c.tochar();
        else
            key += c.tochar();
    }
    if (seenEq && key != "" && val != "")
        out[key] <- val;
}

function TikTools_ParseConfig(text) {
    local out = {};
    local line = "";
    local n = text.len();
    for (local i = 0; i <= n; i++) {
        local c = i < n ? text[i] : 10;
        if (c == 10 || c == 13 || c == 0) {
            TikTools_ConfigLine(line, out);
            line = "";
        } else {
            line += c.tochar();
        }
    }
    return out;
}

function TikTools_ApplyConfig(values) {
    foreach (key, spec in ::TikToolsConfigSpec) {
        local v = spec[3];
        if (key in values) {
            local parsed = null;
            try { parsed = values[key].tofloat(); } catch (e) { parsed = null; }
            if (parsed != null && parsed >= spec[1] && parsed <= spec[2])
                v = parsed;
            else
                printl("[TikTools] config: valor invalido para " + key + ", usando " + spec[3]);
        }
        ::TikToolsLimits[spec[0]] = spec[4] ? v.tointeger() : v;
    }
}

// Called from the poll loop; `force` skips the interval (round start).
function TikTools_ReloadConfig(force) {
    local now = Time();
    if (!force && now - ::TikToolsConfig.LAST_CHECK < ::TikToolsConfig.CHECK_INTERVAL)
        return;
    ::TikToolsConfig.LAST_CHECK = now;
    local path = TikTools_EmsPath(::TikToolsConfig.FILE);
    local text = null;
    try { text = FileToString(path); } catch (e) { text = null; }
    if (text == null) {
        // First run: create the file so the user has something to edit.
        text = TikTools_DefaultConfigText();
        StringToFile(path, text);
    }
    if (text == ::TikToolsConfig.LAST_TEXT)
        return;
    ::TikToolsConfig.LAST_TEXT = text;
    TikTools_ApplyConfig(TikTools_ParseConfig(text));
    printl("[TikTools] config cargada: item=" + ::TikToolsLimits.ITEM_COOLDOWN
        + " weapon=" + ::TikToolsLimits.WEAPON_COOLDOWN
        + " common=" + ::TikToolsLimits.COMMON_COOLDOWN
        + " special=" + ::TikToolsLimits.SPECIAL_COOLDOWN
        + " tank=" + ::TikToolsLimits.TANK_COOLDOWN
        + " round_cap=" + ::TikToolsLimits.ROUND_CAP);
}
