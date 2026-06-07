import { useState, useCallback } from "react";
import { Search, RefreshCw, FolderOpen } from "lucide-react";
import { useAgentStore } from "../../stores/agentStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useChatStore } from "../../stores/chatStore";
import { useFileEditorStore } from "../../stores/fileEditorStore";
import { FileTree } from "./FileTree/FileTree";
import { WorkspaceSelector } from "./WorkspaceSelector";
import type { TreeEntry } from "../../stores/workspaceStore";
import { useTranslation } from "../../i18n/useTranslation";

export function WorkspaceExplorer() {
    const { t } = useTranslation();
    const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
    const agents = useAgentStore((s) => s.agents);
    const selectedAgent = agents.find((a) => a.agent_id === selectedAgentId);
    const invalidateTreeCache = useWorkspaceStore((s) => s.invalidateTreeCache);
    const fetchTree = useWorkspaceStore((s) => s.fetchTree);
    const sessionWorkspaceMap = useWorkspaceStore((s) => s.sessionWorkspaceMap);
    const openFile = useFileEditorStore((s) => s.openFile);

    // Get the current workspace ID for the active session
    const activeSessionId = useChatStore((s) =>
        selectedAgentId ? s.getActiveSessionId(selectedAgentId) : null,
    );
    const currentWorkspaceId = activeSessionId
        ? (sessionWorkspaceMap[activeSessionId] ?? "__agent_home__")
        : "__agent_home__";

    const [searchQuery, setSearchQuery] = useState("");

    const handleRefresh = useCallback(() => {
        if (!selectedAgentId) return;
        invalidateTreeCache(selectedAgentId);
        fetchTree(selectedAgentId, currentWorkspaceId, "");
    }, [selectedAgentId, currentWorkspaceId, invalidateTreeCache, fetchTree]);

    const handleFileDoubleClick = useCallback((_entry: TreeEntry, relPath: string) => {
        if (!selectedAgentId) return;
        void openFile(selectedAgentId, currentWorkspaceId, relPath);
    }, [selectedAgentId, currentWorkspaceId, openFile]);

    if (!selectedAgent?.running) {
        return (
            <div className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-xs text-zinc-500 dark:text-zinc-400">
                <FolderOpen className="h-6 w-6" />
                <span>{t("workspace.explorer.agentNotRunning")}</span>
            </div>
        );
    }

    return (
        <div className="flex flex-1 flex-col overflow-hidden">
            {/* Workspace selector + root name */}
            <div className="flex items-center gap-1.5 border-b border-zinc-200 px-2 py-1.5 dark:border-zinc-800">
                <WorkspaceSelector dropDirection="down" />
                <button
                    onClick={handleRefresh}
                    className="ml-auto rounded p-0.5 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600 dark:hover:bg-zinc-800 dark:hover:text-zinc-300"
                    title={t("workspace.explorer.refresh")}
                >
                    <RefreshCw className="h-3 w-3" />
                </button>
            </div>

            {/* Search box */}
            <div className="flex items-center gap-1.5 border-b border-zinc-200 px-2 py-1 dark:border-zinc-800">
                <Search className="h-3 w-3 shrink-0 text-zinc-400" />
                <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder={t("workspace.explorer.searchPlaceholder")}
                    className="flex-1 bg-transparent text-xs text-zinc-700 outline-none placeholder:text-zinc-400 dark:text-zinc-400 dark:placeholder:text-zinc-500"
                />
                {searchQuery && (
                    <button
                        onClick={() => setSearchQuery("")}
                        className="text-[10px] text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
                    >
                        ✕
                    </button>
                )}
            </div>

            {/* File tree */}
            {selectedAgentId && (
                <FileTree agentId={selectedAgentId} workspaceId={currentWorkspaceId} searchQuery={searchQuery} onFileDoubleClick={handleFileDoubleClick} />
            )}
        </div>
    );
}
