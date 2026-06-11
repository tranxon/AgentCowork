import { type ReactElement } from "react";
import { cn } from "../../lib/utils";

/**
 * Inverted-color tooltip — light theme shows dark bubble, dark theme shows light bubble.
 * Rounded, no border, shadow-lg. Replaces all native HTML `title` attributes
 * for a consistent visual style across the app.
 *
 * Usage:
 *   <Tooltip content="Send message">
 *     <button>...</button>
 *   </Tooltip>
 *
 * For dropdown triggers that need the tooltip visible only when text is collapsed
 * (container-query), use `tipClass` prop:
 *   <Tooltip content="Model" tipClass="tb-model-tip">
 *     <button>...</button>
 *   </Tooltip>
 */

type TooltipPosition = "top" | "bottom" | "left" | "right";

interface TooltipProps {
  /** Tooltip text content */
  content: string;
  /** The trigger element — must be a single ReactElement */
  children: ReactElement;
  /** Tooltip position relative to trigger. Default: 'top' */
  position?: TooltipPosition;
  /** Max width for long content. Default: '200px' */
  maxWidth?: string;
  /** Optional CSS class for the tooltip wrapper (e.g. container-query collapse class) */
  tipClass?: string;
  /** Delay before showing tooltip (ms). Default: 400 */
  delayMs?: number;
}

const positionClasses: Record<TooltipPosition, string> = {
  top: "bottom-full left-1/2 -translate-x-1/2 mb-1.5",
  bottom: "top-full left-1/2 -translate-x-1/2 mt-1.5",
  left: "right-full top-1/2 -translate-y-1/2 mr-1.5",
  right: "left-full top-1/2 -translate-y-1/2 ml-1.5",
};

export function Tooltip({
  content,
  children,
  position = "top",
  maxWidth = "200px",
  tipClass,
  delayMs = 400,
}: TooltipProps) {
  return (
    <div className="relative inline-flex group/tooltip">
      {children}
      <div
        className={cn(
          positionClasses[position],
          "pointer-events-none absolute hidden group-hover/tooltip:block z-50",
          tipClass,
        )}
        style={{ transitionDelay: `${delayMs}ms` }}
      >
        <div
          className={cn(
            "whitespace-nowrap rounded-md px-2.5 py-1.5 text-[11px] leading-tight shadow-lg",
            "bg-zinc-800 text-white dark:bg-zinc-200 dark:text-zinc-800",
          )}
          style={{ maxWidth }}
        >
          {content}
        </div>
      </div>
    </div>
  );
}