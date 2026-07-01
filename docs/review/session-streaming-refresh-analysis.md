# Session 流数据实时刷新链路分析报告 (v2 — 含日志验证)

## 问题现象

think 和 assistant 流数据没有在前端实时刷新。流数据传完后，前端一次性刷新出来。

## v2 修订说明

v1 报告基于纯代码静态分析，推测根因为前端 `AbortController` 竞态。v2 引入了实际运行日志验证，发现**真正的根因比 v1 推测的更底层**——`new_data_available` 事件根本没有被产生和传输。

---

## 日志验证的关键发现

### 🔴 发现 1: Gateway 日志中没有任何 `new_data_available` 事件

**日志文件**: `C:\Users\nicholas\.acowork\acowork-gateway\data\logs\20260701_090020.log`

在 3200+ 行日志中搜索 `new_data_available` / `NewDataAvailable`：

```
搜索结果: 0 匹配
```

Gateway 日志只记录了 `session_state_changed` 事件（来自 `com.acowork.system`），**没有任何来自 `com.acowork.senior-engineer` 的 `new_data_available` 事件**。

**对比**: `session_state_changed` 事件在日志中有 16 条记录，证明 WebSocket 推送链路本身是通的。

### 🔴 发现 2: Agent Runtime 日志在启动后立即停止

**日志文件**: `C:\Users\nicholas\.acowork\acowork-gateway\config\packages\com.acowork.senior-engineer\workspace\logs\20260701_090044.log`

```
第 1 行: 09:00:44.318 — Runtime 启动
第 89 行: 09:00:45.139 — 最后一行日志 (Gateway message loop started)
```

**Agent Runtime 日志只有 89 行，09:00:45 后完全沉默**。但 Gateway 日志显示前端在 09:11-09:12 期间有大量 messages 请求和频繁的 activate/deactivate 操作。

搜索 agent 日志中的 `EnableNotify` / `notify_enabled` / `flush_streaming` / `append_streaming`：

```
搜索结果: 0 匹配
```

**Agent Runtime 从未记录过任何 streaming 相关日志**——这意味着在这次运行中，LLM 流式输出、`append_streaming_delta`、`notify_new_data_available` 可能根本没有执行。

### 🔴 发现 3: 所有 messages 请求都没有 `line_number` 和 `line_char_offset` 参数

Gateway 日志中所有 `/messages` 请求的 URL：

```
GET /api/agents/com.acowork.senior-engineer/sessions/20260701_085528_9629c4/messages?limit=50&direction=backward
GET /api/agents/com.acowork.senior-engineer/sessions/20260630_190434_ec3d12/messages?limit=50&direction=backward
```

**没有一个请求带有 `line_number` 或 `line_char_offset` 参数**。这意味着：

1. 前端从未进入增量 poll 模式——所有请求都是全量加载
2. `PollingManager` 要么从未启动，要么 `notify()` 从未被调用
3. `new_data_available` WebSocket 事件从未到达前端（与发现 1 吻合）

### 🔴 发现 4: 前端在疯狂切换 session

Gateway 日志 09:11:27 - 09:12:16 期间的模式：

```
09:11:27.731  GET messages (session ec3d12)
09:11:28.477  GET messages (session 9629c4)
09:11:30.763  GET messages (session ec3d12)
09:11:33.533  GET messages (session 9629c4)
09:11:35.233  GET messages (session ec3d12)
09:11:35.991  GET messages (session 9629c4)
09:11:37.227  GET messages (session ec3d12)
09:11:38.064  GET messages (session 9629c4)
09:11:39.205  GET messages (session ec3d12)
09:11:40.371  GET messages (session 9629c4)
09:11:43.678  GET messages (session ec3d12)
09:11:44.528  GET messages (session 9629c4)
09:11:45.659  GET messages (session ec3d12)
09:11:46.161  GET messages (session 9629c4)
```

每隔 ~2 秒在两个 session 之间反复切换，每次切换都触发 deactivate → activate → get messages → list sessions。这是**前端 session 切换逻辑的 bug**，但不是流式刷新不工作的直接原因。

### 🔴 发现 5: `session_state_changed` 只来自 `com.acowork.system`

Gateway 日志中所有 `session_state_changed` 事件：

