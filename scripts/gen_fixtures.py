#!/usr/bin/env python3
"""Regenerate tests/fixtures/*.txt with valid v1 framing.

Bodies are hand-written below; FNV-1a checksums are computed so fixtures
always match the canonical encoding (body lines + trailing newline each).
Malformed fixtures are derived from valid ones by construction, except
where the corruption itself must be hand-built (noted inline).

Framing-valid fixtures that decode successfully but must never execute
(old generation, out-of-order, duplicate) carry a `.note` sidecar
explaining the session-level rule; decoders accept them, session logic
rejects them.

Run from the repo root:  python3 scripts/gen_fixtures.py [--check]
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIX = ROOT / "tests" / "fixtures"


def fnv1a(body: str) -> str:
    h = 0x811C9DC5
    for byte in body.encode("ascii"):
        h ^= byte
        h = (h * 0x01000193) & 0xFFFFFFFF
    return f"{h:08x}"


def framed(lines: list[str]) -> str:
    body = "".join(line + "\n" for line in lines)
    return body + f"crc={fnv1a(body)}\nEND"


def base_command(extra: list[str] | None = None) -> list[str]:
    lines = [
        "v=1", "seq=7", "id=r_12_7", "gen=g-9f2c",
        "action=spawn_common", "count=5",
    ]
    return lines + (extra or [])


FIXTURES: dict[str, str] = {
    # --- valid commands ---
    "command_valid.txt": framed(base_command()),
    "command_test_connection.txt": framed([
        "v=1", "seq=1", "id=r_ping_1", "gen=g-9f2c",
        "action=test_connection",
    ]),
    "command_spawn_item.txt": framed([
        "v=1", "seq=2", "id=r_item_2", "gen=g-9f2c",
        "action=spawn_item", "item=first_aid_kit", "count=2",
    ]),
    "command_spawn_weapon.txt": framed([
        "v=1", "seq=3", "id=r_weapon_3", "gen=g-9f2c",
        "action=spawn_weapon", "weapon=weapon_rifle_ak47", "count=1",
    ]),
    "command_spawn_infected.txt": framed([
        "v=1", "seq=4", "id=r_infected_4", "gen=g-9f2c",
        "action=spawn_infected", "class=boomer", "count=3",
    ]),
    # --- valid results / heartbeat ---
    "result_applied.txt": framed([
        "v=1", "seq=7", "id=r_12_7", "gen=g-9f2c",
        "status=applied", "code=OK", "requested=5", "actual=5",
        "diag=spawned near survivor Ellis",
    ]),
    "result_partial.txt": framed([
        "v=1", "seq=8", "id=r_12_8", "gen=g-9f2c",
        "status=partial", "code=PARTIAL", "requested=5", "actual=3",
        "diag=spawned 3 of 5 weapon_first_aid_kit",
    ]),
    "result_rejected_not_host.txt": framed([
        "v=1", "seq=9", "id=r_12_9", "gen=g-9f2c",
        "status=rejected", "code=NOT_HOST", "requested=1", "actual=0",
        "diag=not the authoritative host",
    ]),
    "heartbeat_ready.txt": framed([
        "v=1", "gen=g-9f2c", "addon=0.1.0", "map=c1m1_hotel",
        "round=1", "host=1", "ready=1",
        "caps=test_connection,spawn_common,spawn_item", "tick=42",
    ]),
    # --- framing-valid but never executable (session-level rule) ---
    "expired_old_generation.txt": framed([
        "v=1", "seq=3", "id=r_11_3", "gen=g-old",
        "action=test_connection",
    ]),
    "out_of_order_command.txt": framed([
        "v=1", "seq=2", "id=r_12_2", "gen=g-9f2c",
        "action=spawn_common", "count=1",
    ]),
    "duplicate_request.txt": framed(base_command()),
}

NOTES: dict[str, str] = {
    "expired_old_generation.txt.note":
        "Decodes fine; gen=g-old differs from the live session generation, "
        "so both sides must ignore it (client drops cross-generation replies, "
        "game ignores stale commands).",
    "out_of_order_command.txt.note":
        "Decodes fine (seq=2); a session that already accepted seq=7 in "
        "gen=g-9f2c must reject it as out-of-order and never re-execute.",
    "duplicate_request.txt.note":
        "Byte-identical to command_valid.txt; a session that already accepted "
        "id=r_12_7 must not execute it twice.",
}

# Derived malformed fixtures.
valid = FIXTURES["command_valid.txt"]
FIXTURES["malformed_bad_crc.txt"] = valid.replace(
    valid.splitlines()[-2], "crc=00000000"
)
FIXTURES["truncated_no_end.txt"] = "\n".join(valid.splitlines()[:-1])
partial_cut = valid[: len(valid) // 2]
FIXTURES["truncated_partial.txt"] = partial_cut
dup_lines = valid.splitlines()
dup_lines.insert(2, "id=r_evil")
dup_body = "\n".join(dup_lines[:-2]) + "\n"
FIXTURES["malformed_duplicate_key.txt"] = dup_body + f"crc={fnv1a(dup_body)}\nEND"
unknown_lines = [ln if not ln.startswith("action=") else "action=spawn_tank"
                 for ln in valid.splitlines()[:-2]]
unknown_body = "\n".join(unknown_lines) + "\n"
FIXTURES["malformed_unknown_action.txt"] = unknown_body + f"crc={fnv1a(unknown_body)}\nEND"
extra_lines = valid.splitlines()[:-2] + ["extra=1"]
extra_body = "\n".join(extra_lines) + "\n"
FIXTURES["malformed_unknown_key.txt"] = extra_body + f"crc={fnv1a(extra_body)}\nEND"
badint_lines = [ln if not ln.startswith("seq=") else "seq=abc"
                for ln in valid.splitlines()[:-2]]
badint_body = "\n".join(badint_lines) + "\n"
FIXTURES["malformed_invalid_integer.txt"] = badint_body + f"crc={fnv1a(badint_body)}\nEND"
badcount_lines = [ln if not ln.startswith("count=") else "count=99"
                  for ln in valid.splitlines()[:-2]]
badcount_body = "\n".join(badcount_lines) + "\n"
FIXTURES["malformed_invalid_params.txt"] = badcount_body + f"crc={fnv1a(badcount_body)}\nEND"
v2_lines = [ln if not ln.startswith("v=") else "v=2"
            for ln in valid.splitlines()[:-2]]
v2_body = "\n".join(v2_lines) + "\n"
FIXTURES["unsupported_version.txt"] = v2_body + f"crc={fnv1a(v2_body)}\nEND"
big_body = "v=1\n" + "diag=" + "x" * 2000 + "\n"
FIXTURES["oversized_record.txt"] = big_body + f"crc={fnv1a(big_body)}\nEND"


def main() -> int:
    check = "--check" in sys.argv[1:]
    FIX.mkdir(parents=True, exist_ok=True)
    dirty = 0
    for name, text in sorted(FIXTURES.items()):
        path = FIX / name
        if check:
            if not path.is_file() or path.read_text(encoding="ascii") != text:
                print(f"STALE: tests/fixtures/{name}", file=sys.stderr)
                dirty = 1
            continue
        path.write_text(text, encoding="ascii")
        print(f"wrote tests/fixtures/{name}")
    for name, text in sorted(NOTES.items()):
        path = FIX / name
        if check:
            if not path.is_file() or path.read_text(encoding="ascii") != text:
                print(f"STALE: tests/fixtures/{name}", file=sys.stderr)
                dirty = 1
            continue
        path.write_text(text, encoding="ascii")
        print(f"wrote tests/fixtures/{name}")
    return dirty


if __name__ == "__main__":
    sys.exit(main())
