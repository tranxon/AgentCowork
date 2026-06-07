import { useState, useCallback, useMemo, useEffect, useRef } from "react";
import { Search, RefreshCw, FolderOpen, FileText, ChevronLeft, ChevronRight } from "lucide-react";
import { useAgentStore } from "../../stores/agentStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useChatStore } from "../../stores/chatStore";
import { useFileEditorStore } from "../../stores/fileEditorStore";
import { FileTree } from "./FileTree/FileTree";
import { WorkspaceSelector } from "./WorkspaceSelector";
import type { TreeEntry } from "../../stores/workspaceStore";
import { useTranslation } from "../../i18n/useTranslation";

const PAGE_SIZE = 15;

/** Abbreviate a file path from the left: "…/parent/filename.ext" */
function abbreviatePath(path: string): string {
    const maxLen = 38;
    if (path.length <= maxLen) return path;
    const parts = path.split("/");
    const filename = parts[parts.length - 1];
    // Try progressively shorter tails until within maxLen
    for (let i = parts.length - 2; i >= 1; i--) {
        const abbreviated = `…/${parts.slice(i).join("/")}`;
        if (abbreviated.length <= maxLen) return abbreviated;
    }
    return `…/${filename}`;
}

export function WorkspaceExplorer() {
    const { t } = useTranslation();
    const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
    const agents = useAgentStore((s) => s.agents);
    const selectedAgent = agents.find((a) => a.agent_id === selectedAgentId);
    const invalidateTreeCache = useWorkspaceStore((s) => s.invalidateTreeCache);
    const fetchTree = useWorkspaceStore((s) => s.fetchTree);
    const treeCache = useWorkspaceStore((s) => s.treeCache);
    const sessionWorkspaceMap = useWorkspaceStore((s) => s.sessionWorkspaceMap);
    const openFile = useFileEditorStore((s) => s.openFile);

    // Get the current workspace ID for the active session
    const activeSessionId = useChatStore((s) =>
        selectedAgentId ? s.getActiveSessionId(selectedAgentId) : null,
    );
    const currentWorkspaceId = activeSessionId
        ? (sessionWorkspaceMap[activeSessionId] ?? "__agent_home__")
        : "__agent_home__";

    const ck = `${selectedAgentId}:${currentWorkspaceId}`;
    const [searchQuery, setSearchQuery] = useState("");
    const [searchFocused, setSearchFocused] = useState(false);
    const [currentPage, setCurrentPage] = useState(1);
    const inputRef = useRef<HTMLInputElement>(null);
    const dropdownRef = useRef<HTMLDivElement>(null);

    // Reset page when search query changes
    useEffect(() => {
        setCurrentPage(1);
    }, [searchQuery]);

    const showDropdown = searchFocused && searchQuery.length > 0;

    // Auto-fetch unfetched directories when searching to broaden match scope
    useEffect(() => {
        if (!searchQuery || !selectedAgentId) return;
        const doFetch = () => {
            const toFetch: string[] = [];
            for (const [key, entries] of Object.entries(treeCache)) {
                if (!key.startsWith(`${ck}:`)) continue;
                for (const entry of entries) {
                    if (entry.type !== "directory") continue;
                    const dirPath = key.slice(ck.length + 1);
                    const childPath = dirPath ? `${dirPath}/${entry.name}` : entry.name;
                    if (!treeCache[`${ck}:${childPath}`]) {
                        toFetch.push(childPath);
                    }
                }
            }
            for (const p of toFetch.slice(0, 20)) {
                fetchTree(selectedAgentId, currentWorkspaceId, p);
            }
        };
        doFetch();
        const timer = setInterval(doFetch, 300);
        return () => clearInterval(timer);
    }, [searchQuery, ck, treeCache, selectedAgentId, currentWorkspaceId, fetchTree]);

    // Collect matching files from cached tree entries
    const matchingFiles = useMemo(() => {
        if (!searchQuery) return [];
        const q = searchQuery.toLowerCase();
        const qParts = q.split(/[\s\/\\]+/).filter(Boolean);
        const results: { name: string; relPath: string; dir: string; score: number }[] = [];
        const seen = new Set<string>();

        for (const [key, entries] of Object.entries(treeCache)) {
            if (!key.startsWith(`${ck}:`)) continue;
            const dirPath = key.slice(ck.length + 1);

            for (const entry of entries) {
                if (entry.type !== "file") continue;
                const fullPath = dirPath ? `${dirPath}/${entry.name}` : entry.name;
                if (seen.has(fullPath)) continue;
                seen.add(fullPath);

                const nameLower = entry.name.toLowerCase();
                const pathLower = fullPath.toLowerCase();

                let score = 0;
                if (qParts.every((p) => nameLower.includes(p))) {
                    score = nameLower === q ? 3 : nameLower.startsWith(q) ? 2 : 1;
                } else if (qParts.every((p) => pathLower.includes(p))) {
                    score = 0.5;
                } else {
                    continue;
                }

                results.push({ name: entry.name, relPath: fullPath, dir: dirPath, score });
            }
        }

        results.sort((a, b) => b.score - a.score || a.relPath.length - b.relPath.length);
        return results;
    }, [searchQuery, treeCache, ck]);

    const totalPages = Math.max(1, Math.ceil(matchingFiles.length / PAGE_SIZE));
    const currentPageFiles = matchingFiles.slice(
        (currentPage - 1) * PAGE_SIZE,
        currentPage * PAGE_SIZE,
    );
    const start = matchingFiles.length > 0 ? (currentPage - 1) * PAGE_SIZE + 1 : 0;
    const end = Math.min(currentPage * PAGE_SIZE, matchingFiles.length);

    const handleRefresh = useCallback(() => {
        if (!selectedAgentId) return;
        invalidateTreeCache(selectedAgentId);
        fetchTree(selectedAgentId, currentWorkspaceId, "");
    }, [selectedAgentId, currentWorkspaceId, invalidateTreeCache, fetchTree]);

    const handleFileDoubleClick = useCallback((_entry: TreeEntry, relPath: string) => {
        if (!selectedAgentId) return;
        void openFile(selectedAgentId, currentWorkspaceId, relPath);
    }, [selectedAgentId, currentWorkspaceId, openFile]);

    const handleSearchSelect = useCallback((relPath: string) => {
        if (!selectedAgentId) return;
        void openFile(selectedAgentId, currentWorkspaceId, relPath);
        setSearchQuery("");
        setSearchFocused(false);
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

            {/* Search box with dropdown */}
            <div className="relative border-b border-zinc-200 dark:border-zinc-800">
                <div className="flex items-center gap-1.5 px-3 py-1">
                    <Search className="h-3 w-3 shrink-0 text-zinc-400" />
                    <input
                        ref={inputRef}
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        onFocus={() => setSearchFocused(true)}
                        onBlur={() => setTimeout(() => setSearchFocused(false), 200)}
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

                {/* Dropdown results */}
                {showDropdown && (
                    <div
                        ref={dropdownRef}
                        className="absolute left-0 right-0 top-full z-50 mt-1 rounded-lg border border-zinc-200 bg-white shadow-lg dark:border-zinc-700 dark:bg-zinc-800"
                    >
                        {/* Header with count */}
                        <div className="flex items-center justify-between border-b border-zinc-200 px-3 py-1.5 text-[11px] text-zinc-500 dark:border-zinc-700 dark:text-zinc-400">
                            {matchingFiles.length > 0 ? (
                                <span>Showing {start}&ndash;{end} of {matchingFiles.length}</span>
                            ) : (
                                <span>No matching files</span>
                            )}
                        </div>

                        {/* Items */}
                        <div className="max-h-80 overflow-y-auto py-1">
                            {currentPageFiles.length === 0 ? (
                                <div className="px-3 py-4 text-center text-xs text-zinc-400 dark:text-zinc-500">
                                    No matching files
                                </div>
                            ) : (
                                currentPageFiles.map((f) => (
                                    <div
                                        key={f.relPath}
                                        onMouseDown={(e) => e.preventDefault()}
                                        onClick={() => handleSearchSelect(f.relPath)}
                                        className="group flex items-center gap-2 px-3 py-1.5 transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-700/50"
                                    >
                                        <FileText className="h-3.5 w-3.5 shrink-0 text-zinc-400" />
                                        <span className="shrink-0 text-xs text-zinc-700 dark:text-zinc-300">
                                            {f.name}
                                        </span>
                                        <span className="min-w-0 truncate text-[10px] text-zinc-400 dark:text-zinc-500 ml-3">
                                            {abbreviatePath(f.dir)}
                                        </span>
                                    </div>
                                ))
                            )}
                        </div>

                        {/* Pagination */}
                        {totalPages > 1 && (
                            <div className="flex items-center justify-between border-t border-zinc-200 px-1 py-1.5 dark:border-zinc-700">
                                <button
                                    onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                                    disabled={currentPage <= 1}
                                    className="inline-flex items-center rounded-md px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-100 disabled:opacity-30 dark:text-zinc-400 dark:hover:bg-zinc-800"
                                >
                                    <ChevronLeft className="h-3.5 w-3.5" />
                                </button>
                                <span className="text-[11px] text-zinc-500 dark:text-zinc-400">
                                    Page {currentPage} of {totalPages}
                                </span>
                                <button
                                    onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                                    disabled={currentPage >= totalPages}
                                    className="inline-flex items-center rounded-md px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-100 disabled:opacity-30 dark:text-zinc-400 dark:hover:bg-zinc-800"
                                >
                                    <ChevronRight className="h-3.5 w-3.5" />
                                </button>
                            </div>
                        )}
                    </div>
                )}
            </div>

            {/* File tree (normal mode, no search filtering) */}
            {selectedAgentId && (
                <FileTree agentId={selectedAgentId} workspaceId={currentWorkspaceId} onFileDoubleClick={handleFileDoubleClick} />
            )}
        </div>
    );
}
