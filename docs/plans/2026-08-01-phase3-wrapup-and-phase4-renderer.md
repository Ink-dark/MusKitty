# Phase 3 收尾 + Phase 4 Renderer 启动计划

> **创建时间**：2026-08-01
> **前置状态**：Phase 3 Layout（L-0~L-6 + 审计修复 B1-B5/B7/B8）已完成，46 个 layout 测试全绿
> **用户决策**：自行决定方向，目标「把浏览器跑起来」；Renderer 后端优先 GPUI（Zed 编辑器的 GPU UI 框架）

## 目标

1. **Phase 3 收尾**（文档同步 + crate 剥离）
2. **Phase 4 Renderer 启动**（架构设计 + GPUI 集成调研 + 最小可运行 demo）

最终交付物：能在窗口里渲染一个简单 HTML 页面（带 CSS 布局），证明 DOM→CSS→Layout→Render 全链路打通。

---

## Part A：Phase 3 收尾（3 个 commit）

### A-1: 文档同步 — 标记 Phase 3 完成

**文件**：
- `PROGRESS.md`：总览表加 `muskitty-layout` 行；"下一步"更新；加 Phase 3 章节
- `CLAUDE.md`：项目阶段描述改为"Phase 3 已完成，Phase 4 即将启动"；架构图加 layout crate
- `README.md`：workspace 成员表更新

**步骤**：
1. 更新 PROGRESS.md 总览表加 layout 行（46 tests, 本地 v0.1.0 未发布）
2. 在 PROGRESS.md 加 "## Phase 3 (Layout 层) — 已完成" 章节，列 L-0~L-6 + 审计修复
3. 更新"下一步"：Phase 4 Renderer 为下一目标
4. 更新 CLAUDE.md 当前阶段描述
5. 提交：`[docs] mark Phase 3 Layout complete`

### A-2: 剥离 muskitty-cascade 为独立仓库

**理由**：cascade API 已稳定（71 tests），与 layout 的协作接口已固化。剥离后可独立 CI/发布。

**步骤**：
1. 在 `crates/muskitty-cascade/` 内 git init + 初始 commit
2. 加 `[workspace]` 块（hard extraction 模式）
3. path 依赖改为 `../muskitty-css`、`../muskitty-selectors`、`../muskitty-cssom`、`../muskitty-dom`
4. 加 `.github/workflows/ci.yml` + `publish.yml` + `scripts/setup-deps.sh`
5. 加 `LICENSE` + `README.md`
6. 主仓库 `Cargo.toml` `members` 移除 cascade，`exclude` 加入
7. 主仓库 `.gitignore` 加 `crates/muskitty-cascade/`
8. 推送到 `muskitty-dev/muskitty-cascade`
9. 发布 v0.1.0 到 crates.io
10. 提交：`[workspace] extract muskitty-cascade as standalone crate`

### A-3: 剥离 muskitty-layout 为独立仓库

**理由**：layout 依赖 cascade，cascade 剥离后 layout 也应剥离保持依赖拓扑一致。

**步骤**：同 A-2，目标仓库 `muskitty-dev/muskitty-layout`。
- 注意：layout 的 `dev-dependencies` 包含 `muskitty-html5-parser`，CI 脚本需克隆
- 提交：`[workspace] extract muskitty-layout as standalone crate`

---

## Part B：Phase 4 Renderer 启动（4 个 commit）

### B-0: GPUI 调研与后端决策

**目标**：评估 GPUI 集成可行性，确定渲染层架构。

**调研内容**：
1. GPUI 的 crate 结构（是否在 crates.io？需要 git 依赖？）
2. GPUI 的渲染原语（rect/text/image/transform）
3. GPUI 的窗口系统集成（winit？自带？）
4. GPUI 的文本测量能力（是否需要配合 cosmic-text？）
5. GPUI 与 taffy 的协作模式（taffy 输出 LayoutResult → GPUI 渲染指令）

**交付物**：`docs/research/gpui-integration.md` 调研报告

**风险点**：
- GPUI 可能不在 crates.io（Zed 用 git 依赖）
- GPUI 可能强绑定 macOS/Linux，Windows 支持度未知
- GPUI 可能是完整 UI 框架而非纯渲染层，集成成本高

**备选方案**（如果 GPUI 不可用）：
- tiny-skia（纯 Rust 软件渲染，简单但慢）
- wgpu + 自绘（复杂但灵活）
- vello（Linebender 的 GPU 2D 渲染库）

### B-1: muskitty-renderer crate 骨架

**目标**：新建 `crates/muskitty-renderer/` crate，定义渲染层接口。

**架构**（前后端分离）：
```
muskitty-layout 的 LayoutResult (DOM + ComputedStyle + 位置/尺寸)
        │
        ▼  build_render_tree
RenderTree (带样式信息的渲染节点树)
        │  paint
        ▼
RenderCommand[] (绘制指令列表：rect/text/clip/transform)
        │  backend (GPUI / tiny-skia / ...)
        ▼
像素输出 (窗口/图片)
```

**文件**：
- `crates/muskitty-renderer/Cargo.toml`
- `src/lib.rs`：crate 文档 + 模块声明
- `src/render_tree.rs`：RenderTree / RenderNode 类型
- `src/command.rs`：RenderCommand 枚举（Rect/Text/Clip/Transform）
- `src/paint.rs`：paint 函数（LayoutResult → RenderCommand[]）
- `src/backend/mod.rs`：Backend trait（抽象渲染后端）
- `src/backend/gpui.rs`：GPUI 后端实现（B-0 调研后填充）

