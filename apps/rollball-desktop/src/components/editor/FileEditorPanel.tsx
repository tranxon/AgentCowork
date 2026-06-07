import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "../../i18n/useTranslation";
import { useFileEditorStore, type OpenFile } from "../../stores/fileEditorStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useLspClient, type LspStatus } from "../../hooks/useLspClient";
import { cn } from "../../lib/utils";
import { X, Save, Loader2, FileText, CircleDot, Circle } from "lucide-react";
import Editor, { type OnMount } from "@monaco-editor/react";
import { ScrollableTabBar } from "../common/ScrollableTabBar";
import { TabItem } from "../common/tab";
import { registerLspProviders, sendDidOpenForModel } from "./lspProviders";
import type { IDisposable } from "monaco-editor";

// ── LSP Status Indicator ──────────────────────────────────────────────

function LspIndicator({ status, statusMessage, language }: { status: LspStatus; statusMessage: string; language: string }) {
    if (status === "disconnected") return null;

    if (status === "connecting") {
        return (
            <span className="flex items-center gap-1 text-[10px] text-zinc-400">
                <Circle className="h-2 w-2 animate-pulse" />
                <span>{language} connecting</span>
            </span>
        );
    }

    if (status === "indexing") {
        return (
            <span className="flex items-center gap-1 text-[10px] text-blue-500 dark:text-blue-400">
                <Circle className="h-2 w-2 animate-pulse" />
                <span>{statusMessage || `${language} analyzing`}</span>
            </span>
        );
    }

    if (status === "connected") {
        return (
            <span className="flex items-center gap-1 text-[10px] text-emerald-600 dark:text-emerald-400">
                <CircleDot className="h-2 w-2" />
                <span>{language}</span>
            </span>
        );
    }

    // error — show the actual error reason as tooltip
    const tooltip = statusMessage || "unknown error";
    return (
        <span className="flex items-center gap-1 text-[10px] text-amber-500" title={tooltip}>
            <Circle className="h-2 w-2" />
            <span>{language} unavailable</span>
        </span>
    );
}

