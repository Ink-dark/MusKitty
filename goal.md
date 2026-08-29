# Goal — 下一步任务清单（给 Codex / AI Agent 用）

> **更新时间**：2026-08-29
> **当前状态**：窗口化轨道 **W-1~W-5 全部完成**（`PlatformWindow` 抽象 → DPI → 输入 → Headless 后端 → 多标签状态管理；`HeadlessWindow` + `render_to_png` 无窗口渲染，`--no-default-features` 下 check/test/clippy 全绿）本轮新增 **W-5 多标签完成**（WebViewCollection + Ctrl+T/W/1~9/PageUp/Down + 脏位延迟更新，webview.rs 10 条 + input.rs 5 条新单测）。主线 **M-3 batch 1 已完成**（`border:` 简写展开 + media 视口接线）。本轮**双轨并行**：主线 M-3（batch 2，按需，余项全延后）+ 并行轨道窗口化（下一阶段 **W-5 多标签状态管理**，规划见 [docs/plans/2026-08-23-windowing.md](docs/plans/2026-08-23-windowing.md)）。两轨触碰不同 crate，穿插推进互不阻塞。

---

## 当前阶段定位

- **主线**：M-3 CSS 补全 — batch 1（border 简写 + media 视口接线）✅ 已完成；@layer 排序（audit B8）已完整实现无需再做；余项 revert/revert-layer 真语义（B7）、background-image、方向性 border、outline 按需求裁剪延后（原因见 PROGRESS.md item 15）。按渲染真实页面的需求驱动裁剪范围。
- **并行轨道**：窗口化 — `muskitty-shell`（浏览器外壳）：`PlatformWindow` trait 抽象（W-1 ✅）→ DPI（W-2 ✅）→ 输入（W-3 ✅）→ Headless（W-4 ✅）→ 多标签（W-5 ✅）。窗口化轨道收官。
- **中期主线**：M-1 网络接轨 / M-2 交互基础排在 M-3 之后；inline formatting context 独立远期 Phase。

## 穿插节奏（双轨并行）

| 轮次 | 轨道 | 内容 | 触碰 crate |
|------|------|------|-----------|
| 已完成 | 并行 | W-1 窗口化整块（6 commit + 收尾修订：WinitWindow 私有化） | shell（新）/ renderer |
| 已完成 | 并行 | W-2 DPI（3 commit：Backend/render_page scale + 窗口流接线） | shell / renderer |
| 已完成 | 并行 | W-3 输入（4 commit：InputEvent 抽象 + 快捷键层 + handle_event 页面层 + 文档；命中测试单列延后） | shell |
| 已完成 | 并行 | W-4 Headless 后端（3 commit：HeadlessWindow + render_to_png/encode_png + 无窗口渲染测试） | shell |
| 已完成 | 并行 | W-5 多标签状态管理（4 commit：WebViewCollection + 标签快捷键 + 脏位延迟更新 + 文档） | shell |
| 穿插 | 主线 | M-3 一批（按需，余项全延后） | cascade / layout |

**节奏**：每轮一个窗口化阶段收尾（含测试 + commit）→ 切回 M-3 一批 → 下一窗口化阶段。两轨无文件冲突（cascade/layout vs shell/renderer）。

---

## 任务列表

### 并行轨道：W-1 窗口化（✅ 已完成）

**目标**：`muskitty-shell` crate + `PlatformWindow` trait 抽象，`window_demo` 从 renderer 迁入，renderer 回归纯净。功能与现状一致（可缩放、可关闭）。

**Commit 序列**：

- [x] C-1 crate 骨架 + workspace member（Cargo.toml feature-gate winit-backend + 6 path deps + lib.rs；根 members 加入）
- [x] C-2 `PlatformWindow` trait 最小集 + `Cursor`/`WindowGeometry`（无外部依赖类型泄漏）
- [x] C-3 `page.rs` 渲染管线 `render_page(html, css, w, h) -> RenderOutput`
- [x] C-4 `WinitWindow: PlatformWindow` + `app.rs` + `main.rs`（RGBA→0RGB 在 present 内，抽纯函数）
- [x] C-5 `window_demo` 迁入 shell examples，删除 renderer 版本
- [x] C-6 renderer 移除 winit/softbuffer dev-deps
- [x] 收尾修订：`WinitWindow` 降 `pub(crate)`（构造参数含 winit 类型，用户决策严格合规），示例改用 `App::run`；直接构造 `PlatformWindow` 的演示迁至 W-4 `HeadlessWindow`

**退出条件（全部满足才可 commit）**：

