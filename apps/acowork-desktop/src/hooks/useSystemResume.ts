import { useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * Detects system resume from sleep/hibernation and triggers webview recovery.
 *
 * **Detection and recovery are handled primarily by the Rust backend**
 * (Windows / macOS / Linux).  The backend samples two monotonic clocks — one
 * that includes sleep time (biased) and one that excludes it (unbiased).  The
 * difference between their deltas is the *exact* amount of time the system spent
 * asleep — zero for a normal minimise/restore, non-zero for a real sleep/wake.
 *
 * Platform clock pairs:
 *   • Windows: `GetTickCount64()` vs `QueryUnbiasedInterruptTime()`
 *   • macOS:   `clock_gettime(CLOCK_MONOTONIC_RAW)` vs `CLOCK_UPTIME_RAW`
 *   • Linux:   `clock_gettime(CLOCK_BOOTTIME)` vs `CLOCK_MONOTONIC`
 *
 * When real sleep is detected, the backend calls `WebviewWindow::reload()`
 * natively (equivalent to F5), which works even when the WebView2 renderer/IPC
 * is broken after a GPU compositor crash during sleep.  The backend also sets
 * the `acowork_recovery_reload` sessionStorage flag via `eval()` so App.tsx
 * can skip the splash screen on recovery reload.
 *
 * This hook is a **backup** — it listens for the `"system-resume"` Tauri event
 * in case the native `reload()` fails but the JS event pipeline is still alive.
 * In normal operation, the backend's native reload fires first and this listener
 * never triggers.
 *
 * The hook should be mounted once, as high in the tree as possible (App.tsx).
 */
export function useSystemResume() {
    const recover = useCallback(() => {
        console.warn(
            "[useSystemResume] System resume detected — reloading webview to recover GPU compositor",
        );
        // Flag for App.tsx to skip the splash screen on recovery reload.
        // sessionStorage survives location.reload() but is cleared on tab close,
        // so it won't interfere with future cold starts.
        sessionStorage.setItem("acowork_recovery_reload", "1");
        window.location.reload();
    }, []);

    useEffect(() => {
        let unlisten: (() => void) | undefined;
        listen("system-resume", () => {
            console.warn("[useSystemResume] Received system-resume event from Tauri backend");
            recover();
        }).then((fn) => {
            unlisten = fn;
        });

        return () => {
            unlisten?.();
        };
    }, [recover]);
}
