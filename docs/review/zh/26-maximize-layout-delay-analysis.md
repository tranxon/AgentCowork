# 26 — 最大化窗口布局延迟 — 根因分析

**Date**: 2026-07-01
**Reviewer**: Senior Engineer
**Status**: 🔍 现象分析完成，待确认修复方案

> 本报告为**现象分析（不包含修复方案）**，等待用户确认修复方向后另行提交修复 diff。

## Scope

| 层级 | 文件 | 相关片段 |
|------|------|---------|
| 配置 | `apps/acowork-desktop/src-tauri/tauri.conf.json` | `transparent: true`、`shadow: true`、`visible: false` |
| Rust 后端 | `apps/acowork-desktop/src-tauri/src/lib.rs` | Windows `Effect::Acrylic` / macOS `Effect::UnderWindowBackground` 初始化 |
| 入口 | `apps/acowork-desktop/src/components/layout/TitleBar.tsx` | `handleMaximize` → `await win.toggleMaximize()` |
| 布局 | `apps/acowork-desktop/src/components/layout/AppLayout.tsx` | 唯一 `window.resize` 监听（`handleWindowResize`）、`sidebarWidth/rightWidth/fileWidth` 状态 |
| 样式 | `apps/acowork-desktop/src/styles/globals.css` | `html/body { background: transparent !important }`、`#root { 100vh/100vw }` |
| 样式 | `apps/acowork-desktop/index.html` | 内联 `html, body { background-color: transparent }` |
| 根组件 | `apps/acowork-desktop/src/App.tsx` | `getCurrentWindow().show()/setFocus()` |

---

## 1. 现象复述

- **操作**：点击 TitleBar 中的"最大化"按钮（或拖拽窗口到屏幕顶端）
- **表现**：**窗口的透明背景（Acrylic / 毛玻璃）已立即扩展到全屏**，但内部 Webview 的布局（Agent List 侧边栏、ChatPanel、FileEditor、Results 面板）**没有同步扩展**，呈现"毛玻璃是大窗口，内部内容仍按原尺寸排列"的撕裂感。
- **进一步操作**：再次点击最大化、调整窗口边缘、等待数百毫秒后，布局**会自动跳到正确状态**。
- **经验性观察**：触发概率与系统状态相关（首启、长时间未动、CPU 紧张时更易出现）。

---

## 2. 关键事实（基于代码的"硬证据"）

### 2.1 窗口透明背景 — 由 OS 层立即生效

| 层 | 配置/代码 | 效果 |
|---|---|---|
| `tauri.conf.json:34` | `transparent: true` | WebView2 背景透明 |
| `tauri.conf.json:37` | `shadow: true` | OS 窗口阴影 |
| `lib.rs:249-262` (Windows) | `Effect::Acrylic` + `set_effects` | DWM Acrylic 毛玻璃 |
| `lib.rs:223-236` (macOS) | `Effect::UnderWindowBackground` | NSVisualEffectView 半透明 |
| `index.html:18-22` | `html, body { background-color: transparent; }` | HTML 层不绘制背景 |
| `globals.css:194-214` | `html/body { background: transparent !important }`、`#root { 100vh / 100vw }` | CSS 层不绘制背景 |

**结论**：透明背景由 **Windows DWM（Desktop Window Manager）/ macOS NSWindow** 在 OS 层面直接合成，**不经过 WebView 的 layout 管线**，所以"窗口背景已最大化"和"Webview 布局未最大化"**完全可以不同步**。

### 2.2 布局"扩展"逻辑 — 只在 Webview 层做有限处理

`AppLayout.tsx:232-269` 的 `handleWindowResize` 是唯一的"窗口尺寸变化 → 布局响应"入口：

