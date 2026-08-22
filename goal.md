# Goal — 下一步任务清单（给 Codex / AI Agent 用）

> **更新时间**：2026-08-22
> **当前状态**：文本渲染（cosmic-text）+ 布局增强（position/overflow/grid）+ 窗口化（winit+softbuffer）+ 外部依赖解耦（layout/renderer/network）均已完成并推送。本轮执行 T-3。

---

## 当前阶段定位

- HTML/CSS/Layout/Render 四层链路打通，文字可渲染（cosmic-text 整形 + tiny-skia 光栅化），支持 position/overflow/grid 布局，可窗口显示（winit + softbuffer）
- 外部依赖已解耦：layout / renderer / network 的公共 API 不含 taffy / tiny-skia / cosmic-text / reqwest 类型，上层可抽离（见 [docs/decisions/2026-08-16-external-dependency-decoupling.md](docs/decisions/2026-08-16-external-dependency-decoupling.md)）
- **本轮目标**：T-3 换行 + 字体属性
- **中期主线（用户已确认，T-3 之后）**：M-3 CSS 补全 — media query 求值（cascade filter 目前透传）、@layer 排序（audit B8）、revert/revert-layer（B7）、剩余 shorthand、background-image。按渲染真实页面的需求驱动裁剪范围。M-1 网络接轨 / M-2 交互基础排在 M-3 之后；inline formatting context 独立远期 Phase。

---

## 任务列表

### Task 1 — T-3 换行 + 字体属性完善

**目标**：文本按容器宽度换行，`font-family` / `font-weight` / `text-align` 生效。

**位置**：
- `crates/muskitty-layout/src/text.rs`、`convert.rs`、`tree.rs`、`lib.rs`
- `crates/muskitty-renderer/src/backend/tiny_skia.rs`、`paint.rs`

**技术要点**：
1. 换行需要 taffy measure function 机制：`TaffyTree<()>` → `TaffyTree<NodeContext>`（context 携带文本内容 + 字体样式），`compute_layout` → `compute_layout_with_measure`，容器可用宽度确定后回调测量换行后的文本尺寸。
2. `font-family` → fontdb 系统字体匹配；`font-weight` → cosmic-text `Attrs::weight`；`text-align` 影响 glyph 起始 x 偏移。

**退出条件（全部满足才可 commit）**：

- [x] 多行文本 + 不同字号/字重/对齐的端到端用例通过（layout `text_wrap.rs` 5 用例 + renderer `end_to_end.rs` 换行/字重/字号/对齐用例；过程中发现并修复 `draw_text` 漏加 `run.line_y` 的多行叠行 bug）
- [x] `cd crates/muskitty-layout && cargo test` 全绿
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo fmt --all -- --check` 通过
- [x] 每子任务单 commit，message `[module] what + why`
- [x] 已 `git push origin main`（layout + 主仓库）

---

## 不在本轮范围（显式排除）

- Network 自研 HTTP 栈 / Fetch 接轨（保持 trait + reqwest 基础）
- 完整 inline formatting context（多 text + inline 元素同行合并、bidi）：T-3 只做换行，inline 流留后续
- overflow scroll 交互、CSS Grid 高级特性（命名线/区域）
- 文本渲染性能优化（FontSystem 复用、glyph 缓存）
