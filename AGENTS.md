# AGENTS.md — AgentCowork.AI

## Overview

AgentCowork.AI is a decentralized, high-security, scalable AI Agent runtime platform. Each Agent is a declarative `.agent` package (config + prompts + skills, no binary), loaded by a universal Agent Runtime binary and managed by a lightweight Gateway.

## Build & Test

```bash
cd core && cargo build --release
cd core && cargo clippy --all-targets -- -D warnings
cd core && cargo test
./dev/ci.sh all
```

## Project Structure

```
├── core/                    # Rust workspace (library crates + integration tests)
│   ├── acowork-core/        # Shared types, errors, config
│   ├── acowork-embed/       # embedding model running on onnx runtime
│   ├── acowork-gateway/     # Gateway (lifecycle, IPC, HTTP API)
│   ├── acowork-grafeo/      # Memory engine (graph-based, layered)
│   ├── acowork-mcp/         # mcp protocol wrapper
│   ├── acowork-memory/      # Memory manager (trait, middleware)
│   ├── acowork-runtime/     # Agent runtime (main loop, tools, providers, sessions)
│   ├── acowork-sign/        # Package signing & verification
│   ├── acowork-vault/       # Encrypted key/value store
│   └── tests/               # Integration tests
├── apps/                    # Application layer (executables)
│   ├── cli/                 # Gateway CLI (planned)
│   └── desktop/             # Tauri v2 Desktop App
├── docs/                    # Architecture design docs (Chinese, v3.x)
│   ├── design/              # architecture design docs
│   ├── module-design/       # Detailed module specs (crate structure)
│   ├── plan/                # Planning docs
│   ├── review/              # Design & code review reports (numbered)
│   └── reference/           # Reference materials (ZeroClaw, Grafeo, memory research)
├── examples/                # Example .agent packages
├── ref-repo/                # Reference implementation ONLY (not source of truth)
└── dev/                     # Build/Package/CI/CD scripts
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
Gateway (keep alive process)
└── HTTP API — Desktop App / CLI (Axum, localhost only)
├── Package Manager — install/upgrade .agent packages
├── Lifecycle Manager — spawn/kill agent processes
├── Intent Router — cross-agent messaging
├── Global resources — secure API key storage, provider list, mcp list, embedding model list
├── Budget Tracker — usage accounting
├── Rate Limiter — request throttling
├── Socket API — Agent Runtime IPC (Unix Socket / Named Pipe)
        │
        │ Gateway Service API (Socket)
        ▼
Agent Runtime (universal )
├── System Agent (com.acowork.system) — identity, preferences
├── User Agents — each has private Grafeo + LLM direct connection
└── DevMode — Debug Protocol (WebSocket, for developers)
```

## Conventions

- Design docs in both Chinese and English; Rust code comments (`//`, `//!`, `///`) **MUST be in English**
- Rust implementation follows workspace pattern under `core/` (7 crates, structure defined in `docs/module-design/00-overview.md`)
- Code reviews follow `.opencode/style-guide.md`
- The Desktop App serves solely as a state presentation and interaction interface for the gateway/runtime backend. It neither hosts business logic nor persists any state.

## Rules (Do NOT)

- Do NOT edit `ref-repo/` — it is a separate reference project, not source of truth
- Do NOT commit in Chinese
- Do NOT act before the user confirms your plan.
- Do NOT prioritize simple and quick solutions over the correct and reasonable one; always make the correct and reasonable solution your first choice.

## Key Documentation

```
├── docs/
│   ├── AGENTS.md            # guide of design docs
```
