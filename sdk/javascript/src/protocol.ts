/** Canonical v1 protocol codec: framing, FNV-1a checksums, allowlists.
 *
 * This is the TypeScript twin of the `l4d2api-protocol` Rust crate, which
 * stays the reference implementation. Every encoder/decoder here agrees
 * with the Rust codec on checksums and framing; the test suite pins the
 * shared fixtures in `tests/fixtures/`.
 */

import { ProtocolError } from "./errors.js";
import type { ActionResult, Heartbeat } from "./types.js";

export const PROTOCOL_VERSION = 1;
export const MAX_RECORD_BYTES = 1024;
export const MAX_ID_LEN = 64;
export const MAX_GEN_LEN = 64;

export const COMMON_MAX = 10;
export const SPECIAL_MAX = 8;
export const ITEM_MAX = 5;
export const WEAPON_MAX = 5;

export const INFECTED_ALLOWLIST = new Set([
  "tank", "witch", "smoker", "boomer", "hunter", "spitter", "jockey", "charger",
]);
export const ITEM_ALLOWLIST = new Set([
  "first_aid_kit", "pain_pills", "adrenaline", "pipe_bomb", "molotov", "defibrillator",
]);
export const WEAPON_ALLOWLIST = new Set([
  "weapon_pistol", "weapon_pistol_magnum", "weapon_smg", "weapon_smg_silenced",
  "weapon_smg_mp5", "weapon_rifle", "weapon_rifle_desert", "weapon_rifle_ak47",
  "weapon_rifle_sg552", "weapon_shotgun_chrome", "weapon_shotgun_spas",
  "weapon_pumpshotgun", "weapon_autoshotgun", "weapon_hunting_rifle",
  "weapon_sniper_military", "weapon_sniper_awp", "weapon_sniper_scout",
  "weapon_grenade_launcher", "weapon_chainsaw", "weapon_melee_spawn",
]);
export const CAPABILITY_ALLOWLIST = new Set([
  "test_connection", "spawn_common", "spawn_infected", "spawn_item", "spawn_weapon",
]);
export const ACTION_NAMES = new Set([
  "test_connection", "spawn_common", "spawn_infected", "spawn_item", "spawn_weapon",
]);

/** FNV-1a (32-bit), rendered as 8 lowercase hex digits. */
export function checksum(body: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < body.length; i++) {
    const code = body.charCodeAt(i);
    if (code > 127) throw new ProtocolError("record is not ASCII");
    hash ^= code;
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function isToken(text: string, maxLen: number): boolean {
  if (!text || text.length > maxLen) return false;
  return /^[A-Za-z0-9_-]+$/.test(text);
}

function isCode(text: string): boolean {
  return !!text && text.length <= 32 && /^[A-Z0-9_]+$/.test(text);
}

function isDiag(text: string): boolean {
  if (text.length > 128) return false;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    if (!(c === 0x20 || (c >= 0x21 && c <= 0x7e))) return false;
  }
  return true;
}

function splitVerified(text: string): string[] {
  if (text.length > MAX_RECORD_BYTES) {
    throw new ProtocolError(`record exceeds ${MAX_RECORD_BYTES} bytes`);
  }
  if (!/^[\x00-\x7f]*$/.test(text)) throw new ProtocolError("record is not ASCII");
  const lines = text.split("\n");
  if (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].endsWith("\r")) lines[i] = lines[i].slice(0, -1);
  }
  if (lines.length === 0 || lines[lines.length - 1] !== "END") {
    throw new ProtocolError("record must end with an END sentinel line");
  }
  if (lines.length < 2) throw new ProtocolError("record is missing its crc line");
  const crcLine = lines[lines.length - 2];
  if (crcLine.length !== 12 || !crcLine.startsWith("crc=")) {
    throw new ProtocolError("record is missing its crc line");
  }
  const expected = crcLine.slice(4);
  if (!/^[0-9a-fA-F]{8}$/.test(expected)) {
    throw new ProtocolError("crc must be 8 hex digits");
  }
  const body = lines.slice(0, -2).map((l) => l + "\n").join("");
  const computed = checksum(body);
  if (computed !== expected.toLowerCase()) {
    throw new ProtocolError(`checksum mismatch: expected ${expected}, computed ${computed}`);
  }
  return lines.slice(0, -2);
}

