# 27 — 流式输出 Thinking / Assistant / Tool 显示 Bug 分析

**Date**: 2026-07-01
**Reviewer**: Senior Engineer
**Status**: ✅ 6 个根因全部修复 | 架构合理性优先，三层对称设计

---

## 现象复述

对话文件 `20260701_094805_85468e.jsonl` 中，从 "存档" 用户输入开始，后端记录了：
- **4 次 thought**
- **8 次 tool_call + 8 次 tool_result**（合计 16 条工具事件）
- **1 条 assistant 最终回复**

但前端仅显示：
- **3 次 thought**（丢失 1 次）
- **约 8 条工具事件**（丢失约一半）
- **assistant 内容显示在列表中间位置**而非末尾

附加现象：
- 流式输出期间，长 thinking 内容错误地显示到了 assistant 位置
- 新的 thinking 流式内容覆盖/叠加到旧的 thinking 位置上
- 工具调用已渲染出来，上方 thinking 块仍在刷新（违反时间顺序）

---

## 🔑 回归确认：ADR-021 重构引入

**MiniMax 之前显示正常，是 ADR-021（commit `945dd23`, 2026-06-30）重构前端轮询机制时引入的回归。**

### 重构前后对比

| 维度 | 重构前（WebSocket streaming） | 重构后（HTTP polling） |
|------|------------------------------|----------------------|
| **数据通道** | WebSocket `chunk` 事件实时推送 | HTTP `loadSessionMessages` 轮询 |
| **流式状态** | `streamingMessageId`, `streamBuffer`, `thinkingMessageId`, `isInThinkPhase` | `pollLineNumber`, `pollCharOffset`（仅坐标） |
| **`<think>` 解析** | chunk handler 实时解析 `<thinking>` 标签，动态切换 thought/assistant 角色 | **无** — 仅依赖 backend `streaming.role` 字段（始终为 `"assistant"`） |
| **显示层保护** | `displayMessages` 检测 `streamingMessageId`，对流式 assistant 内容做 `<think>` 标签预处理 | **已删除** — 注释写 "Messages arrive complete via HTTP poll — no streaming state" |

### 被删除的关键代码（3 处）

**1. WebSocket chunk handler 的 `<thinking>` 标签解析**（`chatStore.ts`, 约 150 行）
```typescript
// 旧代码已删除：case "chunk" 中检测 <thinking> 标签
if (trimmed.startsWith("<thinking>")) {
  // 进入 think 阶段，创建 thought 类型消息
  return updateSessionState(..., { isInThinkPhase: true, thinkingMessageId: thinkMsgId });
}
if (ss.isInThinkPhase && ss.thinkingMessageId) {
  const closeIdx = newBuffer.indexOf("</thinking>");
  // 检测到 </thinking>，关闭 thought，创建 assistant 消息
}
```

**2. `displayMessages` 的流式 `<think>` 预处理**（`ChatPanel.tsx`, 约 10 行）
```typescript
// 旧代码已删除：处理流式 assistant 消息中的未闭合 <think> 标签
if (msg.id === streamingMessageId) {
  const trimmed = msg.content.trimStart();
  if (trimmed.startsWith('<think>') && !trimmed.includes('</think>')) {
    exploreBuffer.push({ ...msg, type: 'thought', content: thinkContent });
    continue;
  }
}
```

**3. 状态字段全部移除**：`streamingMessageId`, `streamBuffer`, `thinkingMessageId`, `isInThinkPhase`, `isReasoning` 全部删除，无等效替代。

### 为什么旧代码能正常工作而新代码不行

旧代码有三层防护：
1. **chunk handler** 实时解析 `<thinking>` 标签 → 流式时就能正确分离 thought/assistant
2. **displayMessages** 对流式未完成消息做 `<think>` 预处理 → 即使 chunk handler 有遗漏，显示层兜底
3. **persist_think_to_conversation** 持久化时解析 → 轮询到的历史数据正确

