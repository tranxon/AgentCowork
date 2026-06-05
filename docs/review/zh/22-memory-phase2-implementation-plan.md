# Memory Phase 2 实施方案

> 编号：22-memory-phase2-implementation-plan
> 创建时间：2026-06-06
> 前置文档：`21-memory-design-vs-impl-gap-analysis.md` §三（Phase 2 ~40%）
> 设计基准：`docs/design/zh/05-memory.md` §Phase 2（§1265-1312）

---

## 一、Phase 2 现状校准

文档声称 Phase 2 完成度 ~40%。经代码验证，校准如下：

### 1.1 文档准确项（无需修正）

| 设计项 | 文档说法 | 代码验证 |
|--------|---------|---------|
| ProceduralNode 数据结构 | "已完成" | ✅ `rollball-grafeo/src/types.rs` 存在，但**缺 `source_skill`、`learned_from`、`activation_count`** |
| ProceduralNode Grafeo 存储 | "已完成" | ✅ `grafeo.rs store_procedural()` 完整 CRUD |
| RetrievalMetrics | "已完成" | ✅ `rollball-memory/src/types.rs` 基础指标在 runtime 中填充 |
| Judge 轻量评估 | "已完成" | ⚠️ 骨架存在但 **MOCK 实现**（硬编码 relevance_score: 4），从未被 runtime 调用 |
| Abstention 可配置 min_score | "已完成" | ✅ |

### 1.2 文档遗漏/偏差项

| 设计项 | 文档说法 | 实际代码 |
|--------|---------|---------|
| **memory_store 不支持 category="procedure"** | "路径A部分可用" | ❌ **完全不可用**：`memory_store.rs` 的 `parse_category()` 只接受 `fact/preference/relation`，提交 `procedure` 直接报错 |
| **Generalization 模块** | 未提及 | ✅ 完整实现在 `consolidation/generalization.rs`（47KB），含 `run_generalization()` + `discover_patterns_llm()`，但**从未被 runtime 调用** |
| **Ambiguous 用户确认** | "未实现" | ⚠️ **部分实现**：`ambiguous.rs` 有 `should_trigger_confirmation()` + `generate_confirmation_hint()`，但从未被 runtime 注入管线调用 |
| **MetricsAggregator** | 未提及 | ⚌ 库代码完整（NRR/Abstention/Degradation alerting），但 runtime 从未实例化 |
| **ProceduralNode 字段偏差** | "已完成" | ❌ 缺 `source_skill`、`learned_from`、`activation_count`（用 `success_count`+`fail_count` 替代） |
| **detect_merge_candidates()** | "未在运行时调用" | ❌ **函数根本不存在**，设计文档 §3.2 要求的"主动假设验证"从未实现 |

### 1.3 校准后统计

| 分类 | 文档数据 | 校准后 |
|------|---------|--------|
| 已完成 | 5 | **4**（Judge 是 mock，不算"完成"） |
| 部分实现 | 6 | **8**（+Generalization 未接线、+Ambiguous 未接线、+MetricsAggregator 未接线、+ProceduralNode 字段缺失） |
| 未实现 | 7 | **6**（-3 移至部分实现） |
| **Phase 2 完成度** | **~40%** | **~30%**（大量库代码存在但未接线，实际可用功能更少） |

---

## 二、核心设计决策

### ADR-P2-001：memory_store category 扩展策略

**决策**：在现有 `fact/preference/relation` 基础上新增 `procedure`，而非创建独立工具。

**理由**：
- LLM 只需记住一个工具（`memory_store`），认知负担最低
- `procedure` 和其他 category 共享同一去重/冲突检测管线
- 向后兼容：旧 Agent 不使用 `procedure` 则无影响

**风险**：System Prompt 的提取指引需要新增 procedure 用法说明（~30 tokens 增量）

### ADR-P2-002：SkillFailure 提取路径（Path B）实现策略

