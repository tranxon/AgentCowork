import { type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
import { cn } from "../../lib/utils";
import { toolbarButton } from "../../lib/ui-styles";

/**
 * Shared toolbar dropdown trigger — icon + text + chevron + hover tooltip.
 *
 * Text and chevron carry `collapseClass` so CSS container-query rules
 * in globals.css can hide them at specific toolbar widths.
 * The tooltip carries `tipClass` and is shown on hover only when text is hidden.
 */
export function ToolbarDropdownTrigger({
    icon,
    label,
    collapseClass,
    tipClass,
    open,
    onToggle,
    wrapperRef,
    buttonClassName,
    children,
}: {
    icon: ReactNode;
    label: string;
    /** CSS class that container-query rules target to hide text + chevron */
    collapseClass: string;
    /** CSS class that container-query rules target to show tooltip */
    tipClass: string;
    open: boolean;
    onToggle: () => void;
    wrapperRef?: React.Ref<HTMLDivElement>;
    buttonClassName?: string;
    children: ReactNode;
}) {
    return (
        <div ref={wrapperRef} className="relative inline-block group">
            <button
                type="button"
                onClick={onToggle}
                className={cn(
                    toolbarButton,
                    open && "bg-zinc-200 dark:bg-zinc-700 text-zinc-900 dark:text-zinc-100",
                    buttonClassName,
                )}
            >
                {icon}
                <span className={cn(collapseClass, "max-w-[120px] truncate")}>{label}</span>
                <ChevronDown className={cn("h-3 w-3 text-zinc-400", collapseClass)} />
            </button>
            {/* Tooltip — visible on hover when text is collapsed (CSS-controlled) */}
            <div className={cn(
                tipClass,
                "pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-1.5 hidden group-hover:block z-50",
            )}>
                <div className="whitespace-nowrap rounded-md bg-zinc-800 dark:bg-zinc-200 px-2.5 py-1.5 text-[11px] leading-tight text-white dark:text-zinc-800 shadow-lg max-w-[200px] truncate">
                    {label}
                </div>
            </div>
            {children}
        </div>
    );
}
