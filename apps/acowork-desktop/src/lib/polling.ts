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
//! - Auto-stop: NEVER — the poller is stopped only by explicit stop()
//!   calls (done/error/stopped/session switch). LLM thinking phases can
//!   last 10-30 seconds with no data; auto-stopping on empty polls would
//!   kill the poller before the first token arrives.
//!
//! ## Lifecycle
//!
//! - `start()` — begin polling (called when session becomes active/streaming)
//! - `stop()`  — stop polling (called on done/error/stopped or session switch)
//! - `notify(totalLines)` — trigger immediate poll

import { useChatStore } from "../stores/chatStore";

/** Initial polling interval in milliseconds */
const POLL_INITIAL_MS = 500;
/** Maximum backoff interval in milliseconds */
const POLL_MAX_MS = 5000;
/** Backoff multiplier per empty response */
const POLL_BACKOFF_MULTIPLIER = 2.0;

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
   * The Runtime already throttles these notifications to ≤ 2/sec (500ms
   * cooldown per ADR-021 §难点 1), so this method always fires an immediate
   * fetch without additional frontend-side rate limiting.
   *
   * If the poller was stopped (e.g., by a previous done/error event but
   * the session was re-activated), it is restarted automatically.
   *
   * @param totalLines - Total JSONL line count from the backend notification.
   */
  notify(totalLines: number): void {
    if (!this.running) {
      // Poller was stopped — restart it. This handles the case where
      // the fallback timer expired during a long thinking phase but
      // the session is still streaming.
      this.start();
    }

    // Update line number from the notification.
    // NOTE: Do NOT reset charOffset here. The offset tracks how much
    // of the streaming line content the frontend has already consumed.
    // Resetting it to 0 on every notification causes the backend to
    // return the full accumulated content each time, which the frontend
    // then appends — producing exponential duplication and preventing
    // incremental display. The offset is correctly maintained by poll
    // responses (streaming.char_offset).
    if (totalLines > 0) {
      this.lineNumber = totalLines;
    }

    console.log(
      `[PollingManager] notify: totalLines=${totalLines}`,
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

        // Backoff: if no new data this cycle, double interval (max 5s).
        // Do NOT auto-stop — LLM thinking phases can last 10-30 seconds
        // with no data. The poller is stopped only by explicit stop()
        // calls (done/error/stopped/session_state_changed to idle).
        if (this.lineNumber === prevLineNumber && this.charOffset === 0) {
          this.intervalMs = Math.min(
            this.intervalMs * POLL_BACKOFF_MULTIPLIER,
            POLL_MAX_MS,
          );
        } else {
          // New data — reset backoff
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
 */
export function notifyNewData(
  agentId: string,
  sessionId: string,
  totalLines: number,
): void {
  const key = managerKey(agentId, sessionId);
  let mgr = managers.get(key);
  if (!mgr) {
    mgr = new PollingManager(agentId, sessionId);
    managers.set(key, mgr);
  }
  mgr.notify(totalLines);
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
