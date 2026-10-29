//! `l4d2api-protocol`: codec and validation for the v1 file-IPC text protocol.
//!
//! Records are ASCII `key=value` lines terminated by an `END` sentinel plus an
//! FNV-1a integrity field. Bounded (1 KiB), strictly typed, allowlisted.
//!
//! ```text
//! v=1
//! seq=7
//! id=r_12_7
//! gen=g-9f2c
//! action=spawn_common
//! count=5
//! crc=1a2b3c4d
//! END
//! ```
//!
//! The checksum covers every byte before the `crc=` line (each content line
//! followed by `\n`). Anything failing validation is ignored by both sides:
//! a partial write is never interpreted as a command.

use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 1;
/// Initial request ceiling: 1 KiB, configurable downward if the game demands.
pub const MAX_RECORD_BYTES: usize = 1024;
pub const MAX_ID_LEN: usize = 64;
pub const MAX_GEN_LEN: usize = 64;
pub const MAX_DIAG_LEN: usize = 128;
pub const MAX_MAP_LEN: usize = 64;

/// Counts enforced on the Rust side (the VScript side enforces its own caps
/// and reports the verified actual count).
pub const COMMON_MAX: u32 = 10;
pub const SPECIAL_MAX: u32 = 8;
pub const ITEM_MAX: u32 = 5;

