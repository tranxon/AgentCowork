//! PollingManager — ADR-021 Phase 3
//!
//! Manages HTTP polling for session message data. Replaces the old WebSocket
//! streaming data channel (Delta/ReasoningDelta/ToolCall/ToolResult) with
//! incremental HTTP pulls triggered by `new_data_available` notifications
//! and a fallback interval with exponential backoff.
//!
//! ## Architecture
//!
//! ```
//! WebSocket "new_data_available" ──→ notify() ──→ immediate poll
//! Fallback timer (500ms→…→5s)     ──→ scheduled poll
//!                                                  │
//!                                                  ▼
//!                              chatStore.loadSessionMessages()
//!                              ?line_number=N&line_char_offset=M
//! ```
//!
//! ## Backoff strategy (ADR-021 §难点 2)
//!
//! - Normal: 500ms interval, reset on new data
//! - Empty response: double interval (max 5s)
//! - 3 consecutive empty responses → stop polling
//!
//! ## Lifecycle
//!
//! - `start()` — begin polling (called when session becomes active/streaming)
//! - `stop()`  — stop polling (called on done/error/stopped or session switch)
//! - `notify(totalLines, streamingLineNumber)` — trigger immediate poll

import { useChatStore } from "../stores/chatStore";

/** Initial polling interval in milliseconds */
const POLL_INITIAL_MS = 500;
/** Maximum backoff interval in milliseconds */
const POLL_MAX_MS = 5000;
/** Backoff multiplier per empty response */
const POLL_BACKOFF_MULTIPLIER = 2.0;
/** Consecutive empty responses before auto-stop */
const POLL_MAX_EMPTY_RETRIES = 3;

/**
 * Per-session polling manager.
 *
 * Each active streaming session gets its own PollingManager instance.
 * The manager is stored in a module-level Map keyed by `agentId:sessionId`.
 */
export class PollingManager {
  private agentId: string;
  private sessionId: string;
  private intervalMs: number;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lineNumber: number = 0;
  private charOffset: number = 0;
  private running: boolean = false;
  private consecutiveEmpty: number = 0;

  constructor(
    agentId: string,
    sessionId: string,
  ) {
    this.agentId = agentId;
    this.sessionId = sessionId;
    this.intervalMs = POLL_INITIAL_MS;
  }

  /** Start the polling loop. Idempotent — safe to call multiple times. */
  start(): void {
    if (this.running) return;
    this.running = true;
    this.intervalMs = POLL_INITIAL_MS;
    this.consecutiveEmpty = 0;
    console.log(
      `[PollingManager] Starting poll for ${this.agentId}/${this.sessionId}`,
    );
    this.scheduleNext();
  }

  /** Stop the polling loop. Idempotent. */
  stop(): void {
    if (!this.running) return;
    this.running = false;
    this.clearTimer();
    console.log(
      `[PollingManager] Stopped poll for ${this.agentId}/${this.sessionId}`,
    );
  }

  /**
   * Called when a `new_data_available` WebSocket event arrives.
   * Updates the line coordinate and triggers an immediate poll.
   *
   * @param totalLines - Total JSONL line count from the backend notification.
   * @param streamingLineNumber - The line number of the current streaming line.
   */
  notify(totalLines: number, streamingLineNumber: number): void {
    if (!this.running) return;

    // Update line number from the notification
    if (totalLines > 0) {
      this.lineNumber = totalLines;
    }
    // streaming_line tells us the current streaming line number;
    // reset char offset since content is always accumulating
    this.charOffset = 0;

    console.log(
      `[PollingManager] notify: totalLines=${totalLines}, streamingLine=${streamingLineNumber}`,
    );

    // Trigger immediate poll (cancel any pending timer first)
    this.clearTimer();
    this.doPoll();
  }

  /** Schedule the next fallback poll with current backoff interval */
  private scheduleNext(): void {
    if (!this.running) return;
    this.timer = setTimeout(() => {
      this.doPoll();
    }, this.intervalMs);
  }