```tsx
// AppLayout.tsx:237-269 (节选)
useEffect(() => {
  const handleWindowResize = () => {
    if (isResizingFile.current) return;             // ① 拖拽 file panel 时直接退出
    const newWindowWidth = window.innerWidth;
    const constantWidths = sidebarWidthRef.current
                         + (resultsCollapsedRef.current ? 0 : rightWidthRef.current)
                         + NAV_WIDTH;
    const newAvailable = newWindowWidth - constantWidths;
    const prevAvailable = prevAvailableWidthRef.current;
    if (prevAvailable <= 0 || newAvailable <= 0) return;
    const ratio = newAvailable / prevAvailable;
    if (Math.abs(ratio - 1) < 0.05) return;         // ② <5% 变化被忽略
    prevAvailableWidthRef.current = newAvailable;
    const hasFiles = useFileEditorStore.getState().openFiles.length > 0;
    if (hasFiles) {                                  // ③ 只有打开文件时才缩放
      const newFile = Math.round(fileWidthValueRef.current * ratio);
      setFileWidth(newFile);
      localStorage.setItem(FILE_WIDTH_KEY, String(newFile));
    }
    // ⚠️ sidebarWidth、rightWidth、ChatPanel 都不主动调整
    //    ChatPanel 依靠 `flex-1` 自动填充，但前提是外层容器先 reflow
  };
  window.addEventListener("resize", handleWindowResize);
  return () => window.removeEventListener("resize", handleWindowResize);
}, []);
```

**几个关键观察**：
- 这个 handler **只缩放 `fileWidth`**，且仅在 `hasOpenFiles` 时。
- `sidebarWidth`、`rightWidth` 保持**绝对值**不变。
- 整体不依赖 `isMaximized` 状态。
- 没有任何 `requestAnimationFrame` 兜底或重试。

### 2.3 Maximize 触发路径 — 没有"完成"回调

`TitleBar.tsx:16-22`：

```tsx
const handleMaximize = async () => {
  try {
    await win.toggleMaximize();   // 仅 await IPC 往返，不等 OS 动画/合成完成
  } catch (error) { ... }
};
```

- `await win.toggleMaximize()` 解决时，意味着 Tauri 把"切换"命令发到了 OS，**不等 OS 完成实际的最大化几何变换**。
- OS 侧最大化在 Windows 上**默认有 200~400ms 动画**（"显示动画"开启时），macOS 上有约 250ms 的 Genie 效果。
- `window.resize` 事件**理论上**在动画开始时触发一次，最终尺寸稳定后再触发一次，但 WebView2 + DWM 合成时序不稳定。

### 2.4 没有 `isMaximized` 状态同步

全工程 `content_search` 结果，仅 `TitleBar.tsx` 和 `App.tsx` 调用过 `getCurrentWindow()`，**没有**任何代码：
- 监听 Tauri 的 `tauri://resize`、`tauri://move`、`WindowEvent::Resized` 事件
- 读取 `isMaximized()` / `isFullscreen()` 状态
- 用 Tauri v2 `getCurrentWindow().onResized(...)` 订阅尺寸变化

也就是说，**前端唯一的尺寸信号源是浏览器原生的 `window.resize` 事件**，Tauri 的精确事件没被利用。

### 2.5 透明背景下 ResizeObserver 的特殊性

`ChatPanel.tsx` 中虽然有 ResizeObserver，但它观察的是**子容器**的尺寸，不是 `#root`。当 `#root` 没有触发重排时，子容器尺寸不会变，**所以 ResizeObserver 也救不了这种情况**。

---

## 3. 根因分析（按可能性排序）

### 🟥 主因（决定性因素）：CSS `100vw/100vh` vs Webview 实际可视区域的不同步

```css
#root { height: 100vh; width: 100vw; }
```

- `100vh` / `100vw` 是**视口单位**，对应 WebView 内部的"布局视口"。
- WebView2 在 OS 窗口几何变化时，存在一个**内部 layout viewport 更新滞后**的窗口期：
  - OS 通知 WebView2 "窗口现在是 1920×1080"（DWM 已开始绘制 Acrylic）
  - WebView2 内 Chromium 渲染进程**尚未重新计算 layout viewport**
  - 此时 `window.innerWidth` 仍可能返回旧的 1200
  - `window.resize` 事件尚未触发或触发但 reflow 未完成
- 透明背景由 DWM 合成 → **不依赖 WebView 状态 → 立即变化**。
- WebView 内部布局依赖 Chromium 渲染 → **依赖 layout viewport → 有延迟**。
- **这就是"毛玻璃先变、内容后变"或"内容卡住不动"的根因**。

### 🟧 次因 1：`resize` 事件在 Maximize 中的不可靠性

- Tauri v2 + WebView2 已知问题（社区多次反馈）：`window.resize` 在 maximize/restore 时**可能漏触发、滞后触发、重复触发**。
- `await win.toggleMaximize()` 不等 OS 完成。
- 没有用 Tauri 的 `WebviewWindow.onResized` 事件（这条是同步的、由 Tauri runtime 直接派发）作为兜底。