**决策**：在 `agent_core.rs` 的 Skill 执行结果处理点（`execute_tool()` 返回 Error 时），新增轻量摘要逻辑，将 failure 信息写入 ProceduralNode。不引入独立的 SkillExperience 类型。

**理由**：
- 当前代码中不存在 `SkillExperience` 类型，引入全套类型成本高
- Skill 执行失败已有错误信息（`ToolError`），直接利用即可
- 完整的 `SkillExperience` + `failure_cases ≥3` 联动属于 Phase 3 离线巩固的深度分析，Phase 2 先做即时路径

**权衡**：放弃"failure_cases 积累后跨 Skill 提炼"的能力，但这条路径（Core 2）本就依赖离线巩固，Phase 2 做即时单次提取足够。

### ADR-P2-003：主动假设验证简化

**决策**：Phase 2 不实现完整的 `detect_merge_candidates()` + LLM 假设验证。改为在 `generalization.rs` 的 `detect_simple_patterns()` 中增加"同类 trigger_condition 多个 action_pattern"的检测 + 日志告警，Phase 3 再做 LLM 假设合并。

**理由**：
- 主动假设验证是"锦上添花"，不是 Phase 2 验收必须项
- 需要额外的 LLM 调用（成本），且触发条件（≥3 变体）在初期数据量少时几乎不会命中
- 先把基础三路径接通，再谈假设验证

### ADR-P2-004：Relationship 自动维护简化

**决策**：在 compaction/session-end 时检查"是否与同一用户对话超过 N 天"（基于 episode 时间跨度），超过则自动创建 Relationship 节点。不做独立的定时扫描。

**理由**：
- Session end 是已有触发点，无需新增后台任务
- 避免引入 30 天定时器等复杂调度逻辑
- 用户首次合作时间可从最早的 episode 推算

---

## 三、实施方案（4 Sub-phase）

### P0：基础对齐（解锁所有后续工作）

**目标**：让 ProceduralNode 从"数据结构存在"变为"可以被写入和检索"。

| # | 任务 | 涉及文件 | 依赖 |
|---|------|---------|------|
| P0-1 | memory_store 新增 `category="procedure"` | `tools/builtin/memory_store.rs` | 无 |
| P0-2 | `parse_category()` 新增 `Procedure` 分支 → `ProceduralSubType` | 同上 | P0-1 |
| P0-3 | `process_memory_store()` 新增 procedure 处理分支 → 调用 `store_procedural()` | `consolidation/instant.rs` | P0-2 |
| P0-4 | System Prompt 提取指引新增 procedure 用法 | `system_prompt.rs` 或 prompt 模板 | P0-1 |
| P0-5 | ProceduralNode struct 新增 `source_skill: Option<String>`、`learned_from: String`、`activation_count: u32` | `rollball-grafeo/src/types.rs` + `procedural.rs` | 无 |
| P0-6 | ProceduralNode `to_properties()` / `from_properties()` 对齐新字段 | `rollball-grafeo/src/semantic/procedural.rs` | P0-5 |
| P0-7 | 数据迁移：已有 ProceduralNode 的 `success_count+fail_count` → `activation_count`（默认 0） | `procedural.rs` 或 migration 脚本 | P0-5 |

**风险**：
- P0-1~P0-3 改动 `memory_store` 接口，所有使用该工具的 Agent 都会受影响。但只是**扩展**（新增枚举值），不影响现有 `fact/preference/relation` 路径
- P0-7 数据迁移需考虑已有 `.grafeo` 文件的向后兼容

### P1：ProceduralNode 三路径接线

**目标**：ProceduralNode 的三条来源路径端到端可工作。

