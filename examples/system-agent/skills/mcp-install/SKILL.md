---
name: mcp-install
description: Install or uninstall MCP servers via mcp_install / mcp_uninstall tools, including dependency resolution and connection verification
version: "1.0.0"
author: developer
triggers:
  - install mcp
  - 安装mcp
  - add mcp server
  - 添加mcp
  - remove mcp
  - 卸载mcp
  - uninstall mcp
tool_deps:
  - mcp_install
  - mcp_uninstall
  - web_search
  - shell
---

# MCP Install / Uninstall Skill

When the user asks to install or uninstall an MCP server, follow this workflow.

## Install Workflow

### Step 1: Research the MCP server configuration

- Use `web_search` to find the official MCP server configuration
- Determine the correct `transport`, `command`, `args`, `env`, and (if applicable) `url` parameters
- Most MCP servers publish their configuration in their README, docs, or registry pages

### Step 2: Verify dependencies

- **Python-based servers**: Check if `uvx` is available by running `uvx --version` or `pip show uvx`
  - `uvx` is the recommended launcher for Python MCP servers (auto-creates isolated environments)
  - If `uvx` is missing, suggest: `pip install uv` or `uv tool install <package>`
- **Node.js servers**: Check `npx --version` or `npm --version`
- **Binary servers**: Verify the binary path exists and is executable

If dependencies are missing, inform the user and assist with installation before proceeding.

### Step 3: Invoke `mcp_install`

Call `mcp_install` with the researched configuration:

```
mcp_install(name="xxx", transport="stdio", command="uvx", args=["xxx-mcp-server"])
```

- `name`: A unique identifier for this MCP server (e.g. `"docling"`, `"brave-search"`)
- `transport`: `"stdio"` (most common), `"sse"`, or `"http"`
- `command`: The executable to launch (e.g. `"uvx"`, `"npx"`, or an absolute path)
- `args`: Array of command-line arguments
- `env`: Optional object of environment variables (e.g. `{"API_KEY": "sk-xxx"}`)
- `url`: Required for SSE/HTTP transport

The tool will:
1. Validate parameters
2. Check for name conflicts with existing MCP servers
3. Write the config to agent_mcp.json
4. Run a scratch test (temporary connect → initialize → list tools)
5. **On success**: Keep the config. The MCP tools become available after the next config reload.
6. **On failure**: Roll back the config and report the error.

### Step 4: Report results to the user

- **On success**: "MCP server `xxx` installed and verified. Available tools (N): [tool1, tool2, ...]"
- **On failure**: "Installation failed: [error]. Suggestion: [diagnostic advice based on the error message]"

## Uninstall Workflow

### Step 1: Invoke `mcp_uninstall`

Call `mcp_uninstall` with the server name:

```
mcp_uninstall(name="xxx")
```

- Only local (agent-installed, via `mcp_install`) MCP servers can be uninstalled
- Catalog MCP servers (managed by Gateway) cannot be removed via this tool

### Step 2: Report results

- **On success**: "MCP server `xxx` uninstalled. Its tools will be removed after the next config reload."
- **On failure**: Report the error (e.g., not found, or is a catalog MCP)

## Common MCP Server Configuration Patterns

| Server type | transport | command | args pattern |
|------------|-----------|---------|-------------|
| Python (uvx) | stdio | uvx | [package-name] |
| Python (pip) | stdio | python | [-m, module-name] |
| Node.js | stdio | npx | [-y, @scope/package] |
| Go binary | stdio | /path/to/binary | [] |
| HTTP API | http | — | url: https://... |
| SSE | sse | — | url: https://... |

## Important Notes

- `mcp_install` performs a live scratch test — the MCP server must be functional at install time
- Environment variables for API keys should be provided in the `env` parameter, not hardcoded in args
- If the scratch test fails, check that the command is installed and accessible from the current PATH
- MCP tools become available **after** the config reload, not immediately during the same turn
- Use `web_search` to find the latest configuration — MCP server packages evolve frequently
