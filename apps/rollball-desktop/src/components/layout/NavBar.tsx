import { useId, type ComponentType } from "react";
import type { NavView } from "../../lib/types";
import { cn } from "../../lib/utils";
import { Settings } from "lucide-react";
import { UserAvatar } from "../common/UserAvatar";
import { useUserProfileStore } from "../../stores/userProfileStore";
import { useSettingsStore } from "../../stores/settingsStore";

interface NavBarProps {
  currentView: NavView;
  onViewChange: (view: NavView) => void;
  /** Called when user clicks their avatar — navigate to profile settings */
  onAvatarClick: () => void;
}

const navItems: { view: NavView; icon: ComponentType<{ className?: string }>; label: string }[] = [
  { view: "chat", icon: OutlineChatIcon, label: "Chat" },
  { view: "harness", icon: OutlineHarnessIcon, label: "Harness" },
  { view: "settings", icon: OutlineSettingsIcon, label: "Settings" },
];

/** Filled chat bubble SVG — oval/pill style */
function FilledChatIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="2">
      <g transform="translate(1.2, 1.2) scale(0.9)">
        <path d="M12 3C6.5 3 2 7.1 2 12c0 2.5 1.1 4.8 2.9 6.5L3 22l5.3-2.3C9.6 20.5 11.2 21 13 21c5.5 0 10-4.1 10-9s-4.5-9-11-9z" />
      </g>
    </svg>
  );
}

/** Outline chat bubble SVG — same oval shape, stroke-only */
function OutlineChatIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
      <g transform="translate(1.2, 1.2) scale(0.9)">
        <path d="M12 3C6.5 3 2 7.1 2 12c0 2.5 1.1 4.8 2.9 6.5L3 22l5.3-2.3C9.6 20.5 11.2 21 13 21c5.5 0 10-4.1 10-9s-4.5-9-11-9z" />
      </g>
    </svg>
  );
}

/** Outline gear icon — stroke-only for non-selected settings state */
function OutlineSettingsIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

/** Filled gear icon with center hole punched out via SVG mask */
function FilledSettingsIcon({ className }: { className?: string }) {
  const maskId = useId();
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="2">
      <defs>
        <mask id={maskId}>
          <rect width="24" height="24" fill="white" />
          <circle cx="12" cy="12" r="3" fill="black" />
        </mask>
      </defs>
      <g mask={`url(#${maskId})`}>
        <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      </g>
    </svg>
  );
}

/** Outline puzzle piece icon — stroke-only for non-selected harness state */
function OutlineHarnessIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
      <g transform="translate(1.2, 1.2) scale(0.9, 0.9)">
        <path d="M19.439 7.85c-.049.322.059.648.289.878l1.568 1.568c.47.47.706 1.087.706 1.704s-.235 1.233-.706 1.704l-1.611 1.611a.98.98 0 0 1-.837.276c-.47-.07-.802-.48-.968-.925a2.501 2.501 0 1 0-3.214 3.214c.446.166.855.497.925.968a.979.979 0 0 1-.276.837l-1.61 1.611a2.404 2.404 0 0 1-1.705.706 2.404 2.404 0 0 1-1.704-.706l-1.568-1.568a1.026 1.026 0 0 0-.877-.29c-.493.074-.84.504-1.02.968a2.5 2.5 0 1 1-3.237-3.237c.464-.18.894-.527.967-1.02a1.026 1.026 0 0 0-.289-.877l-1.568-1.568A2.404 2.404 0 0 1 1.998 12c0-.617.236-1.234.706-1.704L4.315 8.685a.98.98 0 0 1 .837-.276c.47.07.802.48.968.925a2.501 2.501 0 1 0 3.214-3.214c-.446-.166-.855-.497-.925-.968a.979.979 0 0 1 .276-.837l1.611-1.611a2.404 2.404 0 0 1 1.704-.706c.617 0 1.234.236 1.704.706l1.568 1.568c.23.23.556.338.877.29.493-.074.84-.504 1.02-.969a2.5 2.5 0 1 1 3.237 3.237c-.464.18-.894.527-.967 1.02Z" />
      </g>
    </svg>
  );
}

