# Goal — 下一步任务清单（给 Codex / AI Agent 用）

> **更新时间**：2026-08-29
> **当前状态**：窗口化轨道 **W-2 DPI 已完成**（renderer `Backend::render` + shell `render_page` 加 scale：layout 用逻辑视口 CSS px、栅格化物理分辨率 `round(logical×scale)`，整数 1x/2x 有 scale=1-vs-2 单测兜底；窗口流读 hidpi scale、脏检查含 scale、ScaleFactorChanged 重绘）。本轮**双轨并行**：主线 M-3（CSS 补全收尾）+ 并行轨道窗口化（下一阶段 **W-3 输入**，规划见 [docs/plans/2026-08-23-windowing.md](docs/plans/2026-08-23-windowing.md)）。两轨触碰不同 crate，穿插推进互不阻塞。

---

## 当前阶段定位

- **主线**：M-3 CSS 补全 — media query 求值（cascade filter 目前透传）、@layer 排序（audit B8）、revert/revert-layer（B7）、剩余 shorthand、background-image。按渲染真实页面的需求驱动裁剪范围。
- **并行轨道**：窗口化 — `muskitty-shell`（浏览器外壳）：`PlatformWindow` trait 抽象（W-1 ✅）→ DPI（W-2 ✅）→ 输入（W-3）→ Headless（W-4）→ 多标签（W-5）。
- **中期主线**：M-1 网络接轨 / M-2 交互基础排在 M-3 之后；inline formatting context 独立远期 Phase。

## 穿插节奏（双轨并行）

| 轮次 | 轨道 | 内容 | 触碰 crate |
|------|------|------|-----------|
| 已完成 | 并行 | W-1 窗口化整块（6 commit + 收尾修订：WinitWindow 私有化） | shell（新）/ renderer |
| 已完成 | 并行 | W-2 DPI（3 commit：Backend/render_page scale + 窗口流接线） | shell / renderer |
| 下一轮 | 并行 | W-3 输入（鼠标/键盘事件 → 命中测试 / 滚动） | shell |
| 穿插 | 主线 | M-3 一批（按需） | cascade / layout |

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

### 主线：M-3 CSS 补全（穿插推进）

**范围**：media query 求值 / @layer 排序（B8）/ revert + revert-layer（B7）/ 剩余 shorthand / background-image。

**细节**：待 M-3 任务拆解（按渲染真实页面需求裁剪范围后逐项列出）。

---

## 不在本轮范围（显式排除）

- Network 自研 HTTP 栈 / Fetch 接轨（保持 trait + reqwest 基础）
- 完整 inline formatting context（多 text + inline 元素同行合并、bidi）
- overflow scroll 交互、CSS Grid 高级特性（命名线/区域）
- 文本渲染性能优化（FontSystem 复用、glyph 缓存）
- 窗口化：双 RenderingContext GPU 合成、页面事件命中测试、favicon 完整实现、多窗口管理（见规划文档"不在范围"）
