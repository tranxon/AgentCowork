# ADR-022: 流式行 Role 变化即 Flush — 让 JSONL 成为如实的实时记录

**状态**：已采纳 / 待实施确认  
**日期**：2026-07-01  
**决策者**：架构讨论  
**前置**：ADR-021（统一 Session 数据加载）  
**影响范围**：

- `core/acowork-runtime/src/providers/openai.rs` 及其他 provider adapter（把原始 provider chunk 归一化为结构化 `StreamEvent`）
- `core/acowork-runtime/src/agent/agent_core.rs`（StreamingLine 生命周期与 flush 语义）
- `core/acowork-runtime/src/agent/loop_llm.rs`（只消费结构化事件，不解析前端/展示语义）
- `core/acowork-runtime/src/agent/loop_tools.rs`（tool_call 前的文本边界 flush）
- `core/acowork-runtime/src/conversation.rs`（`line + char_offset` 流式读取语义）
- `apps/acowork-desktop/src/stores/chatStore.ts`（严格按 JSONL 行序 + streaming line 临时占位渲染）
- `apps/acowork-desktop/src/components/chat/ChatPanel.tsx`（移除运行时输出的 role 解析责任）

---

## 1. 背景

ADR-021 将前端数据加载统一成 HTTP Pull：

1. 前端读取 JSONL 中已完成的行。
2. 前端同时读取 Runtime 内存里的一个未完成 `StreamingLine`。
3. 前端用 `(line_number, char_offset)` 继续拉取新增完整行和未完成行增量。

这个方向是对的：**JSONL 是持久化真相，StreamingLine 只是 JSONL 下一行尚未完成时的临时表现**。

但当前问题暴露出 ADR-021 里一个不够严谨的假设：

> 一个 `StreamingLine` 的 role 在生命周期内不会变化。

这个假设不成立。LLM 的一次响应可能天然包含多个语义段：

- assistant 正文
- reasoning / thought
- assistant 正文继续
- tool_call
- tool_result
- 下一轮 assistant 正文

如果 Runtime 允许一个 `StreamingLine` 混合多个 role，再把拆分责任交给前端，就会出现：

- JSONL 行内同时包含 assistant 与 thought 内容；
- assistant 文本被错误地显示到 tool_call 前后；
- 前端需要 `parseThinkContent` / `stripThinkTags` 等补丁；
- 流式占位符的 type 需要反复同步；
- JSONL 与用户看到的顺序语义不一致。

这不是前端显示 bug 的根因，而是**写入边界定义不清**。

---

## 2. 决策

采用 **Role 变化即 Flush**：

> Runtime 一旦确认下一个语义段的 role 与当前 `StreamingLine.role` 不同，就必须先把当前非空 `StreamingLine` flush 成一条 JSONL 完整行，再开启新的 `StreamingLine`。JSONL 中每个 message 行只能有一个清晰 role。

更精确地说：

1. **JSONL 是持久化真相**：前端必须严格按 JSONL 行序渲染完整行。
2. **StreamingLine 是未完成行**：它只代表“下一条尚未写入 JSONL 的单 role 行”。
3. **role 不允许原地变更**：同一个 `StreamingLine` 生命周期内 role 不变；role 变化只能通过 flush 边界完成。
4. **前端不负责 role 推断**：前端不解析 think 标签、不纠正 role、不重排消息。
5. **provider adapter 负责原始协议归一化**：Runtime 主循环只消费结构化 `StreamEvent`，不直接从 UI 或混合文本里猜 role。

---

## 3. 关键定义

### 3.1 JSONL 完整行

已经写入 conversation JSONL 文件的一行 `ConversationEntry`。

性质：

- append-only；
- 一行一个 role；
- 行序就是展示顺序；
- 一旦写入，不再由前端补拆、补排或补修。

### 3.2 StreamingLine

Runtime 内存中的未完成行：

```rust
pub struct StreamingLine {
    pub line_number: usize,
    pub role: String,
    pub accumulated_content: String,
    pub started_at: String,
}
```

语义：

- `line_number` 是它 flush 后将成为的 JSONL 行号；
- `role` 是这条未完成行唯一允许的 role；
- `accumulated_content` 只包含该 role 下的内容；
- flush 后该对象必须从 `StreamingStateMap` 移除或替换为新 role 的空对象。

### 3.3 Role segment

一个连续的、已归一化的模型输出片段。片段内 role 恒定，片段之间可以发生 role transition。

例如：

```text
assistant: "我先看一下文件。"
thought:   "需要定位配置读取路径。"
assistant: "接下来调用搜索工具。"
tool_call: grep(...)
```

