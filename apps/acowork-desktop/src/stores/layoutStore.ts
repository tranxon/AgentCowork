import { create } from "zustand";

/** Right-side results panel tabs — keep in sync with AppLayout.PanelTab. */
export type PanelTab = "debug" | "status" | "setup" | "tools" | "memory" | "workspace";

interface LayoutState {
    /** Currently active tab in the right-side results panel. */
    activePanelTab: PanelTab;
    setActivePanelTab: (tab: PanelTab) => void;

    /** Whether the right-side results panel is collapsed. */
    resultsCollapsed: boolean;
    /**
     * Update the collapsed state. Accepts either a boolean or an updater
     * function (mirrors React's `setState(prev => !prev)` API) so call sites
     * can avoid stale-closure bugs.
     */
    setResultsCollapsed: (collapsed: boolean | ((prev: boolean) => boolean)) => void;

    /**
     * Monotonically-increasing counter for "show me the workspace panel" requests.
     * AppLayout consumes each new value once (via a local ref) to expand the
     * results panel and switch the active tab to "workspace", even when the
     * user clicks the trigger repeatedly for the same file.
     */
    workspacePanelRequestSeq: number;
    requestShowWorkspacePanel: () => void;
}

export const useLayoutStore = create<LayoutState>((set) => ({
    activePanelTab: "workspace",
    setActivePanelTab: (tab) => set({ activePanelTab: tab }),

    resultsCollapsed: false,
    setResultsCollapsed: (collapsed) =>
        set((state) => ({
            resultsCollapsed:
                typeof collapsed === "function"
                    ? (collapsed as (prev: boolean) => boolean)(state.resultsCollapsed)
                    : collapsed,
        })),

    workspacePanelRequestSeq: 0,
    requestShowWorkspacePanel: () =>
        set((state) => ({ workspacePanelRequestSeq: state.workspacePanelRequestSeq + 1 })),
}));