- [x] `cargo check --workspace` / `cargo test --workspace` 通过
- [x] `cargo fmt --all -- --check` 通过
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo run -p muskitty-shell --example window_demo` 真窗口渲染与现状一致
- [x] shell `pub` 导出无 winit/softbuffer 类型；renderer 无 winit/softbuffer dev-deps
- [x] 单测：RGBA→0RGB 转换 + page 管线像素断言

### 并行轨道：W-2 DPI（✅ 已完成）

**目标**：`hidpi_scale_factor()` 返回真实值；`render_page`/`Backend` 加 scale 参数——layout 用逻辑视口（CSS px），渲染用物理分辨率（`logical × scale`）。整数倍（1x/2x）先行。同组命令 scale=1 与 scale=2 输出分辨率分别为 `w×h` 与 `2w×2h`，且 scale=2 左上角像素与 scale=1 一致（非简单插值）。

**Commit 序列**：

- [x] C-1 `[renderer]` Backend::render 加 scale；rect/border/clip 用 from_scale 向量缩放、文本 scale∘translate；stroke 宽度保持逻辑 px（tiny-skia 局部描边后再变换）；新增 scale=1-vs-2 退出条件单测
- [x] C-2 `[shell]` render_page/render_html_file 加 scale；布局保持逻辑尺寸；新增 scale=2 管线单测（逻辑 200×100 → 输出 400×200）
- [x] C-3 `[shell]` 窗口流接线：geometry() 物理→逻辑换算、RedrawRequested 读 scale + 脏检查含 scale、ScaleFactorChanged 重绘

**退出条件（全部满足才可 commit）**：

- [x] `cargo check --workspace` / `cargo test --workspace` 通过
- [x] `cargo fmt --all -- --check` 通过
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] 单测：同组命令 scale=1→`w×h`、scale=2→`2w×2h`，关键像素与 scale=1 对应逻辑点一致（非插值）；`render_page` scale=2 输出分辨率与红块物理坐标正确
- [x] `cargo run -p muskitty-shell --example render_file` 输出不变（scale=1）

### 并行轨道：W-3 输入（✅ 已完成）

**目标**：`input.rs` 输入事件抽象 + shell 快捷键层（Esc 关闭、Ctrl+R 刷新）+ 事件分发结构。页面级命中测试**不在本轮**（单列延后，见规划文档）。

**Commit 序列**：

- [x] C-1 `[shell]` input.rs：InputEvent/Key/Modifiers/MouseButton/ButtonState/TouchPhase/ShortcutAction + 纯函数 `match_shortcut`（12 条无窗口单测）
- [x] C-2 `[shell]` `PlatformWindow::handle_event` 页面层入口（winit 后端 W-3 恒 false）
- [x] C-3 `[shell]` App 事件接线：ModifiersChanged/CursorMoved 跟踪 + 键盘/鼠标/滚轮/触摸 → InputEvent 转换（逻辑 px）+ `dispatch_input` 分层 + `reload`（17 条单测，含 2 条快捷键集成）
- [x] C-4 `[docs]` 规划文档 W-3 完成 + 架构修正记录；goal.md/PROGRESS.md 同步

**架构修正**：快捷键层在 `App::dispatch_input`（Esc 需 `event_loop.exit()`、Ctrl+R 需渲染管线，均 App 独有）；`PlatformWindow::handle_event` 为页面/转发层。详见规划文档 §W-3。

**退出条件（全部满足才可 commit）**：

- [x] `cargo check --workspace` / `cargo test --workspace` 通过
- [x] `cargo fmt --all -- --check` 通过
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo check -p muskitty-shell --no-default-features` 无头仍可编译
- [x] 单测：InputEvent 转换 + 快捷键匹配（Esc→Close、Ctrl+R→Reload、非快捷键→None），无需真实窗口
- [x] 手工验证：`cargo run -p muskitty-shell` Esc 关窗、Ctrl+R 重渲染（人工执行）

### 并行轨道：W-4 Headless 后端（✅ 已完成 2026-08-29）

**目标**：`headless_window.rs` 无窗口 `PlatformWindow` 实现 + `render_to_png` 便捷函数，无窗口环境（CI）可跑 shell 渲染测试。feature gate 在此兑现价值——`default-features = false`（`--no-default-features`）时无 winit/softbuffer 也能编译。规划源：[docs/plans/2026-08-23-windowing.md](docs/plans/2026-08-23-windowing.md) §W-4。

**Commit 序列**：

- [x] C-1 `[shell]` `headless_window.rs`：`HeadlessWindow: PlatformWindow`，`present` 保存像素（`frame()` 访问 + `save_png`；Cell 内部可变性；无外部依赖类型可公开构造，W-1 收尾修订迁入 `070e013`）
- [x] C-2 `[shell]` lib 顶层 `render_to_png(html, css, width, height, scale, path)` 便捷函数（走 `page::render_page` 全管线 → `page::encode_png` 编码；tiny-skia 升正式依赖但类型不入 pub API；window_demo 声明 required-features，`25610a6`）
- [x] C-3 `[shell]` 测试集成：无窗口环境渲染测试（`tests/render_to_png.rs`：PNG 解码像素与直接渲染逐字节一致 + HeadlessWindow 帧/编码产物一致 + scale=2 分辨率，`--no-default-features` 全绿，`2c83e44`）

**架构点**：`HeadlessWindow` 可无参构造（相对 `WinitWindow` 构造参数含 winit 类型必须 `pub(crate)`）；`present` 仍走裸像素 `(data, width, height)`（RGBA），PNG 编码在 shell 侧（或复用 renderer 的 encode）。

**退出条件（全部满足才可 commit）**：