上面是四个 role segment，至少应产生三条 message JSONL 行 + 一条 tool_call JSONL 行。

---

## 4. “新 role” 的判定标准

这是本 ADR 的核心澄清。

### 4.1 新 role 只能来自 provider-normalized StreamEvent

Runtime 主循环不得根据以下信息推断新 role：

- 当前 `StreamingLine.role`；
- 前端消息 type；
- JSONL 历史行的最后 role；
- 累积文本里临时出现的半截标签；
- `finish_reason`；
- provider 名称硬编码分支。

新 role 只能由 provider adapter 输出的结构化事件决定：

| Provider-normalized event | 对应 role / 边界 | Runtime 动作 |
|---|---|---|
| `StreamEvent::Content(text)` | `assistant` | 如果当前 streaming role 不是 `assistant`，先 flush，再追加 text |
| `StreamEvent::ReasoningContent(text)` | `thought` | 如果当前 streaming role 不是 `thought`，先 flush，再追加 text |
| `StreamEvent::ToolCallStart` | 非文本边界 | flush 当前文本行；随后 tool_call 按 JSONL 行写入 |
| `StreamEvent::ToolCallChunk` | tool_call 参数增量 | 不参与 StreamingLine；只累积 tool_call arguments |
| `StreamEvent::Finished` | 响应结束边界 | flush 当前文本行；合并 usage / finish_reason / tool_calls |
| `StreamEvent::Error` | 异常边界 | flush 当前文本行后返回错误 |
| 用户 Stop / Pause | 控制边界 | flush 当前文本行后停止或暂停 |

换句话说，Runtime 里的“新 role”不是任意字符串参数，而是 `StreamEvent` 类型映射出的有限集合。

建议实现上使用内部 enum 收紧边界：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamingRole {
    Assistant,
    Thought,
}

