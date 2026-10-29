"""Parity with the `l4d2api-protocol` Rust reference codec.

Decodes every shared fixture in `tests/fixtures/` both ways: the Python
encoder must reproduce each valid fixture byte-for-byte (checksums and
framing included), and every malformed fixture must raise.
"""

import unittest
from pathlib import Path

from l4d2api import protocol

FIX = Path(__file__).resolve().parents[3] / "tests" / "fixtures"


def fixture(name):
    return (FIX / name).read_text(encoding="ascii")


class TestFixtures(unittest.TestCase):
    def test_valid_command(self):
        cmd = protocol.decode_command(fixture("command_valid.txt"))
        self.assertEqual(cmd.seq, 7)
        self.assertEqual(cmd.id, "r_12_7")
        self.assertEqual(cmd.generation, "g-9f2c")
        self.assertEqual(cmd.action, "spawn_common")
        self.assertEqual(cmd.params, {"count": "5"})

    def test_per_action_commands(self):
        cmd = protocol.decode_command(fixture("command_test_connection.txt"))
        self.assertEqual((cmd.action, cmd.params), ("test_connection", {}))
        cmd = protocol.decode_command(fixture("command_spawn_item.txt"))
        self.assertEqual(cmd.params, {"item": "first_aid_kit", "count": "2"})
        cmd = protocol.decode_command(fixture("command_spawn_weapon.txt"))
        self.assertEqual(cmd.params, {"weapon": "weapon_rifle_ak47", "count": "1"})
        cmd = protocol.decode_command(fixture("command_spawn_infected.txt"))
        self.assertEqual(cmd.params, {"class": "boomer", "count": "3"})

    def test_encoder_reproduces_fixtures_byte_for_byte(self):
        for name in (
            "command_valid.txt",
            "command_test_connection.txt",
            "command_spawn_item.txt",
            "command_spawn_weapon.txt",
            "command_spawn_infected.txt",
        ):
            raw = fixture(name)
            self.assertEqual(protocol.encode_command(protocol.decode_command(raw)), raw, name)

    def test_result_and_heartbeat(self):
        result = protocol.decode_result(fixture("result_applied.txt"))
        self.assertEqual((result.status, result.code), ("applied", "OK"))
        self.assertEqual((result.requested, result.actual), (5, 5))
        partial = protocol.decode_result(fixture("result_partial.txt"))
        self.assertEqual((partial.status, partial.code), ("partial", "PARTIAL"))
        hb = protocol.decode_heartbeat(fixture("heartbeat_ready.txt"))
        self.assertTrue(hb.host_authority and hb.gameplay_ready)
        self.assertEqual(hb.tick, 42)
        self.assertEqual(
            protocol.encode_heartbeat(hb), fixture("heartbeat_ready.txt")
        )

    def test_malformed_rejected(self):
        for name in (
            "malformed_bad_crc.txt",
            "truncated_no_end.txt",
            "truncated_partial.txt",
            "malformed_duplicate_key.txt",
            "malformed_unknown_action.txt",
            "malformed_unknown_key.txt",
            "malformed_invalid_integer.txt",
            "malformed_invalid_params.txt",
            "unsupported_version.txt",
            "oversized_record.txt",
        ):
            with self.assertRaises(protocol.ProtocolError, msg=name):
                protocol.decode_command(fixture(name))

    def test_session_level_fixtures_stay_framing_valid(self):
        # Decoders accept them; session logic (generation/ordering/
        # duplication) must reject them. See test_client.py.
        for name in (
            "expired_old_generation.txt",
            "out_of_order_command.txt",
            "duplicate_request.txt",
        ):
            protocol.decode_command(fixture(name))

    def test_checksum_matches_reference(self):
        # FNV-1a offset basis is the empty-input vector; the encoder
        # reproducing every valid fixture byte-for-byte pins the rest.
        self.assertEqual(protocol.checksum(""), "811c9dc5")
        cmd = protocol.Command(seq=7, id="r_12_7", generation="g-9f2c",
                               action="spawn_common", params={"count": "5"})
        self.assertEqual(protocol.encode_command(cmd), fixture("command_valid.txt"))


if __name__ == "__main__":
    unittest.main()