- [x] `cargo check --workspace` / `cargo test --workspace` 通过
- [x] `cargo fmt --all -- --check` 通过
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo check -p muskitty-shell --no-default-features` 无头可编译（feature gate 兑现）
- [x] 无窗口环境跑通 `render_to_png`；输出 PNG 像素与 renderer 直接渲染一致（逐字节比对）
- [x] CI 可无窗口跑 shell 渲染测试（集成测试不依赖 winit/softbuffer）

### 并行轨道：W-5 多标签状态管理（✅ 已完成 2026-08-29）

**目标**：`app.rs` 从单 WebView 升级为多 WebView 集合 + 标签快捷键 + 脏位延迟更新（Servo §1.7）。规划源：[docs/plans/2026-08-23-windowing.md](docs/plans/2026-08-23-windowing.md) §W-5。

**现状基线**（已核实 2026-08-29）：`App` 当前单 `html`/`css`（&'static str）+ 单窗口渲染状态（`pixels`/`width`/`height`/`logical_*`/`scale`）；`dispatch_input` 事件分层已就位（shell 快捷键 → `PlatformWindow::handle_event` 页面层）；`input::match_shortcut` 现支持 Close（Esc）/ Reload（Ctrl+R）。标签切换快捷键需先扩展 `ShortcutAction` + `match_shortcut`。

**Commit 序列**：

- [x] C-1 `[shell]` `webview.rs`（不 feature 门控）：`WebView { html, css, needs_repaint, close_scheduled, 渲染状态 }` + `WebViewCollection`（new_tab/close_active/flush_close/select_next/select_prev/select，active 不变量 + 切换自动标脏；10 条单测，App 集合化 `a618c4e`）
- [x] C-2 `[shell]` 标签快捷键：`ShortcutAction` 扩展 NewTab / CloseTab / NextTab / PrevTab / TabSelect(n)；`Key` 增 PageUp/PageDown；`match_shortcut` Ctrl+T/W/1~9/PageUp/PageDown（5 条新单测）；`dispatch_input` 接线（Ctrl+T 开默认内容，全部关闭退出，`9e5c3dc`）
- [x] C-3 `[shell]` 脏位标记 + 延迟更新：shell 动作只标脏 + request_repaint，`RedrawRequested` 统一 flush——先移除 close_scheduled（空则退出）、active 脏或几何/scale stale 才重渲染、提交 active 帧（`ea99d22`）
- [x] C-4 `[docs]` 规划文档 W-5 完成 + goal.md/PROGRESS.md 同步

**范围裁剪**（同规划文档）：favicon 依赖 network + `<link rel=icon>`，降级为占位；标签栏可视化 UI（tab strip）不在本轮（先做状态 + 快捷键，视觉标签栏待后续）。

**退出条件（全部满足才可 commit）**：

- [x] `cargo check --workspace` / `cargo test --workspace` 通过
- [x] `cargo fmt --all -- --check` 通过
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo check -p muskitty-shell --no-default-features` 无头可编译
- [x] 单测：标签集合 CRUD（webview.rs 10 条：新建/延迟关闭/flush 重定位/切换循环/越界忽略/同索引不标脏）+ 新快捷键匹配（input.rs 5 条：Ctrl+T/W/1~9/PageUp/Down → 正确 action，Alt/Meta 不匹配）
- [ ] 手工验证：`cargo run -p muskitty-shell` Ctrl+T 新建、Ctrl+W 关闭、Ctrl+1/PageUp 切换正确刷新（人工执行，待架构师跑窗口 demo）

### 主线：M-3 CSS 补全（穿插推进）

**Batch 1（✅ 已完成 2026-08-29）**：
- `border:` 简写展开 → border-width/style/color（CSS Backgrounds & Borders L3 §4.4；顺序无关 `<width>||<style>||<color>`、每类至多一次、缺失类别取注册表初始值 medium/none/currentcolor；cascade `b87b820`，renderer paint e2e `de48621`）
- media 视口接线：`compute_styles` 用 `StyleTreeOptions.viewport_width/height` 构造 `MediaContext`（cascade `fcde127`）；`render_page` 传逻辑布局视口（shell `f0c5619`）；默认 1920×1080 行为不变

**已核实无需做**：@layer 排序（audit B8）已完整实现（`layer_order` + LayerTracker + 5 元 cascade key）。

**延后（原因）**：revert/revert-layer 真语义（B7，需低 origin/层回滚，零真实页面需求）；background-image（renderer 无 image 消费方：无 image 命令/解码，network 未接轨）；方向性 border（border-left 等）；outline。逐一见 PROGRESS.md item 15。

---

## 不在本轮范围（显式排除）

- Network 自研 HTTP 栈 / Fetch 接轨（保持 trait + reqwest 基础）
- 完整 inline formatting context（多 text + inline 元素同行合并、bidi）
- overflow scroll 交互、CSS Grid 高级特性（命名线/区域）
- 文本渲染性能优化（FontSystem 复用、glyph 缓存）
- 窗口化：双 RenderingContext GPU 合成、页面事件命中测试、favicon 完整实现、多窗口管理（见规划文档"不在范围"）