pub const INFECTED_ALLOWLIST: [&str; 8] = [
    "tank", "witch", "smoker", "boomer", "hunter", "spitter", "jockey", "charger",
];
pub const ITEM_ALLOWLIST: [&str; 6] = [
    "first_aid_kit",
    "pain_pills",
    "adrenaline",
    "pipe_bomb",
    "molotov",
    "defibrillator",
];
/// Capability tokens advertised in heartbeats.
pub const CAPABILITY_ALLOWLIST: [&str; 4] = [
    "test_connection",
    "spawn_common",
    "spawn_infected",
    "spawn_item",
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("record exceeds {MAX_RECORD_BYTES} bytes ({0} bytes)")]
    TooLarge(usize),
    #[error("record is not ASCII")]
    NonAscii,
    #[error("record must end with an END sentinel line")]
    MissingSentinel,
    #[error("record is missing its crc line")]
    MissingChecksum,
    #[error("checksum mismatch: expected {expected}, computed {computed}")]
    BadChecksum { expected: String, computed: String },
    #[error("missing field `{0}`")]
    MissingField(&'static str),
    #[error("duplicate field `{0}`")]
    DuplicateField(String),
    #[error("unknown field `{0}`")]
    UnknownField(String),
    #[error("unsupported protocol version `{0}`")]
    UnsupportedVersion(String),
    #[error("field `{field}` has an invalid value `{value}`: {reason}")]
    BadValue {
        field: &'static str,
        value: String,
        reason: &'static str,
    },
    #[error("record failed to encode: {0}")]
    Encode(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    TestConnection,
    SpawnCommon { count: u32 },
    SpawnInfected { class: String, count: u32 },
    SpawnItem { item: String, count: u32 },
}

impl Action {
    pub fn id(&self) -> &'static str {
        match self {
            Action::TestConnection => "test_connection",
            Action::SpawnCommon { .. } => "spawn_common",
            Action::SpawnInfected { .. } => "spawn_infected",
            Action::SpawnItem { .. } => "spawn_item",
        }
    }

    /// Count requested by TikTools (0 when the action takes no count).
    pub fn requested_count(&self) -> u32 {
        match self {
            Action::TestConnection => 0,
            Action::SpawnCommon { count }
            | Action::SpawnInfected { count, .. }
            | Action::SpawnItem { count, .. } => *count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub seq: u64,
    pub id: String,
    pub generation: String,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Applied,
    Rejected,
    NotReady,
    Partial,
    Unknown,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Applied => "applied",
            Status::Rejected => "rejected",
            Status::NotReady => "not_ready",
            Status::Partial => "partial",
            Status::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Status> {
        match value {
            "applied" => Some(Status::Applied),
            "rejected" => Some(Status::Rejected),
            "not_ready" => Some(Status::NotReady),
            "partial" => Some(Status::Partial),
            "unknown" => Some(Status::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultRecord {
    pub seq: u64,
    pub id: String,
    pub generation: String,
    pub status: Status,
    pub code: String,
    pub requested: u32,
    pub actual: u32,
    pub diag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heartbeat {
    pub generation: String,
    pub addon_version: String,
    pub map: String,
    pub round_active: bool,
    pub host_authority: bool,
    pub gameplay_ready: bool,
    pub capabilities: Vec<String>,
    pub tick: u64,
}

/// FNV-1a (32-bit), rendered as 8 lowercase hex digits.
pub fn checksum(body: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in body.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

fn is_token(text: &str, max_len: usize) -> bool {
    !text.is_empty()
        && text.len() <= max_len
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_dotted_version(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_ID_LEN
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

fn is_code(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 32
        && text
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn is_diag(text: &str) -> bool {
    text.len() <= MAX_DIAG_LEN
        && text
            .bytes()
            .all(|b| b == b' ' || (0x21..=0x7e).contains(&b))
}

fn check_id(id: &str) -> Result<(), Error> {
    if is_token(id, MAX_ID_LEN) {
        Ok(())
    } else {
        Err(Error::BadValue {
            field: "id",
            value: id.to_owned(),
            reason: "must be 1-64 chars of [A-Za-z0-9_-]",
        })
    }
}

fn check_generation(generation: &str) -> Result<(), Error> {
    if is_token(generation, MAX_GEN_LEN) {
        Ok(())
    } else {
        Err(Error::BadValue {
            field: "gen",
            value: generation.to_owned(),
            reason: "must be 1-64 chars of [A-Za-z0-9_-]",
        })
    }
}

fn parse_u64(field: &'static str, value: &str) -> Result<u64, Error> {
    value.parse::<u64>().map_err(|_| Error::BadValue {
        field,
        value: value.to_owned(),
        reason: "must be a non-negative integer",
    })
}

fn parse_u32(field: &'static str, value: &str) -> Result<u32, Error> {
    value.parse::<u32>().map_err(|_| Error::BadValue {
        field,
        value: value.to_owned(),
        reason: "must be a non-negative integer",
    })
}

fn parse_flag(field: &'static str, value: &str) -> Result<bool, Error> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(Error::BadValue {
            field,
            value: value.to_owned(),
            reason: "must be 0 or 1",
        }),
    }
}

/// Split a raw record into content lines after verifying size, ASCII range,
/// `END` sentinel and checksum. Returns the content lines (without `crc`).
fn split_verified(text: &str) -> Result<Vec<&str>, Error> {
    if text.len() > MAX_RECORD_BYTES {
        return Err(Error::TooLarge(text.len()));
    }
    if !text.is_ascii() {
        return Err(Error::NonAscii);
    }
    // Tolerate one trailing newline; anything else must be exact lines.
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    let mut lines: Vec<&str> = trimmed.split('\n').collect();
    // Tolerate a single trailing \r per line (game-side writes).
    for line in &mut lines {
        *line = line.strip_suffix('\r').unwrap_or(line);
    }
    if lines.last() != Some(&"END") {
        return Err(Error::MissingSentinel);
    }
    if lines.len() < 2 {
        return Err(Error::MissingChecksum);
    }
    let crc_line = lines[lines.len() - 2];
    let expected = crc_line
        .strip_prefix("crc=")
        .ok_or(Error::MissingChecksum)?;
    if expected.len() != 8 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::BadValue {
            field: "crc",
            value: expected.to_owned(),
            reason: "must be 8 hex digits",
        });
    }
    let mut body = String::new();
    for line in &lines[..lines.len() - 2] {
        body.push_str(line);
        body.push('\n');
    }
    let computed = checksum(&body);
    if computed != expected.to_ascii_lowercase() {
        return Err(Error::BadChecksum {
            expected: expected.to_owned(),
            computed,
        });
    }
    Ok(lines[..lines.len() - 2].to_vec())
}

struct Fields<'a> {
    pairs: Vec<(&'a str, &'a str)>,
}

impl<'a> Fields<'a> {
    fn parse(content: &[&'a str], allowed: &[&str]) -> Result<Fields<'a>, Error> {
        let mut pairs = Vec::with_capacity(content.len());
        for line in content {
            let (key, value) = line.split_once('=').ok_or_else(|| Error::BadValue {
                field: "line",
                value: line.to_string(),
                reason: "must be key=value",
            })?;
            if !allowed.contains(&key) {
                return Err(Error::UnknownField(key.to_owned()));
            }
            if pairs.iter().any(|(k, _)| *k == key) {
                return Err(Error::DuplicateField(key.to_owned()));
            }
            pairs.push((key, value));
        }
        Ok(Fields { pairs })
    }

    fn get(&self, key: &str) -> Result<&'a str, Error> {
        self.pairs
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .ok_or(Error::MissingField(match key {
                "v" => "v",
                "seq" => "seq",
                "id" => "id",
                "gen" => "gen",
                "action" => "action",
                "status" => "status",
                "code" => "code",
                "requested" => "requested",
                "actual" => "actual",
                "diag" => "diag",
                "addon" => "addon",
                "map" => "map",
                "round" => "round",
                "host" => "host",
                "ready" => "ready",
                "caps" => "caps",
                "tick" => "tick",
                "count" => "count",
                "class" => "class",
                "item" => "item",
                _ => "field",
            }))
    }

    fn maybe(&self, key: &str) -> Option<&'a str> {
        self.pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    fn check_version(&self) -> Result<(), Error> {
        let version = self.get("v")?;
        if version == PROTOCOL_VERSION.to_string() {
            Ok(())
        } else {
            Err(Error::UnsupportedVersion(version.to_owned()))
        }
    }
}

fn finish(content_lines: &[String]) -> String {
    let mut body = String::new();
    for line in content_lines {
        body.push_str(line);
        body.push('\n');
    }
    let crc = checksum(&body);
    body.push_str("crc=");
    body.push_str(&crc);
    body.push('\n');
    body.push_str("END");
    body
}

pub fn encode_command(command: &Command) -> Result<String, Error> {
    check_id(&command.id)?;
    check_generation(&command.generation)?;
    let mut lines = vec![
        format!("v={}", PROTOCOL_VERSION),
        format!("seq={}", command.seq),
        format!("id={}", command.id),
        format!("gen={}", command.generation),
        format!("action={}", command.action.id()),
    ];
    match &command.action {
        Action::TestConnection => {}
        Action::SpawnCommon { count } => {
            if !(1..=COMMON_MAX).contains(count) {
                return Err(Error::Encode("spawn_common count out of range 1-10"));
            }
            lines.push(format!("count={count}"));
        }
        Action::SpawnInfected { class, count } => {
            if !INFECTED_ALLOWLIST.contains(&class.as_str()) {
                return Err(Error::Encode("unknown infected class"));
            }
            let max = if class == "tank" { 1 } else { SPECIAL_MAX };
            if !(1..=max).contains(count) {
                return Err(Error::Encode("spawn_infected count out of range"));
            }
            lines.push(format!("class={class}"));
            lines.push(format!("count={count}"));
        }
        Action::SpawnItem { item, count } => {
            if !ITEM_ALLOWLIST.contains(&item.as_str()) {
                return Err(Error::Encode("unknown item"));
            }
            if !(1..=ITEM_MAX).contains(count) {
                return Err(Error::Encode("spawn_item count out of range 1-5"));
            }
            lines.push(format!("item={item}"));
            lines.push(format!("count={count}"));
        }
    }
    let record = finish(&lines);
    if record.len() > MAX_RECORD_BYTES {
        return Err(Error::Encode("encoded command exceeds 1 KiB"));
    }
    Ok(record)
}

const COMMAND_KEYS: [&str; 8] = ["v", "seq", "id", "gen", "action", "count", "class", "item"];

pub fn decode_command(text: &str) -> Result<Command, Error> {
    let content = split_verified(text)?;
    let fields = Fields::parse(&content, &COMMAND_KEYS)?;
    fields.check_version()?;
    let seq = parse_u64("seq", fields.get("seq")?)?;
    let id = fields.get("id")?;
    check_id(id)?;
    let generation = fields.get("gen")?;
    check_generation(generation)?;
    let action = match fields.get("action")? {
        "test_connection" => {
            if fields.maybe("count").is_some()
                || fields.maybe("class").is_some()
                || fields.maybe("item").is_some()
            {
                return Err(Error::BadValue {
                    field: "action",
                    value: "test_connection".into(),
                    reason: "takes no count/class/item",
                });
            }
            Action::TestConnection
        }
        "spawn_common" => {
            let count = parse_u32("count", fields.get("count")?)?;
            if !(1..=COMMON_MAX).contains(&count) {
                return Err(Error::BadValue {
                    field: "count",
                    value: count.to_string(),
                    reason: "must be 1-10",
                });
            }
            if fields.maybe("class").is_some() || fields.maybe("item").is_some() {
                return Err(Error::BadValue {
                    field: "action",
                    value: "spawn_common".into(),
                    reason: "takes only count",
                });
            }
            Action::SpawnCommon { count }
        }
        "spawn_infected" => {
            let class = fields.get("class")?;
            if !INFECTED_ALLOWLIST.contains(&class) {
                return Err(Error::BadValue {
                    field: "class",
                    value: class.to_owned(),
                    reason: "unknown infected class",
                });
            }
            let count = parse_u32("count", fields.get("count")?)?;
            let max = if class == "tank" { 1 } else { SPECIAL_MAX };
            if !(1..=max).contains(&count) {
                return Err(Error::BadValue {
                    field: "count",
                    value: count.to_string(),
                    reason: "tank is exactly 1, others 1-8",
                });
            }
            if fields.maybe("item").is_some() {
                return Err(Error::BadValue {
                    field: "action",
                    value: "spawn_infected".into(),
                    reason: "takes only class+count",
                });
            }
            Action::SpawnInfected {
                class: class.to_owned(),
                count,
            }
        }
        "spawn_item" => {
            let item = fields.get("item")?;
            if !ITEM_ALLOWLIST.contains(&item) {
                return Err(Error::BadValue {
                    field: "item",
                    value: item.to_owned(),
                    reason: "unknown item",
                });
            }
            let count = parse_u32("count", fields.get("count")?)?;
            if !(1..=ITEM_MAX).contains(&count) {
                return Err(Error::BadValue {
                    field: "count",
                    value: count.to_string(),
                    reason: "must be 1-5",
                });
            }
            if fields.maybe("class").is_some() {
                return Err(Error::BadValue {
                    field: "action",
                    value: "spawn_item".into(),
                    reason: "takes only item+count",
                });
            }
            Action::SpawnItem {
                item: item.to_owned(),
                count,
            }
        }
        other => {
            return Err(Error::BadValue {
                field: "action",
                value: other.to_owned(),
                reason: "unknown action",
            });
        }
    };
    Ok(Command {
        seq,
        id: id.to_owned(),
        generation: generation.to_owned(),
        action,
    })
}

pub fn encode_result(result: &ResultRecord) -> Result<String, Error> {
    check_id(&result.id)?;
    check_generation(&result.generation)?;
    if !is_code(&result.code) {
        return Err(Error::Encode("code must be 1-32 chars of [A-Z0-9_]"));
    }
    if !is_diag(&result.diag) {
        return Err(Error::Encode("diag must be <=128 printable ASCII chars"));
    }
    let lines = vec![
        format!("v={}", PROTOCOL_VERSION),
        format!("seq={}", result.seq),
        format!("id={}", result.id),
        format!("gen={}", result.generation),
        format!("status={}", result.status.as_str()),
        format!("code={}", result.code),
        format!("requested={}", result.requested),
        format!("actual={}", result.actual),
        format!("diag={}", result.diag),
    ];
    let record = finish(&lines);
    if record.len() > MAX_RECORD_BYTES {
        return Err(Error::Encode("encoded result exceeds 1 KiB"));
    }
    Ok(record)
}

const RESULT_KEYS: [&str; 9] = [
    "v",
    "seq",
    "id",
    "gen",
    "status",
    "code",
    "requested",
    "actual",
    "diag",
];

pub fn decode_result(text: &str) -> Result<ResultRecord, Error> {
    let content = split_verified(text)?;
    let fields = Fields::parse(&content, &RESULT_KEYS)?;
    fields.check_version()?;
    let seq = parse_u64("seq", fields.get("seq")?)?;
    let id = fields.get("id")?;
    check_id(id)?;
    let generation = fields.get("gen")?;
    check_generation(generation)?;
    let status = Status::parse(fields.get("status")?).ok_or_else(|| Error::BadValue {
        field: "status",
        value: fields.get("status").unwrap_or("").to_owned(),
        reason: "must be applied|rejected|not_ready|partial|unknown",
    })?;
    let code = fields.get("code")?;
    if !is_code(code) {
        return Err(Error::BadValue {
            field: "code",
            value: code.to_owned(),
            reason: "must be 1-32 chars of [A-Z0-9_]",
        });
    }
    let requested = parse_u32("requested", fields.get("requested")?)?;
    let actual = parse_u32("actual", fields.get("actual")?)?;
    if actual > requested {
        return Err(Error::BadValue {
            field: "actual",
            value: actual.to_string(),
            reason: "actual must not exceed requested",
        });
    }
    let diag = fields.get("diag")?;
    if !is_diag(diag) {
        return Err(Error::BadValue {
            field: "diag",
            value: diag.to_owned(),
            reason: "must be <=128 printable ASCII chars",
        });
    }
    Ok(ResultRecord {
        seq,
        id: id.to_owned(),
        generation: generation.to_owned(),
        status,
        code: code.to_owned(),
        requested,
        actual,
        diag: diag.to_owned(),
    })
}

pub fn encode_heartbeat(heartbeat: &Heartbeat) -> Result<String, Error> {
    check_generation(&heartbeat.generation)?;
    if !is_dotted_version(&heartbeat.addon_version) {
        return Err(Error::Encode("addon version is malformed"));
    }
    if heartbeat.map.len() > MAX_MAP_LEN
        || (!heartbeat.map.is_empty() && !is_token(&heartbeat.map, MAX_MAP_LEN))
    {
        return Err(Error::Encode("map name is malformed"));
    }
    for capability in &heartbeat.capabilities {
        if !CAPABILITY_ALLOWLIST.contains(&capability.as_str()) {
            return Err(Error::Encode("unknown capability token"));
        }
    }
    let flag = |value: bool| if value { "1" } else { "0" };
    let lines = vec![
        format!("v={}", PROTOCOL_VERSION),
        format!("gen={}", heartbeat.generation),
        format!("addon={}", heartbeat.addon_version),
        format!("map={}", heartbeat.map),
        format!("round={}", flag(heartbeat.round_active)),
        format!("host={}", flag(heartbeat.host_authority)),
        format!("ready={}", flag(heartbeat.gameplay_ready)),
        format!("caps={}", heartbeat.capabilities.join(",")),
        format!("tick={}", heartbeat.tick),
    ];
    let record = finish(&lines);
    if record.len() > MAX_RECORD_BYTES {
        return Err(Error::Encode("encoded heartbeat exceeds 1 KiB"));
    }
    Ok(record)
}

const HEARTBEAT_KEYS: [&str; 9] = [
    "v", "gen", "addon", "map", "round", "host", "ready", "caps", "tick",
];

pub fn decode_heartbeat(text: &str) -> Result<Heartbeat, Error> {
    let content = split_verified(text)?;
    let fields = Fields::parse(&content, &HEARTBEAT_KEYS)?;
    fields.check_version()?;
    let generation = fields.get("gen")?;
    check_generation(generation)?;
    let addon_version = fields.get("addon")?;
    if !is_dotted_version(addon_version) {
        return Err(Error::BadValue {
            field: "addon",
            value: addon_version.to_owned(),
            reason: "must be a dotted version",
        });
    }
    let map = fields.get("map")?;
    if map.len() > MAX_MAP_LEN || (!map.is_empty() && !is_token(map, MAX_MAP_LEN)) {
        return Err(Error::BadValue {
            field: "map",
            value: map.to_owned(),
            reason: "must be empty or a map token",
        });
    }
    let round_active = parse_flag("round", fields.get("round")?)?;
    let host_authority = parse_flag("host", fields.get("host")?)?;
    let gameplay_ready = parse_flag("ready", fields.get("ready")?)?;
    let caps_raw = fields.get("caps")?;
    let mut capabilities = Vec::new();
    if !caps_raw.is_empty() {
        for capability in caps_raw.split(',') {
            if !CAPABILITY_ALLOWLIST.contains(&capability) {
                return Err(Error::BadValue {
                    field: "caps",
                    value: capability.to_owned(),
                    reason: "unknown capability token",
                });
            }
            if capabilities.contains(&capability.to_owned()) {
                return Err(Error::BadValue {
                    field: "caps",
                    value: capability.to_owned(),
                    reason: "duplicate capability token",
                });
            }
            capabilities.push(capability.to_owned());
        }
    }
    let tick = parse_u64("tick", fields.get("tick")?)?;
    Ok(Heartbeat {
        generation: generation.to_owned(),
        addon_version: addon_version.to_owned(),
        map: map.to_owned(),
        round_active,
        host_authority,
        gameplay_ready,
        capabilities,
        tick,
    })
}

#[cfg(test)]
mod round_trip {
    use super::*;

    #[test]
    fn command_round_trip() {
        let command = Command {
            seq: 7,
            id: "r_12_7".into(),
            generation: "g-9f2c".into(),
            action: Action::SpawnCommon { count: 5 },
        };
        let text = encode_command(&command).unwrap();
        assert_eq!(decode_command(&text).unwrap(), command);
    }

    #[test]
    fn result_round_trip() {
        let result = ResultRecord {
            seq: 7,
            id: "r_12_7".into(),
            generation: "g-9f2c".into(),
            status: Status::Applied,
            code: "OK".into(),
            requested: 5,
            actual: 5,
            diag: "spawned near survivor Ellis".into(),
        };
        let text = encode_result(&result).unwrap();
        assert_eq!(decode_result(&text).unwrap(), result);
    }

    #[test]
    fn heartbeat_round_trip() {
        let heartbeat = Heartbeat {
            generation: "g-9f2c".into(),
            addon_version: "0.1.0".into(),
            map: "c1m1_hotel".into(),
            round_active: true,
            host_authority: true,
            gameplay_ready: true,
            capabilities: vec!["spawn_common".into(), "spawn_item".into()],
            tick: 42,
        };
        let text = encode_heartbeat(&heartbeat).unwrap();
        assert_eq!(decode_heartbeat(&text).unwrap(), heartbeat);
    }

    #[test]
    fn rejects_partial_write() {
        let command = Command {
            seq: 1,
            id: "r_1".into(),
            generation: "g-1".into(),
            action: Action::TestConnection,
        };
        let mut text = encode_command(&command).unwrap();
        text.truncate(text.len() - 8);
        assert!(decode_command(&text).is_err());
    }

    #[test]
    fn tank_is_exactly_one() {
        let command = Command {
            seq: 1,
            id: "r_1".into(),
            generation: "g-1".into(),
            action: Action::SpawnInfected {
                class: "tank".into(),
                count: 2,
            },
        };
        assert!(encode_command(&command).is_err());
    }

    #[test]
    fn actual_cannot_exceed_requested() {
        let text = encode_result(&ResultRecord {
            seq: 1,
            id: "r_1".into(),
            generation: "g-1".into(),
            status: Status::Partial,
            code: "ENTITY_CAP".into(),
            requested: 3,
            actual: 3,
            diag: String::new(),
        })
        .unwrap();
        let inflated = text.replace("actual=3", "actual=9");
        // Recompute a valid checksum over the tampered body on purpose is
        // out of scope here; the point is the semantic check fires even
        // when framing is intact.
        let tampered = {
            let mut lines: Vec<&str> = inflated.lines().collect();
            lines.pop();
            lines.pop();
            let mut body = String::new();
            for line in &lines {
                body.push_str(line);
                body.push('\n');
            }
            let crc = checksum(&body);
            format!("{body}crc={crc}\nEND")
        };
        assert!(decode_result(&tampered).is_err());
    }
}
