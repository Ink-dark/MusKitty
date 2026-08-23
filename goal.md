# Goal — 下一步任务清单（给 Codex / AI Agent 用）

> **更新时间**：2026-08-23
> **当前状态**：T-3 换行 + 字体属性已完成并推送。本轮**双轨并行**：主线 M-3（CSS 补全收尾）+ 并行轨道窗口化（W-1 进行中，规划见 [docs/plans/2026-08-23-windowing.md](docs/plans/2026-08-23-windowing.md)，依据 [docs/research/2026-08-23-servo-window-layer-analysis.md](docs/research/2026-08-23-servo-window-layer-analysis.md)）。两轨触碰不同 crate，穿插推进互不阻塞。

---

## 当前阶段定位

- **主线**：M-3 CSS 补全 — media query 求值（cascade filter 目前透传）、@layer 排序（audit B8）、revert/revert-layer（B7）、剩余 shorthand、background-image。按渲染真实页面的需求驱动裁剪范围。
- **并行轨道**：窗口化 — 新建 `muskitty-shell`（浏览器外壳）：`PlatformWindow` trait 抽象 + `WinitWindow` 迁移 + DPI + 输入 + Headless + 多标签。
- **中期主线**：M-1 网络接轨 / M-2 交互基础排在 M-3 之后；inline formatting context 独立远期 Phase。

## 穿插节奏（双轨并行）

| 轮次 | 轨道 | 内容 | 触碰 crate |
|------|------|------|-----------|
| 本轮 | 并行 | W-1 窗口化整块（6 commit：骨架 → trait → 管线 → WinitWindow → 迁移 → 清理） | shell（新）/ renderer |
| 穿插 | 主线 | M-3 一批（按需） | cascade / layout |

**节奏**：每轮一个窗口化阶段收尾（含测试 + commit）→ 切回 M-3 一批 → 下一窗口化阶段。两轨无文件冲突（cascade/layout vs shell/renderer）。

---

## 任务列表

### 并行轨道：W-1 窗口化（进行中）

**目标**：`muskitty-shell` crate + `PlatformWindow` trait 抽象，`window_demo` 从 renderer 迁入，renderer 回归纯净。功能与现状一致（可缩放、可关闭、Esc 关闭）。

**Commit 序列**：

- [ ] C-1 crate 骨架 + workspace member（Cargo.toml feature-gate winit-backend + 6 path deps + lib.rs；根 members 加入）
- [ ] C-2 `PlatformWindow` trait 最小集 + `Cursor`/`WindowGeometry`（无外部依赖类型泄漏）
- [ ] C-3 `page.rs` 渲染管线 `render_page(html, css, w, h) -> RenderOutput`
- [ ] C-4 `WinitWindow: PlatformWindow` + `app.rs` + `main.rs`（RGBA→0RGB 在 present 内，抽纯函数）
- [ ] C-5 `window_demo` 迁入 shell examples，删除 renderer 版本
- [ ] C-6 renderer 移除 winit/softbuffer dev-deps

**退出条件（全部满足才可 commit）**：

- [ ] `cargo check --workspace` / `cargo test --workspace` 通过
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo run -p muskitty-shell --example window_demo` 真窗口渲染与现状一致
- [ ] shell `pub` 导出无 winit/softbuffer 类型；renderer 无 winit/softbuffer dev-deps
- [ ] 单测：RGBA→0RGB 转换 + page 管线像素断言

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