新代码只剩第 3 层（持久化解析），第 1、2 层全部丢失。因此：
- **流式阶段**：`<think>...</think>` 标签原样出现在 assistant 气泡中，thought 内容不可见
- **持久化后**：轮询到的已完成 entry 正确分离，但之前的流式混乱已造成"内容跳到中间""工具调用上方 thinking 还在刷新"等时序错乱

---

## 根因分析

### 根因 1（🔴 P0）：单一流式占位符 + 角色切换时 type 未更新

**文件**: `apps/acowork-desktop/src/stores/chatStore.ts`
**行号**: 1252–1274

```typescript
const streamingLineId = `msg-streaming-${sessionId}`;  // 唯一占位符

if (streaming && streaming.content) {
  const existingIdx = messages.findIndex((m) => m.id === streamingLineId);
  if (existingIdx >= 0) {
    messages[existingIdx] = {
      ...messages[existingIdx],                          // ← BUG: 保留了旧 type!
      content: messages[existingIdx].content + streaming.content,
    };
  } else {
    const streamingMsg: ChatMessage = {
      id: streamingLineId,
      type: streaming.role === "thought" ? "thought" : "assistant",  // 仅新建时正确
      content: streaming.content,
      // ...
    };
    messages = [...messages, streamingMsg];
  }
}
```

**时序问题:**

```
T1: streaming.role = "thought"  → 创建占位符 type="thought", content="think..."
T2: streaming.role = "assistant" → existingIdx >= 0
    → type 仍是 "thought"（BUG!），content 变成 "think...assistant..."
T3: displayMessages 中 type="thought" → 推入 exploreBuffer（ExploreBlock 内）
T4: assistant 内容出现在 ExploreBlock 的折叠面板里，而非独立消息气泡
```

**导致的症状:**
- ✅ "assistant 内容显示在中间位置" — type=thought 所以进了 ExploreBlock
- ✅ "长 thinking 显示到 assistant 位置" — type 不随角色切换更新
- ✅ "新的 thinking 覆盖旧 thinking" — 共享同一个 `id`，React key 冲突

**修复方向:**
```typescript
// 在 append 时同步更新 type
messages[existingIdx] = {
  ...messages[existingIdx],
  type: streaming.role === "thought" ? "thought" : "assistant",  // ← 补充
  content: messages[existingIdx].content + streaming.content,
};
```

---

### 根因 2（🔴 P0）：流式占位符「创建后立即被删除」

**文件**: `apps/acowork-desktop/src/stores/chatStore.ts`
**行号**: 1254–1284

```typescript
// Step 1: 创建/更新流式占位符
if (streaming && streaming.content) {
  // ...
  messages = [...messages, streamingMsg];  // 刚创建
}

// Step 2: 判断是否删除
const hasStreamingMsg = messages.some((m) => m.id === streamingLineId); // ← 此时为 true!
if (hasStreamingMsg && (!streaming || streaming.line > prevStreamingLine)) {
  messages = messages.filter((m) => m.id !== streamingLineId); // ← 刚创建的立即删了
}

// Step 3: 追加持久化条目
if (newMessages.length > 0) {
  messages = [...messages, ...newMessages];  // 但流式占位符已经没了
}
```

**场景：首次收到新行的流式增量时:**

