import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useWorkspaceStore, type TreeEntry } from "../../../stores/workspaceStore";
import { useAgentStore } from "../../../stores/agentStore";
import { useChatStore } from "../../../stores/chatStore";
import { useFileEditorStore } from "../../../stores/fileEditorStore";
import { useSettingsStore } from "../../../stores/settingsStore";
import { FileTreeNode } from "./FileTreeNode";

const EMPTY_ARRAY: string[] = [];

/** Flattened tree node for virtualized rendering */
interface FlatNode {
    entry: TreeEntry;
    depth: number;
    relPath: string;
}

interface FileTreeProps {
    agentId: string;
    workspaceId: string;
    sessionId: string;
    onFileDoubleClick?: (entry: TreeEntry, relPath: string) => void;
    onContextNewItem?: (type: "file" | "dir", parentPath: string) => void;
    onDelete?: (relPath: string, isDir: boolean) => void;
    onCopy?: (relPath: string, isDir: boolean) => void;
    onPaste?: (parentPath: string) => void;
}

export function FileTree({ agentId, workspaceId, sessionId, onFileDoubleClick, onContextNewItem, onDelete, onCopy, onPaste }: FileTreeProps) {
    const [selectedPath, setSelectedPath] = useState<string | null>(null);
    const treeCache = useWorkspaceStore((s) => s.treeCache);
    const fetchTree = useWorkspaceStore((s) => s.fetchTree);
    const treeLoadingPaths = useWorkspaceStore((s) => s.treeLoadingPaths);
    const toggleTreeExpandedPath = useChatStore((s) => s.toggleTreeExpandedPath);
    const expandTreeToPath = useChatStore((s) => s.expandTreeToPath);
    const selectedAgentId = useAgentStore((s) => s.selectedAgentId);

    /** Build cache key prefix: agentId:workspaceId (tree cache is NOT per-session) */
    const treeCachePrefix = `${agentId}:${workspaceId}`;
    const treeRoots = useWorkspaceStore((s) => s.treeRoots);
    const workspaceRoot = treeRoots[`${agentId}:${workspaceId}`] ?? "";

    // Expanded paths from the session — Zustand selector is reactive
    const expandedPathsArr = useChatStore((s) => {
        const ss = s.agentStates[agentId]?.sessionStates[sessionId];
        return ss?.treeExpandedPaths ?? EMPTY_ARRAY;
    });
    const expandedPaths = useMemo(() => new Set(expandedPathsArr), [expandedPathsArr]);

    // Compute set of directory paths that contain at least one open editor tab.
    // e.g. open file "src/components/Foo.tsx" → dirs: "src", "src/components"
    const openFiles = useFileEditorStore((s) => s.openFiles);
    const openFileDirSet = useMemo(() => {
        const dirs = new Set<string>();
        for (const f of openFiles) {
            const parts = f.relPath.split("/");
            for (let i = 0; i < parts.length - 1; i++) {
                dirs.add(parts.slice(0, i + 1).join("/"));
            }
        }
        return dirs;
    }, [openFiles]);

    // Reset state when agent or workspace changes
    useEffect(() => {
        setSelectedPath(null);
    }, [selectedAgentId, workspaceId]);

    // Fetch root when agent or workspace changes
    useEffect(() => {
        if (agentId) {
            fetchTree(agentId, workspaceId, "");
        }
    }, [agentId, workspaceId, fetchTree]);

    // ── Locate-in-tree: expand ancestors, lazy-load, select, scroll ───
    // The FileEditorPanel's "locate" button publishes a request via
    // workspaceStore.requestLocate. We expand all ancestor directories
    // synchronously, kick off `fetchTree` for any not-yet-cached ancestor,
    // then poll flatNodes until the target node appears so we can select
    // and center-scroll it.
    const locateRequest = useWorkspaceStore((s) => s.locateRequest);
    const consumedLocateSeqRef = useRef<number>(-1);

    // Step 1+2: expand ancestors and pre-fetch (only runs once per request).
    useEffect(() => {
        if (!locateRequest) return;
        if (locateRequest.agentId !== agentId) return;
        if (locateRequest.workspaceId !== workspaceId) return;
        if (locateRequest.sessionId !== sessionId) return;
        if (consumedLocateSeqRef.current === locateRequest.seq) return;
        consumedLocateSeqRef.current = locateRequest.seq;

        const { relPath } = locateRequest;

        // Expand ancestor dirs (synchronous Zustand update).
        expandTreeToPath(agentId, sessionId, relPath);

        // Lazy-fetch each ancestor dir that isn't already cached so the
        // target node eventually appears in flatNodes.
        const parts = relPath.split("/");
        const ancestors: string[] = [];
        for (let i = 0; i < parts.length - 1; i++) {
            ancestors.push(parts.slice(0, i + 1).join("/"));
        }
        for (const p of ancestors) {
            const key = `${treeCachePrefix}:${p}`;
            if (!treeCache[key]) {
                void fetchTree(agentId, workspaceId, p);
            }
        }
    }, [locateRequest, agentId, workspaceId, sessionId, treeCachePrefix, treeCache, expandTreeToPath, fetchTree]);

    // Flatten the tree into a list respecting expanded state
    const flatNodes = useMemo<FlatNode[]>(() => {
        const result: FlatNode[] = [];

        function walk(relPath: string, depth: number) {
            const cacheKey = `${treeCachePrefix}:${relPath}`;
            const entries = treeCache[cacheKey];
            if (!entries) return;

            for (const entry of entries) {
                const childRelPath = relPath ? `${relPath}/${entry.name}` : entry.name;

                result.push({ entry, depth, relPath: childRelPath });

                if (entry.type === "directory" && expandedPaths.has(childRelPath)) {
                    walk(childRelPath, depth + 1);
                }
            }
        }

        walk("", 0);
        return result;
    }, [treeCachePrefix, treeCache, expandedPaths]);

    const handleToggle = useCallback(
        (relPath: string) => {
            const isCurrentlyExpanded = expandedPaths.has(relPath);
            toggleTreeExpandedPath(agentId, sessionId, relPath);
            // Lazy-load children when expanding
            if (!isCurrentlyExpanded && !treeCache[`${treeCachePrefix}:${relPath}`]) {
                fetchTree(agentId, workspaceId, relPath);
            }
        },
        [agentId, workspaceId, sessionId, treeCachePrefix, expandedPaths, treeCache, fetchTree, toggleTreeExpandedPath],
    );

    const handleSelect = useCallback((_entry: TreeEntry, relPath: string) => {
        setSelectedPath(relPath);
    }, []);

    // Virtual scrolling setup — row height scales with global font size
    const scrollRef = useRef<HTMLDivElement | null>(null);
    const fontSize = useSettingsStore((s) => s.fontSize);
    const rowHeight = useMemo(() => Math.round(fontSize * 16 * 1.55), [fontSize]);
    const virtualizer = useVirtualizer({
        count: flatNodes.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => rowHeight,
        overscan: 20,
    });

    // Step 3 of locate-in-tree: select the matched node and center-scroll it.
    // Re-runs whenever flatNodes changes, so once the lazy-loaded children
    // arrive in the cache we'll center-scroll automatically.
    useEffect(() => {
        if (!locateRequest) return;
        if (consumedLocateSeqRef.current !== locateRequest.seq) return;
        const idx = flatNodes.findIndex((n) => n.relPath === locateRequest.relPath);
        if (idx < 0) return;
        setSelectedPath(locateRequest.relPath);
        // Defer one frame so the virtualizer has updated totalSize for the
        // newly-loaded flatNodes length before we ask it to scroll.
        const frame = requestAnimationFrame(() => {
            virtualizer.scrollToIndex(idx, { align: "center" });
        });
        return () => cancelAnimationFrame(frame);
    }, [flatNodes, locateRequest, virtualizer]);

    // Empty state
    if (flatNodes.length === 0) {
        const rootEntries = treeCache[`${treeCachePrefix}:`];
        if (!rootEntries) {
            return (
                <div className="flex items-center justify-center py-8 text-zinc-400" style={{ fontSize: "var(--ui-font-size, 0.875rem)" }}>
                    Loading...
                </div>
            );
        }
        if (rootEntries.length === 0) {
            return (
                <div className="flex flex-col items-center justify-center py-8 text-zinc-400" style={{ fontSize: "var(--ui-font-size, 0.875rem)" }}>
                    <span>Empty workspace</span>
                </div>
            );
        }
    }

    return (
        <div
            ref={scrollRef}
            className="flex-1 min-h-0 overflow-auto"
        >
            <div
                style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    width: "fit-content",
                    minWidth: "100%",
                    position: "relative",
                }}
            >
                {virtualizer.getVirtualItems().map((virtualRow) => {
                    const node = flatNodes[virtualRow.index];
                    const isLoading = treeLoadingPaths.has(`${treeCachePrefix}:${node.relPath}`);

                    return (
                        <div
                            key={node.relPath}
                            style={{
                                position: "absolute",
                                top: 0,
                                left: 0,
                                minWidth: "100%",
                                width: "fit-content",
                                height: `${virtualRow.size}px`,
                                transform: `translateY(${virtualRow.start}px)`,
                            }}
                        >
                            <FileTreeNode
                                entry={node.entry}
                                depth={node.depth}
                                agentId={agentId}
                                sessionId={sessionId}
                                relPath={node.relPath}
                                absPath={workspaceRoot ? `${workspaceRoot}/${node.relPath}` : node.relPath}
                                isExpanded={expandedPaths.has(node.relPath)}
                                isLoading={isLoading}
                                isSelected={selectedPath === node.relPath}
                                hasOpenDescendant={openFileDirSet.has(node.relPath)}
                                onToggle={handleToggle}
                                onSelect={handleSelect}
                                onDoubleClick={onFileDoubleClick}
                                onContextNewItem={onContextNewItem}
                                onDelete={onDelete}
                                onCopy={onCopy}
                                onPaste={onPaste}
                            />
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