impl StreamingRole {
    fn as_jsonl_role(self) -> &'static str {
        match self {
            StreamingRole::Assistant => "assistant",
            StreamingRole::Thought => "thought",
        }
    }
}
```

即使当前实现阶段仍用 `&str`，也必须遵守同一约束：只有 `Content` 与 `ReasoningContent` 能开启文本 streaming role。

### 4.2 原始 think 标签属于 provider adapter 责任

有些 OpenAI-compatible provider 不使用 `delta.reasoning_content`，而是把 thinking 内容嵌在 `delta.content` 中，例如：

```text
"我先说明一下<!think>这里需要分析路径willReturn然后调用工具"
```

这类原始 chunk 不应进入前端，也不应作为混合 role 文本进入 JSONL。

provider adapter 必须先把它归一化为结构化事件序列：

```text
Content("我先说明一下")
ReasoningContent("这里需要分析路径")
Content("然后调用工具")
```

然后 Runtime 主循环只按第 4.1 节的 `StreamEvent -> role` 映射处理。

### 4.3 半截标签不能触发 role 变化

provider adapter 解析 raw `delta.content` 时必须处理跨 chunk 标签：

```text
chunk 1: "abc<!thi"
chunk 2: "nk>reasoning willRet"
chunk 3: "urn answer"
```

规则：

1. 只有完整识别 opening marker 后，才能从 `Content` 切到 `ReasoningContent`。
2. 只有完整识别 closing marker 后，才能从 `ReasoningContent` 切回 `Content`。
3. marker 本身不是用户可见内容，不写入 JSONL。
4. 未决的半截 marker 留在 parser scratch buffer 中，不得提前发送给 Runtime 主循环。

这样“role 变化”是结构化事件边界，不是字符串扫描时的猜测。

### 4.4 tool_call 不是 StreamingLine role

`tool_call` 是完整 JSONL 行，不是文本 streaming line。

因此：

- `ToolCallStart` 到来时，必须先 flush 当前 assistant/thought 文本行；
- tool_call 参数增量进入 tool_call accumulator；
- 参数完整后写 `role="tool_call"` JSONL 行；
- 工具执行结果写 `role="tool_result"` JSONL 行。

这能保证 assistant text + tool_calls 的常规模型输出顺序：

```jsonl
{"role":"assistant","content":"我先搜索相关文件。"}
{"role":"tool_call","content":"..."}
{"role":"tool_result","content":"..."}
```

不会再让 assistant text 悬挂在内存占位符里跨 iteration 累积。

---

## 5. Runtime 写入模型

### 5.1 核心不变量

必须满足以下不变量：

1. `append_streaming_delta(role, delta)` 只能向同 role 的 `StreamingLine` 追加内容。
2. 如果当前 line role 与目标 role 不同，调用方必须先走 transition helper。
3. transition helper 负责：
   - 当前 line 非空：写入 JSONL；
   - 当前 line 为空：丢弃；
   - 创建或保持目标 role 的 streaming line。
4. `flush_streaming_line()` 是唯一把 streaming 文本写入 JSONL 的路径。
5. 任何直接 `conversation.append_message()` 绕过 `flush_streaming_line()` 的路径，都必须同步更新 `total_lines`，否则 HTTP Pull 的 `total_lines` 会失真。

建议 transition helper 语义如下：

```rust
fn transition_streaming_role(
    target: StreamingRole,
    conversation: Option<&ConversationSession>,
) {
    match current_streaming_line() {
        None => create_empty_line(target),
        Some(line) if line.role == target.as_jsonl_role() => {}
        Some(line) if line.accumulated_content.is_empty() => replace_empty_line(target),
        Some(_) => {
            flush_streaming_line(conversation);
            create_empty_line(target);
        }
    }
}
```

### 5.2 结构化事件处理

Runtime 主循环应接近下面的形态：

```rust
match event {
    StreamEvent::Content(text) => {
        transition_streaming_role(StreamingRole::Assistant, conversation);
        append_streaming_delta(StreamingRole::Assistant, &text);
        notify_new_data_available();
    }
    StreamEvent::ReasoningContent(text) => {
        transition_streaming_role(StreamingRole::Thought, conversation);
        append_streaming_delta(StreamingRole::Thought, &text);
        notify_new_data_available();
    }
    StreamEvent::ToolCallStart(tool_call) => {
        flush_streaming_line(conversation);
        begin_tool_call(tool_call);
        notify_new_data_available();
    }
    StreamEvent::Finished(response) => {
        flush_streaming_line(conversation);
        merge_final_response_metadata(response);
    }
    StreamEvent::Error(error) => {
        flush_streaming_line(conversation);
        return Err(error);
    }
    StreamEvent::ToolCallChunk { index, arguments } => {
        accumulate_tool_call_arguments(index, arguments);
    }
}
```

### 5.3 Finished 事件中的 tool_calls

部分 provider 可能不发送完整的 `ToolCallStart` / `ToolCallChunk` 序列，而是在 `Finished` response 中返回完整 `tool_calls`。

规则不变：

1. `Finished` 先 flush 当前文本 streaming line；
2. 再从 final response 合并 tool_calls；
3. `prepare_tool_calls` 写入 tool_call JSONL 行；
4. 不得为了 tool_calls 回头改写上一条 assistant 行。

---

## 6. JSONL 与前端读取模型

### 6.1 JSONL 行序是唯一展示顺序

前端展示顺序：

```text
messages from JSONL, ordered by file line number
+ optional current StreamingLine placeholder at its future line_number
```

前端不做：

- 不按 timestamp 重排；
- 不把 tool_call 移到 assistant 后面；
- 不解析 assistant content 里的 think 标签；
- 不把 thought 合并进 assistant；
- 不根据 type 变化修改已经创建的 JSONL 行。

### 6.2 `line + char_offset` 语义

前端拉取参数：

- `line_number`：前端已经看到的 JSONL 完整行数 / 最新行位置；
- `line_char_offset`：当前 streaming line 已经读取到的字符偏移。

Runtime 返回：

- `messages`：`line_number` 之后新增的 JSONL 完整行；
- `streaming`：当前未完成行从 `line_char_offset` 之后的 delta；
- `total_lines`：当前 JSONL 完整行数。

当 role transition 发生时：

```text
poll N:
  streaming line=12 role=assistant content="我先"

Runtime:
  Content continues -> assistant line grows
  ReasoningContent arrives -> flush line 12 assistant to JSONL, create line 13 thought

poll N+1:
  messages contains JSONL line 12 assistant
  streaming line=13 role=thought content="需要分析"
