"""Canonical v1 protocol codec: framing, FNV-1a checksums, allowlists.

This is the Python twin of the `l4d2api-protocol` Rust crate, which stays
the reference implementation. Every encoder/decoder here agrees with the
Rust codec on checksums and framing; `tests/test_protocol.py` pins the
shared fixtures in `tests/fixtures/`.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from .errors import ProtocolError

PROTOCOL_VERSION = 1
MAX_RECORD_BYTES = 1024
MAX_ID_LEN = 64
MAX_GEN_LEN = 64
MAX_DIAG_LEN = 128
MAX_MAP_LEN = 64

COMMON_MAX = 10
SPECIAL_MAX = 8
ITEM_MAX = 5
WEAPON_MAX = 5

INFECTED_ALLOWLIST = frozenset(
    ["tank", "witch", "smoker", "boomer", "hunter", "spitter", "jockey", "charger"]
)
ITEM_ALLOWLIST = frozenset(
    ["first_aid_kit", "pain_pills", "adrenaline", "pipe_bomb", "molotov", "defibrillator"]
)
WEAPON_ALLOWLIST = frozenset(
    [
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
    ]
)
CAPABILITY_ALLOWLIST = frozenset(
    ["test_connection", "spawn_common", "spawn_infected", "spawn_item", "spawn_weapon"]
)
ACTION_NAMES = frozenset(
    ["test_connection", "spawn_common", "spawn_infected", "spawn_item", "spawn_weapon"]
)


def checksum(body: str) -> str:
    """FNV-1a (32-bit) over ASCII bytes, as 8 lowercase hex digits."""
    h = 0x811C9DC5
    for byte in body.encode("ascii"):
        h ^= byte
        h = (h * 0x01000193) & 0xFFFFFFFF
    return f"{h:08x}"


def _is_token(text: str, max_len: int) -> bool:
    if not text or len(text) > max_len:
        return False
    return all(c.isascii() and (c.isalnum() or c in "_-") for c in text)


def _is_code(text: str) -> bool:
    return bool(text) and len(text) <= 32 and all(
        c.isascii() and (c.isupper() or c.isdigit() or c == "_") for c in text
    )


def _is_diag(text: str) -> bool:
    return len(text) <= MAX_DIAG_LEN and all(
        c == " " or (0x21 <= ord(c) <= 0x7E) for c in text
    )


def _split_verified(text: str) -> list[str]:
    if len(text) > MAX_RECORD_BYTES:
        raise ProtocolError(f"record exceeds {MAX_RECORD_BYTES} bytes")
    if not text.isascii():
        raise ProtocolError("record is not ASCII")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    for i, line in enumerate(lines):
        if line.endswith("\r"):
            lines[i] = line[:-1]
    if not lines or lines[-1] != "END":
        raise ProtocolError("record must end with an END sentinel line")
    if len(lines) < 2:
        raise ProtocolError("record is missing its crc line")
    crc_line = lines[-2]
    if len(crc_line) != 12 or not crc_line.startswith("crc="):
        raise ProtocolError("record is missing its crc line")
    expected = crc_line[4:]
    if len(expected) != 8 or any(c not in "0123456789abcdefABCDEF" for c in expected):
        raise ProtocolError("crc must be 8 hex digits")
    body = "".join(line + "\n" for line in lines[:-2])
    if checksum(body) != expected.lower():
        raise ProtocolError(
            f"checksum mismatch: expected {expected}, computed {checksum(body)}"
        )
    return lines[:-2]


def _parse_fields(content: list[str], allowed: frozenset[str]) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in content:
        key, sep, value = line.partition("=")
        if not sep or not key:
            raise ProtocolError(f"line is not key=value: {line!r}")
        if key in fields:
            raise ProtocolError(f"duplicate field `{key}`")
        if key not in allowed:
            raise ProtocolError(f"unknown field `{key}`")
        fields[key] = value
    return fields


def _parse_u64(field: str, value: str) -> int:
    if not value.isascii() or not value.isdigit():
        raise ProtocolError(f"field `{field}` must be a non-negative integer")
    try:
        return int(value)
    except ValueError:
        raise ProtocolError(f"field `{field}` must be a non-negative integer")


def _finish(lines: list[str]) -> str:
    body = "".join(line + "\n" for line in lines)
    return body + f"crc={checksum(body)}\nEND"


@dataclass
class Command:
    seq: int
    id: str
    generation: str
    action: str
    params: dict[str, str] = field(default_factory=dict)

    def requested_count(self) -> int:
        return int(self.params.get("count", "0")) if self.action != "test_connection" else 0


_COMMAND_KEYS = frozenset(["v", "seq", "id", "gen", "action", "count", "class", "item", "weapon"])


def _check_action(action: str, fields: dict[str, str]) -> dict[str, str]:
    def count(lo: int, hi: int) -> int:
        if "count" not in fields:
            raise ProtocolError("missing field `count`")
        value = _parse_u64("count", fields["count"])
        if not lo <= value <= hi:
            raise ProtocolError(f"field `count` must be {lo}-{hi}")
        return value

    if action == "test_connection":
        if any(k in fields for k in ("count", "class", "item", "weapon")):
            raise ProtocolError("test_connection takes no count/class/item/weapon")
        return {}
    if action == "spawn_common":
        return {"count": str(count(1, COMMON_MAX))}
    if action == "spawn_infected":
        cls = fields.get("class")
        if cls is None:
            raise ProtocolError("missing field `class`")
        if cls not in INFECTED_ALLOWLIST:
            raise ProtocolError(f"unknown infected class `{cls}`")
        hi = 1 if cls == "tank" else SPECIAL_MAX
        return {"class": cls, "count": str(count(1, hi))}
    if action == "spawn_item":
        item = fields.get("item")
        if item is None:
            raise ProtocolError("missing field `item`")
        if item not in ITEM_ALLOWLIST:
            raise ProtocolError(f"unknown item `{item}`")
        return {"item": item, "count": str(count(1, ITEM_MAX))}
    if action == "spawn_weapon":
        weapon = fields.get("weapon")
        if weapon is None:
            raise ProtocolError("missing field `weapon`")
        if weapon not in WEAPON_ALLOWLIST:
            raise ProtocolError(f"unknown weapon `{weapon}`")
        return {"weapon": weapon, "count": str(count(1, WEAPON_MAX))}
    raise ProtocolError(f"unknown action `{action}`")


def _expected_keys(action: str) -> frozenset[str]:
    base = {"v", "seq", "id", "gen", "action"}
    return base | {
        "test_connection": set(),
        "spawn_common": {"count"},
        "spawn_infected": {"class", "count"},
        "spawn_item": {"item", "count"},
        "spawn_weapon": {"weapon", "count"},
    }[action]


def encode_command(command: Command) -> str:
    if not _is_token(command.id, MAX_ID_LEN):
        raise ProtocolError("id must be 1-64 chars of [A-Za-z0-9_-]")
    if not _is_token(command.generation, MAX_GEN_LEN):
        raise ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]")
    if command.action not in ACTION_NAMES:
        raise ProtocolError(f"unknown action `{command.action}`")
    params = _check_action(command.action, command.params)
    lines = [
        f"v={PROTOCOL_VERSION}",
        f"seq={command.seq}",
        f"id={command.id}",
        f"gen={command.generation}",
        f"action={command.action}",
    ]
    for key in ("class", "item", "weapon", "count"):
        if key in params:
            lines.append(f"{key}={params[key]}")
    record = _finish(lines)
    if len(record) > MAX_RECORD_BYTES:
        raise ProtocolError("encoded command exceeds 1 KiB")
    return record


def decode_command(text: str) -> Command:
    fields = _parse_fields(_split_verified(text), _COMMAND_KEYS)
    if fields.get("v") != str(PROTOCOL_VERSION):
        raise ProtocolError(f"unsupported protocol version `{fields.get('v')}`")
    for key in ("seq", "id", "gen", "action"):
        if key not in fields:
            raise ProtocolError(f"missing field `{key}`")
    seq = _parse_u64("seq", fields["seq"])
    if not _is_token(fields["id"], MAX_ID_LEN):
        raise ProtocolError("id must be 1-64 chars of [A-Za-z0-9_-]")
    if not _is_token(fields["gen"], MAX_GEN_LEN):
        raise ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]")
    action = fields["action"]
    if action not in ACTION_NAMES:
        raise ProtocolError(f"unknown action `{action}`")
    expected = _expected_keys(action)
    for key in fields:
        if key not in expected:
            raise ProtocolError(f"unexpected field `{key}` for action `{action}`")
    params = _check_action(action, fields)
    return Command(seq=seq, id=fields["id"], generation=fields["gen"],
                   action=action, params=params)


@dataclass
class ResultRecord:
    seq: int
    id: str
    generation: str
    status: str
    code: str
    requested: int
    actual: int
    diag: str = ""


_RESULT_KEYS = frozenset(
    ["v", "seq", "id", "gen", "status", "code", "requested", "actual", "diag"]
)
_STATUSES = frozenset(["applied", "rejected", "not_ready", "partial", "unknown"])


def encode_result(result: ResultRecord) -> str:
    if not _is_token(result.id, MAX_ID_LEN):
        raise ProtocolError("id must be 1-64 chars of [A-Za-z0-9_-]")
    if not _is_token(result.generation, MAX_GEN_LEN):
        raise ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]")
    if result.status not in _STATUSES:
        raise ProtocolError("status must be applied|rejected|not_ready|partial|unknown")
    if not _is_code(result.code):
        raise ProtocolError("code must be 1-32 chars of [A-Z0-9_]")
    if not _is_diag(result.diag):
        raise ProtocolError("diag must be <=128 printable ASCII chars")
    if result.actual > result.requested:
        raise ProtocolError("actual must not exceed requested")
    record = _finish(
        [
            f"v={PROTOCOL_VERSION}",
            f"seq={result.seq}",
            f"id={result.id}",
            f"gen={result.generation}",
            f"status={result.status}",
            f"code={result.code}",
            f"requested={result.requested}",
            f"actual={result.actual}",
            f"diag={result.diag}",
        ]
    )
    if len(record) > MAX_RECORD_BYTES:
        raise ProtocolError("encoded result exceeds 1 KiB")
    return record


def decode_result(text: str) -> ResultRecord:
    fields = _parse_fields(_split_verified(text), _RESULT_KEYS)
    if fields.get("v") != str(PROTOCOL_VERSION):
        raise ProtocolError(f"unsupported protocol version `{fields.get('v')}`")
    for key in ("seq", "id", "gen", "status", "code", "requested", "actual", "diag"):
        if key not in fields:
            raise ProtocolError(f"missing field `{key}`")
    if fields["status"] not in _STATUSES:
        raise ProtocolError("status must be applied|rejected|not_ready|partial|unknown")
    if not _is_code(fields["code"]):
        raise ProtocolError("code must be 1-32 chars of [A-Z0-9_]")
    requested = _parse_u64("requested", fields["requested"])
    actual = _parse_u64("actual", fields["actual"])
    if actual > requested:
        raise ProtocolError("actual must not exceed requested")
    if not _is_diag(fields["diag"]):
        raise ProtocolError("diag must be <=128 printable ASCII chars")
    return ResultRecord(
        seq=_parse_u64("seq", fields["seq"]),
        id=fields["id"],
        generation=fields["gen"],
        status=fields["status"],
        code=fields["code"],
        requested=requested,
        actual=actual,
        diag=fields["diag"],
    )


@dataclass
class Heartbeat:
    generation: str
    addon_version: str
    map: str
    round_active: bool
    host_authority: bool
    gameplay_ready: bool
    capabilities: list[str] = field(default_factory=list)
    tick: int = 0


_HEARTBEAT_KEYS = frozenset(
    ["v", "gen", "addon", "map", "round", "host", "ready", "caps", "tick"]
)


def _is_version(text: str) -> bool:
    return bool(text) and len(text) <= MAX_ID_LEN and all(
        c.isascii() and (c.isalnum() or c in "._-") for c in text
    )


def encode_heartbeat(hb: Heartbeat) -> str:
    if not _is_token(hb.generation, MAX_GEN_LEN):
        raise ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]")
    if not _is_version(hb.addon_version):
        raise ProtocolError("addon version is malformed")
    if hb.map and not _is_token(hb.map, MAX_MAP_LEN):
        raise ProtocolError("map name is malformed")
    for cap in hb.capabilities:
        if cap not in CAPABILITY_ALLOWLIST:
            raise ProtocolError(f"unknown capability token `{cap}`")
    flag = lambda v: "1" if v else "0"
    record = _finish(
        [
            f"v={PROTOCOL_VERSION}",
            f"gen={hb.generation}",
            f"addon={hb.addon_version}",
            f"map={hb.map}",
            f"round={flag(hb.round_active)}",
            f"host={flag(hb.host_authority)}",
            f"ready={flag(hb.gameplay_ready)}",
            f"caps={','.join(hb.capabilities)}",
            f"tick={hb.tick}",
        ]
    )
    if len(record) > MAX_RECORD_BYTES:
        raise ProtocolError("encoded heartbeat exceeds 1 KiB")
    return record


def decode_heartbeat(text: str) -> Heartbeat:
    fields = _parse_fields(_split_verified(text), _HEARTBEAT_KEYS)
    if fields.get("v") != str(PROTOCOL_VERSION):
        raise ProtocolError(f"unsupported protocol version `{fields.get('v')}`")
    for key in ("gen", "addon", "map", "round", "host", "ready", "caps", "tick"):
        if key not in fields:
            raise ProtocolError(f"missing field `{key}`")
    if not _is_token(fields["gen"], MAX_GEN_LEN):
        raise ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]")
    if not _is_version(fields["addon"]):
        raise ProtocolError("addon must be a dotted version")
    if fields["map"] and not _is_token(fields["map"], MAX_MAP_LEN):
        raise ProtocolError("map must be empty or a map token")
    for flag_key in ("round", "host", "ready"):
        if fields[flag_key] not in ("0", "1"):
            raise ProtocolError(f"field `{flag_key}` must be 0 or 1")
    caps: list[str] = []
    if fields["caps"]:
        for cap in fields["caps"].split(","):
            if cap not in CAPABILITY_ALLOWLIST:
                raise ProtocolError(f"unknown capability token `{cap}`")
            if cap in caps:
                raise ProtocolError(f"duplicate capability token `{cap}`")
            caps.append(cap)
    return Heartbeat(
        generation=fields["gen"],
        addon_version=fields["addon"],
        map=fields["map"],
        round_active=fields["round"] == "1",
        host_authority=fields["host"] == "1",
        gameplay_ready=fields["ready"] == "1",
        capabilities=caps,
        tick=_parse_u64("tick", fields["tick"]),
    )