| # | 任务 | 涉及文件 | 依赖 |
|---|------|---------|------|
| P1-1 | **Path A 接线**：`process_memory_store()` procedure 分支完整实现（去重→冲突→`store_procedural()`） | `consolidation/instant.rs` | P0-3 |
| P1-2 | **Path B 新增**：`agent_core.rs` Skill 执行失败时，提取 `(trigger_condition, action_pattern)` 写入 ProceduralNode，`learned_from = "execution_failure"` | `agent_core.rs` + `memory/manager.rs` | P0-5 |
| P1-3 | **Path C 接线**：将 `generalization.rs` 的 `run_generalization()` 接入运行时。在 compaction 后触发（如果 compaction 产出了足够多的 episode） | `loop_memory.rs` 或 `episode_distill.rs` | P0 |
| P1-4 | ProceduralNode 激活逻辑：检索时按 `trigger_condition` 匹配当前上下文，命中时 `activation_count += 1`，注入 System Prompt 行为准则区 | `memory/manager.rs` retrieve/inject | P0-5 |
| P1-5 | ProceduralNode 注入格式："当 [trigger_condition] 时，优先 [action_pattern]" | `memory/manager.rs` inject | P1-4 |
| P1-6 | 测试：三路径端到端测试 + 激活/注入测试 | `manager.rs` tests | P1-1~P1-5 |

**关键实现细节**：

Path B（Skill 执行失败）的具体切入点：
```
agent_core.rs execute_tool()
  → ToolError 返回时
  → 构造 ProceduralNode {
      trigger_condition: format!("使用 {} 工具时", tool_name),
      action_pattern: format!("避免 {}，替代方案: {}", error_pattern, suggestion),
      learned_from: "execution_failure",
      source_skill: Some(tool_name),
      confidence: 0.6,  // 失败经验置信度较低
    }
  → MemoryManager.record_procedural()
```

Path C（Generalization 接线）的关键决策：
- 触发时机：compaction 完成后，如果未巩固 episode 数量 > 5
- 不做独立后台任务，复用 compaction 触发链路
- `run_generalization()` 需要 LLM 调用（`discover_patterns_llm()`），使用 `select_cheapest_model()` 控制成本

### P2：自我认知 — 自传体记忆自动更新

**目标**：Agent 能从自身经验中认知能力边界，并自动维护关系记忆。

| # | 任务 | 涉及文件 | 依赖 |
|---|------|---------|------|
| P2-1 | **自我评估 → Limitation 节点**：compaction 结束时，统计近期 Skill 执行成功率（按 model+task_type 分组），成功率 < 60% 时创建/更新 `AutobiographicalNode { aspect: Limitation }` | `loop_memory.rs` + `autobiographical.rs` | P1-4（需要 activation_count 统计） |
| P2-2 | **Relationship 自动生成**：session-end 时，计算与当前用户的首次交互距今天数，超过 30 天则创建 `AutobiographicalNode { aspect: Relationship }` | `loop_memory.rs` + `autobiographical.rs` | 无 |
| P2-3 | **History 节点摘要压缩**：当 `AutobiographicalNode { aspect: History }` 超过 10 条时，离线巩固阶段将旧 History 合并为摘要节点，原始节点标记 Dormant | `consolidation/offline.rs` + `autobiographical.rs` | P1-3（复用 consolidation 触发） |
| P2-4 | History 压缩 Prompt 设计：LLM 输入多条 History，输出合并摘要（< 100 tokens） | 新增 prompt 模板 | P2-3 |
| P2-5 | 测试：自我评估 + Relationship + History 压缩 | `manager.rs` tests | P2-1~P2-4 |

**关键实现细节**：

P2-1（自我评估）的数据来源：
- 当前代码中没有 `SkillExperience.model_compatibility` 字段
- 替代方案：在 `MemoryManager` 中维护一个轻量的 `HashMap<(model_id, task_type), (success_count, fail_count)>`
- 每个 Agent 进程内维护，进程重启时从 episode 统计重建
- 成功率计算：`success / (success + fail)`，样本量 < 5 时不触发（避免噪声）