/** Filled puzzle piece icon — solid version for active harness state */
function FilledHarnessIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="2">
      <g transform="translate(1.2, 1.2) scale(0.9, 0.9)">
        <path d="M19.439 7.85c-.049.322.059.648.289.878l1.568 1.568c.47.47.706 1.087.706 1.704s-.235 1.233-.706 1.704l-1.611 1.611a.98.98 0 0 1-.837.276c-.47-.07-.802-.48-.968-.925a2.501 2.501 0 1 0-3.214 3.214c.446.166.855.497.925.968a.979.979 0 0 1-.276.837l-1.61 1.611a2.404 2.404 0 0 1-1.705.706 2.404 2.404 0 0 1-1.704-.706l-1.568-1.568a1.026 1.026 0 0 0-.877-.29c-.493.074-.84.504-1.02.968a2.5 2.5 0 1 1-3.237-3.237c.464-.18.894-.527.967-1.02a1.026 1.026 0 0 0-.289-.877l-1.568-1.568A2.404 2.404 0 0 1 1.998 12c0-.617.236-1.234.706-1.704L4.315 8.685a.98.98 0 0 1 .837-.276c.47.07.802.48.968.925a2.501 2.501 0 1 0 3.214-3.214c-.446-.166-.855-.497-.925-.968a.979.979 0 0 1 .276-.837l1.611-1.611a2.404 2.404 0 0 1 1.704-.706c.617 0 1.234.236 1.704.706l1.568 1.568c.23.23.556.338.877.29.493-.074.84-.504 1.02-.969a2.5 2.5 0 1 1 3.237 3.237c-.464.18-.894.527-.967 1.02Z" />
      </g>
    </svg>
  );
}

export function NavBar({ currentView, onViewChange, onAvatarClick }: NavBarProps) {
  const profile = useUserProfileStore((s) => s.profile);
  const { opacity, theme } = useSettingsStore();
  const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  // Original gray: #E2E3E9 (light) / #292A2C (dark), modulated by opacity
  const bgColor = isDark ? `rgba(41,42,44,${opacity})` : `rgba(226,227,233,${opacity})`;

  return (
    <nav
      className="flex w-[var(--spacing-nav)] flex-col items-center gap-0 py-2"
      role="navigation"
      aria-label="Main navigation"
      style={{
        backgroundColor: bgColor,
      } as React.CSSProperties}
    >
      {/* User avatar — click to edit profile (WeChat-style top placement) */}
      <button
        onClick={onAvatarClick}
        className="mb-3 flex items-center justify-center rounded-md transition-colors duration-150 hover:ring-2 hover:ring-zinc-400 dark:hover:ring-zinc-500"
        title="Edit Profile"
        aria-label="Edit Profile"
      >
        <UserAvatar
          displayName={profile.displayName}
          size={40}
          className="shrink-0"
        />
      </button>

      {/* Navigation items */}
      {navItems.map(({ view, icon: Icon, label }) => (
        <button
          key={view}
          onClick={() => onViewChange(view)}
          className={cn(
            "flex h-12 w-10 items-center justify-center rounded-md transition-colors duration-150",
            currentView === view
              ? ""
              : "text-zinc-500 hover:text-zinc-600 hover:bg-[#D8D9DC] dark:text-zinc-400 dark:hover:text-zinc-300 dark:hover:bg-[#3D3D3F]",
          )}
          style={currentView === view ? { color: "var(--color-accent)" } : undefined}
          title={label}
          aria-label={label}
          aria-current={currentView === view ? "page" : undefined}
        >
          {currentView === view ? (
            view === "chat" ? (
              <FilledChatIcon className="h-6 w-6" />
            ) : view === "harness" ? (
              <FilledHarnessIcon className="h-6 w-6" />
            ) : (
              <FilledSettingsIcon className="h-6 w-6" />
            )
          ) : (
            <Icon className="h-6 w-6" />
          )}
        </button>
      ))}
    </nav>
  );
}
