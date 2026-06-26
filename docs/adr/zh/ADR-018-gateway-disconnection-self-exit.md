# ADR-018: Runtime 和 Embed 在 Gateway 断连后超时自动退出

**状态**：待决策  
**日期**：2026-06-26  
**决策者**：架构讨论  
**影响范围**：

- `core/acowork-runtime/src/grpc/client.rs`（GatewayGrpcClient 新增超时自退出机制）
- `core/acowork-runtime/src/cli.rs`（run_gateway_loop 断连后处理逻辑调整）
- `core/acowork-embed/src/main.rs`（Embed 启动参数或新增 Gateway 健康探测）
- `core/acowork-embed/src/server.rs`（可选：新增 Gateway 健康探测端点配置）
- `core/acowork-gateway/src/gateway/mod.rs`（正常退出时主动 kill Runtime + Embed）

---

## 背景

### 问题

当前 Gateway、Runtime、Embed 三者构成一个进程树：

```
Desktop App (Tauri)
  └── acowork-gateway  (process)
        ├── acowork-runtime (process) — 每个 agent 一个
        └── acowork-embed   (process) — 全局一个
```

Gateway 是唯一的生命周期管理者。当 Gateway 退出时，目前只保证两种情况：

1. **正常退出（ctrl_c 信号处理）**：仅 Kill Embed，不 Kill Runtime
2. **异常退出（panic/SIGKILL/TerminateProcess）**：无任何清理，Runtime 和 Embed 全部成为孤儿进程

虽然 ADR 系列正在修复正常退出路径（通过 Gateway 主动 Kill），但异常退出永远无法由 Gateway 处理 —— 进程已死，无法执行清理代码。因此必须让 Runtime 和 Embed 自身具备检测 Gateway 存活并在超时后自动退出的能力，作为兜底防线。

### 当前断连行为分析

**Runtime：**

- gRPC 连接断开后，`recv_message()` 返回 `Ok(None)`
- 触发 `try_reconnect_gateway()`，内部调用 `reconnect_and_reregister()`
- 重连使用指数退避（100ms 起步，最大 30s），总超时 300s
- 超时后 `LoopAction::Break` 触发 Runtime 退出

也就是说 Runtime 最终会退出 —— 但最快也要等 300s，且这 300s 内持续占用内存、网络等资源。

**Embed：**

- Embed 完全不连接 Gateway，它是一个独立的 HTTP 服务
- Embed 仅通过 `/events` SSE 向外发送心跳（2s 间隔），但**不消费**任何 Gateway 的心跳
- Embed 完全不知道 Gateway 的生死状态
- Embed 只在收到 OS 信号（SIGTERM/SIGINT）时才退出

**结论**：Embed 一旦启动，除非被手动杀死或收到 OS 信号，否则**永远不退出**。这是最严重的问题。

## 目标

1. Gateway **异常崩溃**时，Runtime 和 Embed 能在可接受的时间内（例如 300s）自动退出释放资源
2. Gateway **正常重启**时（例如升级），Runtime 能在重连窗口内成功重新连接，不会因超时误退出
3. 新增机制尽可能简单，不引入复杂的分布式共识协议

## 可选方案

### 方案 A：Gateway 下发「预期心跳间隔」，双方独立检测

**原理**：Gateway 在 AgentHello / Embed 注册时将 `heartbeat_interval_secs` 和 `missed_heartbeat_limit` 下发给 Runtime 和 Embed。双方各自独立检测对方心跳，超时后退出。

- **Runtime 侧**：Gateway gRPC 连接断开后启动倒计时（如 300s），倒计时内重连成功则取消，超时则自动 `exit(0)`
- **Embed 侧**：Embed 定期（如每 10s）请求 Gateway `/health`，超时（如 300s）后自动 `exit(0)`
- **Gateway 侧**：现有的 embed_supervisor 已通过 SSE 心跳（2s 间隔，10s 超时）检测 Embed 存活，无需改动

**优点**：
- 覆盖所有场景（正常/异常退出）
- 超时参数可配置，适应不同部署需求
- 不依赖 OS 信号

**缺点**：
- Embed 需要新增 Gateway 健康探测逻辑，增加复杂度
- Runtime 需要从「无限重连」改为「有限超时后退出」

### 方案 B：仅加强 Gateway 正常退出清理 + 维持 Runtime 现有重连机制

**原理**：
- Gateway 正常退出时：主动 Kill 所有 Runtime + Embed（正在实施）
- Runtime：维持现有重连机制，300s 后放弃退出
- Embed：不做改动，依赖 OS 信号或 Gateway 主动 Kill

**优点**：
- 改动最小
- 正常退出场景完全覆盖

**缺点**：
- 异常崩溃场景下，Runtime 仍占用 300s 资源
- Embed 永不为 Gateway 崩溃所杀，资源泄漏最严重

### 方案 C：Runtime/Embed 增加 Graceful Timeout，Gateway 增加 shutdown API

**原理**：
- Gateway 新增 `POST /api/shutdown` HTTP 端点，接收 shutdown 信号后执行正常清理
- Runtime 维持现有重连超时 300s
- Embed 新增 Gateway 心跳探测（同方案 A）
- Desktop App 退出时优先调用 `/api/shutdown`，再杀 Gateway 进程

**优点**：
- 三层防御：优雅 shutdown API → Gateway 进程树杀 → Runtime/Embed 超时自退出
- 重连时间短，资源释放快

**缺点**：
- 改动范围大
- 需要新增 HTTP 端点

## 决策

推荐采用 **方案 A**：

### 理由

