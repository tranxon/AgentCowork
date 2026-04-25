# Phase 3: 权限与沙箱 — 实施计划

> 版本：v1.1 | 更新日期：2026-04-25
> 前置条件：Phase 2（M5~M9）全部完成
> 预计周期：15~19 周
> 预计里程碑：M10~M15

---

## 背景与目标

Phase 1 交付了 MVP 执行链路，Phase 2 交付了 Grafeo 仿生记忆 + System Agent + Intent 路由 + 多 Provider。当前安全能力仅为策略级隔离（Runtime 路径白名单检查），存在 shell 子进程越权、WASM 工具仅有声明无沙箱执行、无运行时权限请求机制等缺口。

Phase 3 的核心价值：从"能运行"升级到"安全运行"——

1. **OS 级进程隔离**：内核强制文件系统/网络隔离，子进程也无法绕过
2. **WASM 沙箱完整实现**：Wasmtime 集成 + WIT 组件模型升级
3. **权限框架**：声明 + 授权 + 运行时请求 + Approval Gate 全链路
4. **Shell 安全分级**：FileProvenance + ShellRisk + 命令-文件关联分析
5. **离线巩固**：Phase 2 的即时提取对应的离线批量处理（三元组提取 + 经验泛化）
6. **质量评估**：LongMemEval 集成 + 检索质量可观测

---

## 阶段划分

### S1：权限声明与授权框架（3 周，8 项任务）

Phase 2 的 `rollball-core` 已定义 `Permission` 类型，但缺少完整的授权链路。S1 建立从声明到校验的完整权限框架。

**涉及 crate**：`rollball-core`、`rollball-gateway`、`rollball-runtime`

| 任务 | 内容 | 预期测试 | 验收标准 |
|------|------|---------|---------|
| S1.1 权限模型完善 | 扩展 `Permission` 枚举覆盖全部权限类型（filesystem/network/memory/intent/identity/shell/wasm）；定义 `PermissionGrant`（授权记录：who/what/when/scope）；定义 `PermissionPolicy`（默认/允许/拒绝/每次询问） | 12 | Permission 序列化/反序列化、grant 匹配逻辑 |
| S1.2 权限持久化存储 | Gateway 侧 `PermissionStore`（rusqlite），存储用户对每个 Agent 的授权决策；支持 CRUD + 按 agent_id 查询 | 8 | 存储/查询/撤销/过期授权 |
| S1.3 安装时权限审查 | Package Manager 安装流程增加权限审查步骤：解析 manifest 权限声明 → 对比已授权权限 → 新权限需用户确认 → 记录授权 | 6 | 安装时弹出权限列表、拒绝安装、部分授权 |
| S1.4 运行时权限校验器 | Runtime 侧 `PermissionChecker`：工具执行前校验权限，缓存已授权权限（启动时从 Gateway 获取）| 8 | 工具调用被权限拦截、缓存命中/miss |
| S1.5 运行时权限请求 | Runtime → Gateway `PermissionRequest` 消息类型；Gateway 向用户展示请求（CLI 交互 / 未来 Desktop App 弹窗）；用户响应后更新 PermissionStore 并回传 | 6 | 运行时请求 → 用户确认 → 授权生效 |
| S1.6 权限升级通知 | Agent 升级时，新版 manifest 新增权限 → Package Manager 检测差异 → 用户确认新增权限 | 4 | 升级检测权限差异、用户确认流程 |
| S1.7 权限撤销与重置 | Gateway CLI 命令 `rollball permission revoke/reset`；撤销后 Runtime 缓存失效机制 | 4 | CLI 撤销、Runtime 感知 |
| S1.8 权限集成测试 | 端到端：安装带权限声明的 Agent → 授权 → 运行时校验 → 运行时请求 → 撤销 | 6 | 全链路通过 |

**里程碑 M10：权限框架可用** — 安装时权限审查 + 运行时权限校验 + 运行时权限请求 全链路打通

预期测试合计：54 项

---

### S2：进程级沙箱隔离（5 周，8 项任务）

实现 OS 级强制隔离，对应 `docs/08-security.md` §2.2 和 §8 的设计。Phase 3 覆盖 Linux（bubblewrap）和 Windows（AppContainer + Job Objects），macOS（Seatbelt）留给 Phase 7。

