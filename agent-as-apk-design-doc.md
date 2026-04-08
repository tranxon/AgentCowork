# Agent as APK：个人化、安全、可组合的 AI Agent 平台设计文档

---

## 1. 概述

### 1.1 背景与目标

设计一个去中心化、高安全、可扩展的 AI Agent 运行时平台。核心思想是将每个 Agent 视为一个独立的"应用包"（类似 Android APK），运行在客户端（用户电脑）并由轻量级 Gateway 管理生命周期。Agent 的工作流代码统一、可复制，差异仅在于本地配置和持久化记忆（Memory）。Memory 作为独立后端服务，提供跨设备同步和向量检索能力。

### 1.2 核心特性

- **标准化打包**：Agent 以压缩包（.agent）分发，内含可执行文件、配置、私有数据库、清单文件。
- **进程级隔离**：每个 Agent 运行在独立进程，拥有独立工作区、文件系统隔离、可选资源限制（cgroups/容器）。
- **轻量级 Gateway**：负责消息转发、生命周期管理（启动/停止/休眠/唤醒）、权限控制、Agent 安装/卸载。
- **分离式 Memory**：Memory 作为云端/自托管服务，Agent 通过网络 API 读写记忆，支持本地缓存和离线队列。
- **权限声明与授权**：Agent 在清单中声明所需权限（网络、文件、调用其他 Agent 等），用户在安装时或运行时授权。
- **跨平台支持**：优先 Linux（完整沙箱），Windows/macOS 提供降级隔离。

---

## 2. 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│                        用户设备（客户端）                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Gateway（常驻进程）                  │  │
│  │  - Package Manager   - 生命周期管理   - 消息路由        │  │
│  │  - 权限校验           - 日志聚合        - 配置管理       │  │
│  └───────────┬───────────────────────┬───────────────────┘  │
│              │                       │                       │
│              ▼                       ▼                       │
│  ┌─────────────────────┐   ┌─────────────────────┐          │
│  │   Agent 1 进程       │   │   Agent 2 进程       │   ...    │
│  │  - 独立工作区目录     │   │  - 独立工作区目录     │          │
│  │  - 私有 SQLite       │   │  - 私有 SQLite       │          │
│  │  - stdin/stdout 通信  │   │  - stdin/stdout 通信  │          │
│  └─────────────────────┘   └─────────────────────┘          │
│              │                       │                       │
│              └───────────┬───────────┘                       │
│                          │                                   │
│                    本地消息队列                               │
│                    （跨 Agent 通信）                          │
└──────────────────────────┼───────────────────────────────────┘
                           │ HTTPS / gRPC
                           ▼
              ┌─────────────────────────┐
              │     Memory 服务（云端）    │
              │  - 向量存储（pgvector）   │
              │  - 键值存储               │
              │  - 用户/Agent 鉴权        │
              │  - 冲突解决（CRDT/LWW）   │
              └─────────────────────────┘
