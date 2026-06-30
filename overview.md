# Fix: Frontend Session Polling Real-Time Display

## Problem 1: 10s batch display
用户消息发出去后，thinking 和 tool calls 延迟约 10 秒才批量出现，前端不是实时流式显示。

### Root Cause — Three Cascading Bugs

**Bug 1: Gateway 丢弃了 `total_lines` 和 `streaming` 字段**
`SessionMessagesResponse` 结构体只有 `messages/cursor/has_more`，Runtime 返回的 `total_lines` 和 `streaming` delta 被 Gateway 丢弃。前端 `pollLineNumber` 永远为 0，增量轮询路径从未激活。

**Bug 2: 前端使用 truthy 检查 (`if (lineNumber)`)**
`line_number=0` 和 `line_char_offset=0` 是合法的轮询坐标值，但被当作 falsy 跳过。导致 `line_number` 参数从不上送 HTTP 请求，Runtime 每次走全量 cursor 分页路径（重载 50 条消息）。

**Bug 3: Runtime 要求两个参数必须同时存在**
`if let (Some(ln), Some(co)) = (line_number, line_char_offset)` — 因为前端不传 `line_char_offset`（0 falsy），解构失败，回退到 cursor 分页。

**最终效果**: 前端每 500ms 做一次完整的 50 条消息重载，从不接收 streaming delta。tool call/thinking 只有在被 flush 到 JSONL 后才被下一次全量重载发现，形成 10s+ 的批量刷新。

### Fixes

| 层级 | 文件 | 修改 |
|------|------|------|
| Gateway | `core/acowork-gateway/src/http/chat.rs` | `SessionMessagesResponse` 新增 `total_lines`/`streaming` 字段；`SessionMessagesQuery` 新增 `line_number`/`line_char_offset`；handler 透传参数和响应字段 |
| Runtime | `core/acowork-runtime/src/cli.rs:2864` | 解构改为 `if let Some(ln) = line_number`，`co` 默认 0 |
| Frontend | `apps/acowork-desktop/src/stores/chatStore.ts` | `if (lineNumber)` → `if (lineNumber != null)`；同样修复 `charOffset` 和 `isIncremental` |
| Frontend Types | `apps/acowork-desktop/src/lib/types.ts` | `PaginatedMessages` 新增 `total_lines`/`streaming` 字段 |

## Problem 2: Thinking 与 Explore block 分离
thinking 内容显示在 explore 块外面，和上面的 assistant 文本一起显示。

### Root Cause
`displayMessages` useMemo 中，当 assistant 消息同时包含 `<thinking>` 和 reply 内容时，`flushExplore()` 立刻把 thinking 打入独立的 ExploreBlock，后续 tool_call 又形成另一个 ExploreBlock，导致 thinking 与 tools 分离。

### Fix
`ChatPanel.tsx` — assistant reply 的 flush 条件改为：只有 exploreBuffer 中已有 tool_call/tool_result 时才 flush。如果 buffer 中只有 thought（当前 assistant 刚推入的 thinking），延迟 flush，让后续 tool calls 加入同一个 explore group。

## Verified
- ✅ `cargo check` — gateway + runtime 编译通过
- ✅ `cargo test` — 22 个 conversation 单元测试全绿
- ✅ `tsc --noEmit` — TypeScript 编译通过