function parseFields(content: string[], allowed: Set<string>): Map<string, string> {
  const fields = new Map<string, string>();
  for (const line of content) {
    const eq = line.indexOf("=");
    if (eq <= 0) throw new ProtocolError(`line is not key=value: ${line}`);
    const key = line.slice(0, eq);
    const value = line.slice(eq + 1);
    if (!/^[A-Za-z0-9_-]{1,16}$/.test(key)) {
      throw new ProtocolError(`bad field name \`${key}\``);
    }
    if (fields.has(key)) throw new ProtocolError(`duplicate field \`${key}\``);
    if (!allowed.has(key)) throw new ProtocolError(`unknown field \`${key}\``);
    fields.set(key, value);
  }
  return fields;
}

function parseU64(field: string, value: string): number {
  if (!/^[0-9]+$/.test(value)) {
    throw new ProtocolError(`field \`${field}\` must be a non-negative integer`);
  }
  const n = Number(value);
  if (!Number.isSafeInteger(n)) {
    throw new ProtocolError(`field \`${field}\` overflows`);
  }
  return n;
}

function finish(lines: string[]): string {
  const body = lines.map((l) => l + "\n").join("");
  return `${body}crc=${checksum(body)}\nEND`;
}

export interface CommandInput {
  seq: number;
  id: string;
  generation: string;
  action: string;
  params?: Record<string, string>;
}

export interface DecodedCommand extends CommandInput {
  params: Record<string, string>;
}

const COMMAND_KEYS = new Set([
  "v", "seq", "id", "gen", "action", "count", "class", "item", "weapon",
]);

function checkActionParams(action: string, fields: Map<string, string>): Record<string, string> {
  const count = (lo: number, hi: number): number => {
    const raw = fields.get("count");
    if (raw === undefined) throw new ProtocolError("missing field `count`");
    const value = parseU64("count", raw);
    if (value < lo || value > hi) {
      throw new ProtocolError(`field \`count\` must be ${lo}-${hi}`);
    }
    return value;
  };
  switch (action) {
    case "test_connection":
      for (const k of ["count", "class", "item", "weapon"]) {
        if (fields.has(k)) {
          throw new ProtocolError("test_connection takes no count/class/item/weapon");
        }
      }
      return {};
    case "spawn_common":
      return { count: String(count(1, COMMON_MAX)) };
    case "spawn_infected": {
      const cls = fields.get("class");
      if (cls === undefined) throw new ProtocolError("missing field `class`");
      if (!INFECTED_ALLOWLIST.has(cls)) {
        throw new ProtocolError(`unknown infected class \`${cls}\``);
      }
      return { class: cls, count: String(count(1, cls === "tank" ? 1 : SPECIAL_MAX)) };
    }
    case "spawn_item": {
      const item = fields.get("item");
      if (item === undefined) throw new ProtocolError("missing field `item`");
      if (!ITEM_ALLOWLIST.has(item)) throw new ProtocolError(`unknown item \`${item}\``);
      return { item, count: String(count(1, ITEM_MAX)) };
    }
    case "spawn_weapon": {
      const weapon = fields.get("weapon");
      if (weapon === undefined) throw new ProtocolError("missing field `weapon`");
      if (!WEAPON_ALLOWLIST.has(weapon)) {
        throw new ProtocolError(`unknown weapon \`${weapon}\``);
      }
      return { weapon, count: String(count(1, WEAPON_MAX)) };
    }
    default:
      throw new ProtocolError(`unknown action \`${action}\``);
  }
}

const EXPECTED_KEYS: Record<string, Set<string>> = {
  test_connection: new Set(["v", "seq", "id", "gen", "action"]),
  spawn_common: new Set(["v", "seq", "id", "gen", "action", "count"]),
  spawn_infected: new Set(["v", "seq", "id", "gen", "action", "class", "count"]),
  spawn_item: new Set(["v", "seq", "id", "gen", "action", "item", "count"]),
  spawn_weapon: new Set(["v", "seq", "id", "gen", "action", "weapon", "count"]),
};