P2-3（History 压缩）的触发时机：
- 不做独立后台扫描，复用离线巩固触发条件
- `offline.rs` 中新增 `compress_history_nodes()` 步骤
- 合并策略：按时间窗口分组（同月内的 History 合并），LLM 生成摘要

### P3：可观测性 — 指标接入与质量门禁

**目标**：让记忆系统"看得见"，为 Phase 3 离线巩固提供数据基础。

| # | 任务 | 涉及文件 | 依赖 |
|---|------|---------|------|
| P3-1 | **MetricsAggregator 接线**：`MemoryManager::retrieve()` 完成后，将 `RetrievalMetrics` 喂入 `MetricsAggregator`，返回 `Vec<MetricsAlert>` | `memory/manager.rs` + `retrieval_metrics.rs` | P1 |
| P3-2 | **NRR/Abstention 告警日志**：当 alert 触发时，写入 Agent 日志（`tracing::warn!`），Desktop App 可通过日志流订阅 | `memory/manager.rs` | P3-1 |
| P3-3 | **Judge 实际实现**：替换 mock `evaluate_retrieval()`，用 `select_cheapest_model()` 做 LLM 评估（采样率 10%，仅评估 Top-3 结果） | `judge.rs` | P1 |
| P3-4 | **Ambiguous 确认流程接线**：当 `should_trigger_confirmation()` 返回 true 时，将 `generate_confirmation_hint()` 输出注入到下一轮的 System Prompt 中，引导 Agent 自然询问 | `memory/manager.rs` + `ambiguous.rs` | P1 |
| P3-5 | **LongMemEval 基准测试**：实现至少 IE + Abs 两个维度的实际评测用例（不再用硬编码分数） | `eval.rs` | P1 |
| P3-6 | 测试：指标聚合 + 告警 + 评测 | 各模块 tests | P3-1~P3-5 |

---

## 四、依赖关系与执行顺序

```
P0 (基础对齐)
 │
 ├── P1 (三路径接线) ← 依赖 P0 全部
 │    │
 │    ├── P2 (自我认知) ← 依赖 P1-4 (activation_count)
 │    │
 │    └── P3 (可观测性) ← 依赖 P1 全部
 │
 └── P2-2 (Relationship) 和 P2-3 (History 压缩) 可与 P1 并行
```

**可并行的任务组**：
- P0-5/P0-6/P0-7（ProceduralNode struct 对齐）可与 P0-1~P0-4（memory_store 扩展）并行
- P2-2（Relationship 自动生成）不依赖 P1，可在 P0 完成后立即开始
- P2-3（History 压缩）不依赖 P1，但需要 consolidation 触发链路

**关键路径**：P0 → P1-4（activation_count） → P2-1（自我评估） → P2 验收

---

## 五、不做的事（显式排除）

| 排除项 | 理由 | 移至 |
|--------|------|------|
| `SkillExperience` 独立类型 + 完整 failure_cases 联动 | 成本高，依赖离线巩固的深度分析 | Phase 3 |
| `detect_merge_candidates()` + LLM 假设验证 | 触发条件苛刻（≥3 变体），初期数据量不足以命中 | Phase 3 |
| `MemoryMiddleware` trait + 洋葱模型 | 设计 §10.5 标注 Phase 2，但实际收益低（目前只有 2 个内置中间件候选），架构成本高 | Phase 3+ |
| 跨 Agent 知识共享（Intent 查询 / ContentProvider） | 依赖 Intent 系统的成熟度，当前仅 `intent_send` 可用 | Phase 4+ |
| `RemoteMemoryStore` / 云端同步 | Phase 5+ 范围 | Phase 5+ |
| WASM 中间件 | Phase 3+ 设计预留 | Phase 3+ |
| 完整 LongMemEval 5 维评测 | IE + Abs 足以验证 Phase 2 质量，其他维度需 Phase 3 离线巩固 | Phase 3 |
| `Event Bus` for MemoryEvent | Desktop App 订阅机制未设计 | Phase 3+ |

