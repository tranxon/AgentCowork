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

/** Polling interval when no `interval_ms` is provided by backend (fallback). */
const POLL_FALLBACK_MS = 500;
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
  private baseIntervalMs: number;
  private currentIntervalMs: number;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lineNumber: number = 0;
  private charOffset: number = 0;
  private running: boolean = false;

  constructor(
    agentId: string,
    sessionId: string,
    initialLineNumber?: number,
    initialCharOffset?: number,
  ) {
    this.agentId = agentId;
    this.sessionId = sessionId;
    this.baseIntervalMs = POLL_FALLBACK_MS;
    this.currentIntervalMs = POLL_FALLBACK_MS;
    // ADR-021 Phase 4: Initialize from store's last-known coordinates so
    // the first scheduled poll doesn't start from line 0 and accidentally
    // overwrite optimistically rendered messages.
    if (initialLineNumber != null) this.lineNumber = initialLineNumber;
    if (initialCharOffset != null) this.charOffset = initialCharOffset;
  }

  /** Start the polling loop. Idempotent — safe to call multiple times. */
  start(): void {
    if (this.running) return;
    this.running = true;
    this.currentIntervalMs = this.baseIntervalMs;
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
   * The Runtime already throttles these notifications to the configured
   * `interval_ms` (from DataFlowConfig), so this method always fires an
   * immediate fetch without additional frontend-side rate limiting.
   *
   * If the poller was stopped (e.g., by a previous done/error event but
   * the session was re-activated), it is restarted automatically.
   *
   * @param totalLines - Total JSONL line count from the backend notification.
   * @param intervalMs - Notify throttle interval from backend (DataFlowConfig).
   *                     Used as the base polling interval.  When omitted,
   *                     POLL_FALLBACK_MS is used.
   */
  notify(totalLines: number, intervalMs?: number): void {
    if (!this.running) {
      this.start();
    }

    // Update base interval from backend notification if provided.
    // This ensures the polling rate matches the Runtime's throttle rate.
    if (intervalMs != null && intervalMs > 0) {
      this.baseIntervalMs = intervalMs;
      this.currentIntervalMs = intervalMs;
    }

    // Update line number from the notification.
    // When totalLines increases, the previous streaming line has been flushed
    // to JSONL (e.g., role transition from thought→assistant) and a new
    // streaming line has started. The old charOffset belongs to the flushed
    // line and must be reset to 0 — otherwise the new line's first poll skips
    // its opening characters, causing the "first line truncated" bug.
    if (totalLines > this.lineNumber) {
      this.charOffset = 0;
    }
    if (totalLines > 0) {
      this.lineNumber = totalLines;
    }

    console.log(
      `[PollingManager] notify: totalLines=${totalLines}, intervalMs=${intervalMs ?? "not set"}`,
    );

    // Trigger immediate poll (cancel any pending timer first)
    this.clearTimer();
    this.doPoll();
  }

  /** Return the current base polling interval in ms. */
  getIntervalMs(): number {
    return this.baseIntervalMs;
  }
  private scheduleNext(): void {
    if (!this.running) return;
    this.timer = setTimeout(() => {
      this.doPoll();
    }, this.currentIntervalMs);
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
          this.currentIntervalMs = Math.min(
            this.currentIntervalMs * POLL_BACKOFF_MULTIPLIER,
            POLL_MAX_MS,
          );
        } else {
          // New data — reset backoff
          this.currentIntervalMs = this.baseIntervalMs;
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
    // ADR-021 Phase 4: Read current poll coordinates from store so the first
    // scheduled poll uses the correct lineNumber (not 0 by default), preventing
    // accidental full-range fetches that could overwrite optimistic messages.
    const store = useChatStore.getState();
    const session = store.agentStates[agentId]?.sessionStates[sessionId];
    const lineNumber = session?.pollLineNumber;
    const charOffset = session?.pollCharOffset;
    mgr = new PollingManager(agentId, sessionId, lineNumber, charOffset);
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
 * @param intervalMs - Notify throttle interval from backend (DataFlowConfig).
 *                     Used as the polling interval base.
 */
export function notifyNewData(
  agentId: string,
  sessionId: string,
  totalLines: number,
  intervalMs?: number,
): void {
  const key = managerKey(agentId, sessionId);
  let mgr = managers.get(key);
  if (!mgr) {
    mgr = new PollingManager(agentId, sessionId);
    managers.set(key, mgr);
  }
  mgr.notify(totalLines, intervalMs);
}

/**
 * Get the current polling interval for a session.
 * Returns undefined if no manager exists (no streaming in progress).
 */
export function getPollingIntervalMs(
  agentId: string,
  sessionId: string,
): number | undefined {
  const key = managerKey(agentId, sessionId);
  return managers.get(key)?.getIntervalMs();
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
