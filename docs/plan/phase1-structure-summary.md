# Phase 1 代码结构总结

> 创建日期：2026-04-17
> 状态：✅ 完成 - 所有数据结构和函数声明已创建

---

## 已创建的 Crate 结构

### 1. rollball-core（共享类型与协议）
**位置**: `crates/rollball-core/`

**核心模块**:
- ✅ `manifest.rs` - AgentManifest 及所有子类型（LlmConfig, ToolDeclaration 等）
- ✅ `protocol.rs` - GatewayRequest/Response, Frame 传输帧
- ✅ `intent.rs` - Intent 消息结构
- ✅ `permission.rs` - Permission 枚举及匹配逻辑
- ✅ `identity.rs` - Identity 结构
- ✅ `budget.rs` - Budget 和 UsageReport 类型
- ✅ `error.rs` - RollballError 统一错误类型
- ✅ `tools/traits.rs` - Tool trait, ToolSpec, ToolResult
- ✅ `tools/schema.rs` - JSON Schema 清洗（TODO）
- ✅ `providers/traits.rs` - Provider trait, ChatMessage, ChatRequest/Response, StreamEvent
- ✅ `memory/traits.rs` - MemoryStore trait, MemoryNode, PrivacyLevel

**依赖**: serde, serde_json, async-trait, thiserror, chrono, uuid

---

### 2. rollball-sign（签名工具链）
**位置**: `crates/rollball-sign/`

**核心模块**:
- ✅ `signing_block.rs` - SigningBlock, Signer, Certificate 结构
- ✅ `keygen.rs` - Ed25519 密钥对生成（TODO）
- ✅ `sign.rs` - 包签名逻辑（TODO）
- ✅ `verify.rs` - 包验签逻辑（TODO）
- ✅ `certificate.rs` - X.509 证书处理（TODO）
- ✅ `error.rs` - SignError 错误类型
- ✅ `bin/keygen.rs` - rollball-keygen CLI
- ✅ `bin/sign.rs` - rollball-sign CLI
- ✅ `bin/verify.rs` - rollball-verify CLI

**依赖**: rollball-core, ed25519-dalek, x509-cert, sha2, zip, clap

---

### 3. rollball-vault（密钥加密存储）
**位置**: `crates/rollball-vault/`

**核心模块**:
- ✅ `vault.rs` - Vault 主结构（open/unlock/store/retrieve/list）
- ✅ `encryption.rs` - ChaCha20-Poly1305 加解密（TODO）
- ✅ `key_derivation.rs` - Argon2id 密钥派生（TODO）
- ✅ `error.rs` - VaultError 错误类型

**依赖**: rollball-core, chacha20poly1305, rand, secrecy, sha2

---

### 4. rollball-memory（MemoryStore 抽象层）
**位置**: `crates/rollball-memory/`

**核心模块**:
- ✅ `store.rs` - MemoryStore trait re-export
- ✅ `types.rs` - MemoryZone 枚举（Working, Episodic, Semantic 等）

**依赖**: rollball-core, async-trait

---

### 5. rollball-grafeo（图数据库引擎 - Phase 2）
**位置**: `crates/rollball-grafeo/`

**核心模块**（骨架）:
- ✅ `grafeo.rs` - Grafeo 主结构
- ✅ `graph.rs` - 图数据结构（TODO Phase 2）
- ✅ `decay.rs` - 遗忘机制（TODO Phase 2）
- ✅ `retrieval.rs` - 关联扩散检索（TODO Phase 2）
- ✅ `error.rs` - GrafeoError

**依赖**: rollball-core, rollball-memory, rusqlite, tokio

---

### 6. rollball-runtime（Agent Runtime）
**位置**: `crates/rollball-runtime/`

**核心模块**:

#### Agent 主循环
- ✅ `agent/loop_.rs` - 9步主循环框架
- ✅ `agent/context.rs` - 上下文构建
- ✅ `agent/history.rs` - HistoryManager（FIFO 裁剪 + Tool Result 折叠）
- ✅ `agent/loop_detector.rs` - LoopDetector（三种检测模式）
- ✅ `agent/budget_guard.rs` - BudgetGuard（本地预算预检）

#### 包加载
- ✅ `package/loader.rs` - ZIP 包加载器
- ✅ `package/prompt_builder.rs` - System Prompt 组装

#### LLM Providers
- ✅ `providers/openai.rs` - OpenAI Provider（TODO）
- ✅ `providers/ollama.rs` - Ollama Provider（TODO）
- ✅ `providers/router.rs` - LLM 路由（TODO Phase 2）
- ✅ `providers/reliable.rs` - 重试 + fallback（TODO Phase 2）

#### 工具系统（Phase 1: 13个内置工具）
- ✅ `tools/registry.rs` - ToolRegistry
- ✅ `tools/permission.rs` - 权限校验
- ✅ `tools/wrappers.rs` - 安全装饰器（TODO）
- ✅ `tools/builtin/shell.rs` - Shell 工具
- ✅ `tools/builtin/file_read.rs` - 文件读取
- ✅ `tools/builtin/file_write.rs` - 文件写入
- ✅ `tools/builtin/file_edit.rs` - 文件编辑
- ✅ `tools/builtin/glob_search.rs` - Glob 搜索
- ✅ `tools/builtin/content_search.rs` - 内容搜索
- ✅ `tools/builtin/calculator.rs` - 计算器
- ✅ `tools/builtin/http_request.rs` - HTTP 请求
- ✅ `tools/builtin/web_fetch.rs` - 网页抓取
- ✅ `tools/builtin/web_search.rs` - 网络搜索
- ✅ `tools/builtin/weather.rs` - 天气查询
- ✅ `tools/builtin/identity_query.rs` - 身份查询
- ✅ `tools/builtin/memory_store.rs` - 记忆存储
- ✅ `tools/builtin/memory_recall.rs` - 记忆检索