export function FileEditorPanel({ width }: { width: number }) {
    const { t } = useTranslation();
    const openFiles = useFileEditorStore((s) => s.openFiles);
    const activeFileId = useFileEditorStore((s) => s.activeFileId);
    const setActiveFile = useFileEditorStore((s) => s.setActiveFile);
    const updateContent = useFileEditorStore((s) => s.updateContent);
    const saveFile = useFileEditorStore((s) => s.saveFile);
    const closeFile = useFileEditorStore((s) => s.closeFile);

    const theme = useSettingsStore((s) => s.theme);
    const [closingFileId, setClosingFileId] = useState<string | null>(null);
    const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
    const monacoRef = useRef<typeof import("monaco-editor") | null>(null);
    const [cursor, setCursor] = useState({ line: 1, column: 1 });
    const [selectedCount, setSelectedCount] = useState(0);
    const lspProvidersRef = useRef<IDisposable | null>(null);
    // When Monaco's peek widget navigates to a different file, we store the
    // target position here and apply it after the editor is remounted with
    // the new file (because key={activeFile.id} causes editor recreation).
    const pendingNavigationRef = useRef<{ line: number; column: number } | null>(null);
    // Guard to prevent overriding ICodeEditorService.openCodeEditor more than once
    // (it's a shared singleton service, not per-editor).
    const codeEditorOverriddenRef = useRef(false);

    const activeFile = openFiles.find((f) => f.id === activeFileId) ?? null;

    // Resolve workspace root for LSP URI mapping.
    // Monaco's Uri.parse() cannot handle Windows file URIs (file:///C:/...),
    // so we use relPath as the model path (producing file:///core/... which
    // Monaco accepts). The LSP layer then maps relative URIs to absolute ones
    // using the workspace root. See lspProviders.ts → toLspUri().
    const treeRoots = useWorkspaceStore((s) => s.treeRoots);
    const workspaceRoot = useMemo(() => {
        if (!activeFile) return undefined;
        const rootKey = `${activeFile.agentId}:${activeFile.workspaceId}`;
        return treeRoots[rootKey];
    }, [activeFile, treeRoots]);

    // Determine the active language for LSP — use the active file's language
    const lspLanguage = activeFile?.language ?? null;

    // Determine if LSP should be enabled: there must be at least one open file
    // of the active language (not loading)
    const lspEnabled = lspLanguage != null && openFiles.some(
        (f) => f.language === lspLanguage && !f.loading
    );

    // LSP client
    console.log("[LSP] FileEditorPanel — lspLanguage:", lspLanguage, "agentId:", activeFile?.agentId, "workspaceId:", activeFile?.workspaceId, "lspEnabled:", lspEnabled);
    const { status: lspStatus, statusMessage: lspStatusMessage, client: lspClient } = useLspClient(
        lspLanguage,
        activeFile?.agentId,
        activeFile?.workspaceId,
        lspEnabled,
        workspaceRoot
    );

    // Determine Monaco theme based on app theme
    const monacoTheme = useMemo(() => {
        if (theme === "dark") return "vs-dark";
        if (theme === "light") return "vs";
        // system: check DOM
        return document.documentElement.classList.contains("dark") ? "vs-dark" : "vs";
    }, [theme]);

    // System theme change listener
    const [systemDark, setSystemDark] = useState(() =>
        document.documentElement.classList.contains("dark")
    );
    useEffect(() => {
        if (theme !== "system") return;
        const mq = window.matchMedia("(prefers-color-scheme: dark)");
        const handler = () => setSystemDark(mq.matches);
        mq.addEventListener("change", handler);
        return () => mq.removeEventListener("change", handler);
    }, [theme]);

    const resolvedMonacoTheme = theme === "system"
        ? (systemDark ? "vs-dark" : "vs")
        : monacoTheme;

    const handleEditorMount: OnMount = useCallback((editor, monaco) => {
        editorRef.current = editor;
        monacoRef.current = monaco;
        // Track cursor position + selection
        editor.onDidChangeCursorPosition((e) => {
            setCursor({ line: e.position.lineNumber, column: e.position.column });
            // Sync selection count
            const sel = editor.getSelection();
            if (sel && !sel.isEmpty()) {
                const model = editor.getModel();
                if (model) {
                    setSelectedCount(model.getValueInRange(sel).length);
                    return;
                }
            }
            setSelectedCount(0);
        });

        // Handle cross-file navigation from LSP peek widget (F12 / Shift+F12).
        // When the user clicks a reference/definition in a different file, Monaco
        // sets the editor model to the target file's model. We need to update
        // React state so the tab bar and editor stay in sync.
        editor.onDidChangeModel(() => {
            const newModel = editor.getModel();
            if (!newModel) {
                console.log("[LSP] onDidChangeModel — model is null");
                return;
            }

            // The model's URI path is the relative path (e.g. "core/runtime/src/foo.rs")
            const relPath = newModel.uri.path.replace(/^\/+/, "");
            console.log("[LSP] onDidChangeModel — new relPath:", relPath, "uri:", newModel.uri.toString());
            const store = useFileEditorStore.getState();
            const activeFile = store.openFiles.find((f) => f.id === store.activeFileId);

            // Skip if the model change is due to React switching files (same relPath)
            if (activeFile && activeFile.relPath === relPath) {
                console.log("[LSP] onDidChangeModel — same file as active, skipping");
                return;
            }

            // Check if the file is already open
            const existingFile = store.openFiles.find((f) => f.relPath === relPath);
            if (existingFile) {
                // Just activate the existing tab
                console.log("[LSP] onDidChangeModel — activating existing tab:", existingFile.id);
                store.setActiveFile(existingFile.id);
                return;
            }

            // The file isn't open — it must be a model created by ensureModelsForUris
            // for LSP cross-file reference preview. Open it via the store which will
            // re-use the existing model content (already fetched).
            // Find agentId/workspaceId from the current active file
            if (activeFile) {
                console.log("[LSP] onDidChangeModel — cross-file navigation, opening:", relPath);
                void store.openFile(activeFile.agentId, activeFile.workspaceId, relPath);
            }
        });

        // Ctrl+S / Cmd+S to save
        editor.addCommand(
            // eslint-disable-next-line no-bitwise
            2048 | 49, // KeyMod.CtrlCmd | KeyCode.KeyS
            () => {
                const currentId = useFileEditorStore.getState().activeFileId;
                if (currentId) void saveFile(currentId);
            },
        );

        // ── Override ICodeEditorService.openCodeEditor ───────────────
        // In Monaco standalone, the default ICodeEditorService.openCodeEditor()
        // can only navigate within the same file. For cross-file navigation
        // (from LSP peek widgets like definition/references), it returns null.
        // We override it to detect cross-file navigation and switch the
        // active file in the store, which causes the editor to remount via
        // key={activeFile.id} with the target file loaded.
        if (!codeEditorOverriddenRef.current) {
            // Diagnostic: inspect what internal services are available
            const editorAny = editor as any;
            const svcKeys = Object.keys(editorAny).filter(k => k.toLowerCase().includes("service") || k.toLowerCase().includes("codeeditor"));
            // Use console.warn so it stands out in the console
            console.warn("[LSP] ═══ Editor internal service keys:", svcKeys);
            console.warn("[LSP] ═══ _codeEditorService:", !!editorAny._codeEditorService,
                "openCodeEditor:", !!editorAny._codeEditorService?.openCodeEditor);
            console.warn("[LSP] ═══ _instantiationService:", !!editorAny._instantiationService);

            let codeEditorSvc = editorAny._codeEditorService;

            // Fallback: try to get ICodeEditorService via _instantiationService
            if (!codeEditorSvc && editorAny._instantiationService) {
                try {
                    const instSvc = editorAny._instantiationService;
                    // Try common service access patterns
                    if (typeof instSvc.invokeFunction === "function") {
                        codeEditorSvc = instSvc.invokeFunction((accessor: any) => {
                            // Try known service IDs
                            for (const id of ["codeEditorService", "ICodeEditorService", "codeEditor"]) {
                                try { return accessor.get(id); } catch { /* skip */ }
                            }
                            return null;
                        });
                        console.log("[LSP] _instantiationService lookup result:", !!codeEditorSvc);
                    }
                } catch (e) {
                    console.warn("[LSP] _instantiationService lookup failed:", e);
                }
            }

            if (codeEditorSvc?.openCodeEditor) {
                const originalOpenCodeEditor = codeEditorSvc.openCodeEditor.bind(codeEditorSvc);
                codeEditorSvc.openCodeEditor = async (
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    input: any,
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    source: any
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                ): Promise<any> => {
                    console.log("[LSP] openCodeEditor — input.resource:", input?.resource?.toString(),
                        "selection:", JSON.stringify(input?.options?.selection));

                    // Try default behavior first (same-file navigation)
                    const result = await originalOpenCodeEditor(input, source);
                    if (result) {
                        console.log("[LSP] openCodeEditor — default handled it (same file)");
                        return result;
                    }

                    // Cross-file navigation: the default service couldn't handle it
                    const targetUri = input?.resource;
                    const selection = input?.options?.selection;
                    if (!targetUri) {
                        console.warn("[LSP] openCodeEditor — no target URI, giving up");
                        return null;
                    }

                    // Extract relPath from model URI (e.g. file:///core/.../foo.rs → core/.../foo.rs)
                    const relPath = targetUri.path.replace(/^\/+/, "");
                    console.log("[LSP] openCodeEditor — cross-file navigation to:", relPath);

                    // Store target position for applying after editor remount
                    if (selection) {
                        pendingNavigationRef.current = {
                            line: selection.startLineNumber,
                            column: selection.startColumn,
                        };
                    }

                    // Switch to the target file
                    const store = useFileEditorStore.getState();
                    const existingFile = store.openFiles.find((f) => f.relPath === relPath);

                    if (existingFile) {
                        console.log("[LSP] openCodeEditor — activating existing tab:", existingFile.id);
                        store.setActiveFile(existingFile.id);
                    } else {
                        const currentActive = store.openFiles.find((f) => f.id === store.activeFileId);
                        if (currentActive) {
                            // Check if a Monaco model already exists for this file
                            // (created by ensureModelsForUris). If so, reuse its
                            // content to avoid a second fetch and ensure the line
                            // numbers match the reference locations.
                            const monacoInst = monacoRef.current;
                            const targetMonacoUri = monacoInst?.Uri.parse(relPath);
                            const existingModel = targetMonacoUri
                                ? monacoInst!.editor.getModel(targetMonacoUri)
                                : null;

                            if (existingModel && monacoInst) {
                                const content = existingModel.getValue();
                                const lang = existingModel.getLanguageId();
                                console.log("[LSP] openCodeEditor — reusing model content, lines:", content.split("\n").length);
                                store.openFileWithContent(
                                    currentActive.agentId, currentActive.workspaceId,
                                    relPath, content, lang
                                );
                            } else {
                                console.log("[LSP] openCodeEditor — opening new file (fetch):", relPath);
                                void store.openFile(currentActive.agentId, currentActive.workspaceId, relPath);
                            }
                        }
                    }

                    return null; // We handled navigation via React state
                };
                codeEditorOverriddenRef.current = true;
                console.warn("[LSP] ═══ ICodeEditorService.openCodeEditor OVERRIDDEN — cross-file navigation enabled");
            } else {
                console.warn("[LSP] ═══ Could not access _codeEditorService — cross-file navigation won't work");
            }
        }

        // ── Apply pending navigation ──────────────────────────────────
        // If a cross-file navigation was queued before this editor mount,
        // apply the target position after the model is fully loaded.
        // NOTE: We do NOT apply it here because at this point the editor
        // might still be loading the file content. Instead, we apply it
        // in a separate useEffect that watches activeFile.loading.
    }, [saveFile]);

    // ── Apply pending cross-file navigation ──────────────────────────────
    // When a cross-file navigation was queued by openCodeEditor, apply the
    // target position once the file is loaded and the editor is ready.
    useEffect(() => {
        if (!pendingNavigationRef.current) return;
        if (!activeFile || activeFile.loading) return;
        if (!editorRef.current) return;

        const { line, column } = pendingNavigationRef.current;
        pendingNavigationRef.current = null;
        console.log("[LSP] Applying pending navigation — line:", line, "column:", column,
            "file:", activeFile.relPath, "loading:", activeFile.loading);

        // Double-check the model's line count to avoid setting position
        // beyond the file length (content mismatch between ensureModelsForUris
        // and openFile can cause this).
        const model = editorRef.current.getModel();
        console.log("[LSP] Applying pending navigation — model URI:", model?.uri?.toString(),
            "lineCount:", model?.getLineCount(), "target line:", line, "target column:", column);
        if (model && line > model.getLineCount()) {
            console.warn("[LSP] Pending navigation line", line, "exceeds model line count",
                model.getLineCount(), "— clamping to last line");
            editorRef.current.revealLineInCenter(model.getLineCount());
            editorRef.current.setPosition({ lineNumber: model.getLineCount(), column: 1 });
        } else {
            editorRef.current.revealLineInCenter(line);
            editorRef.current.setPosition({ lineNumber: line, column });
        }
    }, [activeFile]);

    // ── Send textDocument/didOpen for newly mounted models ────────────
    // When the editor mounts a new file (tab switch or cross-file navigation),
    // @monaco-editor/react may create a new model. We must notify the LSP
    // server about it so that hover/definition/references work for this file.
    useEffect(() => {
        if (!lspClient || !workspaceRoot || !activeFile || activeFile.loading) return;
        if (!monacoRef.current) return;

        const relPath = activeFile.relPath;
        const monacoUri = monacoRef.current.Uri.parse(relPath);
        const model = monacoRef.current.editor.getModel(monacoUri);
        if (model) {
            sendDidOpenForModel(lspClient, model, workspaceRoot);
        }
    }, [activeFile, lspClient, workspaceRoot]);

    // ── LSP providers registration ──────────────────────────────────────

    useEffect(() => {
        // Unregister previous providers
        if (lspProvidersRef.current) {
            lspProvidersRef.current.dispose();
            lspProvidersRef.current = null;
        }

        // Register providers when both monaco and LSP client are ready
        if (monacoRef.current && lspClient && lspLanguage && workspaceRoot && activeFile) {
            try {
                console.log("[LSP] Registering providers for:", lspLanguage, "client:", !!lspClient);
                lspProvidersRef.current = registerLspProviders(monacoRef.current, {
                    client: lspClient,
                    language: lspLanguage,
                    workspaceRoot,
                    agentId: activeFile.agentId,
                    workspaceId: activeFile.workspaceId,
                });
            } catch (err) {
                console.warn("[LSP] Failed to register providers:", err);
            }
        } else {
            console.log("[LSP] Skipping provider registration — monaco:", !!monacoRef.current, "client:", !!lspClient, "language:", lspLanguage);
        }

        return () => {
            if (lspProvidersRef.current) {
                lspProvidersRef.current.dispose();
                lspProvidersRef.current = null;
            }
        };
    }, [lspClient, lspLanguage]);

    const handleEditorChange = useCallback((value: string | undefined) => {
        if (value === undefined) return;
        const currentId = useFileEditorStore.getState().activeFileId;
        if (currentId) updateContent(currentId, value);
    }, [updateContent]);

    const handleClose = useCallback((e: React.MouseEvent, file: OpenFile) => {
        e.stopPropagation();
        if (file.dirty) {
            setClosingFileId(file.id);
            return;
        }
        closeFile(file.id);
    }, [closeFile]);

    const confirmClose = useCallback(() => {
        if (!closingFileId) return;
        closeFile(closingFileId, true);
        setClosingFileId(null);
    }, [closingFileId, closeFile]);

    return (
        <div
            className="flex flex-col border-l border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900"
            style={{ width }}
        >
            {/* Tab bar */}
            <div className="flex items-center bg-[#FAFAFA] dark:bg-zinc-900 select-none px-0.5 gap-0.5 mt-[5px] border-b border-zinc-200 dark:border-zinc-800">
                <ScrollableTabBar
                    activeItemSelector={activeFileId ? `[data-file-id="${activeFileId}"]` : undefined}
                    activeItemId={activeFileId ?? undefined}
                >
                    {openFiles.map((file) => {
                        const isActive = file.id === activeFileId;
                        return (
                            <TabItem
                                key={file.id}
                                data-file-id={file.id}
                                onClick={() => setActiveFile(file.id)}
                                active={isActive}
                                title={file.relPath}
                            >
                                {/* Dirty indicator / loading */}
                                {file.loading ? (
                                    <Loader2 className="h-3 w-3 shrink-0 animate-spin text-zinc-400" />
                                ) : file.dirty ? (
                                    <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--color-accent)]" />
                                ) : null}
                                {/* File name */}
                                <span className="min-w-0 flex-1 truncate text-[length:var(--tab-font-size)] leading-[var(--tab-line-height)]">
                                    {file.fileName}
                                </span>
                                {/* Close button */}
                                <button
                                    onClick={(e) => handleClose(e, file)}
                                    className={cn(
                                        "shrink-0 rounded p-0.5 transition-opacity",
                                        isActive
                                            ? "opacity-60 hover:opacity-100 hover:bg-zinc-200 dark:hover:bg-zinc-600"
                                            : "opacity-0 group-hover:opacity-60 hover:!opacity-100 hover:bg-zinc-300 dark:hover:bg-zinc-600",
                                    )}
                                    title="Close"
                                >
                                    <X className="h-3 w-3" />
                                </button>
                            </TabItem>
                        );
                    })}
                </ScrollableTabBar>

                {/* Save button */}
                {activeFile && !activeFile.loading && (
                    <button
                        onClick={() => activeFile.dirty && void saveFile(activeFile.id)}
                        disabled={!activeFile.dirty || activeFile.saving}
                        className={cn(
                            "flex items-center justify-center rounded p-1 transition-colors shrink-0",
                            activeFile.dirty
                                ? "text-[var(--color-accent)] hover:bg-zinc-200 dark:hover:bg-zinc-700"
                                : "text-zinc-300 dark:text-zinc-600 cursor-default",
                        )}
                        title="Save (Ctrl+S)"
                    >
                        {activeFile.saving ? (
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                        ) : (
                            <Save className="h-3.5 w-3.5" />
                        )}
                    </button>
                )}
            </div>

            {/* Editor area */}
            <div className="flex-1 overflow-hidden">
                {!activeFile ? (
                    <div className="flex h-full items-center justify-center text-xs text-zinc-400 dark:text-zinc-500">
                        {t("fileEditor.emptyState")}
                    </div>
                ) : activeFile.loading ? (
                    <div className="flex h-full items-center justify-center gap-2 text-xs text-zinc-400">
                        <Loader2 className="h-4 w-4 animate-spin" />
                        Loading...
                    </div>
                ) : (
                    <Editor
                        key={activeFile.id}
                        path={activeFile.relPath}
                        value={activeFile.content}
                        language={activeFile.language}
                        theme={resolvedMonacoTheme}
                        onChange={handleEditorChange}
                        onMount={handleEditorMount}
                        options={{
                            minimap: { enabled: false },
                            fontSize: 13,
                            lineNumbers: "on",
                            scrollBeyondLastLine: false,
                            wordWrap: "on",
                            tabSize: 2,
                            renderWhitespace: "selection",
                            padding: { top: 8 },
                            automaticLayout: true,
                            readOnly: false,
                        }}
                    />
                )}
            </div>

            {/* Status bar */}
            {activeFile && !activeFile.loading && (
                <div className="flex items-center justify-between border-t border-zinc-200 bg-zinc-100 px-3 h-5 text-[11px] text-zinc-500 select-none dark:border-zinc-800 dark:bg-zinc-800 dark:text-zinc-400">
                    <span className="uppercase">{activeFile.language || "plain text"}</span>
                    {lspEnabled && lspLanguage && (
                        <LspIndicator status={lspStatus} statusMessage={lspStatusMessage} language={lspLanguage} />
                    )}
                    <span>Ln {cursor.line}, Col {cursor.column}{selectedCount > 0 ? ` (${selectedCount} selected)` : ""}</span>
                </div>
            )}

            {/* Close confirmation dialog */}
            {closingFileId && (
                <div
                    className="fixed inset-0 z-[60] flex items-center justify-center bg-black/50"
                    onClick={() => setClosingFileId(null)}
                >
                    <div
                        className="mx-4 w-full max-w-sm rounded-xl border border-zinc-200 bg-white p-5 shadow-xl dark:border-zinc-700 dark:bg-zinc-800"
                        onClick={(e) => e.stopPropagation()}
                    >
                        <div className="flex items-start gap-3">
                            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-100 dark:bg-amber-900/30">
                                <FileText className="h-5 w-5 text-amber-600 dark:text-amber-400" />
                            </div>
                            <div className="flex-1">
                                <h3 className="text-sm font-medium text-zinc-800 dark:text-zinc-200">
                                    {t("fileEditor.unsavedChanges")}
                                </h3>
                                <p className="mt-1 text-xs text-zinc-500 dark:text-zinc-400">
                                    {t("fileEditor.saveChanges")}
                                </p>
                            </div>
                        </div>
                        <div className="mt-4 flex justify-end gap-2">
                            <button
                                onClick={() => setClosingFileId(null)}
                                className="rounded-lg btn-solid px-3 py-1.5 text-xs"
                            >
                                {t("fileEditor.cancel")}
                            </button>
                            <button
                                onClick={confirmClose}
                                className="rounded-lg btn-accent px-3 py-1.5 text-xs"
                            >
                                {t("fileEditor.discard")}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