**涉及 crate**：新建 `rollball-sandbox`（独立 crate，跨平台沙箱抽象）、`rollball-gateway`

| 任务 | 内容 | 预期测试 | 验收标准 |
|------|------|---------|---------|
| S2.1 沙箱抽象层 | 定义 `Sandbox` trait：`fn spawn(config: SandboxConfig) -> Result<SandboxedProcess>`；`SandboxConfig`：allowed_paths（读/写/exec）、network_policy（deny_all / allow_list）、resource_limits（memory_mb/cpu_time_s/pids）；平台检测自动选择实现 | 8 | trait 定义 + Config 序列化 + 平台路由 |
| S2.2 Linux bubblewrap 集成 | `BubblewrapSandbox` 实现：构建 bwrap 命令行参数（--ro-bind / --bind / --dev /proc / --unshare-net / --seccomp）；从 manifest 权限声明自动推导挂载点 | 10 | bwrap 参数生成、Agent 在沙箱内正常运行 |
| S2.3 Linux seccomp-bpf 过滤器 | 定义 seccomp 白名单策略（允许基本 syscall + 读写 / 禁止 clone/ptrace/mount 等）；BPF 规则与 SandboxConfig 联动 | 8 | 危险 syscall 被拒绝、正常工具执行不受影响 |
| S2.4 Windows AppContainer 集成 | `AppContainerSandbox` 实现：创建 AppContainer profile、设置 Capability SID、文件系统 ACL 配置；Job Objects 资源限制（memory/cpu/process count） | 10 | Windows 沙箱创建、Agent 在 AppContainer 内运行 |
| S2.5 网络隔离（跨平台） | Linux：默认 `--unshare-net` + slirp4netns 白名单；Windows：AppContainer 网络隔离 + Windows Firewall 规则（按 manifest 网络权限）| 8 | 无网络权限时请求失败、有权限时白名单通过（双平台） |
| S2.6 资源限制（跨平台） | Linux：cgroups v2 或 rlimit（memory_mb/cpu_time_s/max_pids）；Windows：Job Objects（ProcessMemoryLimit/ProcessUserTimeLimit/ActiveProcessLimit）；超限信号处理 | 8 | 内存超限被杀、CPU 超时被杀（双平台） |
| S2.7 沙箱配置器集成 | Gateway Lifecycle Manager 启动 Agent 时，根据 manifest 和 PermissionStore 组装 SandboxConfig → 平台检测 → 调用对应 Sandbox::spawn 替代裸 Command::new | 8 | Gateway 自动沙箱化启动 Agent（双平台） |
| S2.8 沙箱降级策略 | 沙箱依赖不可用时（bwrap 未安装 / AppContainer 不支持）：日志警告 + 回退到策略级隔离（Phase 1 行为）；`--no-sandbox` 开发者模式标志 | 4 | 降级日志、开发者模式正常运行 |

**里程碑 M11：双平台沙箱可用** — Agent 进程在 Linux bubblewrap / Windows AppContainer 沙箱内运行，文件系统/网络/资源受限

预期测试合计：64 项

---

### S3：WASM 工具沙箱（3 周，6 项任务）

Phase 1 声明了 WASM 工具格式，Phase 3 实现完整的 Wasmtime 集成和 WIT 组件模型升级（对应 `docs/12-tool-system.md` §3 设计）。

**涉及 crate**：`rollball-runtime`（新建 `tools/wasm/` 模块）

| 任务 | 内容 | 预期测试 | 验收标准 |
|------|------|---------|---------|
| S3.1 Wasmtime 引擎集成 | 引入 wasmtime crate（LTS 版本）；`WasmEngine` 单例管理：预编译缓存、Cranelift 配置、Fuel metering 启用 | 6 | Engine 初始化、模块编译、Fuel 计量 |
| S3.2 WASM 实例管理 | `WasmToolInstance`：加载 .wasm → 编译 → 实例化 → 执行 → 销毁；线性内存分配/回收；execute() Host 函数注册 | 10 | 工具加载执行、内存限制、超时终止 |
| S3.3 WASI Preview 2 权限映射 | manifest 权限 → WASI capability 映射：filesystem 权限 → WASI 目录预开放（preopens）；network 权限 → WASI socket 能力 | 8 | 权限映射正确、越权访问被拒绝 |
| S3.4 WIT 组件模型升级 | 定义 `rollball-tool.wit` 接口（替代 Phase 1 的手动 Host 函数）；工具输入/输出类型安全绑定；向后兼容 Phase 1 的 execute(ptr, len) 方式 | 8 | WIT 接口调用成功、旧格式兼容 |
| S3.5 rollball-tool-sdk | 发布 `rollball-tool-sdk` crate（wasm32-wasip2 目标）：`#[tool]` proc macro、自动 JSON 序列化、schema 导出 | 6 | SDK 示例工具编译运行 |
| S3.6 WASM 工具集成测试 | 编写示例 WASM 工具（image_filter）；端到端：LLM tool_call → WASM 沙箱执行 → 结果返回 | 4 | 示例工具在沙箱中执行成功 |

