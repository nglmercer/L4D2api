#!/usr/bin/env python3
"""Regenerate tests/fixtures/*.txt with valid v1 framing.

Bodies are hand-written below; FNV-1a checksums are computed so fixtures
always match the canonical encoding (body lines + trailing newline each).
Malformed fixtures are derived from the valid ones by construction.
Run from the repo root:  python3 scripts/gen_fixtures.py
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


FIXTURES: dict[str, str] = {
    "command_valid.txt": framed([
        "v=1", "seq=7", "id=r_12_7", "gen=g-9f2c",
        "action=spawn_common", "count=5",
    ]),
    "result_applied.txt": framed([
        "v=1", "seq=7", "id=r_12_7", "gen=g-9f2c",
        "status=applied", "code=OK", "requested=5", "actual=5",
        "diag=spawned near survivor Ellis",
    ]),
    "heartbeat_ready.txt": framed([
        "v=1", "gen=g-9f2c", "addon=0.1.0", "map=c1m1_hotel",
        "round=1", "host=1", "ready=1",
        "caps=test_connection,spawn_common,spawn_item", "tick=42",
    ]),
    "result_rejected_not_host.txt": framed([
        "v=1", "seq=9", "id=r_12_9", "gen=g-9f2c",
        "status=rejected", "code=NOT_HOST", "requested=1", "actual=0",
        "diag=not the authoritative host",
    ]),
    "expired_old_generation.txt": framed([
        "v=1", "seq=3", "id=r_11_3", "gen=g-old",
        "action=test_connection",
    ]),
}

# Derived malformed fixtures.
valid = FIXTURES["command_valid.txt"]
FIXTURES["malformed_bad_crc.txt"] = valid.replace(
    valid.splitlines()[-2], "crc=00000000"
)
FIXTURES["truncated_no_end.txt"] = "\n".join(valid.splitlines()[:-2])
dup_lines = valid.splitlines()
dup_lines.insert(2, "id=r_evil")
dup_body = "\n".join(dup_lines[:-2]) + "\n"
FIXTURES["malformed_duplicate_key.txt"] = dup_body + f"crc={fnv1a(dup_body)}\nEND"
unknown_lines = [ln if not ln.startswith("action=") else "action=spawn_tank"
                 for ln in valid.splitlines()[:-2]]
unknown_body = "\n".join(unknown_lines) + "\n"
FIXTURES["malformed_unknown_action.txt"] = unknown_body + f"crc={fnv1a(unknown_body)}\nEND"


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
    return dirty


if __name__ == "__main__":
    sys.exit(main())
