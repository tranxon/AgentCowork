import { create } from "zustand";
import { useSettingsStore } from "./settingsStore";
import { DEFAULT_GATEWAY_URL } from "../lib/config";

/** Extension → Monaco language ID mapping */
const EXT_LANGUAGE_MAP: Record<string, string> = {
    rs: "rust",
    ts: "typescript",
    tsx: "typescript",
    js: "javascript",
    jsx: "javascript",
    json: "json",
    toml: "ini",
    yaml: "yaml",
    yml: "yaml",
    md: "markdown",
    html: "html",
    htm: "html",
    css: "css",
    scss: "scss",
    less: "less",
    xml: "xml",
    sh: "shell",
    bash: "shell",
    zsh: "shell",
    ps1: "powershell",
    psm1: "powershell",
    psd1: "powershell",
    bat: "bat",
    cmd: "bat",
    py: "python",
    rb: "ruby",
    go: "go",
    java: "java",
    c: "c",
    h: "c",
    cpp: "cpp",
    cc: "cpp",
    cxx: "cpp",
    hpp: "cpp",
    cs: "csharp",
    swift: "swift",
    kt: "kotlin",
    kts: "kotlin",
    sql: "sql",
    graphql: "graphql",
    gql: "graphql",
    dockerfile: "dockerfile",
    ini: "ini",
    cfg: "ini",
    conf: "ini",
};

/** A file opened in the editor */
export interface OpenFile {
    /** Unique ID: `${agentId}:${workspaceId}:${relPath}` */
    id: string;
    agentId: string;
    workspaceId: string;
    relPath: string;
    fileName: string;
    /** Current content (may differ from originalContent when dirty) */
    content: string;
    /** Original content loaded from disk */
    originalContent: string;
    loading: boolean;
    saving: boolean;
    /** Monaco language ID (e.g. "typescript", "rust") */
    language: string;
    /** Whether the file has unsaved changes */
    dirty: boolean;
}

interface FileEditorState {
    openFiles: OpenFile[];
    activeFileId: string | null;

    /** Open a file (or activate if already open). Fetches content from Gateway. */
    openFile: (agentId: string, workspaceId: string, relPath: string) => Promise<void>;
    /** Open a file with pre-loaded content (skips Gateway fetch). Used by LSP cross-file navigation. */
    openFileWithContent: (agentId: string, workspaceId: string, relPath: string, content: string, language: string) => void;
    /** Close a file tab. Returns false if dirty (caller should confirm first). */
    closeFile: (fileId: string, force?: boolean) => boolean;
    /** Set the active (focused) file */
    setActiveFile: (fileId: string) => void;
    /** Update file content (marks as dirty) */
    updateContent: (fileId: string, content: string) => void;
    /** Save file content to Gateway */
    saveFile: (fileId: string) => Promise<void>;
    /** Close all open files */
    closeAllFiles: () => void;
}

function getGatewayUrl(): string {
    return useSettingsStore.getState().gatewayUrl || DEFAULT_GATEWAY_URL;
}

function detectLanguage(fileName: string): string {
    const ext = fileName.split(".").pop()?.toLowerCase() || "";
    // Handle special filenames without extension
    const baseName = fileName.toLowerCase();
    if (baseName === "dockerfile") return "dockerfile";
    if (baseName === "makefile") return "makefile";
    if (baseName === ".gitignore" || baseName === ".editorconfig") return "ini";
    return EXT_LANGUAGE_MAP[ext] || "plaintext";
}

