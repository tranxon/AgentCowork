# rollball-grafeo — Grafeo 图数据库引擎

**定位**：Agent 私有 Memory 的存储引擎，支撑经历层（情景记忆）和沉淀层（语义/程序/自传体记忆）。每个 Agent Runtime 进程内嵌一个 Grafeo 实例。

```
crates/rollball-grafeo/
├── Cargo.toml
└── src/
    ├── lib.rs                     # 公共 API 入口
    ├── grafeo.rs                  # Grafeo 主结构（open/close/query）
    ├── schema.rs                  # 数据库表结构定义
    ├── episodic/
    │   ├── mod.rs                 # 经历层：情景记忆
    │   ├── store.rs               # 写入交互记录
    │   ├── search.rs              # 语义相似性检索（HNSW）
    │   └── consolidate.rs         # 巩固标记与清理
    ├── semantic/
    │   ├── mod.rs                 # 沉淀层：长期记忆
    │   ├── knowledge.rs           # KnowledgeNode（事实/偏好/关系）
    │   ├── procedural.rs          # ProceduralNode（行为模式/操作规则）
    │   ├── autobiographical.rs    # AutobiographicalNode（自我认知）
    │   ├── graph.rs               # LPG 图操作（节点/边/属性）
    │   ├── inference.rs           # 知识推理与合并
    │   └── skill.rs               # Skill 经验节点（Draft/Iteration/Execution/Experience）
    ├── consolidation/
    │   ├── mod.rs                 # 巩固管道
    │   ├── instant.rs             # 即时提取执行层（处理 memory_store 工具调用，去重、写入、标记 consolidated）
    │   └── offline.rs             # 离线巩固（空闲时回放 + 专用 LLM 调用，Phase 3）
    ├── forgetting/
    │   ├── mod.rs                 # 遗忘机制
    │   ├── decay.rs               # 三因子衰减计算（importance + access + recency）
    │   └── scan.rs                # 后台衰减扫描与状态转换
    ├── retrieval/
    │   ├── mod.rs                 # 检索入口
    │   ├── hybrid_search.rs       # 混合搜索（向量 + 全文 + RRF）
    │   ├── graph_expand.rs        # 关联扩散（LPG 图上 1-2 跳扩展）
    │   └── rrf.rs                 # Reciprocal Rank Fusion 排序
    ├── fulltext/
    │   ├── mod.rs                 # 全文检索
    │   └── bm25.rs                # BM25 倒排索引
    ├── embedding/
    │   ├── mod.rs                 # Embedding 生成抽象
    │   ├── local.rs               # ONNX Runtime 本地生成（all-MiniLM-L6-v2）
    │   └── remote.rs              # 远程 embedding API（可选）
    ├── vector/
    │   ├── mod.rs                 # 向量索引抽象
    │   └── hnsw.rs                # HNSW 索引实现（rusqlite + 自定义）
    ├── migration.rs               # 数据库版本迁移
    └── error.rs                   # 错误类型
```

## 关键 API

