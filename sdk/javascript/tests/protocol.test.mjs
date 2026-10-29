/** Fixture parity with the Rust reference codec.
 *
 * Every shared fixture in tests/fixtures/ decodes here; the encoder must
 * reproduce each valid fixture byte-for-byte, and malformed fixtures must
 * throw. Run: npm test (builds first).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  checksum,
  decodeCommand,
  decodeHeartbeat,
  decodeResult,
  encodeCommand,
  encodeHeartbeat,
} from "../dist/index.js";

const FIX = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "tests", "fixtures");
const fixture = (name) => readFileSync(join(FIX, name), "ascii");

// Skip the .note sidecars.
const names = readdirSync(FIX).filter((n) => n.endsWith(".txt"));

describe("shared fixtures", () => {
  it("valid command decodes with exact fields", () => {
    const cmd = decodeCommand(fixture("command_valid.txt"));
    assert.equal(cmd.seq, 7);
    assert.equal(cmd.id, "r_12_7");
    assert.equal(cmd.generation, "g-9f2c");
    assert.equal(cmd.action, "spawn_common");
    assert.deepEqual(cmd.params, { count: "5" });
  });

  it("per-action commands decode", () => {
    assert.equal(decodeCommand(fixture("command_test_connection.txt")).action, "test_connection");
    assert.deepEqual(decodeCommand(fixture("command_spawn_item.txt")).params, {
      item: "first_aid_kit", count: "2",
    });
    assert.deepEqual(decodeCommand(fixture("command_spawn_weapon.txt")).params, {
      weapon: "weapon_rifle_ak47", count: "1",
    });
    assert.deepEqual(decodeCommand(fixture("command_spawn_infected.txt")).params, {
      class: "boomer", count: "3",
    });
  });

  it("encoder reproduces every valid fixture byte-for-byte", () => {
    for (const name of [
      "command_valid.txt",
      "command_test_connection.txt",
      "command_spawn_item.txt",
      "command_spawn_weapon.txt",
      "command_spawn_infected.txt",
    ]) {
      const raw = fixture(name);
      assert.equal(encodeCommand(decodeCommand(raw)), raw, name);
    }
    const hb = decodeHeartbeat(fixture("heartbeat_ready.txt"));
    assert.equal(encodeHeartbeat(hb), fixture("heartbeat_ready.txt"));
  });

  it("checksum matches the reference primitive", () => {
    assert.equal(checksum(""), "811c9dc5");
  });

  it("partial result keeps verified actual", () => {
    const partial = decodeResult(fixture("result_partial.txt"));
    assert.equal(partial.status, "partial");
    assert.equal(partial.code, "PARTIAL");
    assert.equal(partial.actual, 3);
    assert.equal(partial.requested, 5);
  });

  it("malformed fixtures throw", () => {
    for (const name of [
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
    ]) {
      assert.throws(() => decodeCommand(fixture(name)), Error, name);
    }
  });

  it("session-level fixtures stay framing-valid (session logic rejects them)", () => {
    for (const name of [
      "expired_old_generation.txt",
      "out_of_order_command.txt",
      "duplicate_request.txt",
    ]) {
      decodeCommand(fixture(name));
    }
  });

  it("every fixture file is accounted for", () => {
    const known = new Set([
      "command_valid.txt", "command_test_connection.txt", "command_spawn_item.txt",
      "command_spawn_weapon.txt", "command_spawn_infected.txt",
      "result_applied.txt", "result_partial.txt", "result_rejected_not_host.txt",
      "heartbeat_ready.txt", "expired_old_generation.txt",
      "out_of_order_command.txt", "duplicate_request.txt",
      "malformed_bad_crc.txt", "truncated_no_end.txt", "truncated_partial.txt",
      "malformed_duplicate_key.txt", "malformed_unknown_action.txt",
      "malformed_unknown_key.txt", "malformed_invalid_integer.txt",
      "malformed_invalid_params.txt", "unsupported_version.txt", "oversized_record.txt",
    ]);
    for (const name of names) assert.ok(known.has(name), `unpinned fixture: ${name}`);
  });
});