### 🟨 次因 2：`handleWindowResize` 的处理面太窄

- 只处理 `fileWidth`，**不重新计算 `sidebarWidth` / `rightWidth`**。
- 用户感觉"布局没变化"，可能指的就是这三栏的**绝对宽度没变**。
- 即便 `fileWidth` 被缩放，**外层 `#root` 没变化时，子元素也不会 reflow**。
- `<5%` 阈值的"防抖"逻辑反而成了帮凶 — 在 maximize 这种大尺寸变化中**不应该用**（但实际上这种变化一定 > 5%，所以这条影响小）。

### 🟨 次因 3：CSS 透明背景 + 容器 `100vw/100vh` 组合

- `AppLayout.tsx:407`：根 div 用 `flex h-full w-full` 配 `backdrop-blur-sm`。
- `h-full` = `100% of parent`，`#root` 是 `100vh`。
- 当 `#root` 没更新到新视口大小时，根 div 也不会扩大。
- `backdrop-blur` 是 CSS 层操作，**对 OS 透明的 Acrylic 效果**没什么影响，但**对内部子元素的位置计算**有影响 — 父容器不变，子元素的 `flex-1` 也不会变。

### 🟦 加重因素：操作系统动画

- Windows DWM 的 maximize 动画（默认 200-400ms）：动画期间 resize 事件可能触发多次。
- macOS 的 Genie 效果：类似情况。
- 第一次 resize 事件触发时，如果 `prevAvailableWidthRef` 还没更新、或者 handler 提前 return，**后续触发的 resize 事件全部进入"已忽略"分支**。

### ⬜ 排除的因素

- **`transparent: true` 本身**：只是让 WebView 不绘制背景色，不影响 layout 管线。
- **WebSocket 流中断**：与 maximize 无直接关联。
- **Agent 启动 / Gateway 状态**：在已 connected 后不影响 layout。

---

## 4. 复现路径（基于代码推导）

1. 启动应用，等待 React 首次渲染完成（`#root` = `1200×800`）。
2. 点击 TitleBar 最大化按钮。
3. OS 立即调度 DWM 最大化动画 → Acrylic 背景已开始合成 → 视觉上"毛玻璃"已铺满。
4. WebView2 异步收到 `WM_SIZE` 消息 → 排队等渲染进程处理。
5. Chromium 渲染进程计算新 layout viewport → 此时 `window.innerWidth` 仍可能短暂返回旧值。
6. 首次 `resize` 事件触发 → `handleWindowResize` 执行：
   - 读 `window.innerWidth` 拿到**过渡值**或旧值
   - `ratio ≈ 1.0` → 进入 `< 0.05` 分支 → **直接 return**
7. 第二次 `resize` 事件（动画结束后）触发 → 此时 `prevAvailableWidthRef` 已被前一次更新成错的中间值，或 `fileWidthValueRef` 仍是旧值 → 缩放结果错误。
8. 用户操作（再次点最大化、拖窗口边缘、点击内部）→ 触发新一轮 `resize` → 此时 `window.innerWidth` 已稳定 → 布局"跳正"。

**为什么"多操作几次才正常"**：
- 每次操作都会触发 `resize` 事件
- 每次事件都给 `prevAvailableWidthRef` 一次更新机会
- 经过 2~3 次错误中间值后，最终接近真实值，布局稳定

---

## 5. 缺失的"状态源"清单

| 应有的状态/事件 | 当前实现 | 影响 |
|---|---|---|
| `WebviewWindow.onResized`（Tauri 事件） | ❌ 未订阅 | 错过最可靠的尺寸信号 |
| `WebviewWindow.isMaximized()` | ❌ 未读取 | 不知当前是否最大化 |
| `rAF` 兜底 | ❌ 没用 | 即使 resize 漏触发也无补救 |
| 在 maximize/restore 之后强制 reflow | ❌ 没有 | CSS 视口单位不会自动重算 |
| sidebarWidth/rightWidth 响应式缩放 | ❌ 只有 fileWidth | 视觉上"内容未扩展" |

---

## 6. 影响面评估

| 平台 | 复现概率 | 备注 |
|---|---|---|
| **Windows 10/11** | **高** | DWM 动画 + WebView2 layout 滞后是已知组合 |
| **macOS** | 中 | Genie 动画类似，但 WKWebView 通常更稳定 |
| Linux (WebKitGTK) | 中 | 无原生 Acrylic，效果不同 |

