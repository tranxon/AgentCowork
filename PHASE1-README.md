# Rollball.AI Phase 1 - 代码结构

> Phase 1 骨架代码已完成，所有数据结构和函数声明已创建。

## 快速开始

### 1. 检查项目结构

```bash
# 查看所有 crate
ls crates/

# 查看完整结构
tree crates/
```

### 2. 编译检查

```bash
# 检查所有 crate 是否可以编译
cargo check --all

# 运行 clippy lint
cargo clippy --all-targets -- -D warnings

# 运行测试
cargo test --all
```

### 3. 使用 CI 脚本

```bash
# 运行完整 CI 流程
./dev/ci.sh all

# 仅检查编译
./dev/ci.sh check

# 仅运行 clippy
./dev/ci.sh clippy

# 仅运行测试
./dev/ci.sh test
```

## 项目结构

```
rollball-ai/
├── Cargo.toml                    # Workspace 根配置
├── crates/
│   ├── rollball-core/            # 共享类型、协议、trait
│   ├── rollball-memory/          # MemoryStore trait 抽象层
│   ├── rollball-runtime/         # Agent Runtime 二进制
│   ├── rollball-gateway/         # Gateway 二进制
│   ├── rollball-grafeo/          # Grafeo 图数据库（Phase 2）
│   ├── rollball-vault/           # 密钥加密存储
│   └── rollball-sign/            # 签名工具链
├── examples/
│   └── weather-agent/            # 示例天气 Agent
├── tests/
│   └── integration_test.rs       # 集成测试
└── dev/
    └── ci.sh                     # CI 脚本
```

## Crate 依赖关系

```
rollball-core (基础类型)
    ├── rollball-sign (依赖 core)
    ├── rollball-vault (依赖 core)
    ├── rollball-memory (依赖 core)
    │       └── rollball-grafeo (依赖 core + memory)
    ├── rollball-runtime (依赖 core + memory + grafeo)
    └── rollball-gateway (依赖 core + sign + vault)
```

## 当前状态

- ✅ Workspace 结构建立
- ✅ 所有 crate 骨架创建
- ✅ 数据结构和函数声明完成
- ⏳ 具体实现待完成（标记为 `unimplemented!()` 或 `TODO`）

## Phase 1 开发顺序

根据 `docs/plan/plan-p1.md`：

1. **S1: 基础层** - rollball-core, rollball-sign, rollball-vault
2. **S2: Runtime 核心** - Agent 主循环、内置工具、Providers
3. **S3: Gateway** - 包管理、生命周期、IPC
4. **S4: 集成验证** - weather-agent 端到端测试

## 设计文档

所有代码严格遵循设计文档：

- `docs/00-prd.md` - 产品需求定义
- `docs/01-overview.md` - 平台设计总纲
- `docs/module-design/00-overview.md` - 模块设计总览
- `docs/module-design/01-core.md` - rollball-core 详细设计
- `docs/module-design/02-runtime.md` - rollball-runtime 详细设计
- `docs/module-design/03-gateway.md` - rollball-gateway 详细设计
- `docs/module-design/05-vault-sign.md` - vault + sign 详细设计
- `docs/plan/plan-p1.md` - Phase 1 开发计划

## 代码约定

- Rust Edition 2024
- 所有 crate 通过 `cargo clippy --all-targets -- -D warnings`
- 错误处理使用 `thiserror` + `?` 操作符
- 异步代码使用 `tokio` + `async-trait`
- CLI 使用 `clap` derive

## 下一步

开始实现具体功能，从 S1 基础层开始：

1. 实现 rollball-sign 的 Ed25519 签名/验签
2. 实现 rollball-vault 的加密存储
3. 完善 rollball-core 的单元测试
4. 逐步实现 Runtime 和 Gateway 的核心逻辑