---

## 六、验收标准

### 功能验收

| # | 验收项 | 验证方法 | 通过标准 |
|---|--------|---------|---------|
| 1 | memory_store 支持 category="procedure" | 构造 procedure 类型 tool call | ProceduralNode 被创建并存储 |
| 2 | Skill 执行失败自动提取 ProceduralNode | 构造失败 Skill 调用 | ProceduralNode.learned_from = "execution_failure" |
| 3 | Generalization 模块被运行时调用 | compaction 后检查日志 | `run_generalization()` 被执行 |
| 4 | ProceduralNode 激活 + 注入 | 创建 ProceduralNode 后触发匹配上下文的检索 | System Prompt 包含行为准则 |
| 5 | 成功率 <60% → Limitation 节点 | 模拟多次失败执行 | AutobiographicalNode { aspect: Limitation } 被创建 |
| 6 | 30 天合作 → Relationship 节点 | 模拟跨越 30 天的 session | AutobiographicalNode { aspect: Relationship } 被创建 |
| 7 | History >10 → 摘要压缩 | 插入 11 条 History 后触发 consolidation | 旧 History 转为 Dormant，新增摘要节点 |
| 8 | NRR 告警触发 | 连续低质量检索 | tracing::warn 输出告警 |
| 9 | Ambiguous 确认注入 | 累计 ≥3 个 ambiguous 节点 | 下一轮 System Prompt 包含确认提示 |

### 质量门禁

| 指标 | 目标 |
|------|------|
| LongMemEval IE 维度 | ≥ 70% |
| LongMemEval Abs 维度 | ≥ 60% |
| Phase 1 功能回归 | 全部测试通过 |
| ProceduralNode 端到端延迟（P99） | < 50ms（不含 LLM 调用） |

---

## 七、文档修正建议

基于验证结果，`21-memory-design-vs-impl-gap-analysis.md` 需要以下修正：

1. **§3.2 ProceduralNode 三条来源路径**：Path A 应标记为"未实现"（非"部分可用"），`memory_store` 不支持 `procedure` category
2. **§3.2 主动假设验证**：`detect_merge_candidates()` 不存在（非"未在运行时调用"），应改为"函数未实现"
3. **§3.1 Judge 轻量评估**：应标记为"骨架/Mock"而非"已完成"
4. **§3.2 补充**：新增 Generalization 模块（47KB 完整实现但未接线）、MetricsAggregator（完整实现但未接线）、Ambiguous 确认流程（部分实现但未接线）
5. **§五 MemoryMiddleware**：trait 不存在于任何 crate 中，应修正"trait 定义在 rollball-memory 但未使用"的说法

---

## 八、风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|---------|
| memory_store category 扩展导致旧 Agent prompt 不兼容 | 现有 Agent 的 System Prompt 指引不含 procedure 用法，LLM 可能不调用 | 低 | 新增字段是扩展不是破坏，旧 Agent 行为不变；新指引仅 ~30 tokens |
| Path B Skill 失败提取可能误报 | 技术性错误（网络超时）被误记为 ProceduralNode | 中 | `confidence=0.6` + 需要 ≥2 次类似失败才创建正式节点 |
| Generalization 接线引入额外 LLM 调用成本 | `discover_patterns_llm()` 需要便宜模型调用 | 低 | 使用 `select_cheapest_model()` + 仅在有足够 episode 时触发 |
| History 压缩丢失细节 | 多条 History 合并后原始信息可能被 LLM 摘要丢失 | 中 | 原始节点标记 Dormant 而非 Purge，30 天内可恢复 |
| ProceduralNode 字段迁移 | 已有 `.grafeo` 文件中旧格式 ProceduralNode | 低 | `from_properties()` 对缺失字段使用默认值（activation_count=0, learned_from="unknown"） |