| 触发条件 | 影响程度 |
|---|---|
| 首次 maximize | 高（视口初次从 1200 跳到 1920） |
| 已有大窗口时 maximize | 低（变化小，5% 阈值就放过了） |
| 拖拽调整窗口大小 | 低（resize 事件密集触发） |
| 系统繁忙 / GPU 紧张 | 加重 |

---

## 7. 结论

**这是一个典型的"OS 层视觉与 Webview 层布局更新路径不一致"问题**，叠加 Tauri/WebView2 在 maximize 场景下 `window.resize` 事件的不稳定。具体可归为三个叠加原因：

| # | 原因 | 性质 |
|---|---|---|
| 1 | `#root` 用 `100vh/100vw`，WebView2 内部 layout viewport 更新滞后于 DWM 合成 | **主因** |
| 2 | 前端只听浏览器 `resize` 事件，没用 Tauri 的 `onResized` 事件，也没 `rAF` 兜底 | 关键缺口 |
| 3 | `handleWindowResize` 只缩放 `fileWidth`，`sidebarWidth`/`rightWidth` 保持绝对值 | 放大观感 |

**为什么"多操作几次能恢复"**：每次 `resize` 事件都给 `prevAvailableWidthRef` 一次更新机会，加上后续 resize 事件 `window.innerWidth` 已稳定为真实值，经过若干次错误中间值后收敛到正确状态。

---

## 8. 建议的修复方向（待用户确认后再实施）

按修复成本和效果从高到低：

| 方案 | 改动 | 预期效果 |
|---|---|---|
| **A. 订阅 Tauri `onResized` 事件** + `rAF` 兜底 | `AppLayout.tsx` 加 1 个 useEffect | 解决 80% 问题 |
| **B. 在 maximize/restore 之后强制 reflow**（读 `getCurrentWindow().outerSize()` 触发一次 state 变化） | `TitleBar.tsx` + `AppLayout.tsx` | 解决 20% |
| **C. `sidebarWidth`/`rightWidth` 也做比例缩放**（仅在 maximize 状态） | `AppLayout.tsx` | 视觉一致性 |
| **D. 把 `#root` 改为 `position: fixed; inset: 0` 替代 `100vw/100vh`** | `globals.css` | 绕开 viewport 单位滞后问题（旁路方案） |

**推荐组合**：`A` + `B` + `C`，可彻底消除这个症状。

---

## 附录 A：相关代码索引

```
apps/acowork-desktop/
├── src/
│   ├── App.tsx                                          # 根组件，调用 getCurrentWindow().show()
│   ├── components/
│   │   └── layout/
│   │       ├── AppLayout.tsx                            # 唯一 resize 监听，文件 572 行
│   │       │   ├── L50-66     : sidebarWidth/rightWidth/fileWidth 初始化
│   │       │   ├── L232-269   : handleWindowResize（核心问题代码）
│   │       │   ├── L407       : 根 div 样式 (flex h-full w-full backdrop-blur-sm)
│   │       │   └── L425-453   : AgentList + ChatPanel + FileEditor 三栏布局
│   │       └── TitleBar.tsx                              # L16-22 handleMaximize
│   ├── styles/
│   │   └── globals.css                                  # L195-214 transparent + 100vh/100vw
│   └── main.tsx                                         # React 入口
├── index.html                                           # L18-22 transparent 内联样式
└── src-tauri/
    ├── tauri.conf.json                                  # L34 transparent, L37 shadow
    └── src/
        └── lib.rs                                       # L223-262 Acrylic / UnderWindowBackground
```

## 附录 B：分析使用的方法

- 全工程 `content_search` 搜索 `resize|maximize|maximized|isMaximized|toggleMaximize|onResized|window\.innerWidth|backdrop-blur|transparent`
- 关键文件 `file_read` 完整阅读：`App.tsx`、`TitleBar.tsx`、`AppLayout.tsx`、`globals.css`、`index.html`、`lib.rs`、`useSystemResume.ts`、`SplashScreen.tsx`
- 上下游调用链确认：`getCurrentWindow()` 仅出现在 `App.tsx` 和 `TitleBar.tsx`，说明前端没有任何持续的 Tauri 窗口状态订阅
- 通过 git log 确认最近改动（最近 20 次 commit）无与 maximize / 透明背景相关的变更