**里程碑 M12：WASM 沙箱可用** — WASM 工具在 Wasmtime 沙箱中隔离执行，支持 WIT 组件模型

预期测试合计：42 项

---

### S4：Shell 安全分级与 Approval Gate（3 周，7 项任务）

对应 `docs/08-security.md` §11 的完整设计：FileProvenance + ShellRisk + Approval Gate。

**涉及 crate**：`rollball-runtime`（`tools/builtin/shell.rs` 增强 + 新建 `security/` 模块）、`rollball-gateway`

| 任务 | 内容 | 预期测试 | 验收标准 |
|------|------|---------|---------|
| S4.1 FileProvenance 文件来源追踪 | `FileProvenance` 结构体 + `FileSource` 枚举（CreatedByTool/Downloaded/PreExisting/Unknown）；工作区启动扫描；file_write/network_fetch/shell 执行后自动更新来源 | 10 | 来源记录正确、shell 后新文件标记 Unknown |
| S4.2 ShellRisk 风险分级引擎 | `ShellRisk` 四级分类（Low/Medium/High/Blocked）；命令解析器（提取可执行文件路径、检测危险模式）；基础命令白名单/黑名单 | 12 | 各等级命令分类正确、边界 case |
| S4.3 命令-文件关联分析 | `assess_shell_risk()` 函数：base_risk + FileProvenance 交叉查询；Downloaded/Unknown 文件被执行时提升到 High | 8 | 下载文件执行 → High、已知文件执行 → 维持原级 |
| S4.4 Approval Gate | `ApprovalGate` trait + CLI 实现：Medium/High 风险工具执行前暂停 → 向用户展示风险信息 → 等待确认/拒绝/始终允许；Gateway 侧 Approval 消息路由 | 8 | 暂停等待确认、拒绝后工具返回错误 |
| S4.5 工作区文件系统监控 | `FsWatcher`：inotify（Linux）/ FSEvents（macOS）/ ReadDirectoryChangesW（Windows）抽象层；监控新文件创建、权限变更、符号链接；异常模式检测 | 8 | 新可执行文件检测、符号链接告警 |
| S4.6 审计日志 | Shell 执行审计记录（command/risk_level/reason/approved_by/exit_code/files_created/files_modified）；结构化 JSON 日志写入 Agent 工作目录 | 4 | 审计记录完整、可回溯 |
| S4.7 Shell 安全集成测试 | 端到端场景：下载文件 → 尝试执行 → Approval Gate 拦截 → 确认后执行 → 审计日志 | 4 | 完整攻击场景拦截 |

**里程碑 M13：Shell 安全可用** — Shell 命令风险分级 + Approval Gate + 审计日志 全链路打通

预期测试合计：54 项

---

### S5：离线巩固与记忆质量（3 周，6 项任务）

Phase 2 实现了"即时提取"（每轮 tool_call），S5 实现对应的"离线巩固"——批量处理积累的 Episode，提取三元组、发现模式、经验泛化。同时建立记忆质量评估框架。

**涉及 crate**：`rollball-grafeo`、`rollball-runtime`

