import { useState, useEffect, useCallback, createContext, useContext, type ReactNode } from "react";
import { cn } from "../../lib/utils";

type ToastType = "success" | "error" | "warning" | "info";

interface Toast {
  id: number;
  type: ToastType;
  message: string;
  action?: { label: string; onClick: () => void };
}

interface ToastContextValue {
  addToast: (toast: Omit<Toast, "id">) => void;
}

const ToastContext = createContext<ToastContextValue>({ addToast: () => {} });

export function useToast() {
  return useContext(ToastContext);
}

let nextId = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const addToast = useCallback((toast: Omit<Toast, "id">) => {
    const id = nextId++;
    setToasts((prev) => [...prev.slice(-2), { ...toast, id }]); // max 3
  }, []);

  const removeToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ addToast }}>
      {children}
      {/* Toast container */}
      <div className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2">
        {toasts.map((toast) => (
          <ToastItem key={toast.id} toast={toast} onDismiss={() => removeToast(toast.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  const autoDismissMs = toast.type === "error" ? 8000 : 5000;

  useEffect(() => {
    const timer = setTimeout(onDismiss, autoDismissMs);
    return () => clearTimeout(timer);
  }, [autoDismissMs, onDismiss]);

  const borderColor: Record<ToastType, string> = {
    success: "border-l-green-500",
    error: "border-l-red-500",
    warning: "border-l-yellow-500",
    info: "border-l-blue-500",
  };

  const iconMap: Record<ToastType, string> = {
    success: "✅",
    error: "❌",
    warning: "⚠️",
    info: "ℹ️",
  };

  return (
    <div
      className={cn(
        "flex w-80 items-start gap-2 rounded-md border border-zinc-200 border-l-4 bg-white p-3 shadow-lg dark:border-zinc-700 dark:bg-zinc-800",
        borderColor[toast.type],
      )}
      role="alert"
    >
      <span className="shrink-0 text-sm">{iconMap[toast.type]}</span>
      <p className="flex-1 text-sm text-zinc-700 dark:text-zinc-300">{toast.message}</p>
      {toast.action && (
        <button
          onClick={toast.action.onClick}
          className="shrink-0 text-xs font-medium text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          {toast.action.label}
        </button>
      )}
      <button
        onClick={onDismiss}
        className="shrink-0 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
        aria-label="Dismiss"
      >
        ✕
      </button>
    </div>
  );
}