**步骤**：
1. 创建 crate 骨架 + 加入 workspace members
2. 定义 RenderTree / RenderCommand 类型
3. 定义 Backend trait（`fn render(&mut self, commands: &[RenderCommand])`）
4. 实现 paint 函数（从 LayoutResult + ComputedStyle 构造 RenderTree → RenderCommand[]）
5. 写 Backend trait 的 mock 实现 + 单元测试
6. 提交：`[renderer] R-1: crate skeleton + RenderTree/Command types + paint function`

### B-2: paint 实现 — 矩形 + 背景色 + 边框

**目标**：实现最基本的绘制指令生成。

**支持的 CSS 属性**：
- `background-color`：纯色矩形
- `border` + `border-color`：边框矩形
- `width`/`height`：从 LayoutResult 取
- `border-radius`：圆角矩形（推迟，先支持直角）

**RenderCommand 枚举**：
```rust
pub enum RenderCommand {
    Rect {
        x: f32, y: f32, width: f32, height: f32,
        background: Option<Color>,
        border: Option<Border>,
    },
    Text { /* 推迟到 B-3 */ },
    Clip { /* 推迟 */ },
}
```

**步骤**：
1. 写 paint 测试：给定 LayoutResult + ComputedStyle（含 background-color），断言生成正确的 RenderCommand
2. 实现 paint：遍历 LayoutResult.nodes，每个节点生成一个 Rect 命令
3. 颜色解析：CSS color 字符串（`red`/`#ff0000`/`rgb(255,0,0)`）→ Color 类型
4. 提交：`[renderer] R-2: paint implementation (rect + background-color + border)`

### B-3: GPUI 后端集成（或备选）

**前置**：B-0 调研完成，确定 GPUI 可用性。

**如果 GPUI 可用**：
1. 在 `backend/gpui.rs` 实现 Backend trait
2. 创建 GPUI 应用入口（窗口 + 主循环）
3. 接收 RenderCommand[] → GPUI 元素树
4. 集成测试：HTML + CSS → layout → paint → GPUI 窗口显示
5. 提交：`[renderer] R-3: GPUI backend integration`

**如果 GPUI 不可用**（Windows 支持差/不在 crates.io）：
1. 改用 tiny-skia（纯 Rust 软件渲染）
2. 在 `backend/tiny_skia.rs` 实现 Backend trait
3. 输出到 PNG 文件（验证渲染结果，无需窗口）
4. 提交：`[renderer] R-3: tiny-skia backend (software rendering to PNG)`

### B-4: 端到端 demo

**目标**：完整 pipeline 跑通，能在窗口/图片中看到渲染结果。

**测试用例**：
```html
<div style="background: red; padding: 20px">
  <div style="background: blue; width: 100px; height: 100px"></div>
  <div style="background: green; width: 50%; height: 50px"></div>
</div>
```

**步骤**：
1. 写 demo binary：`examples/render_demo.rs`
2. 输入：HTML + CSS 字符串
3. 流程：parse HTML → parse CSS → cascade → compute → layout → paint → render
4. 输出：窗口显示（GPUI）或 PNG 文件（tiny-skia）
5. 提交：`[renderer] R-4: end-to-end render demo`

---

## 优先级与执行顺序

| 优先级 | 批次 | 内容 | 预计 commit 数 |
|--------|------|------|----------------|
| P0 | A-1 | 文档同步 | 1 |
| P0 | A-2 | 剥离 cascade | 1 |
| P0 | A-3 | 剥离 layout | 1 |
| P1 | B-0 | GPUI 调研 | 0（仅文档）|
| P1 | B-1 | renderer 骨架 | 1 |
| P1 | B-2 | paint 实现 | 1 |
| P2 | B-3 | 后端集成 | 1 |
| P2 | B-4 | 端到端 demo | 1 |

**总 commit 数**：7-8 个

---

## 风险与备选

1. **GPUI 不可用风险**：Zed 的 GPUI 可能不在 crates.io，需 git 依赖；Windows 支持可能不完整。
   - **备选**：tiny-skia 软件渲染（纯 Rust，跨平台，简单）
   - **决策点**：B-0 调研后确定

2. **cascade/layout 剥离风险**：剥离后 CI 脚本需克隆所有依赖，可能遇到 GitHub 限流。
   - **缓解**：`GH_TOKEN` 注入（已有模式）

3. **文本测量缺失**：当前 layout 跳过 text 节点，renderer 也无法渲染文本。
   - **推迟**：B-3 之后再做文本测量（可能用 cosmic-text 或 swash）

4. **颜色解析缺失**：cascade 当前不解析 `color`/`background-color` 为 Color 类型，仅保留为 ComponentValue。
   - **B-2 内处理**：在 renderer 层做轻量颜色解析（CSS Color L4 子集）

---

## 延后项

- **CSS Grid 完整属性**：grid-template-columns/rows 等（layout 增强）
- **position: absolute/relative/fixed**（layout 增强）
- **overflow / scroll**（layout + renderer 协作）
- **文本测量**（layout + renderer 协作，需字体库）
- **多字体 / @font-face**（renderer 增强）
- **CSS shorthand 展开**（Bug 6: `flex` 简写，需 CSSOM 层改动）
- **DOM Events / innerHTML**（DOM API 扩展）

---

## 质量门禁

每个批次 commit 前依次执行：
```powershell
cargo fmt -p <crate> -- --check
cargo test -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
```

剥离批次额外执行全 workspace 回归：
```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
