//! MCP preset registry — curated list of popular MCP servers
//!
//! These presets are frontend-only definitions that help users quickly
//! add common MCP servers to their catalog. Each preset defines the
//! server config template, required env vars (API keys), and install hints.

import type { McpPresetDef } from "./types";

export const MCP_PRESETS: McpPresetDef[] = [
  // ── File & System ──
  {
    id: "filesystem",
    name: "Filesystem",
    description: "Read, write, and search files on the local filesystem",
    category: "file",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/dir"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. Change /path/to/allowed/dir to your project directory.",
    icon: "FolderOpen",
  },
  {
    id: "desktop-commander",
    name: "Desktop Commander",
    description: "Execute system commands, manage processes, and file operations",
    category: "utility",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@anthropic/desktop-commander"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. Provides shell execution and process management.",
    icon: "Terminal",
  },

  // ── Search & Web ──
  {
    id: "fetch",
    name: "Fetch",
    description: "Fetch web content and make HTTP requests",
    category: "search",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-fetch"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. No API key required.",
    icon: "Globe",
  },
  {
    id: "brave-search",
    name: "Brave Search",
    description: "Web search via Brave Search API",
    category: "search",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-brave-search"],
    requiredEnv: ["BRAVE_API_KEY"],
    optionalEnv: {},
    installHint: "Get API key at https://brave.com/search/api/",
    icon: "Search",
  },
  {
    id: "context7",
    name: "Context7",
    description: "Fetch up-to-date library documentation and code examples",
    category: "search",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@upstash/context7-mcp@latest"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. No API key required.",
    icon: "BookOpen",
  },

  // ── Version Control ──
  {
    id: "github",
    name: "GitHub",
    description: "Manage PRs, issues, repos, and code search on GitHub",
    category: "vcs",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"],
    requiredEnv: ["GITHUB_PERSONAL_ACCESS_TOKEN"],
    optionalEnv: {},
    installHint: "Create token at https://github.com/settings/tokens",
    icon: "Github",
  },
  {
    id: "git",
    name: "Git",
    description: "Git operations — status, diff, log, commit, branch management",
    category: "vcs",
    transport: "stdio",
    command: "uvx",
    args: ["mcp-server-git", "--repository", "."],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "Requires uv/uvx (pip install uv). Change --repository to your project path.",
    icon: "GitBranch",
  },

  // ── Database ──
  {
    id: "postgres",
    name: "PostgreSQL",
    description: "Read-only SQL queries against PostgreSQL databases",
    category: "database",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-postgres", "postgresql://..."],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "Replace postgresql://... with your connection string. Read-only by default.",
    icon: "Database",
  },
  {
    id: "sqlite",
    name: "SQLite",
    description: "Read and query SQLite database files",
    category: "database",
    transport: "stdio",
    command: "uvx",
    args: ["mcp-server-sqlite", "--db-path", "/path/to/db.sqlite"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "Requires uv/uvx. Change --db-path to your database file.",
    icon: "Database",
  },

  // ── Knowledge & Reasoning ──
  {
    id: "memory",
    name: "Memory",
    description: "Persistent knowledge graph for storing and recalling information",
    category: "knowledge",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-memory"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. Stores memories in ~/.mcp-memory.json.",
    icon: "Brain",
  },
  {
    id: "sequential-thinking",
    name: "Sequential Thinking",
    description: "Dynamic and reflective problem-solving through structured thinking",
    category: "utility",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-sequential-thinking"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. No configuration needed.",
    icon: "Lightbulb",
  },

  // ── Browser ──
  {
    id: "playwright",
    name: "Playwright",
    description: "Browser automation — navigate, screenshot, extract content",
    category: "browser",
    transport: "stdio",
    command: "npx",
    args: ["-y", "@anthropic/mcp-playwright"],
    requiredEnv: [],
    optionalEnv: {},
    installHint: "npx auto-installs. Launches headless browser.",
    icon: "Monitor",
  },
];

/** Get presets grouped by category */
export function getPresetsByCategory(): Record<string, McpPresetDef[]> {
  const groups: Record<string, McpPresetDef[]> = {};
  for (const preset of MCP_PRESETS) {
    if (!groups[preset.category]) {
      groups[preset.category] = [];
    }
    groups[preset.category].push(preset);
  }
  return groups;
}

/** Find a preset by ID */
export function getPresetById(id: string): McpPresetDef | undefined {
  return MCP_PRESETS.find((p) => p.id === id);
}

/** Convert a preset to an McpServerConfigDef (for adding to catalog) */
export function presetToServerConfig(
  preset: McpPresetDef,
  envOverrides?: Record<string, string>
): import("./types").McpServerConfigDef {
  const env: Record<string, string> = { ...preset.optionalEnv, ...envOverrides };
  return {
    name: preset.id,
    transport: preset.transport,
    url: preset.url,
    command: preset.command ?? "",
    args: preset.args ?? [],
    env,
    headers: {},
    tool_timeout_secs: undefined,
  };
}