| 任务 | 内容 | 预期测试 | 验收标准 |
|------|------|---------|---------|
| S5.1 巩固调度器 | `ConsolidationScheduler`：定时触发（可配置间隔，默认 idle 30min 后）+ 手动触发 API；批次管理（未巩固 Episode 队列 → 分批处理 → 进度跟踪）| 6 | 定时触发、批次处理、进度持久化 |
| S5.2 三元组提取 | LLM 驱动的三元组提取：Episode 文本 → (subject, predicate, object) 三元组列表；去重（语义相似度阈值）；写入沉淀层 KnowledgeNode | 10 | 提取准确率、去重逻辑、写入正确 |
| S5.3 冲突分类与证据验证 | 新三元组与已有知识冲突时：Evolution（知识更新）/ Correction（错误修正）/ Ambiguous（保留双方）自动分类；LLM 证据验证 | 8 | 三种冲突类型分类正确、证据链记录 |
| S5.4 经验泛化（模式提炼） | 从多个 Episode 中提炼 ProceduralNode：识别重复行为模式 → 抽象为通用步骤 → 关联到 Skill；泛化置信度评估 | 8 | 模式识别、ProceduralNode 生成 |
| S5.5 质量评估框架 | `RetrievalMetrics`：precision@k、recall@k、MRR；LongMemEval 5 维评测（信息提取/跨会话推理/时序推理/知识更新/信息否定）集成 | 8 | 评测脚本可运行、基线指标记录 |
| S5.6 巩固集成测试 | 端到端：10 轮对话 Episode 积累 → 触发巩固 → 三元组提取 → 冲突处理 → 经验泛化 → 质量评估 | 4 | 巩固后检索质量提升可量化 |

**里程碑 M14：离线巩固可用** — Episode 批量巩固 + 三元组提取 + 经验泛化 + 质量评估

预期测试合计：44 项

---

### S6：集成验证与安全审计（2 周，4 项任务）

全系统集成验证，确保 S1~S5 各模块协同工作。

| 任务 | 内容 | 预期测试 | 验收标准 |
|------|------|---------|---------|
| S6.1 权限 + 沙箱联动 | manifest 权限 → Gateway 授权 → SandboxConfig → bwrap 挂载点，权限变更实时反映到沙箱策略 | 6 | 权限撤销后沙箱路径不可访问 |
| S6.2 WASM + 权限联动 | WASM 工具权限校验 → Wasmtime WASI capability 映射 → 越权访问拒绝 | 4 | WASM 工具越权被阻止 |
| S6.3 Shell + 沙箱联动 | bwrap 沙箱内 shell 执行 → FileProvenance 追踪 → ShellRisk 分级 → Approval Gate | 4 | 沙箱内 shell 安全链路完整 |
| S6.4 安全红队测试 | 模拟攻击场景：Prompt Injection → 恶意 shell → 文件越权 → 网络渗透；验证纵深防御各层有效 | 6 | 所有攻击场景被拦截或告警 |

**里程碑 M15：Phase 3 安全交付** — 全系统安全验证通过、攻击场景拦截率 100%

预期测试合计：20 项

---

## 总览

| 阶段 | 主题 | 任务数 | 预期测试 | 预计周期 |
|------|------|--------|---------|---------|
| S1 | 权限声明与授权框架 | 8 | 54 | 3 周 |
| S2 | 进程级沙箱隔离（Linux + Windows） | 8 | 64 | 5 周 |
| S3 | WASM 工具沙箱 | 6 | 42 | 3 周 |
| S4 | Shell 安全分级与 Approval Gate | 7 | 54 | 3 周 |
| S5 | 离线巩固与记忆质量 | 6 | 44 | 3 周 |
| S6 | 集成验证与安全审计 | 4 | 20 | 2 周 |
| **合计** | | **39** | **278** | **15~19 周** |

---

## 依赖关系

```
S1（权限框架）──┬──→ S2（沙箱隔离）──→ S6.1, S6.3
               ├──→ S3（WASM 沙箱）──→ S6.2
               └──→ S4（Shell 安全）──→ S6.3
S5（离线巩固）──────────────────────→ S6（集成验证）
```

- S1 是 S2/S3/S4 的前置（权限模型 + 校验器是沙箱和安全分级的基础）
- S2/S3/S4 可并行（三条独立的安全能力线）
- S5 独立于 S1~S4（记忆系统增强，不依赖安全模块）
- S6 依赖 S1~S5 全部完成

---

## 关键技术决策点（已确认）

