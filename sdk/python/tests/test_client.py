"""Client tests against a mock file-IPC game peer (stdlib only)."""

import tempfile
import threading
import time
import unittest
from pathlib import Path

from l4d2api import protocol
from l4d2api.client import L4D2Client
from l4d2api.errors import LockedError, StaleHeartbeatError, TimeoutError


def heartbeat_text(generation="g-1", tick=1, caps=None):
    return protocol.encode_heartbeat(
        protocol.Heartbeat(
            generation=generation,
            addon_version="0.1.0",
            map="c1m1_hotel",
            round_active=True,
            host_authority=True,
            gameplay_ready=True,
            capabilities=caps or ["test_connection", "spawn_common", "spawn_item"],
            tick=tick,
        )
    )


class Peer(threading.Thread):
    """Fake VScript side: answers each valid command once, applied."""

    def __init__(self, ipc_dir, delay=0.0):
        super().__init__(daemon=True)
        self.dir = Path(ipc_dir)
        self.delay = delay
        self.seen = []

    def run(self):
        for _ in range(2000):
            try:
                raw = (self.dir / "command.txt").read_text(encoding="ascii")
            except OSError:
                time.sleep(0.005)
                continue
            try:
                cmd = protocol.decode_command(raw)
            except Exception:
                time.sleep(0.005)
                continue
            if cmd.id in self.seen:
                time.sleep(0.005)
                continue
            self.seen.append(cmd.id)
            if self.delay:
                time.sleep(self.delay)
            reply = protocol.encode_result(
                protocol.ResultRecord(
                    seq=cmd.seq,
                    id=cmd.id,
                    generation=cmd.generation,
                    status="applied",
                    code="OK",
                    requested=int(cmd.params.get("count", "0")),
                    actual=int(cmd.params.get("count", "0")),
                    diag="",
                )
            )
            (self.dir / "result.txt").write_text(reply, encoding="ascii")
            return


class TestClient(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        (self.dir / "heartbeat.txt").write_text(heartbeat_text(), encoding="ascii")

    def tearDown(self):
        self.tmp.cleanup()

    def make(self, **kwargs):
        kwargs.setdefault("timeout", 5.0)
        kwargs.setdefault("poll_interval", 0.005)
        return L4D2Client(ipc_dir=self.dir, **kwargs)

    def test_connect_and_round_trip(self):
        peer = Peer(self.dir)
        peer.start()
        with self.make() as client:
            hb = client.connect()
            self.assertEqual(hb.generation, "g-1")
            result = client.send("r_1", "spawn_common", count="3")
            self.assertEqual((result.status, result.actual), ("applied", 3))
            self.assertEqual(result.request_id, "r_1")
        peer.join(timeout=10)

    def test_timeout_is_not_success(self):
        client = self.make(timeout=0.08, poll_interval=0.01)
        client.connect()
        with self.assertRaises(TimeoutError):
            client.send("r_9", "test_connection")

    def test_stale_heartbeat_never_ready(self):
        client = self.make(heartbeat_timeout=0.04)
        client.connect()
        time.sleep(0.08)
        status = client.status()
        self.assertEqual(status.health, "stale")
        with self.assertRaises(StaleHeartbeatError):
            client.send("r_s", "test_connection")

    def test_generation_change_rejected(self):
        client = self.make()
        client.connect()
        (self.dir / "heartbeat.txt").write_text(
            heartbeat_text(generation="g-2", tick=2), encoding="ascii"
        )
        from l4d2api.errors import StaleGenerationError

        with self.assertRaises(StaleGenerationError):
            client.send("r_2", "test_connection")

    def test_second_writer_locked(self):
        first = self.make()
        first.connect()
        first._acquire_lock()
        try:
            second = self.make()
            second.connect()
            with self.assertRaises(LockedError):
                second.send("r_lock", "test_connection")
            self.assertFalse((self.dir / "command.txt").exists())
        finally:
            first._release_lock()

    def test_replacing_existing_command_file(self):
        (self.dir / "command.txt").write_text("stale-bytes", encoding="ascii")
        peer = Peer(self.dir)
        peer.start()
        with self.make() as client:
            client.connect()
            result = client.test_connection()
            self.assertEqual(result.status, "applied")
        peer.join(timeout=10)
        leftovers = list(self.dir.glob("*.tmp"))
        self.assertEqual(leftovers, [])

    def test_unsupported_capability_fails_before_write(self):
        (self.dir / "heartbeat.txt").write_text(
            heartbeat_text(caps=["test_connection"]), encoding="ascii"
        )
        client = self.make()
        client.connect()
        from l4d2api.errors import UnsupportedCapabilityError

        with self.assertRaises(UnsupportedCapabilityError):
            client.spawn_item("first_aid_kit", 1)
        self.assertFalse((self.dir / "command.txt").exists())

    def test_stale_reply_with_recycled_id_ignored(self):
        stale = protocol.encode_result(
            protocol.ResultRecord(
                seq=999, id="r_new", generation="g-1", status="applied",
                code="OK", requested=0, actual=0, diag="",
            )
        )
        (self.dir / "result.txt").write_text(stale, encoding="ascii")
        peer = Peer(self.dir, delay=0.05)
        peer.start()
        with self.make() as client:
            client.connect()
            result = client.send("r_new", "spawn_common", count="3")
            self.assertEqual(result.status, "applied")
            self.assertEqual(result.actual, 3)
        peer.join(timeout=10)


if __name__ == "__main__":
    unittest.main()
