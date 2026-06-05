# Memory Phase 2: P3 可观测性 — 实施完成

## 概述

Phase 2 Memory 系统的 P3（可观测性）子阶段已全部实现。记忆系统现在"看得见"——检索质量被追踪、告警被记录、LLM Judge 采样评估、模糊冲突自动注入确认提示，LongMemEval 使用真实 Grafeo store 操作。

## 变更摘要

### P3-1: MetricsAggregator 接线 — ✅ 完成

| 组件 | 变更 |
|------|------|
| `AgentCore` | 新增 `metrics_aggregator: Mutex<MetricsAggregator>` 字段 |
| `loop_memory.rs` | `retrieve_and_inject_memories()` 中：RetrievalMetrics → OnlineRetrievalMetrics 类型转换 → `record_retrieval()` |
| `retrieval_metrics.rs` | 新增 `max_possible_score()`, `set_max_possible_score()` 访问器 |

### P3-2: NRR/Abstention 告警日志 — ✅ 完成

| 告警类型 | 日志级别 | 含义 |
|----------|---------|------|
| `LowNrr` | warn | NRR 持续低于阈值 — 检查 embedding 模型或索引 |
| `HighAbstentionRate` | warn | 弃用率过高 — 考虑降低 min_score |
| `LowAbstentionRate` | warn | 弃用率过低 — min_score 可能太低 |
| `LowConflictAccuracy` | warn | 冲突解决准确率低于阈值 |
| `HighDegradationRate` | warn | 检索降级频率过高 |

### P3-3: LLM Judge 实际实现 — ✅ 完成

| 组件 | 变更 |
|------|------|
| `judge_llm.rs` (新增) | `evaluate_retrieval_llm()`: 用 Provider 做 LLM 评估，单轮 prompt，返回 1-5 分 |
| `loop_memory.rs` | 确定性采样 10%，`tokio::spawn` 后台评估，不阻塞检索管线 |
| 6 个 parse 测试 | 数字/文本前缀/越界钳位/乱码回退 |

### P3-4: Ambiguous 确认流程接线 — ✅ 完成

| 组件 | 变更 |
|------|------|
| `ContextBuilder` | 新增 `ambiguous_confirmation_hint: Option<String>` 字段 |
| `loop_memory.rs` | `should_trigger_confirmation()` → `generate_confirmation_hint()` → `set_ambiguous_confirmation_hint()` |
| `context.rs build()` | Section 2.5b: 注入 `## Memory Conflicts Needing Confirmation` |

### P3-5: LongMemEval IE+Abs 真实评测 — ✅ 完成

| 维度 | 之前 | 之后 |
|------|------|------|
| IE (Information Extraction) | 硬编码 70.0 | 真实 Grafeo store: 5 测试用例 (4 正面 + 1 负面) |
| Abs (Abstraction) | 硬编码 65.0 | 真实 Grafeo store: 3 测试用例 (dedup/procedural/autobiographical) |
| MR/TR/KU | 硬编码 | 保持 placeholder (Phase 3) |

## 关键修改文件

### runtime crate
- `agent/agent_core.rs`: `metrics_aggregator` 字段 + 构造函数 + `clone_for_session()`
- `agent/loop_memory.rs`: P3-1 (MetricsAggregator), P3-2 (告警日志), P3-3 (LLM Judge spawn), P3-4 (Ambiguous hint)
- `agent/context.rs`: `ambiguous_confirmation_hint` 字段 + setter + build() 注入
- `memory/judge_llm.rs`: 新文件 — LLM Judge 实现 + 6 测试
- `memory/mod.rs`: 注册 `judge_llm` 模块

### grafeo crate
- `retrieval_metrics.rs`: `max_possible_score()`, `set_max_possible_score()` 访问器
- `eval.rs`: `eval_information_extraction()`, `eval_abstraction()` 真实实现

## 测试结果
- rollball-grafeo: **263 tests pass** (含 IE/Abs 评测)
- rollball-runtime: **32 memory tests pass** + **6 judge_llm tests pass**

## 验收状态

| # | 验收项 | 状态 |
|---|--------|------|
| 8 | NRR 告警触发 | ✅ tracing::warn 输出 |
| 9 | Ambiguous 确认注入 | ✅ ≥3 冲突 → System Prompt 包含提示 |

## 已知后续事项
1. MetricsAggregator 当前按 session 独立，跨 session 聚合需 Desktop App 日志订阅
2. LLM Judge 评估结果尚未反馈到 MetricsAggregator（需 record_retrieval 后额外 record_judge）
3. MR/TR/KU 三个维度仍为 placeholder，需 Phase 3 离线巩固数据