| 编号 | 决策项 | 决策 | 理由 |
|------|--------|------|------|
| D1 | 沙箱 crate 归属 | **新建 `rollball-sandbox` 独立 crate** | 沙箱逻辑可独立测试，Linux/Windows/macOS 各有实现 |
| D2 | 平台覆盖范围 | **Linux + Windows，macOS 留 Phase 7** | 方便 Windows 集成测试，开发环境为 Windows |
| D3 | WASM 组件模型升级时机 | **S3 同步完成** | WIT 提供类型安全，越早切越少迁移成本 |
| D4 | 离线巩固 LLM 调用策略 | **本地 qwen 9B 优先 + 远程 API 备用** | 本地有 2080Ti 22G，可流畅运行 qwen 9B；远程 API 作为备用/高质量模式 |
| D5 | Approval Gate UI | **CLI + trait 抽象（Desktop 预留）** | Phase 5 Desktop App 需要接入，提前抽象 |
| D6 | FileProvenance 持久化 | **rusqlite** | Agent 重启后需恢复来源记录 |
| D7 | 巩固触发策略 | **定时 + idle + 手动三种** | 定时保证基线，idle 利用空闲，手动给用户控制权 |

---

## 与后续 Phase 的关系

- **Phase 4（通信与协调）**：Phase 3 的权限框架为 Intent 跨 Agent 通信提供权限校验基础
- **Phase 5（Desktop App）**：Approval Gate 的 CLI 实现升级为 Desktop App 弹窗；权限管理 UI
- **Phase 7（跨平台）**：Sandbox trait 的 macOS（Seatbelt）/ Windows（AppContainer）实现
- **Phase 6（云端生态）**：Agent 仓库安全扫描（`docs/08-security.md` §12）依赖 Phase 3 的安全基础设施

---

## 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| bubblewrap 在 CI 环境无法运行（需 user namespace） | 中 | S2 Linux 测试无法自动化 | CI 使用 Docker privileged 或 skip 沙箱测试标记 |
| Windows AppContainer 需要管理员权限创建 | 中 | S2 Windows 测试需提权 | CI Windows runner 以管理员运行；本地开发用 --no-sandbox 降级 |
| Wasmtime LTS 版本 API 变更 | 低 | S3 编译失败 | 锁定具体版本、Cargo.lock 固定 |
| 三元组提取 LLM 调用成本 | 中 | S5 运行成本高 | 批量化、设置每日巩固上限、优先巩固高 importance Episode |
| Shell 命令解析的逃逸 | 高 | S4 安全分级被绕过 | 明确标注为"尽力检测"，依赖 S2 沙箱作为底线防御 |
| FileProvenance 与并发 shell 的竞态 | 中 | 来源记录不准确 | FsWatcher 异步更新 + 乐观锁 |

---

## 设计决策记录（ADR）

### ADR-P3-001：权限授权模型选型

**背景**：需要选择权限授权的粒度和交互模型。

**决策**：采用 Android 风格的"安装时声明 + 运行时请求"混合模型。

- 安装时：展示所有声明的权限，用户一次性确认
- 运行时：Agent 可请求额外权限，Gateway 弹出确认
- 权限粒度：按资源类型 + 路径/URL 模式匹配

**理由**：兼顾安全性（最小权限原则）和易用性（不频繁打扰用户）。

### ADR-P3-002：沙箱技术选型

**背景**：需要选择 Linux 进程隔离的实现方式。

**决策**：选择 bubblewrap（bwrap）+ seccomp-bpf 组合。

- bubblewrap：轻量级用户态沙箱，不需要 root 权限（使用 user namespace）
- seccomp-bpf：系统调用级过滤，阻止危险操作
- 不选 Docker/OCI：过重，启动慢，不适合单 Agent 隔离场景
- 不选 Firejail：功能更多但攻击面也更大

**理由**：bubblewrap 是 Flatpak 生态的核心组件，经过大规模生产验证，最小化攻击面。

### ADR-P3-003：WASM 运行时选型

**背景**：需要选择 WASM 工具的运行时引擎。

**决策**：选择 Wasmtime（Bytecode Alliance），WASI Preview 2。

- Cranelift 编译器（默认，综合性能最优）
- 锁定 LTS 版本（如 v36.x）
- Phase 7 移动端备选 Wasmi（纯解释器，iOS 兼容）

**理由**：详见 `docs/12-tool-system.md` §3.1 选型对比。标准合规性最强，安全审计成熟，无厂商锁定。