```rust
pub struct Grafeo {
    db: rusqlite::Connection,
    embedding: Box<dyn EmbeddingProvider>,
}

impl Grafeo {
    /// 打开 Grafeo 实例（每个 Agent 独立文件）
    pub fn open(path: &Path, embedding: Box<dyn EmbeddingProvider>) -> Result<Self>;

    // ── 经历层：情景记忆 ──

    /// 写入交互片段（自动分类内容类型，工件性内容压缩为摘要 + artifact_refs）
    /// 分类方式：Tool Call 结果按工具类型模板提取；Agent 回复用 Markdown 正则分离代码块
    /// 零 LLM 调用，纯 Runtime 确定性逻辑
    pub fn store_episode(&self, episode: &Episode) -> Result<()>;

    /// 语义相似性检索
    pub fn search_episodes(&self, query: &str, limit: usize) -> Result<Vec<Episode>>;

    /// 标记情景已巩固
    pub fn mark_consolidated(&self, episode_ids: &[&str]) -> Result<()>;

    /// 清理已巩固且过期的情景
    pub fn cleanup_episodes(&self, older_than_days: u32) -> Result<u64>;

    // ── 沉淀层：语义记忆 ──

    /// 写入/更新知识节点（Fact 自动语义去重）
    /// 如果 (subject, predicate) 已存在且 object 一致，更新 confidence 而非创建新节点
    /// 如果 object 不同，创建新节点并将旧节点标记 Dormant（知识更新）
    pub fn store_knowledge(&self, node: &KnowledgeNode) -> Result<()>;

    /// 写入/更新程序记忆节点
    pub fn store_procedural(&self, node: &ProceduralNode) -> Result<()>;

    /// 写入/更新自传体记忆节点
    pub fn store_autobiographical(&self, node: &AutobiographicalNode) -> Result<()>;

    /// 图查询（支持按类型过滤）
    pub fn query_knowledge(&self, query: &str, node_type: Option<NodeTypeFilter>) -> Result<Vec<MemoryNode>>;

    // ── 沉淀层：Skill 经验 ──

    pub fn get_skill_experience(&self, skill_id: &str) -> Result<Option<SkillExperience>>;
    pub fn update_skill_experience(&self, experience: &SkillExperience) -> Result<()>;
    pub fn store_skill_draft(&self, draft: &SkillDraft) -> Result<()>;
    pub fn store_skill_iteration(&self, iteration: &SkillIteration) -> Result<()>;
    pub fn store_skill_execution(&self, execution: &SkillExecution) -> Result<()>;
    pub fn get_skill_draft(&self, draft_id: &str) -> Result<SkillDraft>;
    pub fn get_skill_iterations(&self, draft_id: &str) -> Result<Vec<SkillIteration>>;
    pub fn get_skill_executions(&self, iteration_id: &str) -> Result<Vec<SkillExecution>>;

    // ── 检索 ──

    /// 混合搜索：融合向量 + 全文检索
    pub fn hybrid_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;

    /// 关联扩散检索：hybrid_search + 图上 1-2 跳扩展
    /// 经历层 episode 通过 source_episode 反向查询沉淀层关联节点
    /// expand_hops 内部 clamp 到 0..=2（超过 2 自动截断，不返回错误）
    pub fn search_with_expansion(&self, query: &str, limit: usize, expand_hops: u8) -> Result<Vec<SearchResult>>;

    // ── 遗忘 ──

    /// 计算并更新所有 Active 节点的 decay_score（乘法模型）
    /// activity_signal = clamp(exp(-λ × days) + min(cap, access × per_hit), floor, 1.0)
    /// decay_score = importance × activity_signal
    pub fn run_decay_scan(&self) -> Result<DecayScanResult>;

    /// 将 Dormant 节点恢复为 Active（被重新引用时）
    /// 清除 dormant_since，access_count += 1，更新 last_accessed
    pub fn reactivate_node(&self, node_id: &str) -> Result<()>;

    /// 执行 Purge 流程（Dormant > 90 天的 Preference/Procedural）
    /// Purge 前检查关联 Active 节点，转移 source_episode 引用
    pub fn purge_expired_dormant(&self) -> Result<PurgeResult>;

    // ── 巩固管道 ──

    /// 获取未巩固的情景记忆（供即时/离线巩固使用）
    pub fn get_unconsolidated_episodes(&self, min_importance: f32, limit: usize) -> Result<Vec<Episode>>;
}

/// 遗忘衰减扫描结果
pub struct DecayScanResult {
    pub active_count: u32,
    pub newly_dormant: u32,
    pub purge_candidates: u32,    // Dormant > 90 天的 Preference/Procedural
}

/// Purge 结果
pub struct PurgeResult {
    pub purged_count: u32,
    pub merged_count: u32,        // Purge 前转移 source_episode 引用
}

/// 检索结果（支持直接匹配 + 扩展节点）
pub struct SearchResult {
    pub node: MemoryNode,
    pub score: f32,
    pub source: ResultSource,        // DirectMatch / GraphExpansion { hops, path_weight }
}

pub enum ResultSource {
    DirectMatch,
    GraphExpansion { hops: u8, path_weight: f32 },
}
```

## 数据库 Schema（核心表）