```
09:00:29.985  from=com.acowork.system  action=session_state_changed
09:00:31.861  from=com.acowork.system  action=session_state_changed
09:00:45.137  from=com.acowork.senior-engineer  action=session_state_changed  ← 唯一一条
09:01:38.610  from=com.acowork.system  action=session_state_changed
09:02:20.298  from=com.acowork.system  action=session_state_changed
09:03:01.750  from=com.acowork.system  action=session_state_changed
```

`com.acowork.senior-engineer` 只在 09:00:45 发了一条 `session_state_changed`（启动时的初始状态），之后再没发过任何状态变更事件。这进一步证实 **agent runtime 在启动后没有执行过任何 LLM 调用**。

---

## 修正后的根因分析

### 真正的根因: `new_data_available` 事件从未产生

基于日志证据，问题链路如下：

```
预期链路:
LLM Delta → append_streaming_delta → notify_new_data_available → WebSocket → 前端

实际链路:
LLM Delta → (从未发生)
                ↓
            Agent Runtime 日志在 09:00:45 后完全沉默
                ↓
            没有 append_streaming_delta 调用
                ↓
            没有 notify_new_data_available 调用
                ↓
            没有 new_data_available WebSocket 事件
                ↓
            前端 PollingManager.notify() 从未被触发
                ↓
            前端从未进入增量 poll 模式 (没有 line_number 参数)
                ↓
            所有 messages 请求都是全量加载
                ↓
            "流数据传完后一次性刷新"
```

**但是**——用户明确说"流数据传完了前端一次性刷新出来"，这意味着 LLM 确实产生了输出，只是没有实时推送。

### 可能的原因链

#### 原因 A: `notify_enabled` 未被设为 `true`

`notify_new_data_available()` 的第一行检查：

```rust
if !self.notify_enabled.load(Ordering::Relaxed) { return; }  // 静默返回!
```

`notify_enabled` 默认为 `false`，只有在收到 `SessionMessage::EnableNotify` 后才设为 `true`。

代码路径追踪显示 `EnableNotify` 在 `activate_session` handler 中发送（`cli.rs:964-969`），但存在**静默失败风险**：

```rust
// cli.rs:964-969
if let Err(e) = session_manager.send_to_session(&session_id, SessionMessage::EnableNotify) {
    tracing::warn!(..., "Failed to enable notify for activated session");
}
```

如果 `send_to_session` 失败（例如 session task 尚未完全就绪），只记录 warn 日志，Runtime 仍返回 `{"activated": true}`。但由于 agent 日志在 09:00:45 后沉默，这条 warn 可能根本没有被记录。

**验证方法**: 检查 agent 日志中是否有 `"enabling NewDataAvailable notifications"` 或 `"Failed to enable notify"` 的 INFO/WARN 记录。日志搜索结果：**0 匹配**。

这说明 `EnableNotify` 消息**要么从未被发送，要么 SessionTask 从未收到**。

#### 原因 B: 前端频繁切换 session 导致状态混乱

日志显示前端在两个 session 之间疯狂切换（每 ~2 秒一次），每次切换触发 deactivate → activate。如果 `DeactivateNotify` 被发送（设 `notify_enabled = false`），但下一个 `EnableNotify` 因为时序问题没有到达，`notify_enabled` 就会保持 `false`。

但 `DeactivateNotify` 的日志搜索也是 0 匹配——这更倾向于原因 A：**session task 从未处理过 Enable/Disable 消息**。

#### 原因 C: Agent Runtime 在这次运行中没有实际执行 LLM 调用

Agent 日志只有启动阶段的输出（89 行，到 09:00:45 为止），没有任何 LLM 调用、工具执行、或 streaming 相关日志。

**可能性**: 用户在这次运行中看到的"流数据传完后一次性刷新"可能是历史会话的全量加载——而不是新的 LLM 流式输出。用户切换到旧 session 时，前端做全量 `GET /messages`（不带 `line_number`），旧消息一次性加载——这完全符合日志看到的请求模式。

---

## 验证假设的下一步

| 假设 | 验证方法 | 预期结果 |
|------|----------|----------|
| A: `notify_enabled` 未设为 true | 在 agent runtime 中发送新消息，观察日志是否有 `EnableNotify` 记录 | 如果没有 INFO 日志，确认 EnableNotify 未到达 |
| B: 前端切换 session 导致状态混乱 | 修复前端 session 切换 bug 后重试 | 如果修复后仍然不刷新，排除 B |
| C: 用户看到的是历史会话全量加载 | 确认用户是在新会话中发送新消息时观察到的"不刷新" | 如果是新消息场景，排除 C |

