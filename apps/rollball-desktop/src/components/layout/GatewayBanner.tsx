import { useGatewayStore } from "../../stores/gatewayStore";

export function GatewayBanner() {
  const checkHealth = useGatewayStore((s) => s.checkHealth);

  return (
    <div className="flex items-center gap-3 border-b border-amber-200 bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200">
      <span>Gateway not connected. Please start Gateway or check connection settings.</span>
      <button
        onClick={() => {
          // Navigate to settings — will be connected to router later
        }}
        className="rounded px-2 py-0.5 text-xs font-medium hover:bg-amber-100 dark:hover:bg-amber-900"
      >
        Settings
      </button>
      <button
        onClick={checkHealth}
        className="rounded px-2 py-0.5 text-xs font-medium hover:bg-amber-100 dark:hover:bg-amber-900"
      >
        Retry
      </button>
    </div>
  );
}
