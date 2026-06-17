# AGENTS.md — AgentCowork.AI

## Overview

AgentCowork.AI is a decentralized, high-security, scalable AI Agent runtime platform modeled after Android. Each Agent is a declarative `.agent` package (config + prompts + skills, no binary), loaded by a universal Agent Runtime binary and managed by a lightweight Gateway.

## Build & Test

```bash
cd core && cargo build --release
cd core && cargo clippy --all-targets -- -D warnings
cd core && cargo test
./dev/ci.sh all
```

## Project Structure

```
agent-study/
├── core/                    # Rust workspace (library crates + integration tests)
│   ├── acowork-core/       # Shared types, errors, config
│   ├── acowork-runtime/    # Agent runtime (main loop, tools, providers)
│   ├── acowork-gateway/    # Gateway (lifecycle, IPC, HTTP API)
│   ├── acowork-grafeo/     # Memory engine (graph-based, layered)
│   ├── acowork-memory/     # Memory manager (trait, middleware)
│   ├── acowork-vault/      # Encrypted key/value store
│   ├── acowork-sign/       # Package signing & verification
│   └── tests/               # Integration tests
├── apps/                    # Application layer (executables)
│   ├── cli/                 # Gateway CLI (planned)
│   └── desktop/             # Tauri v2 Desktop App (planned)
├── docs/                    # Architecture design docs (Chinese, v3.x)
│   ├── module-design/       # Detailed module specs (crate structure)
│   ├── plan/                # Planning docs
│   ├── review/              # Design & code review reports (numbered)
│   └── reference/           # Reference materials (ZeroClaw, Grafeo, memory research)
├── examples/                # Example .agent packages
├── ref-repo/                # Reference implementation ONLY (not source of truth)
└── dev/                     # CI/CD scripts
```

## Architecture

```
Desktop App (Tauri v2, 独立进程)
├── Chat / Agent List / Settings UI
├── Debug Panel (DevMode)
└── System Tray
        │
        │ Gateway HTTP API (http://127.0.0.1:19876)
        ▼
Gateway (常驻独立进程)
├── Package Manager — install/upgrade .agent packages
├── Lifecycle Manager — spawn/kill agent processes
├── Intent Router — cross-agent messaging
├── Key Vault — secure API key storage
├── Budget Tracker — usage accounting
├── Rate Limiter — request throttling
├── Socket API — Agent Runtime IPC (Unix Socket / Named Pipe)
└── HTTP API — Desktop App / CLI (Axum, localhost only)
        │
        │ Gateway Service API (Socket)
        ▼
Agent Runtime (统一二进制)
├── System Agent (com.acowork.system) — identity, preferences
├── User Agents — each has private Grafeo + LLM direct connection
└── DevMode — Debug Protocol (WebSocket, 开发者调试)
```

## Conventions

- Design docs in both Chinese and English; Rust code comments (`//`, `//!`, `///`) **MUST be in English**
- `.agent` bundles are **declarative only** — no executable code
- Rust implementation follows workspace pattern under `core/` (7 crates, structure defined in `docs/module-design/00-overview.md`)
- Code reviews follow `.opencode/style-guide.md`

## Rules (Do NOT)

- Do NOT edit `ref-repo/` — it is a separate reference project, not source of truth
- Do NOT implement executable code in `.agent` packages
- Do NOT commit in Chinese

## Key Documentation

| Topic | Document |
|-------|----------|
| Platform overview | `docs/01-overview.md` |
| Package format | `docs/02-agent-package.md` |
| Runtime internals | `docs/03-agent-runtime.md` |
| Gateway design | `docs/04-gateway.md` · `docs/module-design/03-gateway.md` |
| Memory architecture | `docs/05-memory.md` · `docs/module-design/04-grafeo.md` |
| Communication protocol | `docs/06-communication.md` |
| Security design | `docs/08-security.md` · `docs/module-design/05-vault-sign.md` |
| Implementation roadmap | `docs/09-roadmap-and-scenarios.md` |
| Debug protocol | `docs/10-debug-protocol.md` |
| Tool system | `docs/12-tool-system.md` |
| Skill system | `docs/13-skill-system.md` |
| Desktop app | `docs/14-desktop-app.md` |
| Conversation persistence | `docs/15-conversation-persistence.md` |
| Design & code reviews | `docs/review/` |
