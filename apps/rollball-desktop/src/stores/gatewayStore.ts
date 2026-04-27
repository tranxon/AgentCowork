import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { HealthResponse, GatewayStatus } from "../lib/types";

interface GatewayStore {
  status: GatewayStatus;
  health: HealthResponse | null;
  checkHealth: () => Promise<void>;
}

export const useGatewayStore = create<GatewayStore>((set) => ({
  status: "disconnected",
  health: null,

  checkHealth: async () => {
    try {
      const health = await invoke<HealthResponse>("gateway_health");
      set({ status: "connected", health });
    } catch {
      set({ status: "error", health: null });
    }
  },
}));