**最可能的验证场景**: 用户需要确认——在观察"流数据不刷新"时，是否是在一个**新创建的 session** 中**发送了新消息**并且 **LLM 正在流式输出**。如果只是切换到旧 session 查看历史消息，那是全量加载，不是 bug。

---

## 完整代码路径验证（含 activate → EnableNotify 追踪）

### activate → EnableNotify 的完整路径

```
HTTP POST /api/agents/{id}/sessions/{sid}/activate
  │
  ├─ [chat.rs:1454] activate_session() handler
  │    └─ forward_session_query("activate_session", {session_id})
  │         ├─ 通过 gRPC 推送 IntentReceived 到 Runtime
  │         └─ 等待响应 (超时 10 秒)
  │
  └── Runtime cli.rs:910 — if action == "activate_session"
       ├─ cli.rs:927 — 检查 session 是否在内存中
       │    └─ 不在内存 → ConversationSession::resume() 从磁盘恢复
       │         (失败 → 发送错误响应, EnableNotify 不发送)
       │
       ├─ cli.rs:964-969 — *** 发送 SessionMessage::EnableNotify ***
       │    └─ session_manager.send_to_session(&session_id, SessionMessage::EnableNotify)
       │         (失败只记录 warn, Runtime 仍返回 activated: true) ← ⚠️ 静默失败!
       │
       └─ cli.rs:974 — 读取 session metadata, 返回 {activated: true}
            │
            └── SessionTask 收到 EnableNotify (session_task.rs:1415)
                 ├─ notify_enabled.store(true)
                 └─ emit_session_state()
```

### 可能阻止 `EnableNotify` 的所有条件

| # | 条件 | 位置 | 后果 |
|---|------|------|------|
| 1 | session_id 缺失或为空 | cli.rs:916-924 | Runtime 发送错误响应, EnableNotify 不发送 |
| 2 | Session 磁盘恢复失败 | cli.rs:955-960 | Runtime 发送 "Session not found", EnableNotify 不发送 |
| 3 | 恢复成功但创建 task 失败 | cli.rs:945-951 | Runtime 发送错误, EnableNotify 不发送 |
| 4 | **`send_to_session` 调用失败** | **cli.rs:967-968** | **EnableNotify 不到达 SessionTask, 只记录 warn, Runtime 仍返回 activated: true** |

**条件 4 是最隐蔽的 bug**：lazy-resume 路径成功创建了 session，但 session task 在异步初始化中尚未完全就绪，`send_to_session` 可能失败。Runtime 仍然返回 `{"activated": true}` 给前端，但 `notify_enabled` 保持 `false`。

---

## 前端层面的问题（如果 new_data_available 能到达的话）

即使 `new_data_available` 事件能正常到达前端，仍然存在 v1 报告中发现的问题：

### 前端问题 1: PollingManager 无 inFlight 保护

`doPoll()` 是 async 函数，`notify()` 和 fallback timer 都可能在上一个 `doPoll()` 的 `await` 期间触发新的 `doPoll()`，导致并发 `loadSessionMessages` 调用。

### 前端问题 2: AbortController 互相 abort

`loadSessionMessages` 每次调用都 abort 上一个请求。并发 poll 导致请求被 abort，streaming delta 响应被丢弃。

### 前端问题 3: doPoll 的 status 检查可能导致提前 stop

如果 `session_state_changed` 事件未到达或延迟，`doPoll()` 检查 `session.sessionStatus?.status` 时可能还是 `idle`，导致 polling 直接 stop。

---

## 修复建议（按优先级排序）

### P0: 验证 EnableNotify 是否真正到达 SessionTask

1. **在 `session_task.rs` 的 `EnableNotify` handler 中增加更详细的日志**，确认消息是否到达
2. **在 `send_to_session` 失败时，不要静默返回 `activated: true`**——应该返回错误或重试
3. **在 `notify_new_data_available` 中增加 `notify_enabled == false` 的 debug 日志**，以便确认是否被此条件拦截

### P1: 修复 send_to_session 静默失败

