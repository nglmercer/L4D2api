/** Public API: client, protocol codec, types and errors. */

export { L4D2Client } from "./client.js";
export {
  checksum,
  encodeCommand,
  decodeCommand,
  encodeResult,
  decodeResult,
  encodeHeartbeat,
  decodeHeartbeat,
  PROTOCOL_VERSION,
  MAX_RECORD_BYTES,
  INFECTED_ALLOWLIST,
  ITEM_ALLOWLIST,
  WEAPON_ALLOWLIST,
  CAPABILITY_ALLOWLIST,
} from "./protocol.js";
export type { DecodedResult } from "./protocol.js";
export type {
  ActionResult,
  Heartbeat,
  HealthState,
  SessionStatus,
  ClientOptions,
} from "./types.js";
export {
  L4D2Error,
  ProtocolError,
  TransportError,
  BusyError,
  LockedError,
  TimeoutError,
  StaleGenerationError,
  NoHeartbeatError,
  StaleHeartbeatError,
  NotReadyError,
  UnsupportedCapabilityError,
} from "./errors.js";
