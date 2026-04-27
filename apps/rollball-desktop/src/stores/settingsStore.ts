import { create } from "zustand";
import type { Theme } from "../lib/types";

interface SettingsStore {
  theme: Theme;
  fontSize: number;
  gatewayUrl: string;
  logLevel: string;
  setTheme: (theme: Theme) => void;
  setFontSize: (size: number) => void;
  setGatewayUrl: (url: string) => void;
  setLogLevel: (level: string) => void;
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  theme: "system",
  fontSize: 1.0,
  gatewayUrl: "http://127.0.0.1:19876",
  logLevel: "info",

  setTheme: (theme) => set({ theme }),
  setFontSize: (fontSize) => set({ fontSize }),
  setGatewayUrl: (gatewayUrl) => set({ gatewayUrl }),
  setLogLevel: (logLevel) => set({ logLevel }),
}));
