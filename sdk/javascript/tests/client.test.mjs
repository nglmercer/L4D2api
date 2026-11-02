/** Client tests against a mock file-IPC game peer. */
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

import {
  L4D2Client,
  LockedError,
  StaleHeartbeatError,
  TimeoutError,
  UnsupportedCapabilityError,
  StaleGenerationError,
  encodeHeartbeat,
  encodeResult,
  decodeCommand,
} from "../dist/index.js";

function heartbeatText(overrides = {}) {
  return encodeHeartbeat({
    generation: "g-1",
    addonVersion: "0.1.0",
    map: "c1m1_hotel",
    roundActive: true,
    hostAuthority: true,
    gameplayReady: true,
    capabilities: ["test_connection", "spawn_common", "spawn_item"],
    tick: 1,
    ...overrides,
  });
}

/** Fake VScript side: answers each valid command once, applied. */
async function runPeer(dir, { seen = new Set(), delayMs = 0 } = {}) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    let raw = null;
    try {
      raw = await fs.readFile(path.join(dir, "command.txt"), "ascii");
    } catch { /* not yet written */ }
    if (raw !== null) {
      try {
        const cmd = decodeCommand(raw);
        if (!seen.has(cmd.id)) {
          seen.add(cmd.id);
          if (delayMs) await new Promise((r) => setTimeout(r, delayMs));
          const count = Number(cmd.params.count ?? "0");
          await fs.writeFile(
            path.join(dir, "result.txt"),
            encodeResult({
              seq: cmd.seq, id: cmd.id, generation: cmd.generation,
              status: "applied", code: "OK", requested: count, actual: count, diag: "",
            }),
            "ascii",
          );
          return;
        }
      } catch { /* malformed snapshot: keep polling */ }
    }
    // Poll at a game-like cadence. The real addon polls command.txt roughly
    // every 350 ms; a 5 ms reader keeps the file open almost continuously,
    // and on Windows that blocks the SDK's rename-over for the whole retry
    // budget (measured: ~25% failure at 5 ms, 0/20 at 50 ms and 350 ms).
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error("peer saw no command");
}

describe("L4D2Client", () => {
  let dir;
  beforeEach(async () => {
    dir = await fs.mkdtemp(path.join(os.tmpdir(), "l4d2api-js-"));
    await fs.writeFile(path.join(dir, "heartbeat.txt"), heartbeatText(), "ascii");
  });
  afterEach(async () => {
    await fs.rm(dir, { recursive: true, force: true });
  });

  const make = (opts = {}) =>
    new L4D2Client({ ipcDir: dir, timeoutMs: 5000, pollIntervalMs: 5, ...opts });

  it("connects and round-trips a command", async () => {
    const peer = runPeer(dir);
    const client = make();
    try {
      const hb = await client.connect();
      assert.equal(hb.generation, "g-1");
      const result = await client.send("r_1", "spawn_common", { count: "3" });
      assert.equal(result.status, "applied");
      assert.equal(result.actual, 3);
      assert.equal(result.requestId, "r_1");
    } finally {
      await client.close();
    }
    await peer;
  });

  it("timeout is not success", async () => {
    const client = make({ timeoutMs: 80, pollIntervalMs: 10 });
    try {
      await client.connect();
      await assert.rejects(client.send("r_9", "test_connection"), TimeoutError);
    } finally {
      await client.close();
    }
  });

  it("stale heartbeat never reads ready", async () => {
    const client = make({ heartbeatTimeoutMs: 40 });
    try {
      await client.connect();
      await new Promise((r) => setTimeout(r, 80));
      const status = await client.status();
      assert.equal(status.health, "stale");
      await assert.rejects(client.send("r_s", "test_connection"), StaleHeartbeatError);
    } finally {
      await client.close();
    }
  });

  it("generation change rejects the wait", async () => {
    const client = make();
    try {
      await client.connect();
      await fs.writeFile(path.join(dir, "heartbeat.txt"), heartbeatText({ generation: "g-2", tick: 2 }), "ascii");
      await assert.rejects(client.send("r_2", "test_connection"), StaleGenerationError);
    } finally {
      await client.close();
    }
  });

  it("second writer is locked, not interleaved", async () => {
    const second = make();
    // A live foreign lock, held outside any client instance.
    const holder = await fs.open(path.join(dir, "command.lock"), "wx");
    await holder.writeFile("foreign-holder\n");
    try {
      await second.connect();
      await assert.rejects(second.send("r_lock", "test_connection"), LockedError);
      let exists = true;
      try {
        await fs.stat(path.join(dir, "command.txt"));
      } catch { exists = false; }
      assert.equal(exists, false);
    } finally {
      await second.close();
      await holder.close();
      await fs.rm(path.join(dir, "command.lock"), { force: true });
    }
  });

  it("replaces an existing command file and leaks no temp files", async () => {
    await fs.writeFile(path.join(dir, "command.txt"), "stale-bytes", "ascii");
    const peer = runPeer(dir);
    const client = make();
    try {
      await client.connect();
      const result = await client.testConnection();
      assert.equal(result.status, "applied");
    } finally {
      await client.close();
    }
    await peer;
    const entries = await fs.readdir(dir);
    assert.deepEqual(entries.filter((e) => e.endsWith(".tmp")), []);
  });

  it("unsupported capability fails before any write", async () => {
    await fs.writeFile(
      path.join(dir, "heartbeat.txt"), heartbeatText({ capabilities: ["test_connection"] }), "ascii",
    );
    const client = make({ timeoutMs: 2000 });
    try {
      await client.connect();
      await assert.rejects(client.spawnItem("first_aid_kit", 1), UnsupportedCapabilityError);
      let exists = true;
      try {
        await fs.stat(path.join(dir, "command.txt"));
      } catch { exists = false; }
      assert.equal(exists, false);
    } finally {
      await client.close();
    }
  });

  it("ignores a stale reply with a recycled id", async () => {
    await fs.writeFile(
      path.join(dir, "result.txt"),
      encodeResult({
        seq: 999, id: "r_new", generation: "g-1",
        status: "applied", code: "OK", requested: 0, actual: 0, diag: "",
      }),
      "ascii",
    );
    const peer = runPeer(dir, { delayMs: 50 });
    const client = make();
    try {
      await client.connect();
      const result = await client.send("r_new", "spawn_common", { count: "3" });
      assert.equal(result.status, "applied");
      assert.equal(result.actual, 3);
    } finally {
      await client.close();
    }
    await peer;
  });

  it("cancellation stops the wait without implying rollback", async () => {
    const client = make({ timeoutMs: 5000, pollIntervalMs: 5 });
    const controller = new AbortController();
    try {
      await client.connect();
      const pending = client.testConnection(controller.signal);
      controller.abort();
      await assert.rejects(pending, TimeoutError);
    } finally {
      await client.close();
    }
  });
});
