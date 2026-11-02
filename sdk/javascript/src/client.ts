/** File-IPC session client for the L4D2api game addon.
 *
 * Same contract as the `l4d2api-client` Rust crate: atomic temp-file writes,
 * an atomic cross-process lock file held for the whole request/response
 * transaction, freshness-checked heartbeats, unique request ids, typed
 * errors, and no automatic gameplay retries. Cancellation (via
 * AbortSignal) stops waiting only -- it never rolls back the game.
 */

import { promises as fs } from "node:fs";
import * as path from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

import {
  BusyError,
  LockedError,
  NoHeartbeatError,
  NotReadyError,
  ProtocolError,
  StaleGenerationError,
  StaleHeartbeatError,
  TimeoutError,
  TransportError,
  UnsupportedCapabilityError,
} from "./errors.js";
import { decodeCommand, decodeHeartbeat, decodeResult, encodeCommand } from "./protocol.js";
import type {
  ActionResult,
  ClientOptions,
  Heartbeat,
  SessionStatus,
} from "./types.js";

export const COMMAND_FILE = "command.txt";
export const RESULT_FILE = "result.txt";
export const HEARTBEAT_FILE = "heartbeat.txt";
export const LOCK_FILE = "command.lock";

export const DEFAULT_HEARTBEAT_TIMEOUT_MS = 5000;
export const DEFAULT_LOCK_STALE_AFTER_MS = 30000;
/** Total time budget for retrying a rename blocked by a reader (Windows). */
export const RENAME_RETRY_BUDGET_MS = 2000;
export const RENAME_RETRY_STEP_MS = 10;
/** Re-reads of a heartbeat caught mid-write by the game (~100 ms total). */
const TORN_READ_ATTEMPTS = 5;
const TORN_READ_DELAY_MS = 20;

const sleepSync = (ms: number): Promise<void> => sleep(ms);

export class L4D2Client {
  private readonly dir: string;
  private readonly timeoutMs: number;
  private readonly pollIntervalMs: number;
  private readonly heartbeatTimeoutMs: number;
  private readonly lockStaleAfterMs: number;
  private seq = 0;
  private generation: string | null = null;
  private outstanding = false;
  private readonly owner: string;
  private lastTick: { generation: string; tick: number; at: number } | null = null;

  constructor(options: ClientOptions = {}) {
    if (!options.ipcDir && !options.gameDir) {
      throw new TypeError("pass gameDir or ipcDir");
    }
    this.dir = options.ipcDir ??
      path.join(options.gameDir!, "left4dead2", "ems", "tiktools_l4d2");
    this.timeoutMs = options.timeoutMs ?? 5000;
    this.pollIntervalMs = options.pollIntervalMs ?? 250;
    this.heartbeatTimeoutMs = options.heartbeatTimeoutMs ?? DEFAULT_HEARTBEAT_TIMEOUT_MS;
    this.lockStaleAfterMs = options.lockStaleAfterMs ?? DEFAULT_LOCK_STALE_AFTER_MS;
    this.owner = `pid${process.pid}-${Date.now()}-${Math.floor(Math.random() * 1e9)}`;
  }

  /** IPC directory under management. */
  ipcDir(): string {
    return this.dir;
  }

  /** Pinned session generation, if {@link connect} ran. */
  pinnedGeneration(): string | null {
    return this.generation;
  }

  /** Release per-instance state. The cross-process lock is always released
   * per request; close() only clears the outstanding flag. */
  async close(): Promise<void> {
    this.outstanding = false;
  }

