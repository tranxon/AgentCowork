import { useRef } from "react";
import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function TitleBar() {
  const isMacOS = navigator.platform.includes("Mac");
  const win = getCurrentWindow();

  // Timer to distinguish single-click (drag) from double-click (maximize).
  // On Windows, startDragging() blocks the event loop synchronously, which
  // prevents dblclick from firing.  A short delay before startDragging()
  // gives the second click time to arrive — if it does, we cancel the drag
  // timer and toggle maximize instead.  The 250ms delay is short enough to
  // feel instant for drag but long enough to catch a double-click.
  const dragTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleMinimize = async () => {
    try {
      await win.minimize();
    } catch (error) {
      console.error("Failed to minimize:", error);
    }
  };

  const handleMaximize = async () => {
    try {
      await win.toggleMaximize();
    } catch (error) {
      console.error("Failed to toggle maximize:", error);
    }
  };

  const handleClose = async () => {
    try {
      await win.close();
    } catch (error) {
      console.error("Failed to close:", error);
    }
  };

  const handleMouseDown = async (e: React.MouseEvent) => {
    // Only handle primary mouse button (left click)
    if (e.button !== 0) return;

    if (dragTimer.current) {
      // Second click — double-click detected, cancel drag timer
      clearTimeout(dragTimer.current);
      dragTimer.current = null;

      try {
        await win.toggleMaximize();
      } catch (error) {
        console.error("Failed to toggle maximize:", error);
      }
      return;
    }

    // First click — wait briefly; if no second click comes, start dragging
    dragTimer.current = setTimeout(async () => {
      dragTimer.current = null;
      try {
        await win.startDragging();
      } catch (error) {
        console.error("Failed to start drag:", error);
      }
    }, 250);
  };

  // On macOS, the native traffic lights provide close/minimize/maximize.
  // On Windows/Linux, we render custom buttons.
  return (
    <div
      onMouseDown={handleMouseDown}
      className={`flex h-8 w-full items-center justify-between select-none ${
        isMacOS ? "pl-[80px]" : "pl-3"
      }`}
    >
      {/* Left: App title */}
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium text-zinc-700 dark:text-zinc-300">
          Acowork
        </span>
      </div>

      {/* Right: Window controls (Windows/Linux only) */}
      {!isMacOS && (
        <div className="flex items-center gap-1" onMouseDown={(e) => e.stopPropagation()}>
          <button
            className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 hover:bg-zinc-300 dark:text-zinc-400 dark:hover:bg-zinc-700"
            onClick={handleMinimize}
          >
            <Minus className="h-3.5 w-3.5" />
          </button>
          <button
            className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 hover:bg-zinc-300 dark:text-zinc-400 dark:hover:bg-zinc-700"
            onClick={handleMaximize}
          >
            <Square className="h-3 w-3" />
          </button>
          <button
            className="flex h-8 w-8 items-center justify-center rounded text-zinc-600 hover:bg-red-500 hover:text-white dark:text-zinc-400 dark:hover:bg-red-600"
            onClick={handleClose}
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      )}
    </div>
  );
}