export function encodeCommand(command: CommandInput): string {
  if (!isToken(command.id, MAX_ID_LEN)) {
    throw new ProtocolError("id must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!isToken(command.generation, MAX_GEN_LEN)) {
    throw new ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!ACTION_NAMES.has(command.action)) {
    throw new ProtocolError(`unknown action \`${command.action}\``);
  }
  const params = checkActionParams(
    command.action,
    new Map(Object.entries(command.params ?? {})),
  );
  const lines = [
    `v=${PROTOCOL_VERSION}`,
    `seq=${command.seq}`,
    `id=${command.id}`,
    `gen=${command.generation}`,
    `action=${command.action}`,
  ];
  for (const key of ["class", "item", "weapon", "count"]) {
    if (key in params) lines.push(`${key}=${params[key]}`);
  }
  const record = finish(lines);
  if (record.length > MAX_RECORD_BYTES) {
    throw new ProtocolError("encoded command exceeds 1 KiB");
  }
  return record;
}

export function decodeCommand(text: string): DecodedCommand {
  const fields = parseFields(splitVerified(text), COMMAND_KEYS);
  if (fields.get("v") !== String(PROTOCOL_VERSION)) {
    throw new ProtocolError(`unsupported protocol version \`${fields.get("v")}\``);
  }
  for (const key of ["seq", "id", "gen", "action"]) {
    if (!fields.has(key)) throw new ProtocolError(`missing field \`${key}\``);
  }
  const seq = parseU64("seq", fields.get("seq")!);
  const id = fields.get("id")!;
  const generation = fields.get("gen")!;
  const action = fields.get("action")!;
  if (!isToken(id, MAX_ID_LEN)) {
    throw new ProtocolError("id must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!isToken(generation, MAX_GEN_LEN)) {
    throw new ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!ACTION_NAMES.has(action)) throw new ProtocolError(`unknown action \`${action}\``);
  const expected = EXPECTED_KEYS[action];
  for (const key of fields.keys()) {
    if (!expected.has(key)) {
      throw new ProtocolError(`unexpected field \`${key}\` for action \`${action}\``);
    }
  }
  return { seq, id, generation, action, params: checkActionParams(action, fields) };
}

export interface ResultInput {
  seq: number;
  id: string;
  generation: string;
  status: ActionResult["status"];
  code: string;
  requested: number;
  actual: number;
  diag?: string;
}

const RESULT_KEYS = new Set([
  "v", "seq", "id", "gen", "status", "code", "requested", "actual", "diag",
]);
const STATUSES = new Set(["applied", "rejected", "not_ready", "partial", "unknown"]);

export function encodeResult(result: ResultInput): string {
  if (!isToken(result.id, MAX_ID_LEN)) {
    throw new ProtocolError("id must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!isToken(result.generation, MAX_GEN_LEN)) {
    throw new ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!STATUSES.has(result.status)) {
    throw new ProtocolError("status must be applied|rejected|not_ready|partial|unknown");
  }
  if (!isCode(result.code)) {
    throw new ProtocolError("code must be 1-32 chars of [A-Z0-9_]");
  }
  const diag = result.diag ?? "";
  if (!isDiag(diag)) throw new ProtocolError("diag must be <=128 printable ASCII chars");
  if (result.actual > result.requested) {
    throw new ProtocolError("actual must not exceed requested");
  }
  const record = finish([
    `v=${PROTOCOL_VERSION}`,
    `seq=${result.seq}`,
    `id=${result.id}`,
    `gen=${result.generation}`,
    `status=${result.status}`,
    `code=${result.code}`,
    `requested=${result.requested}`,
    `actual=${result.actual}`,
    `diag=${diag}`,
  ]);
  if (record.length > MAX_RECORD_BYTES) {
    throw new ProtocolError("encoded result exceeds 1 KiB");
  }
  return record;
}

export interface DecodedResult extends ActionResult {
  seq: number;
}

export function decodeResult(text: string): DecodedResult {
  const fields = parseFields(splitVerified(text), RESULT_KEYS);
  if (fields.get("v") !== String(PROTOCOL_VERSION)) {
    throw new ProtocolError(`unsupported protocol version \`${fields.get("v")}\``);
  }
  for (const key of ["seq", "id", "gen", "status", "code", "requested", "actual", "diag"]) {
    if (!fields.has(key)) throw new ProtocolError(`missing field \`${key}\``);
  }
  const status = fields.get("status")!;
  if (!STATUSES.has(status)) {
    throw new ProtocolError("status must be applied|rejected|not_ready|partial|unknown");
  }
  const code = fields.get("code")!;
  if (!isCode(code)) throw new ProtocolError("code must be 1-32 chars of [A-Z0-9_]");
  const requested = parseU64("requested", fields.get("requested")!);
  const actual = parseU64("actual", fields.get("actual")!);
  if (actual > requested) throw new ProtocolError("actual must not exceed requested");
  const diag = fields.get("diag")!;
  if (!isDiag(diag)) throw new ProtocolError("diag must be <=128 printable ASCII chars");
  return {
    requestId: fields.get("id")!,
    generation: fields.get("gen")!,
    status: status as ActionResult["status"],
    code,
    requested,
    actual,
    diagnostic: diag,
    seq: parseU64("seq", fields.get("seq")!),
  };
}

const HEARTBEAT_KEYS = new Set([
  "v", "gen", "addon", "map", "round", "host", "ready", "caps", "tick",
]);

function isVersion(text: string): boolean {
  return !!text && text.length <= MAX_ID_LEN && /^[A-Za-z0-9._-]+$/.test(text);
}

export function encodeHeartbeat(hb: Heartbeat): string {
  if (!isToken(hb.generation, MAX_GEN_LEN)) {
    throw new ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]");
  }
  if (!isVersion(hb.addonVersion)) throw new ProtocolError("addon version is malformed");
  if (hb.map && !isToken(hb.map, 64)) throw new ProtocolError("map name is malformed");
  for (const cap of hb.capabilities) {
    if (!CAPABILITY_ALLOWLIST.has(cap)) {
      throw new ProtocolError(`unknown capability token \`${cap}\``);
    }
  }
  const flag = (v: boolean): string => (v ? "1" : "0");
  const record = finish([
    `v=${PROTOCOL_VERSION}`,
    `gen=${hb.generation}`,
    `addon=${hb.addonVersion}`,
    `map=${hb.map}`,
    `round=${flag(hb.roundActive)}`,
    `host=${flag(hb.hostAuthority)}`,
    `ready=${flag(hb.gameplayReady)}`,
    `caps=${hb.capabilities.join(",")}`,
    `tick=${hb.tick}`,
  ]);
  if (record.length > MAX_RECORD_BYTES) {
    throw new ProtocolError("encoded heartbeat exceeds 1 KiB");
  }
  return record;
}

export function decodeHeartbeat(text: string): Heartbeat {
  const fields = parseFields(splitVerified(text), HEARTBEAT_KEYS);
  if (fields.get("v") !== String(PROTOCOL_VERSION)) {
    throw new ProtocolError(`unsupported protocol version \`${fields.get("v")}\``);
  }
  for (const key of ["gen", "addon", "map", "round", "host", "ready", "caps", "tick"]) {
    if (!fields.has(key)) throw new ProtocolError(`missing field \`${key}\``);
  }
  const generation = fields.get("gen")!;
  if (!isToken(generation, MAX_GEN_LEN)) {
    throw new ProtocolError("gen must be 1-64 chars of [A-Za-z0-9_-]");
  }
  const addonVersion = fields.get("addon")!;
  if (!isVersion(addonVersion)) throw new ProtocolError("addon must be a dotted version");
  const map = fields.get("map")!;
  if (map && !isToken(map, 64)) {
    throw new ProtocolError("map must be empty or a map token");
  }
  for (const key of ["round", "host", "ready"]) {
    const v = fields.get(key)!;
    if (v !== "0" && v !== "1") {
      throw new ProtocolError(`field \`${key}\` must be 0 or 1`);
    }
  }
  const capabilities: string[] = [];
  const capsRaw = fields.get("caps")!;
  if (capsRaw) {
    for (const cap of capsRaw.split(",")) {
      if (!CAPABILITY_ALLOWLIST.has(cap)) {
        throw new ProtocolError(`unknown capability token \`${cap}\``);
      }
      if (capabilities.includes(cap)) {
        throw new ProtocolError(`duplicate capability token \`${cap}\``);
      }
      capabilities.push(cap);
    }
  }
  return {
    generation,
    addonVersion,
    map,
    roundActive: fields.get("round") === "1",
    hostAuthority: fields.get("host") === "1",
    gameplayReady: fields.get("ready") === "1",
    capabilities,
    tick: parseU64("tick", fields.get("tick")!),
  };
}