1. **方案 B 不安全**：Embed 在 Gateway 崩溃后永久残留，长期运行时内存泄漏不可接受
2. **方案 A 比方案 C 更简洁**：不需要新增 shutdown API 端点，依赖双方独立检测，减少耦合
3. **超时参数化**允许正常重启场景下 Runtime 有足够重连窗口

### 详细设计

#### Runtime 侧改动

**文件**：`core/acowork-runtime/src/grpc/client.rs`

在 `GatewayGrpcClient` 中新增 `disconnect_timeout_ms` 字段（默认 300000 = 300s）。当 `recv_message()` 检测到连接断开时，启动一个超时倒计时：

```
[gRPC disconnected] → [start 300s timer]
                        ├── [reconnected within 300s] → cancel timer, continue
                        └── [300s elapsed] → tracing::error + std::process::exit(1)
```

**参数传递**：
- Gateway 在 `AgentHelloResult` 中下发 `disconnect_timeout_ms`
- Runtime 的 `AgentHelloConfig` 新增对应字段

**现状调整**：
- 当前 `try_reconnect_gateway` 最多尝试 300s，行为不变
- 超过 300s 不再返回 `LoopAction::Continue`，而是直接退出进程

#### Embed 侧改动

**文件**：`core/acowork-embed/src/main.rs`

Embed 新增启动参数：
- `--gateway-health-url`：Gateway 的健康检查地址（如 `http://127.0.0.1:19876/health`）
- `--gateway-health-timeout-ms`：连续失败超时（默认 300000 = 300s）
- `--gateway-health-interval-ms`：探测间隔（默认 10000 = 10s）

当参数提供时，启动一个后台任务周期性请求 Gateway `/health`：
```
[tick every 10s]
  └── GET /health
        ├── 成功 → reset failure_count = 0
        └── 失败 → failure_count += 1
                    ├── failure_count * 10s < 300s → continue
                    └── failure_count * 10s >= 300s → tracing::error + std::process::exit(1)
```

Gateway 在 `spawn_embed_process()` 时，通过 CLI 参数 `--gateway-health-url` 传入自身的 health endpoint。

#### Gateway 侧改动

**文件**：`core/acowork-gateway/src/lifecycle/embed.rs`

在 `spawn_embed_process()` 中，新增参数 `--gateway-health-url` 指向 Gateway 自身的 health endpoint（如 `http://127.0.0.1:19876/health`）。

**文件**：`core/acowork-gateway/src/gateway/mod.rs`

同步进行 ADR 正在实施的正常退出清理（ctrl_c 分支 Kill 所有 Runtime + Embed），作为主动防御。

### 参数默认值

| 参数 | Runtime | Embed |
|------|---------|-------|
| 探测/检测方式 | gRPC 流断开 | HTTP GET /health |
| 超时时间 | 300s | 300s |
| 探测间隔 | N/A（被动检测） | 10s |
| 重连尝试 | 300s 内指数退避 | 无重连 |

### 时序图

```
Gateway 异常崩溃:

Gateway          Runtime                Embed
  |                 |                     |
  |[crash]          |                     |
  |---- gRPC broken-|                     |
  |                 |-- detect disconnect  |
  |                 |   start 300s timer   |
  |                 |   (try reconnect)    |
  |                 |                      |-- GET /health timeout
  |                 |                      |   failure_count++
  |                 |   try reconnect...   |   (Gap 10s)
  |                 |                      |-- GET /health timeout
  |                 |                      |   failure_count++
  |                 |<reconnect fails 300s>|   <300s elapsed>
  |                 |-- exit(0)            |-- exit(0)
```

Gateway 正常退出（配合主动 Kill）:

```
Gateway               Runtime              Embed
  |                     |                     |
  |-- [ctrl_c]          |                     |
  |-- Kill(Runtime) ───>|-- exit(0)           |
  |-- Kill(Embed) ─────>|                     |-- exit(0)
  |-- exit(0)           |                     |
```

在此场景下，Runtime 还没来得及触发超时自退出，就被 Gateway 主动 Kill，资源立即释放。
异常崩溃时，超时自退出作为兜底，确保进程最终释放。

## 影响

**正向影响**：
- Gateway 异常崩溃后，Runtime 和 Embed 在 300s 内自动退出，释放内存
- 正常退出时有主动 Kill，资源立即释放（不等超时）
- 超时参数可通过 Gateway 配置下发，灵活调整

**负面影响**：
- Embed 增加对 Gateway 的依赖 —— 如果用户仅想独立运行 Embed（无 Gateway 模式），需要禁用心跳检测
- 需要协调三个 crate 的版本发布

**缓解措施**：
- Embed 的 `--gateway-health-url` 为可选参数，不传则不启动探测
- Runtime 的 `disconnect_timeout_ms` 默认 300s，可通过 AgentHelloResult 覆盖
- 正常重启场景下，Gateway 先启动并监听 health，Embed 再启动，避免启动竞态

## 实施建议

建议分两步实施：

1. **Phase 1（当前 ADR）**：Gateway 正常退出清理（Kill Runtime + Embed）+ 本 ADR 的自退出机制
2. **Phase 2**：观察运行稳定性后，再决定是否调整默认超时值或增加重连窗口自适应

## 未解决的问题

- Runtime 在 300s 超时内如果网络闪断又恢复，是否应该重置重连计时器？当前设计会（每次 gRPC 流建立重新计时）
- Embed 探测 Gateway 时，如果 Gateway 返回非 2xx（如 503），是立即计为失败还是继续重试？建议非 2xx 不计数（Gateway 还活着只是忙）