| 变量 | 值 | 说明 |
|------|-----|------|
| `prevStreamingLine` (pollLineNumber) | 5 | 上次轮询的 total_lines |
| `streaming.line` | 6 | 新的流式行号（line 6） |
| `streaming.content` | "Hello" | 流式内容 |
| `newMessages` | [thought #1] | 刚刚持久化的条目 |

执行流程:
1. `existingIdx = -1` → 创建新占位符 `msg-streaming-xxx`，追加到 messages
2. `hasStreamingMsg = true`
3. `streaming.line (6) > prevStreamingLine (5)` → **删除刚创建的占位符!**
4. `newMessages` 追加持久化条目

结果：流式占位符只存在于一个 set() 周期的瞬间，前端**永远看不到流式占位符**。

只有当 `streaming.line == prevStreamingLine` 时（同一行继续流式输出），占位符才能存活。

**导致的症状:**
- ✅ 流式内容不显示 / 闪烁
- ✅ 用户看到的只是间歇性的最终状态，没有流式过程

**修复方向:**
```typescript
// 在创建占位符之前记录是否存在
const hadStreamingBefore = messages.some((m) => m.id === streamingLineId);

// ... 创建/更新占位符 ...

// 仅删除「本周期之前就存在」的占位符
if (hadStreamingBefore && (!streaming || streaming.line > prevStreamingLine)) {
  messages = messages.filter((m) => m.id !== streamingLineId);
}
```

---

### 根因 3（🟡 P1）：`pollLineNumber` 与 `streaming.line` 语义混淆

**文件**: `apps/acowork-desktop/src/stores/chatStore.ts`
**行号**: 1251, 1282

```typescript
const prevStreamingLine = ss.pollLineNumber;  // 这是 total_lines（JSONL 总行数）
// ...
if (hasStreamingMsg && (!streaming || streaming.line > prevStreamingLine)) {
  // streaming.line 是流式内容所在的具体行号
  // prevStreamingLine 是 total_lines
  // 两者本质含义不同!
```

`pollLineNumber` 存储的是 `data.total_lines`，即 JSONL 文件的总行数。而 `streaming.line` 是流式增量所在的具体行号。虽然数值通常相近，但语义完全不同：

| 场景 | total_lines | streaming.line | 关系 |
|------|-------------|-----------------|------|
| thinking 流式时生成多个 tool_call | 10 | 6 | streaming.line < total_lines |
| done 后最终轮询 | 22 | null | 无 streaming |

当 `total_lines` 多跳几行（一次 poll 中持久化了多个条目），而 streaming.line 不变时，`streaming.line (6) > prevStreamingLine (10)` 为 false，占位符不会被删除。但当 `total_lines` 刚好增加 1 且 streaming 开始新行时，触发条件成立。

**修复方向:**
- 明确引入 `prevStreamingLineNumber` 独立字段，仅追踪 streaming.line 的变化
- 或用 `streaming.line !== prevStreamingLineNumber` 替代 `> prevPollLineNumber`

---

### 根因 4（🟡 P1）：流式占位符进入 ExploreBlock 导致分组异常

**文件**: `apps/acowork-desktop/src/components/chat/ChatPanel.tsx`
**行号**: 281–317

```typescript
for (const msg of messages) {
  if (msg.type === 'tool_call' || msg.type === 'tool_result') {
    exploreBuffer.push(msg);
  } else if (msg.type === 'thought') {
    exploreBuffer.push(msg);  // ← 流式 thought 占位符也进来!
  } else if (msg.type === 'assistant') {
    // ...
    flushExplore();            // 但 assistant 类型才 flush
    grouped.push(msg);
  }
}
```

当流式占位符的 type 为 `thought` 时，它被推入 `exploreBuffer`。但 `exploreBuffer` 只在遇到 `assistant` 类型消息时才 flush。这意味着：

1. **连续多个 thought（持久化 + 流式）都在同一个 ExploreBlock 里**
2. **ExploreBlock 内的 ThinkBlock 组件按 `isStreaming` 渲染**，但 `isStreaming` 仅对最后一个 explore_group 生效（ChatPanel.tsx 第 1162 行）
3. **当工具调用出现在 exploreBuffer 中时，上方的 thinking 仍在流式输出** — 因为 exploreBuffer 是一个整体 block

```
ExploreBlock (explore_group)
├── ThinkBlock: thought #1 (持久化, endTime 有值) → collapsed
├── ToolCall: bash ls
├── ToolResult: bash ls → "24-adr-..."
├── ThinkBlock: thought #2 (流式占位符, isStreaming=true) → 正在刷新!
├── ToolCall: todo_write
├── ToolResult: todo_write → "Todo list updated"
└── ...
```

**导致的症状:**
- ✅ "工具调用显示出来了，上面的 thinking 还在刷新" — 同一 ExploreBlock 内同时存在已完成工具和流式 thinking
- ✅ 视觉上违反了时间顺序预期

**修复方向:**
- 流式占位符不应进入 exploreBuffer
- 或者：exploreBuffer 遇到流式 thought 占位符时立即 flush，让流式内容独立渲染
- 或者：流式占位符使用独立的特殊 type（如 `streaming_thought`），不与持久化 thought 混合

---

## 影响链路总结

```
                          流式角色切换时 type 未更新（根因 1）
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        ▼                           ▼                           ▼
 assistant 内容          type=thought 的流式           content 混合（think
 显示在 ExploreBlock 内   占位符进入 exploreBuffer     + assistant 拼接）
 （位置错误）                  │
                              ▼
                    「工具调用已出现，thinking 还在刷新」
                         （根因 4 — 分组异常）
                              
        流式占位符创建后立即删除（根因 2）
                    │
                    ▼
        占位符只在同一行持续流式时存活
        首次行号推进时永远丢失流式内容
                    │
                    ▼
          流式内容闪烁/不显示，用户只能看到最终持久化结果
          
      pollLineNumber 与 streaming.line 语义混淆（根因 3）
                    │
                    ▼
        删除判断逻辑不可靠，进一步加剧根因 2
```

---

## 修复优先级

| 优先级 | 根因 | 影响范围 | 修复复杂度 |
|--------|------|----------|-----------|
| P0 | 根因 1 — 流式角色切换 type 未更新 | assistant 显示位置、内容混合 | 低（加一行） |
| P0 | 根因 2 — 占位符创建后立即删除 | 流式过程完全不可见 | 低（加一行判断） |
| P1 | 根因 4 — 流式占位符进入 exploreBuffer | 工具调用与 thinking 时间顺序错乱 | 中（需调整分组逻辑） |
| P1 | 根因 3 — 语义混淆 | 长期维护隐患 | 低（独立字段） |

---

## 附录：相关文件索引

| 文件 | 关键行号 | 作用 |
|------|---------|------|
| `apps/acowork-desktop/src/stores/chatStore.ts` | 1252–1284 | 流式占位符的创建/更新/删除逻辑 |
| `apps/acowork-desktop/src/stores/chatStore.ts` | 1241–1317 | 增量轮询消息合并 |
| `apps/acowork-desktop/src/components/chat/ChatPanel.tsx` | 266–326 | displayMessages 聚合分组逻辑 |
| `apps/acowork-desktop/src/components/chat/ChatPanel.tsx` | 1563–1605 | parseThinkContent XML 标签解析 |
| `apps/acowork-desktop/src/components/chat/ExploreBlock.tsx` | 362–398 | buildPairedItems 工具配对 |
| `apps/acowork-desktop/src/components/chat/ThinkBlock.tsx` | 74–143 | 思考块渲染与流式状态 |
| `apps/acowork-desktop/src/lib/polling.ts` | 1–290 | 轮询管理器 |
| `apps/acowork-desktop/src/lib/types.ts` | 651–665 | PaginatedMessages / streaming 类型定义 |

---

---

## 补充：Runtime 端分析（`20260701_101657.log`）

### 对话数据核实

JSONL `20260701_094805_85468e.jsonl` 中 "存档" 之后的数据：

| 角色 | 数量 | 说明 |
|------|------|------|
| `thought` | 4 | `<think>` 块提取后独立存储 |
| `tool_call` | 8 | 每次工具调用独立条目 |
| `tool_result` | 8 | 每次工具执行结果 |
| `assistant` | 1 | 最终文本回复（去除 `<think>` 后的内容） |
| **合计** | **21** | 实际 8 次工具调用 + 8 次结果，用户说的"16 次工具调用"是把 call + result 合计 |

> 后端数据完整且正确，**无数据丢失**。

### Runtime 处理流程

```
MiniMax API SSE Stream
   │
   ├── delta.content = "<think>\n用户要把报告..."   ────┐
   │                                                      │
   │  loop_llm.rs:134                                    │
   │  StreamEvent::Content(chunk)                        │
   │  → append_streaming_delta("assistant", chunk)   ←   │ 所有内容（含<think>）
   │                                                     │ 都标记为 "assistant"!
   │  delta.content = "...报告存档到...</think>\n好的" ────┘
   │
   ├── delta.tool_calls[...]  ──── StreamEvent::ToolCallStart + ToolCallChunk
   │
   └── StreamEvent::Finished ── ── finish_reason="tool_calls"
                                       reasoning_len=0  ← 永远是 0!
                                       content_len=438  (含<think>标签)
```

### 根因 5（新增 — Runtime 端）：MiniMax 用 `<think>` 嵌入 content，流式时不区分角色

**文件**: `core/acowork-runtime/src/agent/loop_llm.rs` 行 134–144

```rust
StreamEvent::Content(chunk) => {
    accumulated_content.push_str(&chunk);
    self.core.append_streaming_delta("assistant", &chunk);  // ← 全部标为 assistant
    self.core.notify_new_data_available();
}
StreamEvent::ReasoningContent(chunk) => {
    accumulated_reasoning_content.push_str(&chunk);
    self.core.append_streaming_delta("thought", &chunk);  // ← only for reasoning_content field
    self.core.notify_new_data_available();
}
```

**关键事实:**

| 维度 | DeepSeek 原生 | MiniMax-M3 |
|------|-------------|------------|
| thinking 位置 | `delta.reasoning_content` | `delta.content` 内嵌 `<think>` 标签 |
| reasoning_len | > 0 | 永远 = 0 |
| 流式角色 | `thought` | **`assistant`** |
| 持久化角色 | `thought` + `assistant`（分离） | `thought` + `assistant`（分离） |

**影响:**

1. **流式期间前端看到的 thinking 内容标记为 `assistant`** — 因为 `append_streaming_delta("assistant", ...)` 被调用
2. **`<think>...</think>` 标签作为纯文本出现在流式输出中** — 前端 `parseThinkContent` (ChatPanel.tsx:1563) 仅在持久化轮询的数据中解析
3. **流式占位符 type="assistant"** — 不会进入 exploreBuffer，而是作为独立 assistant 气泡渲染。如果 thinking 很长，用户看到"assistant 气泡里出现 `<think>` 开头的内容"，然后流式切换到 `</think>` 后的真实 assistant 内容 → **视觉上 assistant 位置出现 thinking 文本的错觉**
4. **持久化后轮询到的数据角色才是正确的** — `persist_think_to_conversation` (loop_session.rs:395) 正确地将 `<think>` 块提取为 `role="thought"`，剩余内容为 `role="assistant"`

**这是"assistant 内容显示在中间位置"问题的上游原因：** 流式时 role 全部是 `assistant`，前端组件按 `assistant` 渲染流式占位符；但当流式结束后轮询到分离的 `thought` + `assistant` 条目时，`displayMessages` 将 thought 推入 exploreBuffer，而 assistant 作为独立消息渲染，造成位置跳变。

### Runtime 健康状态

- ✅ Chunk relay 正常（line 76: `Chunk relay started (single channel)`）
- ✅ Session 状态转换正常：Idle → Streaming → Idle
- ✅ 每次迭代的 Finished 事件正确触发
- ✅ 无错误、无 panic、无重试超限
- ✅ JSONL 持久化无异常
- ⚠️ Grafeo memory 未连接（line 53: `Failed to open Grafeo memory store`）— 不影响显示

---

## 修复策略

### 设计原则

> **架构合理性第一，禁止打补丁，追求质量。**

核心问题：ADR-021 重构删除了流式阶段的 think 标签处理，导致流式和持久化两层对 think 标签的处理不对称。修复不是"补回来"，而是**重建对称性**。

### 三层对称架构

| 层 | 职责 | Think 标签处理 | 文件 |
|----|------|---------------|------|
| **Runtime** | 流式 role 判定 | think 标签状态机解析，动态调用 append_streaming_delta("thought"/"assistant") | `loop_llm.rs` |
| **Store** | 占位符生命周期 | `lastStreamingLine` 独立字段跟踪行号；role 切换时同步更新 type；仅在 !streaming 或 line推进+新消息到达 时删除 | `chatStore.ts` |
| **Display** | 显示层解析 | `parseThinkContent` 分离 think/reply；`stripThinkTags` 清除流式 thought 内容中的标签 | `ChatPanel.tsx` |

### 流式与持久化的对称性

```
┌─────────────────────────────────────────────────────────────┐
│                    Runtime (loop_llm.rs)                     │
│                                                              │
│  StreamEvent::Content(chunk)                                 │
│    ├─ accumulated_content.push_str(chunk)  ← 原样保留标签    │
│    │   → 传给 persist_think_to_conversation                  │
│    │   → extract_think_block 解析标签                        │
│    │                                                         │
│    └─ chunk_scratch 状态机解析标签                           │
│        → append_streaming_delta("thought"/"assistant")       │
│        → StreamingLine.role 反映当前阶段                     │
│        → 前端 poll 拿到正确的 streaming.role                 │
│                                                              │
│  对称性: accumulated_content 和 StreamingLine.accumulated_   │
│         content 包含相同的原始内容（含标签），仅 role 不同    │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────────┐
│   持久化路径              │     │   流式路径                    │
│                          │     │                              │
│  persist_think_to_       │     │  HTTP poll → streaming.role  │
│  conversation            │     │  → chatStore 占位符 type     │
│  → extract_think_block   │     │  → ChatPanel parseThinkContent│
│  → role="thought"        │     │  → stripThinkTags            │
│  → role="assistant"      │     │  → ThinkBlock / assistant 气泡│
│                          │     │                              │
│  标签解析：extract_think  │     │  标签解析：状态机 +           │
│  _block (loop_session.rs)│     │  parseThinkContent           │
│                          │     │  (loop_llm.rs + ChatPanel)   │
└─────────────────────────┘     └─────────────────────────────┘
```

### 已实施的修复

**根因 1（P0）— 角色切换时 type 未更新** ✅
- `chatStore.ts:1268-1272`：streaming.role 变化时同步更新占位符 type

**根因 2（P0）— 占位符创建后立即删除** ✅
- `chatStore.ts:1287-1300`：删除条件改为 !streaming 或 line推进 && newMessages.length > 0

**根因 3（P1）— pollLineNumber 与 streaming.line 语义混淆** ✅
- `chatStore.ts`：新增 lastStreamingLine 字段，独立跟踪 streaming.line

**根因 4（P1）— 流式占位符进入 exploreBuffer** ✅
- `ChatPanel.tsx:298-310`：流式占位符 parseThinkContent 预处理

**根因 5（Runtime）— MiniMax think 嵌入 content 不区分角色** ✅
- `loop_llm.rs:159-233`：think 标签状态机 + chunk_scratch 跨 chunk 边界处理 + partial_tag_suffix_len 辅助函数

**根因 6 — 流式 think 预处理** ✅
- `loop_llm.rs`：Runtime 端状态机（根因 5 的实现）
- `ChatPanel.tsx:284-288`：stripThinkTags 清除 thought 类型消息中的标签

### 编译验证

- Runtime: `cargo check --package acowork-runtime` ✅
- Runtime: `cargo clippy --package acowork-runtime -- -D warnings` ✅
- Frontend: `tsc --noEmit` ✅

---

## 下一步

1. **验证**：使用 `20260701_094805_85468e.jsonl` 中的对话数据回放测试
2. **集成测试**：用 MiniMax-M3 模型发一条消息，观察流式 thinking/assistant/tool 显示是否正确