```sql
-- 经历层：情景记忆
CREATE TABLE episodes (
    episode_id   TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL,
    timestamp    DATETIME NOT NULL,
    role         TEXT NOT NULL,          -- user / agent / tool
    content      TEXT NOT NULL,         -- 信息性内容原样存储；工件性内容仅存摘要
    content_type TEXT DEFAULT 'Informational',  -- Informational / Artifact / Structural
    embedding    BLOB,                  -- f32 向量（基于 content 而非原始代码生成）
    importance   REAL DEFAULT 0.5,
    consolidated BOOLEAN DEFAULT FALSE,
    metadata     TEXT,                  -- JSON
    artifact_refs TEXT                  -- JSON Array，ArtifactRef[]（仅 Artifact 类型有值）
);

-- 经历层：工件引用（可选，独立表便于查询）
CREATE TABLE artifact_refs (
    ref_id       TEXT PRIMARY KEY,
    episode_id   TEXT NOT NULL REFERENCES episodes(episode_id),
    path         TEXT NOT NULL,         -- 文件路径
    hash         TEXT NOT NULL,         -- 内容 sha256
    description  TEXT NOT NULL,         -- LLM 生成的 1-3 句摘要
    line_start   INTEGER,              -- 涉及的行范围（起始）
    line_end     INTEGER,              -- 涉及的行范围（结束）
    modified_at  DATETIME              -- 文件最后修改时间
);

-- 沉淀层：统一节点表
CREATE TABLE memory_nodes (
    node_id      TEXT PRIMARY KEY,
    node_type    TEXT NOT NULL,          -- Knowledge / Procedural / Autobiographical / SkillDraft / SkillIteration / SkillExecution / SkillExperience
    sub_type     TEXT,                   -- Knowledge: Fact/Preference/Relation; Autobiographical: Identity/Capability/Limitation/Preference/History/Relationship
    content      TEXT NOT NULL,          -- JSON，按 node_type 不同结构不同
    created_at   DATETIME NOT NULL,
    updated_at   DATETIME NOT NULL,
    status       TEXT DEFAULT 'Active',  -- Active / Dormant / Purged（Autobiographical 强制 Active）
    dormant_since DATETIME,              -- 进入 Dormant 的时间（Purge 90 天计时起点）

    -- 遗忘字段（Autobiographical 不使用）
    importance   REAL DEFAULT 0.5,
    access_count INTEGER DEFAULT 0,
    last_accessed DATETIME,
    decay_score  REAL DEFAULT 1.0,

    -- 隐私
    privacy      TEXT DEFAULT 'Personal' -- Public / Personal / Sensitive
);

-- 沉淀层：关系边
CREATE TABLE memory_edges (
    edge_id      TEXT PRIMARY KEY,
    source_id    TEXT NOT NULL REFERENCES memory_nodes(node_id),
    target_id    TEXT NOT NULL REFERENCES memory_nodes(node_id),
    relation     TEXT NOT NULL,          -- LIVES_IN / PREFERS / MANAGED_BY / ...
    weight       REAL DEFAULT 1.0,
    created_at   DATETIME NOT NULL,
    source_episode TEXT                   -- 来源情景 ID
);

-- 向量索引（HNSW，由 rusqlite 扩展或自定义实现）
-- 全文索引（BM25，由 rusqlite FTS5 实现）
```

## 设计决策

- 基于 `rusqlite`（与 ZeroClaw 一致），避免额外数据库依赖
- HNSW 向量索引：初期用纯 Rust 实现或 `instant-distance` crate，不依赖外部服务
- ONNX Runtime 是可选依赖（feature flag `local-embeddings`），不可用时退化为远程 API
- 数据库文件路径：`<agent_workspace>/memory/private.grafeo`
- 遗忘扫描在 Agent 空闲时后台运行，不阻塞正常检索
- 关联扩散深度硬限制 2 跳，每跳 Top-5，总扩展上限 20 节点
- 巩固管道的即时提取通过 Tool Call 机制（memory_store 工具）实现，LLM 自主判断是否调用，无额外 API 调用开销。离线巩固 Phase 3 使用专用 LLM 调用
- Fact 节点写入时自动语义去重（按 subject+predicate 匹配）
- Episode 写入时自动分类内容类型：信息性内容原样存储，工件性内容（代码/文件/命令输出）压缩为摘要 + artifact_refs，避免 Grafeo 膨胀

## 依赖

- `rusqlite` — 存储引擎
- `serde`, `serde_json` — 数据序列化
- `tokio` — 异步封装
- `ort` (feature-gated) — ONNX Runtime

## Feature Flags

```toml
[features]
default = []
local-embeddings = ["dep:ort"]     # 本地 embedding（增加 ~50MB 编译体积）
```