```

---

## 3. 组件详细设计

### 3.1 Agent 打包格式（.agent）

#### 3.1.1 包结构

`.agent` 文件本质是一个 ZIP 压缩包，扩展名可自定义。解压后目录结构：

```
<agent_id>.agent
├── manifest.json          # 必需，元数据与权限声明
├── agent_binary           # 可执行文件（Rust 编译，静态链接优先）
├── config/                # 默认配置文件（用户可覆盖）
│   └── settings.toml
├── data/                  # 私有数据库初始文件（如空 SQLite）
├── resources/             # 图标、本地化等
└── tools/                 # 辅助脚本（可选）
```

#### 3.1.2 manifest.json 架构

```json
{
  "agent_id": "com.example.weather",
  "version": "1.0.0",
  "name": "Weather Agent",
  "description": "查询实时天气并建议穿衣",
  "entry": "./agent_binary",
  "author": "example@domain.com",
  "permissions": [
    "network:https://api.weather.com",
    "filesystem:read:~/Documents",
    "memory:read",
    "memory:write",
    "message:send:com.example.calendar"
  ],
  "triggers": [
    {"type": "schedule", "cron": "0 7 * * *"},
    {"type": "message", "pattern": "天气|weather"}
  ],
  "memory": {
    "sync_mode": "auto",        // "auto", "on_demand", "never"
    "cache_ttl": 3600,
    "required": false
  },
  "resources": {
    "max_memory_mb": 200,
    "max_cpu_percent": 50,
    "network": true
  },
  "sandbox": {
    "enable": true,
    "allow_ptrace": false,
    "read_only_root": true
  }
}
```

---

### 3.2 Gateway 组件

Gateway 是一个常驻的系统级进程（可表现为系统托盘应用），使用 Rust 实现，核心模块：

#### 3.2.1 Package Manager

- **安装**：解压 `.agent` 到 `~/.local/share/agent-gateway/agents/<agent_id>/`，校验 manifest 完整性，记录版本。
- **卸载**：删除对应目录，可选备份用户数据。
- **升级**：保留 `data/` 和用户修改的 `config/`，替换其他文件。
- **仓库支持**：可配置多个 HTTP 仓库源（类似 apt），定期检查更新。

#### 3.2.2 生命周期管理器

**启动策略：**
- 按需启动：当收到匹配 trigger 的消息或用户显式调用时启动。
- 常驻：用户可标记某 Agent 开机自启。
- 定时启动：由 cron 表达式触发。

**进程管理：**
- 使用 `std::process::Command` 创建子进程，设置独立工作目录、环境变量。
- 环境变量注入：`AGENT_ID`, `AGENT_DATA_DIR`, `AGENT_CONFIG_DIR`, `MEMORY_SERVICE_URL`, `MEMORY_TOKEN`。
- `stdin/stdout` 管道：Gateway 写入请求 JSON（每行一个），读取响应 JSON。
- 健康检查：如果 Agent 进程退出，根据退出代码决定是否自动重启（可配置）。

**休眠与唤醒：**
- 对于支持状态序列化的 Agent，可发送 `SIGSTOP` 并换出内存；唤醒时 `SIGCONT`。
- 更简单的方式：空闲超时后直接杀死，下次启动时从零开始（依赖 Memory 恢复状态）。推荐后者。

#### 3.2.3 消息路由器

**输入源：**
- 用户界面（CLI/GUI）发出的请求。
- 定时任务触发器。
- 其他 Agent 通过 Gateway 转发的消息（见 3.4）。

**路由规则：**
- 根据消息中的 `target_agent` 字段直接路由。
- 若未指定，则匹配 manifest 中的 `triggers.message.pattern`（支持简单 glob 或正则）。

**协议：** 每行一个 JSON 对象，格式统一：

```json
{
  "id": "req-123",
  "target": "com.example.weather",
  "payload": {...},
  "reply_to": "gateway"   // 可选，用于异步回调
}
```

响应：Agent 输出类似 JSON，Gateway 根据 `id` 返回给调用者。

#### 3.2.4 权限执行器

**文件系统隔离：**
- Linux：使用 bubblewrap 或 unshare，绑定只读的 `/usr` 等，授予 `~/Documents` 等白名单目录。
- Windows：使用 `CreateRestrictedToken` + Job Object + 文件系统 ACL。

**网络访问控制：**
- 若 manifest 中 `network` 为 false，则通过 seccomp（Linux）或代理拦截。
- 若允许但限制域名，可设置 `HTTP_PROXY` 环境变量指向 Gateway 的过滤代理。

**资源限制：**
- 使用 cgroups v2（Linux）或 Windows Job Object 限制内存、CPU。
- 用户授权：首次安装或权限升级时弹出对话框，用户可允许/拒绝。运行时若权限不足，Gateway 可返回错误。

#### 3.2.5 配置与数据存储

- **Gateway 自身配置**：`~/.config/agent-gateway/config.toml`（包含 Memory 服务地址、用户 token、仓库列表等）。
- **每个 Agent 的工作区**：`~/.local/share/agent-gateway/agents/<agent_id>/workspace/`，其中包含：
  - `data/`：从包中复制，可读写。
  - `config/`：用户可修改的配置（初始来自包内 config）。
  - `runtime/`：临时文件（如 socket、pid 文件）。
- **日志**：Gateway 收集所有 Agent 的 stdout/stderr，写入 `~/.local/share/agent-gateway/logs/`，支持按 Agent 过滤。

---

### 3.3 Memory 服务（后端）

Memory 服务作为独立组件，可云端部署或自托管。提供 REST/gRPC API。

#### 3.3.1 核心能力

- **向量存储**：支持 embedding 向量的插入、相似性搜索（用于长期记忆）。
- **键值存储**：简单 KV 用于配置、状态。
- **多租户隔离**：通过 `user_id + agent_id` 命名空间。
- **同步协议**：客户端可拉取增量更新，支持离线本地缓存（使用 SQLite + 向量扩展）。

#### 3.3.2 API 示例

```
POST /memory/query
{
  "user_id": "alice",
  "agent_id": "com.example.weather",
  "vector": [0.1, 0.2, ...],
  "top_k": 5
}

