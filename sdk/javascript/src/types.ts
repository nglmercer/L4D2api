/** Shared result and session models. */

export interface Heartbeat {
  generation: string;
  addonVersion: string;
  map: string;
  roundActive: boolean;
  hostAuthority: boolean;
  gameplayReady: boolean;
  capabilities: string[];
  tick: number;
}

/** Outcome of one request, mirrored from the game's `result.txt`.
 *
 * `requestId` is the public SDK name for the wire field `id`. A written
 * `command.txt` is never reported as success: only the game's decoded
 * acknowledgement counts.
 */
export interface ActionResult {
  requestId: string;
  generation: string;
  status: "applied" | "rejected" | "not_ready" | "partial" | "unknown";
  code: string;
  requested: number;
  actual: number;
  diagnostic: string;
}

export type HealthState =
  | "disconnected"
  | "connected"
  | "stale"
  | "not_ready"
  | "gameplay_ready";

export interface SessionStatus {
  health: HealthState;
  heartbeat: Heartbeat | null;
}

export interface ClientOptions {
  gameDir?: string;
  ipcDir?: string;
  timeoutMs?: number;
  pollIntervalMs?: number;
  heartbeatTimeoutMs?: number;
  lockStaleAfterMs?: number;
}