#### IPC 通信
- ✅ `ipc/transport.rs` - Transport trait + UnixSocketTransport
- ✅ `ipc/client.rs` - GatewayClient

#### 其他
- ✅ `memory/mod.rs` - Grafeo 客户端（TODO Phase 2）
- ✅ `skills/mod.rs` - SKILL.md 解析（TODO Phase 2）
- ✅ `config.rs` - RuntimeConfig
- ✅ `cli.rs` - CLI 定义（clap）
- ✅ `error.rs` - RuntimeError

**依赖**: rollball-core, rollball-memory, rollball-grafeo, tokio, reqwest, clap

---

### 7. rollball-gateway（Gateway）
**位置**: `crates/rollball-gateway/`

**核心模块**:

#### Gateway 核心
- ✅ `gateway/state.rs` - GatewayState（已安装/运行中的 Agent）

#### 包管理器
- ✅ `package_manager/install.rs` - 安装流程
- ✅ `package_manager/uninstall.rs` - 卸载流程
- ✅ `package_manager/upgrade.rs` - 升级流程（签名一致性校验）

#### 生命周期管理
- ✅ `lifecycle/manager.rs` - LifecycleManager（start/stop agent）
- ✅ `lifecycle/process.rs` - 进程管理（TODO）

#### IPC 服务端
- ✅ `ipc/server.rs` - IpcServer（Unix Socket）
- ✅ `ipc/transport.rs` - 传输层（TODO）
- ✅ `ipc/session.rs` - 会话管理（TODO）

#### 其他模块（Phase 2）
- ✅ `intent/mod.rs` - Intent 路由（TODO Phase 2）
- ✅ `budget/mod.rs` - 预算追踪（TODO Phase 2）
- ✅ `rate/mod.rs` - 速率限制（TODO Phase 2）
- ✅ `vault/mod.rs` - Vault 集成（TODO）

#### 配置和 CLI
- ✅ `config.rs` - GatewayConfig
- ✅ `cli.rs` - CLI（install/uninstall/start/stop/list/daemon）
- ✅ `error.rs` - GatewayError

**依赖**: rollball-core, rollball-sign, rollball-vault, tokio, clap

---

## 示例和测试

### Examples
- ✅ `examples/weather-agent/manifest.toml` - 天气 Agent 配置
- ✅ `examples/weather-agent/prompts/system.md` - System Prompt
- ✅ `examples/weather-agent/prompts/default.md` - Default Prompt

### Tests
- ✅ `tests/integration_test.rs` - 集成测试骨架

---

## CI 配置

- ✅ `.cargo/config.toml` - Cargo 配置
- ✅ `rustfmt.toml` - 代码格式化配置
- ✅ `clippy.toml` - Clippy lint 配置
- ✅ `dev/ci.sh` - CI 脚本（check/clippy/test/all）
- ✅ `.gitignore` - Git 忽略规则

---

## 下一步工作

### Phase 1 实现优先级

1. **S1: 基础层**
   - [ ] rollball-core: 所有类型定义完成 ✅
   - [ ] rollball-sign: 实现 Ed25519 签名/验签
   - [ ] rollball-vault: 实现 ChaCha20-Poly1305 加密存储

2. **S2: Runtime 核心**
   - [ ] Agent 主循环（9步流程）
   - [ ] 13个内置工具实现
   - [ ] OpenAI/Ollama Provider 实现
   - [ ] History Manager（token 计算 + FIFO 裁剪）
   - [ ] Loop Detector 实现
   - [ ] IPC 客户端（Unix Socket）

3. **S3: Gateway**
   - [ ] 包管理器（安装/卸载/升级）
   - [ ] 生命周期管理器（spawn/kill）
   - [ ] IPC 服务端
   - [ ] Key Vault 集成

4. **S4: 集成验证**
   - [ ] weather-agent 端到端测试
   - [ ] 集成测试套件

---

## 代码约定

- **Rust Edition**: 2024
- **Rust Version**: 1.87
- **错误处理**: thiserror + ? 操作符
- **异步**: tokio + async-trait
- **序列化**: serde + serde_json
- **CLI**: clap derive
- **日志**: tracing + tracing-subscriber
- **代码风格**: 所有 crate 通过 `cargo clippy --all-targets -- -D warnings`

---

## 设计文档对照

所有代码结构严格遵循以下设计文档：
- ✅ `docs/module-design/00-overview.md` - Workspace 总览
- ✅ `docs/module-design/01-core.md` - rollball-core 设计
- ✅ `docs/module-design/02-runtime.md` - rollball-runtime 设计
- ✅ `docs/module-design/03-gateway.md` - rollball-gateway 设计
- ✅ `docs/module-design/04-grafeo.md` - rollball-grafeo 设计
- ✅ `docs/module-design/05-vault-sign.md` - rollball-vault + rollball-sign 设计
- ✅ `docs/plan/plan-p1.md` - Phase 1 开发计划

**设计文档完整性**: ✅ 无缺失
