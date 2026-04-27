import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AgentInfo, AgentDetail } from "../lib/types";

interface AgentStore {
  agents: AgentInfo[];
  selectedAgentId: string | null;
  loading: boolean;
  error: string | null;

  fetchAgents: () => Promise<void>;
  selectAgent: (id: string | null) => void;
  installAgent: (packagePath: string) => Promise<void>;
  uninstallAgent: (agentId: string) => Promise<void>;
  startAgent: (agentId: string) => Promise<void>;
  stopAgent: (agentId: string) => Promise<void>;
  getAgentDetail: (agentId: string) => Promise<AgentDetail>;
}

export const useAgentStore = create<AgentStore>((set, get) => ({
  agents: [],
  selectedAgentId: null,
  loading: false,
  error: null,

  fetchAgents: async () => {
    set({ loading: true, error: null });
    try {
      const agents = await invoke<AgentInfo[]>("list_agents");
      set({ agents, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  selectAgent: (id) => {
    set({ selectedAgentId: id });
  },

  installAgent: async (packagePath) => {
    try {
      await invoke("install_agent", { packagePath });
      await get().fetchAgents();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  uninstallAgent: async (agentId) => {
    try {
      await invoke("uninstall_agent", { agentId });
      // If the uninstalled agent was selected, deselect it
      if (get().selectedAgentId === agentId) {
        set({ selectedAgentId: null });
      }
      await get().fetchAgents();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  startAgent: async (agentId) => {
    try {
      await invoke("start_agent", { agentId });
      await get().fetchAgents();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  stopAgent: async (agentId) => {
    try {
      await invoke("stop_agent", { agentId });
      await get().fetchAgents();
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  },

  getAgentDetail: async (agentId) => {
    return await invoke<AgentDetail>("get_agent_detail", { agentId });
  },
}));