```rust
// cli.rs:964-969 — 修改为
match session_manager.send_to_session(&session_id, SessionMessage::EnableNotify) {
    Ok(_) => {
        tracing::info!(session_id = %session_id, "EnableNotify sent successfully");
    }
    Err(e) => {
        tracing::error!(session_id = %session_id, error = %e, "Failed to enable notify — session data push will not work");
        // 仍然返回 activated: true，但在响应中标记 notify_enabled: false
    }
}
```

### P2: 修复前端 PollingManager 竞态

```typescript
// polling.ts — 增加 inFlight 标志
private inFlight: boolean = false;

private async doPoll(): Promise<void> {
    if (!this.running || this.inFlight) return;
    this.inFlight = true;
    try {
        // ... 原有逻辑
    } finally {
        this.inFlight = false;
    }
}
```

### P3: 修复前端 session 频繁切换

日志显示前端每 ~2 秒在两个 session 之间切换，这会导致频繁的 activate/deactivate，可能引发 `notify_enabled` 状态混乱。需要排查前端 session 切换逻辑。

### P4: 修复 think/assistant 角色共享 streaming line 的问题

`ensure_streaming_line` 在角色切换时只更新 role 不清零 `accumulated_content`，导致 think 和 assistant 内容混合。

---

## 完整数据流图（修正版）

```
LLM Delta Chunk
       │
       ▼
append_streaming_delta("assistant"/"thought", chunk)     [agent_core.rs:421]
       │  写入 StreamingStateMap (Arc<RwLock<HashMap>>)
       ▼
notify_new_data_available()                               [agent_core.rs:495]
       │
       ├── ① 检查 notify_enabled
       │      └── ⚠️ 如果 EnableNotify 未到达 SessionTask, 此处为 false → 静默返回!
       │          (日志验证: agent 日志中无 "enabling NewDataAvailable" 记录)
       │
       ├── ② 500ms 节流
       ├── ③ CAS 防竞争
       ├── ④ 读取 total_lines
       └── ⑤ 发送 ChunkEvent::NewDataAvailable
              │
              ▼
       (如果 notify_enabled == false, 以下链路全部不执行)
              │
              ▼
subsystems.rs: relay_intent("new_data_available")
       │  (gRPC → Gateway)
       ▼
Gateway: WebSocket push → 前端
       │  (日志验证: Gateway 日志中无 new_data_available 事件)
       ▼
chatStore.ts: case "new_data_available"
       │  (从未被触发)
       ▼
PollingManager.notify(totalLines)
       │  (从未被调用)
       ▼
PollingManager.doPoll() → loadSessionMessages(lineNumber, charOffset)
       │  (从未进入增量模式 — 所有请求都是全量: ?limit=50&direction=backward)
       │  (日志验证: 所有 messages 请求都无 line_number 参数)
       ▼
前端只在全量加载时看到所有消息 — "流数据传完后一次性刷新"
```

---

## 涉及的所有文件

| 文件 | 层级 | 角色 |
|------|------|------|
| `core/acowork-runtime/src/agent/loop_llm.rs` | 后端 | LLM 流式回调 |
| `core/acowork-runtime/src/agent/agent_core.rs` | 后端 | StreamingStateMap、notify_new_data_available、notify_enabled |
| `core/acowork-runtime/src/agent/session/session_task.rs` | 后端 | EnableNotify/DisableNotify 消息处理 |
| `core/acowork-runtime/src/cli.rs` | 后端 | activate_session handler、EnableNotify 发送 |
| `core/acowork-runtime/src/startup/subsystems.rs` | 后端 | ChunkEvent → gRPC 中继 |
| `core/acowork-gateway/src/http/chat.rs` | Gateway | activate_session HTTP handler、forward_session_query |
| `apps/acowork-desktop/src/stores/chatStore.ts` | 前端 | 事件路由、loadSessionMessages |
| `apps/acowork-desktop/src/lib/polling.ts` | 前端 | PollingManager |

## 日志文件参考

| 日志文件 | 说明 |
|----------|------|
| `C:\Users\nicholas\.acowork\acowork-gateway\data\logs\20260701_090020.log` | Gateway 日志 (3200+ 行) |
| `C:\Users\nicholas\.acowork\acowork-gateway\config\packages\com.acowork.senior-engineer\workspace\logs\20260701_090044.log` | Agent Runtime 日志 (仅 89 行) |
