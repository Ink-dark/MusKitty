# 窗口化路线规划（muskitty-shell）

> **创建时间**：2026-08-23
> **依据**：`docs/research/2026-08-23-servo-window-layer-analysis.md`（Servo 窗口层分析）
> **用户决策**：① 新建 `muskitty-shell` workspace member 承载窗口抽象（按研究文档 §5）；② 窗口化作为独立轨道与 M-3（CSS 补全收尾）**并行穿插**推进
> **当前状态**：W-1 **已完成**。`muskitty-shell` workspace member 落地：`PlatformWindow` trait + `WinitWindow`（pub(crate)，构造参数含 winit 类型不外泄）+ `page::render_page` 管线 + `App::run` 便捷入口 + 迁入的 `examples/window_demo.rs`。renderer 已回归纯净（删除 window_demo、移除 winit/softbuffer dev-deps）
> **关联文件**：`crates/muskitty-shell/`（本 crate）、`crates/muskitty-renderer/Cargo.toml`（已清理）

---

## 目标

1. **W-1（本轮）**：新建 `muskitty-shell` crate，定义 `PlatformWindow` trait 抽象，把 `window_demo` 从 renderer 迁移到 shell，renderer 回归纯净（仅 `render_demo.rs` 保留）。
2. **W-2~W-5（后续）**：按研究文档阶段 2→5 演进——DPI / 输入事件 / Headless 后端 / 多标签状态管理。
3. **架构线（贯穿）**：窗口操作与具体实现解耦。shell 依赖 renderer 抽象类型（`Backend` / `RenderOutput`），公共 API 不泄漏 winit/softbuffer 类型。

**本轮交付物（W-1）**：
- `crates/muskitty-shell/` workspace member：`PlatformWindow` trait + `WinitWindow` 实现 + HTML+CSS→像素管线 + 二进制入口
- `examples/window_demo.rs` 迁入 shell，功能与现状一致（可缩放、可关闭、Esc 关闭）
- renderer 移除 window_demo 及 winit/softbuffer dev-deps
- 本规划文档

---

## 架构

### crate 结构（W-1 落地形态）

```
crates/muskitty-shell/
├── Cargo.toml              # workspace member；deps: renderer + winit/softbuffer（feature-gate）
├── src/
│   ├── lib.rs              # crate 文档 + 模块导出 + 便捷 `render_to_png`（W-4 起）
│   ├── window.rs           # PlatformWindow trait + Cursor + WindowGeometry（无外部依赖类型）
│   ├── input.rs            # InputEvent / Key / Modifiers / MouseButton（W-3 完善）
│   ├── winit_window.rs     # WinitWindow: PlatformWindow（winit + softbuffer）
│   ├── page.rs             # render_page(html, css, w, h) -> RenderOutput 管线
│   ├── app.rs              # ApplicationHandler 实现（W-1 单 WebView）
│   └── main.rs             # 二进制入口
└── examples/
    └── window_demo.rs      # 从 renderer 迁入，改用 PlatformWindow 构造窗口
```

**依赖方向**：`muskitty-shell → muskitty-renderer → muskitty-layout → …`（单向）。
shell 依赖 html5-parser/cssom/cascade/css/dom（render 管线的输入侧，本地 path 可解析）。

### `PlatformWindow` trait（W-1 最小集）

参照研究文档 §3.1，但裁剪到 Muskitty 软件渲染现实：

```rust
pub trait PlatformWindow {
    fn id(&self) -> u64;
    fn hidpi_scale_factor(&self) -> f32;                 // W-2 真正生效
    fn request_repaint(&self);
    fn geometry(&self) -> WindowGeometry;                // 逻辑 px (x, y, w, h)
    fn set_cursor(&self, cursor: Cursor);
    fn set_fullscreen(&self, state: bool);
    fn present(&mut self, data: &[u8], width: u32, height: u32); // RGBA → 显示目标
}
// W-3 追加：fn handle_event(&mut self, event: InputEvent) -> bool;
```

**明确不做**（研究文档 §3.6 低价值项）：
- `rendering_context()` / 双 RenderingContext GPU 合成 —— tiny-skia 软件渲染 + softbuffer 不需要，切 wgpu/vulkan 时再抽象
- 对话框 / IME / 无障碍 —— 无真实需求，不预加

**`present` 参数用裸像素 `(data, width, height)`**（RGBA），不用 `&RenderOutput`：trait 零 renderer 类型依赖，RGBA→0RGB 转换是各窗口实现（softbuffer）自己的事；将来换渲染后端 trait 不变。`render_page` 产出 `RenderOutput::Pixels`，`page.rs` 解出 RGBA 传给 `present`。

### 与 renderer 的边界（保持不变）

- renderer 保持纯净：`Backend` trait / `TinySkiaBackend` / `RenderOutput::Pixels` 公共 API 不变（W-2 可能增量加 scale，见下）
- `examples/render_demo.rs`（HTML+CSS→PNG）保留在 renderer
- `examples/window_demo.rs` 从 renderer 删除，迁入 shell

### winit/softbuffer 依赖形态