  /** Clear the fallback timer */
  private clearTimer(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  /**
   * Execute a single poll cycle.
   *
   * Delegates the actual HTTP fetch to `chatStore.loadSessionMessages()`
   * to avoid double-fetching. After the store updates, reads back the
   * updated poll coordinates for the next cycle.
   */
  private async doPoll(): Promise<void> {
    if (!this.running) return;
    const store = useChatStore.getState();

    try {
      const agent = store.agentStates[this.agentId];
      if (!agent) { this.stop(); return; }

      const session = agent.sessionStates[this.sessionId];
      if (!session) { this.stop(); return; }

      // Don't poll if session is not in an active state
      const status = session.sessionStatus?.status;
      if (
        status !== "streaming" &&
        status !== "waiting_approval" &&
        status !== "paused"
      ) {
        this.stop();
        return;
      }

      // Delegate fetch to loadSessionMessages — single source of truth
      // for HTTP request + store update + coordinate tracking.
      await store.loadSessionMessages(
        this.agentId,
        this.sessionId,
        undefined, // cursor — not used with line_number
        50,
        "backward",
        this.lineNumber,
        this.charOffset,
      );

      // Read back updated coordinates from the store
      const updated = useChatStore.getState()
        .agentStates[this.agentId]
        ?.sessionStates[this.sessionId];
      if (updated) {
        const prevLineNumber = this.lineNumber;
        this.lineNumber = updated.pollLineNumber;
        this.charOffset = updated.pollCharOffset;

        // Backoff: if no new data this cycle, double interval
        if (this.lineNumber === prevLineNumber && this.charOffset === 0) {
          this.consecutiveEmpty++;
          if (this.consecutiveEmpty >= POLL_MAX_EMPTY_RETRIES) {
            console.log(
              `[PollingManager] ${POLL_MAX_EMPTY_RETRIES} consecutive empty polls — stopping`,
            );
            this.stop();
            return;
          }
          this.intervalMs = Math.min(
            this.intervalMs * POLL_BACKOFF_MULTIPLIER,
            POLL_MAX_MS,
          );
        } else {
          // New data — reset backoff
          this.consecutiveEmpty = 0;
          this.intervalMs = POLL_INITIAL_MS;
        }
      }

      this.scheduleNext();
    } catch (e) {
      console.warn(
        `[PollingManager] Poll error for ${this.agentId}/${this.sessionId}:`,
        e,
      );
      this.scheduleNext();
    }
  }
}

// ── Module-level registry ────────────────────────────────────────────────

/** Active polling managers keyed by "agentId:sessionId" */
const managers = new Map<string, PollingManager>();

function managerKey(agentId: string, sessionId: string): string {
  return `${agentId}:${sessionId}`;
}

/**
 * Start polling for a session. If a manager already exists for this session,
 * it is restarted. Safe to call multiple times.
 */
export function startPolling(
  agentId: string,
  sessionId: string,
): PollingManager {
  const key = managerKey(agentId, sessionId);
  let mgr = managers.get(key);
  if (!mgr) {
    mgr = new PollingManager(agentId, sessionId);
    managers.set(key, mgr);
  }
  mgr.start();
  return mgr;
}

/**
 * Stop polling for a session. Safe to call even if no manager exists.
 */
export function stopPolling(agentId: string, sessionId: string): void {
  const key = managerKey(agentId, sessionId);
  const mgr = managers.get(key);
  if (mgr) {
    mgr.stop();
    managers.delete(key);
  }
}

/**
 * Notify a session's PollingManager of new data available.
 * Creates a manager if one doesn't exist yet.
 *
 * @param totalLines - Total JSONL line count from backend.
 * @param streamingLineNumber - The line number of the current streaming line.
 */
export function notifyNewData(
  agentId: string,
  sessionId: string,
  totalLines: number,
  streamingLineNumber: number,
): void {
  const key = managerKey(agentId, sessionId);
  let mgr = managers.get(key);
  if (!mgr) {
    mgr = new PollingManager(agentId, sessionId);
    managers.set(key, mgr);
    mgr.start();
  }
  mgr.notify(totalLines, streamingLineNumber);
}

/**
 * Stop all polling managers. Used during global cleanup.
 */
export function stopAllPolling(): void {
  for (const mgr of managers.values()) {
    mgr.stop();
  }
  managers.clear();
}
