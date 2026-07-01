# ADR-022 实施进度报告

## 概述

本次工作完成了 ADR-022（流式行 Role 变化即 Flush）的实施核查、死代码清理、核心违规修复和测试补齐。ADR-022 的目标是用"Role 变化即 Flush"原则替代之前的补丁链条，让 JSONL 成为如实的、实时的、每行单 role 的记录。

## 实施状态

### 阶段 1：Runtime / provider 边界 — ✅ 完成

| 项 | 文件 | 状态 |
|----|------|------|
| ThinkTagParser（raw think marker 归一化） | `providers/openai.rs:1347-1570` | ✅ 7 个单元测试 |
| `flush_and_new_streaming_line` | `agent_core.rs:491-525` | ✅ role 不同且非空才 flush |
| `ensure_streaming_line` role 不可变 | `agent_core.rs:388-417` | ✅ 本次修复违规 |
| loop_llm.rs 切换到 flush 触发 | `loop_llm.rs:148-175` | ✅ |
| loop_tools.rs prepare_tool_calls 简化 | `loop_tools.rs:625-640` | ✅ |

### 阶段 2：前端简化 — ✅ 完成（本轮真正收尾）

之前该阶段被标为 cancelled 但实际停在中间态，本轮核实并完成到 ADR 终态。

| 项 | 文件 | 状态 |
|----|------|------|
| 移除 placeholder `type` 原地变更 | `chatStore.ts` | ✅ 违反 §6.1，本轮修复 |
| placeholder 改 line-scoped id `msg-streaming-{sid}-{line}` | `chatStore.ts` | ✅ type 随 line 固定 |
| 彻底删除 `lastStreamingLine` 补丁状态 | `chatStore.ts`（定义 + 5 处初始化） | ✅ 本轮删除 |
| ChatPanel.tsx 移除 parseThinkContent 调用 | `ChatPanel.tsx:285-305` | ✅ |
| ChatPanel 渲染兼容 line-scoped id | `ChatPanel.tsx:1169` | ✅ `startsWith` 前缀兼容 |

### 阶段 3：清理 — ✅ 本次完成

| 项 | 文件 | 状态 |
|----|------|------|
| 删除 parseThinkContent 死函数 | `ChatPanel.tsx` | ✅ 本次删除 |
| 删除 stripThinkTags 死函数 | `ChatPanel.tsx` | ✅ 本次删除 |

### ADR §9 测试 — ✅ 基本完成

| # | 测试项 | 状态 |
|---|--------|------|
| 1 | provider parser 单 chunk 多段 | ✅ `test_complete_think_block` |
| 2 | provider parser 跨 chunk marker | ✅ `test_think_spanning_chunks` + `test_close_tag_spanning_chunks` |
| 3 | Runtime role transition | ✅ `test_adr022_role_transition_produces_single_role_lines` |
| 4 | assistant text + tool_call 顺序 | ✅ `test_adr022_assistant_text_then_tool_call_preserves_order` |
| 5 | Finished 中携带 tool_calls | ✅ `test_adr022_finished_with_tool_calls_flushes_text_first` |
| 6 | 前端行序 | ❌ 未补（可选） |

## 本次（阶段 2 收尾）发现并修复的问题

### 1. 前端 placeholder `type` 原地变更（违反 ADR §6.1）

`chatStore.ts` 在已创建的流式 placeholder 上原地改 `type`：
```ts
messages[existingIdx] = {
  ...messages[existingIdx],
  type: streaming.role === "thought" ? "thought" : "assistant",  // ❌ 违规
  content: messages[existingIdx].content + streaming.content,
};
```
配合 `lastStreamingLine` 判断 placeholder 删除时机，是前端侧补丁链条。

**修复**：placeholder id 改为 line-scoped `msg-streaming-{sid}-{line}`。每条 streaming line 一个独立 placeholder，type 在创建时由 role 固定、生命周期内不再改；role transition 触发 Runtime flush + 新行（更高 line_number），前端自然得到不同 id，旧 placeholder 在对应 JSONL 完整行到达后被清理。`lastStreamingLine` 字段整体删除。

### 2. 跨行 char_offset 漏字符 bug（backend，附带修复）

`conversation.rs::read_messages_since` 原逻辑把上一行（已 flush）的 `line_char_offset` 直接套到 flush 后的新 streaming line，`chars().skip(offset)` 会吞掉新行开头字符。role 频繁 flush 时前端会漏字。

**修复**：`if line_number < sl.line_number { 0 } else { line_char_offset.min(current_len) }` —— 前端 line_number 落后于 streaming line 自身 line_number 时，offset 属于已 flush 的旧行，对新行无意义，从头读。补 2 个回归测试。

## 之前修复的核心违规（Runtime）

### ensure_streaming_line role 原地变更

`agent_core.rs` 的 `ensure_streaming_line` 保留了 ADR §2 第 3 条禁止的 role 覆盖逻辑，已改为 `debug_assert_eq!` 捕获违规。

## 验证结果

- `cargo test --package acowork-runtime` → **496 passed, 0 failed**（494 + 2 跨行 offset 回归测试）
- `cargo clippy --package acowork-runtime -- -D warnings` → 零警告
- 前端 `tsc --noEmit` → 零错误

## 下一步建议

1. **实际运行验证**：用 MiniMax 跑一次 think + tool_call 对话，确认 JSONL 每行单 role、前端顺序正确、无漏字
2. **前端测试（可选）**：补 ADR §9 第 6 项前端行序测试
3. **提交**：改动量大，建议分 commit（backend offset 修复+测试 / 前端 store 简化 / 前端死代码清理）
