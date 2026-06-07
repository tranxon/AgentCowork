import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useWorkspaceStore, type TreeEntry } from "../../../stores/workspaceStore";
import { useAgentStore } from "../../../stores/agentStore";
import { FileTreeNode } from "./FileTreeNode";

/** Flattened tree node for virtualized rendering */
interface FlatNode {
    entry: TreeEntry;
    depth: number;
    relPath: string;
}

interface FileTreeProps {
    agentId: string;
    workspaceId: string;
    onFileDoubleClick?: (entry: TreeEntry, relPath: string) => void;
}

export function FileTree({ agentId, workspaceId, onFileDoubleClick }: FileTreeProps) {
    const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set([""]));
    const [selectedPath, setSelectedPath] = useState<string | null>(null);
    const treeCache = useWorkspaceStore((s) => s.treeCache);
    const fetchTree = useWorkspaceStore((s) => s.fetchTree);
    const treeLoadingPaths = useWorkspaceStore((s) => s.treeLoadingPaths);
    const selectedAgentId = useAgentStore((s) => s.selectedAgentId);

    /** Build cache key prefix: agentId:workspaceId */
    const ck = `${agentId}:${workspaceId}`;

    // Reset state when agent or workspace changes
    useEffect(() => {
        setExpandedPaths(new Set([""]));
        setSelectedPath(null);
    }, [selectedAgentId, workspaceId]);

    // Fetch root when agent or workspace changes
    useEffect(() => {
        if (agentId) {
            fetchTree(agentId, workspaceId, "");
        }
    }, [agentId, workspaceId, fetchTree]);

    // Flatten the tree into a list respecting expanded state
    const flatNodes = useMemo<FlatNode[]>(() => {
        const result: FlatNode[] = [];

        function walk(relPath: string, depth: number) {
            const cacheKey = `${ck}:${relPath}`;
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
    }, [ck, treeCache, expandedPaths]);

    const handleToggle = useCallback(
        (relPath: string) => {
            setExpandedPaths((prev) => {
                const next = new Set(prev);
                if (next.has(relPath)) {
                    next.delete(relPath);
                } else {
                    next.add(relPath);
                    // Lazy-load children
                    if (!treeCache[`${ck}:${relPath}`]) {
                        fetchTree(agentId, workspaceId, relPath);
                    }
                }
                return next;
            });
        },
        [agentId, workspaceId, ck, treeCache, fetchTree],
    );

    const handleSelect = useCallback((_entry: TreeEntry, relPath: string) => {
        setSelectedPath(relPath);
    }, []);

    // Virtual scrolling setup
    const scrollRef = useRef<HTMLDivElement | null>(null);
    const virtualizer = useVirtualizer({
        count: flatNodes.length,
        getScrollElement: () => scrollRef.current,
        estimateSize: () => 22,
        overscan: 20,
    });

    // Empty state
    if (flatNodes.length === 0) {
        const rootEntries = treeCache[`${ck}:`];
        if (!rootEntries) {
            return (
                <div className="flex items-center justify-center py-8 text-xs text-zinc-400">
                    Loading...
                </div>
            );
        }
        if (rootEntries.length === 0) {
            return (
                <div className="flex flex-col items-center justify-center py-8 text-xs text-zinc-400">
                    <span>Empty workspace</span>
                </div>
            );
        }
    }

    return (
        <div
            ref={scrollRef}
            className="flex-1 overflow-y-auto"
        >
            <div
                style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    width: "100%",
                    position: "relative",
                }}
            >
                {virtualizer.getVirtualItems().map((virtualRow) => {
                    const node = flatNodes[virtualRow.index];
                    const isLoading = treeLoadingPaths.has(`${ck}:${node.relPath}`);

                    return (
                        <div
                            key={node.relPath}
                            style={{
                                position: "absolute",
                                top: 0,
                                left: 0,
                                width: "100%",
                                height: `${virtualRow.size}px`,
                                transform: `translateY(${virtualRow.start}px)`,
                            }}
                        >
                            <FileTreeNode
                                entry={node.entry}
                                depth={node.depth}
                                relPath={node.relPath}
                                isExpanded={expandedPaths.has(node.relPath)}
                                isLoading={isLoading}
                                isSelected={selectedPath === node.relPath}
                                onToggle={handleToggle}
                                onSelect={handleSelect}
                                onDoubleClick={onFileDoubleClick}
                            />
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