POST /memory/set
{
  "user_id": "alice",
  "agent_id": "com.example.weather",
  "key": "last_city",
  "value": "London"
}
```

#### 3.3.3 鉴权

- Gateway 使用用户的长期 API token 向 Memory 服务请求短期 token 下发给 Agent。
- Agent 调用 Memory API 时携带短期 token，有效期 1 小时，可刷新。

---

### 3.4 跨 Agent 通信（Intent 机制）

Agent 可以通过发送特定格式的消息请求调用另一个 Agent 的能力：

**Agent 输出：**

```json
{
  "type": "intent",
  "target": "com.example.calendar",
  "action": "create_event",
  "params": {"title": "Meeting", "time": "2025-01-01T10:00Z"},
  "async": true,
  "id": "msg-456"
}
```

Gateway 收到后，根据 `target` 查找 Agent，若未运行则启动，转发消息。响应通过另一个消息返回给原 Agent（利用 `reply_to` 字段）。

---

## 4. 安全设计

- **进程隔离**：每个 Agent 独立进程，一个崩溃不影响其他。
- **文件系统隔离**：Agent 只能写入自己的工作区目录和用户明确授权的目录。
- **网络隔离**：默认禁止网络，仅按 manifest 授权域名。
- **权限最小化**：manifest 必须声明所有权限，用户可拒绝。
- **沙箱强化**：Linux 下使用 seccomp-bpf 限制危险系统调用（如 clone、ptrace）。
- **Memory 传输加密**：HTTPS 或 gRPC TLS。
- **审计日志**：Gateway 记录所有 Agent 启动、权限使用、跨 Agent 调用。

---

## 5. 实现路线图

### Phase 1: 基础框架（MVP）

- 定义 manifest 规范，实现简单的 ZIP 解析。
- Gateway 基础功能：安装、卸载、启动/停止进程，stdin/stdout 通信。
- 实现一个示例 Agent（Rust 二进制，读 stdin 打印 hello）。
- 本地目录隔离（不使用命名空间，仅 `--work-dir`）。

### Phase 2: 权限与沙箱

- 集成 bubblewrap（Linux）实现文件系统隔离。
- 实现权限声明和用户授权对话框（CLI 或简单 GUI）。
- 资源限制（cgroups 或 rlimit）。

### Phase 3: Memory 集成

- 部署 Memory 服务（使用 pgvector，提供 REST API）。
- Gateway 为 Agent 注入 Memory 地址和 token。
- Agent SDK 提供简单客户端库（Rust/Python）读写 Memory。

### Phase 4: 高级特性

- 定时触发器（cron 解析）。
- 跨 Agent 消息转发（Intent）。
- 远程仓库支持（添加仓库、更新、自动下载）。
- 图形化管理界面（Tauri 或 egui）。

### Phase 5: 跨平台与优化

- Windows 隔离（Job Object + 受限令牌）。
- macOS sandbox-exec 支持。
- 性能优化：进程池减少启动延迟，零拷贝 IPC（共享内存）。

---

## 6. 使用场景示例

**场景**：用户安装"天气 Agent"和"日历 Agent"。每天早上 7 点，天气 Agent 自动获取当地天气，并通过 Intent 调用日历 Agent 创建提醒"今天带伞"。

**流程：**

1. Gateway 的 cron 触发器启动天气 Agent（若未运行）。
2. 天气 Agent 通过 Memory 读取用户上次保存的城市，调用网络 API 获取天气。
3. 天气 Agent 将结果写入 Memory，并输出 Intent：

```json
{"type":"intent", "target":"com.example.calendar", "action":"add_event", "params":{"summary":"带伞","time":"07:30"}}
```

4. Gateway 收到后，启动日历 Agent 并转发消息。
5. 日历 Agent 调用本地日历 API 创建事件，返回成功。
6. Gateway 将响应返回给天气 Agent（可选），天气 Agent 退出。

---

## 7. 与现有方案对比

| 特性 | Agent as APK | ZeroClaw / OpenClaw | Docker + 微服务 |
|------|-------------|---------------------|----------------|
| 隔离级别 | 进程 + 可选沙箱 | 单进程内逻辑隔离 | 操作系统容器 |
| 资源开销 | 极低（空闲可杀死） | 低（常驻） | 较高 |
| 部署位置 | 客户端 | 服务器 | 服务器/客户端 |
| 分发模型 | 应用商店式 | 代码库/配置 | 镜像仓库 |
| 适用规模 | 个人/小团队 | 大规模公共服务 | 任意（较重） |

---

## 8. 未来扩展

- **Agent 商店**：建立公开仓库，用户可一键安装。
- **付费 Agent**：支持许可证验证（Gateway 内集成）。
- **联邦 Memory**：多个用户之间的 Memory 共享（需授权）。
- **WebAssembly Agent**：将 Agent 编译为 WASM，运行在轻量级运行时（进一步降低开销）。

---