  private async readFile(name: string): Promise<string | null> {
    try {
      const text = await fs.readFile(path.join(this.dir, name), "ascii");
      return (name === HEARTBEAT_FILE || name === RESULT_FILE) && text.endsWith("\0") ? text.slice(0, -1) : text;
    } catch (error: unknown) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
      throw new TransportError(String(error));
    }
  }

  private async fileAgeMs(name: string): Promise<number | null> {
    try {
      const stat = await fs.stat(path.join(this.dir, name));
      return Date.now() - stat.mtimeMs;
    } catch {
      return null;
    }
  }

  async heartbeat(): Promise<Heartbeat | null> {
    // The game rewrites heartbeat.txt every ~0.35 s, so a read can catch it
    // mid-write. Re-read a torn snapshot briefly (reads only; nothing is
    // re-sent) before reporting no heartbeat.
    for (let attempt = 0; attempt < TORN_READ_ATTEMPTS; attempt++) {
      const raw = await this.readFile(HEARTBEAT_FILE);
      if (raw === null) return null;
      try {
        return decodeHeartbeat(raw);
      } catch {
        if (attempt + 1 < TORN_READ_ATTEMPTS) await sleepSync(TORN_READ_DELAY_MS);
      }
    }
    return null;
  }

  async heartbeatFresh(): Promise<{ heartbeat: Heartbeat; ageMs: number } | null> {
    const hb = await this.heartbeat();
    if (!hb) return null;
    const age = await this.fileAgeMs(HEARTBEAT_FILE);
    return { heartbeat: hb, ageMs: age ?? Number.POSITIVE_INFINITY };
  }

  /** Pin the live generation. Clears stale per-instance state. */
  async connect(): Promise<Heartbeat> {
    const hb = await this.heartbeat();
    if (!hb) throw new NoHeartbeatError();
    this.generation = hb.generation;
    this.outstanding = false;
    this.lastTick = { generation: hb.generation, tick: hb.tick, at: Date.now() };
    return hb;
  }

  async capabilities(): Promise<string[]> {
    const hb = await this.heartbeat();
    return hb ? [...hb.capabilities] : [];
  }

  /** Freshness-checked session health. A valid but old heartbeat is
   * `stale`, never ready. Observes `tick` progression across calls. */
  async status(): Promise<SessionStatus> {
    const fresh = await this.heartbeatFresh();
    if (!fresh) return { health: "disconnected", heartbeat: null };
    const { heartbeat: hb, ageMs } = fresh;
    if (ageMs > this.heartbeatTimeoutMs) return { health: "stale", heartbeat: hb };
    const stalled =
      this.lastTick !== null &&
      this.lastTick.generation === hb.generation &&
      this.lastTick.tick === hb.tick;
    this.lastTick = { generation: hb.generation, tick: hb.tick, at: Date.now() };
    if (stalled && ageMs > this.pollIntervalMs * 3) {
      return { health: "stale", heartbeat: hb };
    }
    if (!hb.addonVersion) return { health: "not_ready", heartbeat: hb };
    if (!hb.hostAuthority) return { health: "not_ready", heartbeat: hb };
    if (!hb.gameplayReady || !hb.roundActive) return { health: "connected", heartbeat: hb };
    return { health: "gameplay_ready", heartbeat: hb };
  }

  async ensureReadyFor(action: string): Promise<void> {
    if (!this.generation) {
      throw new NoHeartbeatError("connect() first: no pinned generation");
    }
    const fresh = await this.heartbeatFresh();
    if (!fresh) throw new NoHeartbeatError();
    const { heartbeat: hb, ageMs } = fresh;
    if (ageMs > this.heartbeatTimeoutMs) throw new StaleHeartbeatError(ageMs);
    if (hb.generation !== this.generation) {
      throw new StaleGenerationError(this.generation, hb.generation);
    }
    if (!hb.hostAuthority) throw new NotReadyError("not_host");
    if (action !== "test_connection" && !hb.capabilities.includes(action)) {
      throw new UnsupportedCapabilityError(action);
    }
  }

  private lockPath(): string {
    return path.join(this.dir, LOCK_FILE);
  }

  private async acquireLock(): Promise<void> {
    try {
      const fh = await fs.open(this.lockPath(), "wx", 0o644);
      await fh.writeFile(`${this.owner}\n`, "ascii");
      await fh.close();
    } catch (error: unknown) {
      if ((error as NodeJS.ErrnoException).code === "EEXIST") {
        await this.reclaimOrRefuse();
        return;
      }
      throw new TransportError(String(error));
    }
  }

  private async reclaimOrRefuse(): Promise<void> {
    const first = (await this.readFile(LOCK_FILE)) ?? "";
    const age = await this.fileAgeMs(LOCK_FILE);
    if (age === null || age <= this.lockStaleAfterMs) {
      throw new LockedError(`held for ${age ?? 0}ms`);
    }
    await sleepSync(25);
    const second = (await this.readFile(LOCK_FILE)) ?? "";
    if (second !== first) {
      throw new LockedError("lock changed during reclaim; another client is live");
    }
    await fs.rm(this.lockPath(), { force: true });
    try {
      const fh = await fs.open(this.lockPath(), "wx", 0o644);
      await fh.writeFile(`${this.owner}\n`, "ascii");
      await fh.close();
    } catch (error: unknown) {
      if ((error as NodeJS.ErrnoException).code === "EEXIST") {
        throw new LockedError("lost the reclaim race; another client is live");
      }
      throw new TransportError(String(error));
    }
  }

  private async releaseLock(): Promise<void> {
    try {
      const token = await this.readFile(LOCK_FILE);
      if (token !== null && token.trim() === this.owner) {
        await fs.rm(this.lockPath(), { force: true });
      }
    } catch {
      // Best effort: never fail a completed request on lock cleanup.
    }
  }

  private sanitizeId(id: string): string {
    const safe = id.replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 48) || "id";
    return safe;
  }

  private async writeCommandAtomic(text: string, requestId: string): Promise<void> {
    const target = path.join(this.dir, COMMAND_FILE);
    const tmp = path.join(this.dir, `command.${process.pid}.${this.sanitizeId(requestId)}.tmp`);
    try {
      await fs.writeFile(tmp, text, "ascii");
    } catch (error: unknown) {
      await fs.rm(tmp, { force: true });
      throw new TransportError(String(error));
    }
    // fs.rename atomically replaces the destination on both POSIX and
    // Windows. On Windows the rename fails with EPERM while another process
    // has command.txt open (the game polls it), so retry transient lock
    // errors within a time budget rather than a fixed attempt count.
    const deadline = Date.now() + RENAME_RETRY_BUDGET_MS;
    for (;;) {
      try {
        await fs.rename(tmp, target);
        return;
      } catch (error: unknown) {
        const code = (error as NodeJS.ErrnoException).code;
        const retryable = code === "EPERM" || code === "EBUSY" || code === "EINTR" || code === "EACCES";
        if (!retryable || Date.now() >= deadline) {
          await fs.rm(tmp, { force: true });
          throw new TransportError(String(error));
        }
        await new Promise((resolve) => setTimeout(resolve, RENAME_RETRY_STEP_MS));
      }
    }
  }

  /** Mint a unique request id. Never reuse ids: a stale result from an
   * earlier request with the same id could satisfy a new wait. */
  nextId(): string {
    this.seq += 1;
    return `r-${process.pid}-${Date.now()}-${this.seq}`;
  }

  /** Send one action and wait for the game's typed reply.
   *
   * Replies must match id, sequence and generation together. No automatic
   * retries: a timeout leaves the outcome unknown. `signal` cancels the
   * wait only -- never the game effect.
   */
  async send(
    requestId: string,
    action: string,
    params: Record<string, string> = {},
    signal?: AbortSignal,
  ): Promise<ActionResult> {
    if (this.outstanding) {
      throw new BusyError("another mutation is already outstanding (single-flight)");
    }
    if (!this.generation) {
      throw new NoHeartbeatError("connect() first: no pinned generation");
    }
    const generation = this.generation;
    await this.acquireLock();
    this.outstanding = true;
    try {
      return await this.sendLocked(requestId, action, generation, params, signal);
    } finally {
      this.outstanding = false;
      await this.releaseLock();
    }
  }

  private async sendLocked(
    requestId: string,
    action: string,
    generation: string,
    params: Record<string, string>,
    signal?: AbortSignal,
  ): Promise<ActionResult> {
    await this.ensureReadyFor(action);
    // Recover the previous sequence while holding the cross-process lock.
    for (const [name, decode] of [[COMMAND_FILE, decodeCommand], [RESULT_FILE, decodeResult]] as const) {
      const raw = await this.readFile(name);
      if (raw !== null) {
        try {
          const previous = decode(raw);
          if (previous.generation === generation) this.seq = Math.max(this.seq, previous.seq);
        } catch (error) {
          if (!(error instanceof ProtocolError)) throw error;
        }
      }
    }
    this.seq += 1;
    const text = encodeCommand({ seq: this.seq, id: requestId, generation, action, params });
    await this.writeCommandAtomic(text, requestId);
    return this.waitForReply(requestId, this.seq, generation, signal);
  }

  private async waitForReply(
    requestId: string,
    seq: number,
    generation: string,
    signal?: AbortSignal,
  ): Promise<ActionResult> {
    const deadline = Date.now() + this.timeoutMs;
    for (;;) {
      if (signal?.aborted) {
        throw new TimeoutError(this.timeoutMs);
      }
      const raw = await this.readFile(RESULT_FILE);
      if (raw !== null) {
        try {
          const record = decodeResult(raw);
          // All three must match: a stale reply from an earlier request
          // with a recycled id can never resolve this wait.
          if (record.requestId === requestId && record.seq === seq) {
            if (record.generation !== generation) {
              throw new StaleGenerationError(generation, record.generation);
            }
            const { seq: _seq, ...result } = record;
            return result;
          }
        } catch (error: unknown) {
          if (error instanceof StaleGenerationError) throw error;
          // Malformed or foreign replies: keep waiting.
        }
      }
      const hb = await this.heartbeat();
      if (hb && hb.generation !== generation) {
        throw new StaleGenerationError(generation, hb.generation);
      }
      if (Date.now() >= deadline) throw new TimeoutError(this.timeoutMs);
      await sleepSync(this.pollIntervalMs);
    }
  }

  // -- ergonomic actions (unique ids, capability-checked) ----------------------

  async testConnection(signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "test_connection", {}, signal);
  }

  async spawnCommon(count: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "spawn_common", { count: String(count) }, signal);
  }

  async spawnInfected(className: string, count: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "spawn_infected", { class: className, count: String(count) }, signal);
  }

  async spawnItem(item: string, count: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "spawn_item", { item, count: String(count) }, signal);
  }

  async spawnWeapon(weapon: string, count: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "spawn_weapon", { weapon, count: String(count) }, signal);
  }

  // Player effects: `target` is "all", "host" or "random". The reply's
  // `requested` is the number of survivors targeted, `actual` how many changed.

  async heal(target: string, amount: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "heal", { target, amount: String(amount) }, signal);
  }

  async tempHealth(target: string, amount: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "temp_health", { target, amount: String(amount) }, signal);
  }

  async damage(target: string, amount: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "damage", { target, amount: String(amount) }, signal);
  }

  async revive(target: string, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "revive", { target }, signal);
  }

  async giveItem(target: string, item: string, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "give_item", { target, item }, signal);
  }

  async panic(signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "panic", {}, signal);
  }

  async spawnHazard(kind: string, count: number, signal?: AbortSignal): Promise<ActionResult> {
    return this.send(this.nextId(), "spawn_hazard", { kind, count: String(count) }, signal);
  }
}

export { ProtocolError };