export const useFileEditorStore = create<FileEditorState>((set, get) => ({
    openFiles: [],
    activeFileId: null,

    openFile: async (agentId: string, workspaceId: string, relPath: string) => {
        const fileId = `${agentId}:${workspaceId}:${relPath}`;
        const existing = get().openFiles.find((f) => f.id === fileId);
        if (existing) {
            // Already open, just activate
            set({ activeFileId: fileId });
            return;
        }

        const fileName = relPath.split("/").pop() || relPath;
        const language = detectLanguage(fileName);

        // Add placeholder file
        const newFile: OpenFile = {
            id: fileId,
            agentId,
            workspaceId,
            relPath,
            fileName,
            content: "",
            originalContent: "",
            loading: true,
            saving: false,
            language,
            dirty: false,
        };

        set((state) => ({
            openFiles: [...state.openFiles, newFile],
            activeFileId: fileId,
        }));

        // Fetch content from Gateway
        try {
            const baseUrl = getGatewayUrl();
            const params = new URLSearchParams();
            if (workspaceId && workspaceId !== "__agent_home__") {
                params.set("workspace_id", workspaceId);
            }
            params.set("path", relPath);
            const url = `${baseUrl}/api/agents/${agentId}/workspaces/file?${params.toString()}`;
            const resp = await fetch(url);
            if (!resp.ok) {
                console.error("[FileEditorStore] read_file failed:", resp.status);
                // Remove the file on error
                set((state) => ({
                    openFiles: state.openFiles.filter((f) => f.id !== fileId),
                    activeFileId: state.activeFileId === fileId ? null : state.activeFileId,
                }));
                return;
            }
            const data = (await resp.json()) as { content: string; size: number; mimeType: string };
            set((state) => ({
                openFiles: state.openFiles.map((f) =>
                    f.id === fileId
                        ? { ...f, content: data.content, originalContent: data.content, loading: false }
                        : f,
                ),
            }));
        } catch (e) {
            console.error("[FileEditorStore] openFile error:", e);
            set((state) => ({
                openFiles: state.openFiles.map((f) =>
                    f.id === fileId ? { ...f, loading: false } : f,
                ),
            }));
        }
    },

    openFileWithContent: (agentId: string, workspaceId: string, relPath: string, content: string, language: string) => {
        const fileId = `${agentId}:${workspaceId}:${relPath}`;
        const existing = get().openFiles.find((f) => f.id === fileId);
        if (existing) {
            // Already open, just activate
            set({ activeFileId: fileId });
            return;
        }

        const fileName = relPath.split("/").pop() || relPath;
        const newFile: OpenFile = {
            id: fileId,
            agentId,
            workspaceId,
            relPath,
            fileName,
            content,
            originalContent: content,
            loading: false, // Already have content, no need to fetch
            saving: false,
            language,
            dirty: false,
        };

        set((state) => ({
            openFiles: [...state.openFiles, newFile],
            activeFileId: fileId,
        }));
    },

    closeFile: (fileId: string, force?: boolean) => {
        const file = get().openFiles.find((f) => f.id === fileId);
        if (!file) return true;
        if (file.dirty && !force) return false;

        set((state) => {
            const nextFiles = state.openFiles.filter((f) => f.id !== fileId);
            let nextActive = state.activeFileId;
            if (state.activeFileId === fileId) {
                // Activate adjacent tab or null
                const idx = state.openFiles.findIndex((f) => f.id === fileId);
                nextActive = nextFiles.length > 0
                    ? nextFiles[Math.min(idx, nextFiles.length - 1)].id
                    : null;
            }
            return { openFiles: nextFiles, activeFileId: nextActive };
        });
        return true;
    },

    setActiveFile: (fileId: string) => {
        set({ activeFileId: fileId });
    },

    updateContent: (fileId: string, content: string) => {
        set((state) => ({
            openFiles: state.openFiles.map((f) =>
                f.id === fileId
                    ? { ...f, content, dirty: content !== f.originalContent }
                    : f,
            ),
        }));
    },

    saveFile: async (fileId: string) => {
        const file = get().openFiles.find((f) => f.id === fileId);
        if (!file || file.saving) return;

        set((state) => ({
            openFiles: state.openFiles.map((f) =>
                f.id === fileId ? { ...f, saving: true } : f,
            ),
        }));

        try {
            const baseUrl = getGatewayUrl();
            const params = new URLSearchParams();
            if (file.workspaceId && file.workspaceId !== "__agent_home__") {
                params.set("workspace_id", file.workspaceId);
            }
            params.set("path", file.relPath);
            const url = `${baseUrl}/api/agents/${file.agentId}/workspaces/file?${params.toString()}`;
            const resp = await fetch(url, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ content: file.content }),
            });
            if (!resp.ok) {
                console.error("[FileEditorStore] saveFile failed:", resp.status);
                return;
            }
            set((state) => ({
                openFiles: state.openFiles.map((f) =>
                    f.id === fileId
                        ? { ...f, saving: false, originalContent: f.content, dirty: false }
                        : f,
                ),
            }));
        } catch (e) {
            console.error("[FileEditorStore] saveFile error:", e);
            set((state) => ({
                openFiles: state.openFiles.map((f) =>
                    f.id === fileId ? { ...f, saving: false } : f,
                ),
            }));
        }
    },

    closeAllFiles: () => {
        set({ openFiles: [], activeFileId: null });
    },
}));