```

前端处理策略：

1. 先合并 `messages` 完整行；
2. 如果某个 streaming placeholder 已经对应到完整 JSONL 行，删除 placeholder；
3. 再追加/更新新的 streaming placeholder；
4. 最终视觉顺序仍然是 JSONL 行序 + 当前未完成行。

---

## 7. 典型场景

### 7.1 assistant + thought + assistant + tool_call

归一化事件：

```text
Content("我先看一下。")
ReasoningContent("需要定位 session 读取路径。")
Content("接下来搜索文件。")
ToolCallStart(grep)
Finished(...)
```

JSONL：

```jsonl
{"role":"assistant","content":"我先看一下。"}
{"role":"thought","content":"需要定位 session 读取路径。"}
{"role":"assistant","content":"接下来搜索文件。"}
{"role":"tool_call","content":"...grep..."}
```

### 7.2 raw content 中携带 think marker

provider raw chunks：

```text
"我先看<!think>分析路径willReturn然后搜"
```

provider adapter 输出：

```text
Content("我先看")
ReasoningContent("分析路径")
Content("然后搜")
```

Runtime JSONL：

```jsonl
{"role":"assistant","content":"我先看"}
{"role":"thought","content":"分析路径"}
{"role":"assistant","content":"然后搜"}
```

### 7.3 只有 thought，没有 assistant

归一化事件：

```text
ReasoningContent("分析中...")
Finished(...)
```

JSONL：

```jsonl
{"role":"thought","content":"分析中..."}
```

不需要伪造 assistant 空行。

---

## 8. 迁移策略

### 阶段 1：收紧 Runtime / provider 边界

1. provider adapter 把 raw think marker 归一化为 `ReasoningContent` / `Content`。
2. Runtime 主循环只基于 `StreamEvent` 判定 role。
3. `StreamingLine.role` 不再原地变更；role 变化必须 flush。
4. `ToolCallStart` / `Finished` / Stop / Error 统一 flush 当前文本行。
5. 增加单元测试覆盖跨 chunk marker、role transition、tool_call 前文本 flush。

### 阶段 2：简化前端

1. 前端保留 `(line_number, char_offset)` 拉取逻辑。
2. 前端移除运行时输出的 think 标签解析与 role 修正逻辑。
3. 前端只做 JSONL 行序渲染与 streaming placeholder 生命周期管理。
4. 若需要兼容旧 JSONL 混合行，只能作为 legacy display fallback，不能影响新写入路径。

### 阶段 3：清理补丁路径

1. 删除 `stripThinkTags` / `parseThinkContent` 等运行时补丁。
2. 删除 `lastStreamingLine` 等为 role 原地变化服务的状态。
3. 将 `append_streaming_delta` 改成 typed role 或加 debug assertion，防止再次静默改 role。

---

## 9. 测试要求

必须补齐以下测试，避免继续靠手测定位：

1. **provider parser：单 chunk 多段**  
   输入：`a<!think>bwillReturnc`  
   输出：`Content(a) -> ReasoningContent(b) -> Content(c)`。

2. **provider parser：跨 chunk marker**  
   输入：`a<!thi` + `nk>bwillRet` + `urnc`  
   输出不能泄漏半截 marker，JSONL 不包含 marker。

3. **Runtime role transition**  
   `Content(a) -> ReasoningContent(b) -> Content(c)` 产生三条单 role JSONL 行。

4. **assistant text + tool_call**  
   `Content("我来查") -> ToolCallStart` 必须先写 assistant 行，再写 tool_call 行。

5. **Finished 中携带 tool_calls**  
   即使没有 `ToolCallStart`，也必须先 flush assistant/thought 文本，再进入 `prepare_tool_calls`。

6. **前端行序**  
   `messages + streaming` 合并后展示顺序严格匹配 JSONL 行序，streaming placeholder flush 后不残留重复气泡。

---

## 10. 风险与缓解

### 风险 1：JSONL 行数增加

role 边界会产生更多短行。

缓解：这是正确记录语义的必要成本。典型一次响应只多 1-3 行，JSONL append-only 写入成本可接受。

### 风险 2：provider marker 格式变化

不同 provider 可能使用不同 thinking marker。

缓解：变化局限在 provider adapter；Runtime 与前端只依赖结构化 `StreamEvent`，不感知 provider 私有格式。

### 风险 3：旧 JSONL 文件存在混合 role 行

旧文件可能已经包含 assistant 行内嵌 think marker。

缓解：旧文件兼容只能作为 legacy display fallback；新 Runtime 写入路径必须保证单行单 role。不能为了兼容旧数据继续污染新架构。

### 风险 4：空 StreamingLine 造成空 placeholder

transition helper 可能创建空 line。

缓解：HTTP 返回 streaming delta 时，如果 content 为空，前端不创建可见 placeholder；flush 空 line 时不写 JSONL。

---

## 11. 结论

ADR-022 的本质不是“再补一个 think 解析规则”，而是明确边界：

- provider adapter：把原始 provider 协议归一化为结构化事件；
- Runtime：按结构化事件维护单 role StreamingLine，role 变化即 flush；
- JSONL：保存单 role 完整行，行序即展示顺序；
- Frontend：按 JSONL 顺序渲染，streaming 只是未完成行占位。

这能把当前的 thought/tool/assistant 错位问题从“前端补丁链”拉回到一个可验证、可测试、可维护的持久化模型上。