`winit` / `softbuffer` 放 shell 的**可选依赖 + feature gate**（对齐 network 的 `reqwest-backend` 模式）：

```toml
[features]
default = ["winit-backend"]
winit-backend = ["dep:winit", "dep:softbuffer"]

[dependencies]
winit = { version = "0.30", optional = true }
softbuffer = { version = "0.4", optional = true }
```

理由：W-4 的 HeadlessWindow + `render_to_png` 不需要 winit，CI 无窗口环境（Linux runner 缺 xkbcommon 系统库）也能编译测试。W-1 阶段 `default` 即开箱可跑。

---

## 并行策略（与 M-3 穿插）

M-3（CSS 补全）与窗口化触碰**不同 crate**，无文件冲突：

| 轨道 | 触碰 crate |
|------|-----------|
| M-3 | cascade（media query 求值 / @layer 排序 / revert / shorthand / background-image）、layout |
| 窗口化 | shell（新）、renderer（仅 W-2 改 `Backend` 签名） |

**建议节奏**：每轮一个窗口化阶段收尾（含测试+commit）→ 切回 M-3 一批 → 下一个窗口化阶段。W-1 是一次性较大的 crate 迁移，建议先整块完成再穿插。

---

## 阶段路线

### W-1 — muskitty-shell crate + PlatformWindow trait + 迁移（本轮）

**任务**（每个 commit 独立可编译 + 测试通过）：

| # | commit | 内容 |
|---|--------|------|
| 1 | `[shell] crate skeleton + workspace member` | 新建 crate，`Cargo.toml`（feature gate 如上），根 `Cargo.toml` members 加 `"crates/muskitty-shell"`，`lib.rs` 骨架 + crate doc |
| 2 | `[shell] PlatformWindow trait + Cursor/WindowGeometry` | `window.rs`：trait 最小集 + 枚举，无外部依赖类型泄漏 |
| 3 | `[shell] page.rs render pipeline` | `render_page(html, css, w, h) -> RenderOutput`：从 window_demo 的 `render_page` 抽出（parse→cascade→layout→paint→render），调用 renderer 公共 API |
| 4 | `[shell] WinitWindow: PlatformWindow + app/main` | `winit_window.rs`（winit 事件循环 + softbuffer 表面，RGBA→0RGB 在 `present` 内）+ `app.rs` + `main.rs`，功能对齐 window_demo（可缩放、可关闭） |
| 5 | `[shell] move window_demo to shell examples` | 示例迁入 shell（后续经用户决策改为 `App::run` 入口，`WinitWindow` 降 `pub(crate)`）；删除 renderer 的 `window_demo.rs` |
| 6 | `[renderer] drop winit/softbuffer dev-deps` | renderer `Cargo.toml` 清理，验证 `cargo check -p muskitty-renderer` 零 warning |

**设计修订（W-1 收尾）**：winit 窗口必须经事件循环创建，无法像 `ReqwestFetcher` 那样内部自建资源，故 `WinitWindow::new(id, Rc<Window>)` 的构造参数天然含 winit 类型。用户决策：**严格合规**——`winit_window` 模块降 `pub(crate)`（`WinitWindow`/`new`/`rgba_to_0rgb` 均不外泄），窗口创建由 `App::run` 封装；示例改为 `App::run(HTML, CSS)`。直接构造 `PlatformWindow` 的演示价值迁移到 W-4 的 `HeadlessWindow`（可无参构造）。pub API 仅剩 `PlatformWindow` / `App::run` / `page::render_page`，grep 零 winit/softbuffer 类型。

**技术要点**：
- RGBA→0RGB 转换抽成**纯函数**（如 `WinitWindow::rgba_to_0rgb` 或 `window.rs` 顶层函数）以便无窗口单测
- `page.rs` 管线返回 `RenderOutput`，不碰 softbuffer 格式；`present` 才做格式转换
- W-1 的 `hidpi_scale_factor()` 返回 1.0（真实值 W-2 接）

**退出条件**：
- `cargo check --workspace` / `cargo test --workspace` 通过，`cargo clippy --all-targets -- -D warnings` 零警告
- `cargo run -p muskitty-shell --example window_demo` 渲染与现状一致（HTML+CSS→真窗口，缩放重渲染、Esc 关闭）
- `grep` shell 的 `pub` 导出无 winit/softbuffer 类型；renderer 无 winit/softbuffer dev-deps
- 单测：`rgba_to_0rgb` 转换 + `page.rs` 管线像素断言（含纯色块 + 文本墨迹）

---

### W-2 — DPI / HiDPI 支持

**任务**：
1. `hidpi_scale_factor()` 从 winit window 读取，处理 `ScaleFactorChanged` 事件 → 通知 app 重渲染
2. `render_page` 增加 scale 参数：layout 用**逻辑**视口（CSS px），渲染用**物理**分辨率（`logical × scale`）
3. renderer `Backend` 支持 scale：**先读 `tiny_skia.rs` 全流程**（rect/文本 transform/clip mask 构造）决定注入点；优先方案——命令坐标保持 CSS px，后端绘制时统一应用 `Transform::from_scale(scale, scale)`（清晰，非模糊放大），canvas 白底按物理尺寸填充

