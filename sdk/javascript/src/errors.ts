/** Typed errors: one class per failure mode.
 *
 * A timeout never implies the game acted or did not act; callers must treat
 * an ambiguous mutation as unknown and never retry it automatically.
 */

export class L4D2Error extends Error {
  constructor(message: string) {
    super(message);
    this.name = new.target.name;
  }
}

export class ProtocolError extends L4D2Error {}
export class TransportError extends L4D2Error {}
export class BusyError extends L4D2Error {}

export class LockedError extends L4D2Error {
  readonly detail: string;
  constructor(detail: string) {
    super(`another process holds the IPC write lock (${detail})`);
    this.detail = detail;
  }
}

export class TimeoutError extends L4D2Error {
  readonly timeoutMs: number;
  constructor(timeoutMs: number) {
    super(`timed out waiting for the game reply after ${timeoutMs}ms`);
    this.timeoutMs = timeoutMs;
  }
}

export class StaleGenerationError extends L4D2Error {
  readonly expected: string;
  readonly found: string;
  constructor(expected: string, found: string) {
    super(`game generation changed (was ${expected}, now ${found}); handshake again`);
    this.expected = expected;
    this.found = found;
  }
}

export class NoHeartbeatError extends L4D2Error {
  constructor(message = "no heartbeat yet: game addon is missing or the game is closed") {
    super(message);
  }
}

export class StaleHeartbeatError extends L4D2Error {
  readonly ageMs: number;
  constructor(ageMs: number) {
    super(`heartbeat is stale (last update ${Math.round(ageMs)}ms ago)`);
    this.ageMs = ageMs;
  }
}

export class NotReadyError extends L4D2Error {
  readonly reason: string;
  constructor(reason: string) {
    super(`game is not ready: ${reason}`);
    this.reason = reason;
  }
}

export class UnsupportedCapabilityError extends L4D2Error {
  readonly action: string;
  constructor(action: string) {
    super(`action \`${action}\` is not advertised by the game`);
    this.action = action;
  }
}
