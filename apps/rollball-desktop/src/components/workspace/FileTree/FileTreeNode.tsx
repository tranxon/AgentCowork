import { memo, useCallback } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "../../../lib/utils";
import { getFileIcon } from "./fileIcons";
import type { TreeEntry } from "../../../stores/workspaceStore";

interface FileTreeNodeProps {
  entry: TreeEntry;
  depth: number;
  relPath: string;
  isExpanded: boolean;
  isLoading: boolean;
  isSelected: boolean;
  onToggle: (relPath: string) => void;
  onSelect: (entry: TreeEntry, relPath: string) => void;
  onDoubleClick?: (entry: TreeEntry, relPath: string) => void;
}

export const FileTreeNode = memo(function FileTreeNode({
  entry,
  depth,
  relPath,
  isExpanded,
  isLoading,
  isSelected,
  onToggle,
  onSelect,
  onDoubleClick,
}: FileTreeNodeProps) {
  const isDir = entry.type === "directory";
  const fileIcon = isDir ? null : getFileIcon(entry.name);
  const isDevicon = !isDir && fileIcon?.isDevicon;
  const iconColor = fileIcon?.color ?? "";

  const handleClick = useCallback(() => {
    if (isDir) {
      onToggle(relPath);
    } else {
      onSelect(entry, relPath);
    }
  }, [isDir, onToggle, onSelect, relPath, entry]);

  const handleDoubleClick = useCallback(() => {
    if (!isDir && onDoubleClick) {
      onDoubleClick(entry, relPath);
    }
  }, [isDir, onDoubleClick, entry, relPath]);

  return (
    <div
      className={cn(
        "flex cursor-pointer items-center gap-1 py-[2px] pr-3 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800",
        isSelected && "bg-blue-50 dark:bg-blue-900/20",
      )}
      style={{ paddingLeft: `${depth * 16 + 8}px` }}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      title={relPath}
    >
      {/* Icon — chevron for dirs, file-type for files; both occupy same 16px slot so names align */}
      <span className="flex h-4 w-4 shrink-0 items-center justify-center">
        {isDir ? (
          <ChevronRight
            className={cn(
              "h-3.5 w-3.5 text-zinc-400 transition-transform duration-150",
              isExpanded && "rotate-90",
            )}
          />
        ) : fileIcon ? (
          <fileIcon.icon
            className={cn(
              "h-3.5 w-3.5",
              !isDevicon && iconColor,
            )}
            style={!isDevicon && iconColor.startsWith("#") ? { color: iconColor } : undefined}
          />
        ) : null}
      </span>

      {/* Name — no truncation; horizontal scrollbar on parent handles overflow */}
      <span className="whitespace-nowrap text-zinc-700 dark:text-zinc-400">{entry.name}</span>

      {/* Loading indicator for directories being fetched */}
      {isLoading && isDir && isExpanded && (
        <span className="ml-auto text-[10px] text-zinc-400">...</span>
      )}

      {/* Children count badge for collapsed directories */}
      {isDir && !isExpanded && entry.childrenCount !== undefined && entry.childrenCount > 0 && (
        <span className="ml-auto text-[10px] text-zinc-400">{entry.childrenCount}</span>
      )}
    </div>
  );
});