**技术取舍**：
- **整数倍（1x/2x）先行**（研究文档风险缓解），小数倍延后
- `Backend::render` 改签名加 `scale` 或新增 `render_scaled` —— 需同步 render_demo.rs 与 renderer 单测（renderer 未发布，可改；一次性独立 commit）

**退出条件**：
- 同组命令 scale=1 与 scale=2：输出分辨率分别为 `w×h` 与 `2w×2h`，且 scale=2 的左上角像素与 scale=1 一致（颜色正确而非简单插值）
- HiDPI 屏（scale=2）文字/边框清晰
- `ScaleFactorChanged` 触发重渲染（单测：注入新 scale → app 用新 scale 产出正确尺寸像素）

---

### W-3 — 输入事件抽象

**任务**：
1. `input.rs` 完善 `InputEvent`（Keyboard/MouseButton/MouseMove/MouseWheel/Touch，参照研究文档 §3.3）
2. winit 事件 → `InputEvent` 转换
3. `PlatformWindow::handle_event(&mut self, event: InputEvent) -> bool`（true=已处理，不转发）
4. 事件分层（Servo §1.5）：先 shell 快捷键（Esc 关闭、Ctrl+R 刷新），未处理才转页面

**范围裁剪**：页面级命中测试（事件→元素）依赖 layout 几何 + 命中算法，**不在本轮**（单列延后项）。W-3 只做 shell 快捷键层 + 事件分发结构。

**退出条件**：
- Esc 关闭窗口、Ctrl+R 刷新（重新 parse→render）
- `handle_event` 返回语义正确（快捷键 consumed，其余 false）
- 单测：InputEvent 转换 + 快捷键匹配（无需真实窗口）

---

### W-4 — Headless 后端（测试需要）

**任务**：
1. `headless_window.rs`：`HeadlessWindow: PlatformWindow`，`present` 保存像素 / 写 PNG
2. `render_to_png(html, css, path)` 便捷函数（lib 顶层）
3. 集成到测试：shell 测试无窗口环境验证渲染

**架构点**：`default-features = false` 时无需 winit/softbuffer 编译（feature gate 在此兑现价值）。

**退出条件**：
- 无窗口环境跑通 `render_to_png`（`cargo test -p muskitty-shell --no-default-features` 全绿）
- 输出 PNG 像素与 renderer 直接渲染一致
- CI 可无窗口跑 shell 渲染测试

---

### W-5 — 窗口状态管理（多标签）

**任务**：
1. `app.rs` 升级为 `WebViewCollection`：多 WebView（每份 HTML+CSS+渲染状态）
2. 标签切换：Ctrl+T 新建、Ctrl+W 关闭、Ctrl+1~9 / Ctrl+PageUp/Down 切换
3. 脏位标记：`needs_repaint` / `close_scheduled`（Servo §1.7 延迟更新模式）

**范围裁剪**：favicon 依赖 network + `<link rel=icon>`，**降级为占位**（无 network 接轨前意义有限）。

**退出条件**：
- 可创建/关闭多个标签，切换正确刷新
- 快捷键处理经 `handle_event` 分层正确分发

---

## 验证命令

每个阶段 commit 前（crate 目录或 workspace 根）：

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo run -p muskitty-shell --example window_demo   # W-1 手工验证真窗口
```

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 过早抽象 | trait 只含当前需要的 6 个方法，RenderingContext/IME/对话框一律不预加（Simplicity） |
| 破坏现有 demo | W-1 迁移时逐行对照 window_demo 功能（可缩放/关闭/Esc），行为不变 |
| winit Linux CI 缺系统库（xkbcommon） | winit/softbuffer feature-gate，`--no-default-features` 无头测试不受影响；真窗口测试本地跑 |
| DPI 处理复杂 | 整数倍先行；先读 tiny_skia.rs 定缩放注入点，再写代码 |
| `Backend::render` 签名改动波及 renderer 测试 | W-2 独立 commit 一次性同步 render_demo + 单测；renderer 未发布可改 |
| 新 crate 依赖拓扑 | shell 依赖的 6 个 path crate 本地均存在（fetch-crates），workspace 可解析 |

---

## 不在范围（显式排除）

- 双 RenderingContext GPU 合成 / wgpu-vulkan 后端（研究文档 §3.6 低价值）
- 页面事件命中测试（依赖 layout 几何 + 命中算法，单列远期）
- favicon 完整实现（依赖 network + `<link rel=icon>`）
- 多窗口 / 多显示器布局管理
- 动画 / 过渡渲染、合成器

---

## 下一步

- [x] 用户决策：新建 muskitty-shell + 并行穿插
- [x] W-1 全部 6 个 commit + 收尾修订（设计决策：WinitWindow 私有化）
- [x] 规划落地后同步 `goal.md`（加入窗口化并行轨道，穿插节奏已排布）
- [x] M-3 与 W-1 的穿插节奏由用户在 `goal.md` 中排布
- [ ] **W-2（DPI）启动**：与 M-3 CSS 补全并行穿插（见 goal.md 节奏表）
