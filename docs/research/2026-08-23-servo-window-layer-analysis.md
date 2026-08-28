# Servo 架构分析与 Muskitty 演进指导

> **日期**：2026-08-23
> **参考代码**：servo/servo (2026-08-23 shallow clone)
> **关联 Muskitty 文件**：`crates/muskitty-renderer/examples/window_demo.rs`

---

## 1. Servo 并行渲染与排布管线

### 1.1 Constellation 架构

Servo 的并行模型围绕 **Constellation**（协调器）和三个子系统：Script、Layout、Paint。

```
                    ┌─────────────────┐
                    │   Constellation │  ← 单线程协调器
                    │   (路由消息)      │
                    └────────┬────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
    ┌─────▼─────┐     ┌─────▼─────┐     ┌─────▼─────┐
    │ Script    │     │ Script    │     │ Script    │  ← 每个 Pipeline 一个
    │ Thread 1  │     │ Thread 2  │     │ Thread 3  │     (可共享同源)
    └─────┬─────┘     └─────┬─────┘     └─────┬─────┘
          │                  │                  │
    ┌─────▼─────┐     ┌─────▼─────┐     ┌─────▼─────┐
    │ Layout    │     │ Layout    │     │ Layout    │  ← 每个 Pipeline 独立
    │ (Rayon)   │     │ (Rayon)   │     │ (Rayon)   │     并行布局
    └─────┬─────┘     └─────┬─────┘     └─────┬─────┘
          │                  │                  │
          └──────────────────┼──────────────────┘
                             │
                    ┌────────▼────────┐
                    │      Paint      │  ← WebRender GPU 合成
                    │   (WebRender)   │
                    └─────────────────┘
```

**关键文件**：
- `components/constellation/constellation.rs` — 协调器主循环
- `components/constellation/pipeline.rs` — Pipeline 定义
- `components/constellation/event_loop.rs` — EventLoop（ScriptThread 包装）

### 1.2 无死锁协议

Servo 设计了严格的**有向阻塞图**（`constellation.rs:63-85`）：

```
Constellation → 可阻塞 → Paint
Constellation → 可阻塞 → Embedder
Script → 可阻塞 → 任何（除 Script）
阻塞是传递的
任何事物不能阻塞自身！
```

IPC 通道使用专用路由线程，防止 `sender.send()` 因缓冲区满而阻塞。

### 1.3 并行布局（Rayon Work-Stealing）

Servo 使用 Rayon 在**单次布局 pass 内**并行化多个阶段：

#### 并行调度策略（`layout/context.rs:51-76`）

```rust
fn should_parallelize(&self, number_of_jobs: usize) -> bool {
    self.allow_parallel_layout && number_of_jobs >= self.parallelism_job_count_minimum
}
fn should_parallelize_layout(&self, jobs: impl Iterator<Item = usize>) -> bool {
    self.allow_parallel_layout &&
        jobs.filter(|job| *job >= self.parallelism_job_size_minimum)
            .count() >= self.parallelism_job_count_minimum
}
```

根据**作业数量**和**子树大小**自适应选择并行或串行。

#### 并行模式 1：块级子元素布局（`flow/mod.rs:839-878`）

```rust
// 无浮动时并行
child_boxes
    .par_iter()                    // Rayon 并行迭代器
    .map(|child_box| {
        let mut child_positioning_context = PositioningContext::default();
        let fragment = child_box.borrow().layout(...);
        (fragment, child_positioning_context)
    })
    .collect_into_vec(&mut layout_results);
// 串行后处理：放置片段、调整静态位置
```

**关键设计**：每个并行子元素获得**独立的 `PositioningContext`**，消除竞争。浮动元素强制串行（`SequentialLayoutState`）。

#### 并行模式 2：绝对定位布局（`positioned.rs:414-460`）

```rust
boxes.par_iter_mut()
    .map(|hoisted_box| {
        let mut new_hoisted_boxes = Vec::new();
        let new_fragment = hoisted_box.layout(...);
        (new_fragment, new_hoisted_boxes)
    })
    .unzip_into_vecs(&mut new_fragments, &mut new_hoisted_boxes);
```

#### 并行模式 3：Flexbox 布局（`flexbox/layout.rs:1191-1263`）

```rust
// Flex lines 并行
lines.par_drain(..).map(construct_line).collect()

// 每行内 flex items 并行
items.into_par_iter()
    .zip(item_used_main_sizes.into_par_iter())
    .map(|(item, used_main_size)| { ... })
    .collect()
```

#### 并行模式 4：表格布局（`table/layout.rs:1152-1188`）

```rust
// 二维并行：行 × 列
self.table.slots
    .par_iter()                    // 并行行
    .map(|row_slots| {
        row_slots
            .par_iter()            // 并行列
            .map(|slot| layout_table_slot(...))
            .collect()
    })
    .collect()
```

#### 并行模式 5：Box Tree 构建（`flow/construct.rs:293-307`）

```rust
self.block_level_boxes
    .into_par_iter()
    .map(|block_level_job| block_level_job.finish(context))
    .collect()
```

### 1.4 增量布局（LayoutRoots）

Servo 独有的**增量片段树重布局**（`layout/layout_root.rs:14-128`）：

- `LayoutRoot` 是一个绝对定位片段，不向上泄漏 fixed-position 片段
- 损坏传播时，如果损坏可隔离到某个 LayoutRoot，向上流动的 `Relayout` 损坏转为 `DescendantCollectedAsLayoutRoot`
- 后续布局可**仅重布局该子树**，无需遍历整棵树
- 如果失败（如新的 fixed-position 元素逃逸），回退到全量布局

**Chromium 无等价机制**：LayoutNG 的不可变片段树支持增量缓存，但无此精细度的子树隔离。

### 1.5 完整 Reflow 管线（`layout_impl.rs:980-1374`）

```
handle_reflow(reflow_request)
  │
  ├─ can_skip_reflow_request_entirely()  // 早期退出优化
  │
  ├─ restyle_and_build_trees()
  │    │
  │    ├─ [Phase 1: Style Recalculation]
  │    │    driver::traverse_dom(&recalc_style_traversal, token, rayon_pool)
  │    │    // Stylo/Sparkle + Rayon 并行样式重算
  │    │
  │    ├─ [Phase 2: Box Tree Construction]
  │    │    compute_damage_and_rebuild_box_tree()
  │    │    // 增量损坏传播 + 并行 box 构建
  │    │
  │    └─ [Phase 3: Fragment Tree Layout]
  │         box_tree.layout(context, viewport_size)
  │         // Rayon 并行 block/flex/table/abspos 布局
  │
  ├─ build_stacking_context_tree()
  │
  ├─ build_display_list()
  │    DisplayListBuilder::build()
  │    // 串行 paint 遍历（必须按 paint order）
  │
  └─ handle_accessibility_tree_update()
```

### 1.6 多进程架构（可选）

```rust
// constellation/event_loop.rs:117-189
let event_loop = if opts::get().multiprocess {
    Self::spawn_in_process(constellation, initial_script_state)?
} else {
    Self::spawn_in_thread(constellation, initial_script_state)
};
```

- 每个 EventLoop（ScriptThread）可运行在独立 OS 进程中
- 使用 `gaol` 沙箱（macOS/Linux）
- **不同于 Chromium**：Servo 默认不按 site 隔离；多进程模式下，同源 Pipeline 仍可共享 EventLoop

---

## 2. Servo vs Chromium 并行策略对比

| 维度 | Servo | Chromium |
|------|-------|----------|
| **主要并行策略** | 进程内线程并行（Rayon） | 进程间隔离（Site Isolation） |
| **布局并行** | ✅ Rayon work-stealing（block/flex/table/abspos） | ❌ 单线程（Blink main thread） |
| **样式并行** | ✅ Stylo + Rayon 并行选择器匹配 | ❌ 单线程样式重算 |
| **Paint 并行** | ✅ WebRender GPU 合成 | ✅ Compositor worker threads + Viz tiled raster |
| **增量布局** | ✅ LayoutRoots 子树隔离 | ⚠️ LayoutNG 缓存（无子树隔离） |
| **进程模型** | 可选多进程（per-EventLoop） | 强制多进程（per-site） |
| **安全隔离** | Rust 编译期保证 | OS 进程沙箱 |
| **崩溃隔离** | 无进程级隔离（默认） | 进程级隔离 |
| **GPU 访问** | WebRender 进程内 | Viz GPU 进程（集中式） |
| **跨站 iframe** | 进程内 Pipeline（概念分离） | OOPIF（跨进程合成） |

### Chromium 做得更好的地方

1. **进程级崩溃隔离**：一个 site 崩溃不影响其他 site
2. **Site Isolation 安全模型**：防御 Spectre 类攻击
3. **生产级成熟度**：~98% WPT 通过率，大规模部署验证
4. **Viz GPU 调度**：集中式 GPU 资源管理，跨进程帧聚合

### Servo 做得更好的地方

1. **布局内并行**：Rayon work-stealing 在单次 pass 内利用多核
2. **自适应并行/串行调度**：根据作业数量和子树大小自动选择
3. **PositioningContext 隔离**：并行 worker 无竞争
4. **LayoutRoot 增量布局**：精细度子树隔离重布局
5. **内存安全**：Rust 所有权模型消除整类 bug（use-after-free、data race）
6. **更少的 IPC 开销**：进程内通信 vs Mojo IPC

---

## 3. 对 Muskitty 的价值

### 3.1 并行布局 — ⭐⭐⭐⭐⭐ 最高价值

**Servo 做法**：用 Rayon `par_iter` 并行布局 block/flex/table 的子元素，每个 worker 获得独立 `PositioningContext`。

**Muskitty 价值**：
- 当前 `muskitty-layout` 使用 taffy 0.12，布局是单线程
- Muskitty 的 Rust 代码天然线程安全（零 unsafe）
- 可以直接借鉴 Servo 的并行模式

**建议实现路径**：
1. 在 `muskitty-layout` 中引入 Rayon 依赖
2. 对 block/flex 子元素布局使用 `par_iter`
3. 实现 `PositioningContext` 隔离模式
4. 添加 `should_parallelize()` 启发式判断

### 3.2 增量布局（LayoutRoots） — ⭐⭐⭐⭐ 高价值

**Servo 做法**：检测绝对定位子树的损坏隔离，仅重布局该子树。

**Muskitty 价值**：
- 当前 Muskitty 每次 resize 或样式变化都全量重布局
- 增量布局可显著减少计算量
- 适合单页应用（SPA）场景

### 3.3 并行样式重算 — ⭐⭐⭐⭐ 高价值

**Servo 做法**：Stylo + Rayon 并行选择器匹配。

**Muskitty 价值**：
- 当前 `muskitty-cascade` 是单线程
- 选择器匹配是"尴尬并行"（embarrassingly parallel）
- 可直接用 Rayon 并行化

### 3.4 自适应并行调度 — ⭐⭐⭐ 中价值

**Servo 做法**：根据作业数量和子树大小决定是否并行。

**Muskitty 价值**：
- 避免小树的 Rayon 开销
- 启发式阈值可配置

### 3.5 无死锁协议 — ⭐⭐ 低价值（当前）

**评估**：Muskitty 当前无多线程协调需求，但未来多标签时可参考。

---

## 4. Muskitty 并行管线演进路径

### 阶段 1：Rayon 并行布局（立即可做）

**目标**：在 `muskitty-layout` 中引入并行布局。

**步骤**：
1. 添加 `rayon` 依赖到 `muskitty-layout`
2. 实现 `should_parallelize()` 启发式
3. 对 block/flex 子元素使用 `par_iter`
4. 实现 `PositioningContext` 隔离

**退出条件**：
- `cargo test --workspace` 全绿
- 并行布局结果与串行一致
- 基准测试显示多核加速

### 阶段 2：并行样式重算

**目标**：在 `muskitty-cascade` 中并行选择器匹配。

**步骤**：
1. 添加 `rayon` 依赖到 `muskitty-cascade`
2. 对元素选择器匹配使用 `par_iter`
3. 验证与串行结果一致

**退出条件**：
- 并行样式重算正确
- 测试全绿

### 阶段 3：增量布局（LayoutRoots）

**目标**：支持子树级增量重布局。

**步骤**：
1. 在 fragment tree 中标记 LayoutRoot
2. 实现损坏传播的 LayoutRoot 隔离
3. 实现 `try_layout()` 子树重布局

**退出条件**：
- 绝对定位子树可独立重布局
- 全量布局作为 fallback

### 阶段 4：管线协调（多标签时）

**目标**：支持多 Pipeline 并行。

**步骤**：
1. 实现 Constellation 协调器
2. 每个 Pipeline 独立 Script + Layout
3. 无死锁消息传递

**退出条件**：
- 多标签页并行布局
- 无死锁

---

## 5. 窗口层架构

### 5.1 Servo 窗口层核心文件

| 文件 | 职责 |
|------|------|
| `ports/servoshell/window.rs` | `PlatformWindow` trait 抽象 + `ServoShellWindow` 多 WebView 管理 |
| `ports/servoshell/desktop/headed_window.rs` | winit 窗口实现（~1200 行）：输入/IME/对话框/渲染上下文 |
| `ports/servoshell/desktop/headless_window.rs` | 无头渲染（`SoftwareRenderingContext`） |
| `ports/servoshell/desktop/gui.rs` | egui 工具栏 UI + 与 Servo 内容合成 |

### 5.2 `PlatformWindow` trait（window.rs:380-462）

```rust
pub trait PlatformWindow {
    fn id(&self) -> ServoShellWindowId;
    fn screen_geometry(&self) -> ScreenGeometry;
    fn hidpi_scale_factor(&self) -> Scale<f32, DeviceIndependentPixel, DevicePixel>;
    fn rendering_context(&self) -> Rc<dyn RenderingContext>;
    fn request_repaint(&self, _: &ServoShellWindow);
    fn request_resize(&self, webview: &WebView, outer_size: DeviceIntSize) -> Option<DeviceIntSize>;
    fn set_position(&self, _point: DeviceIntPoint) {}
    fn set_fullscreen(&self, _state: bool) {}
    fn set_cursor(&self, _cursor: Cursor) {}
    fn theme(&self) -> Theme { Theme::Light }
    fn window_rect(&self) -> DeviceIndependentIntRect;
    fn maximize(&self, _: &WebView) {}
    fn focus(&self) {}
}
```

### 5.3 Muskitty 窗口层演进

| 阶段 | 内容 | 价值 |
|------|------|------|
| 1 | `PlatformWindow` trait 定义 | ⭐⭐⭐⭐⭐ |
| 2 | DPI 缩放支持 | ⭐⭐⭐⭐ |
| 3 | 输入事件抽象 | ⭐⭐⭐⭐ |
| 4 | Headless 后端 | ⭐⭐⭐ |
| 5 | 窗口状态管理 | ⭐⭐⭐ |

---

## 6. 关键代码位置参考

### Servo 并行管线

| 文件 | 行号 | 内容 |
|------|------|------|
| `components/constellation/constellation.rs` | 63-85 | 无死锁协议 |
| `components/constellation/constellation.rs` | 276-526 | Constellation 结构体 |
| `components/constellation/event_loop.rs` | 66-189 | EventLoop（ScriptThread 包装） |
| `components/layout/layout_impl.rs` | 980-1374 | 完整 reflow 管线 |
| `components/layout/context.rs` | 51-76 | 并行调度启发式 |
| `components/layout/flow/mod.rs` | 839-878 | 并行块级布局 |
| `components/layout/positioned.rs` | 414-460 | 并行绝对定位布局 |
| `components/layout/flexbox/layout.rs` | 1191-1263 | 并行 Flexbox 布局 |
| `components/layout/table/layout.rs` | 1152-1188 | 并行表格布局 |
| `components/layout/layout_root.rs` | 14-128 | LayoutRoot 增量布局 |
| `components/paint/paint.rs` | 102-171 | Paint 子系统 |

### Servo 窗口层

| 文件 | 行号 | 内容 |
|------|------|------|
| `ports/servoshell/window.rs` | 380-462 | `PlatformWindow` trait 定义 |
| `ports/servoshell/desktop/headed_window.rs` | 106-218 | 窗口创建 + DPI + RenderingContext 初始化 |
| `ports/servoshell/desktop/headed_window.rs` | 520-767 | `handle_winit_window_event` 事件分发 |
| `ports/servoshell/desktop/headed_window.rs` | 800-1000 | `PlatformWindow` 实现 |

### Muskitty 目标

| 文件 | 内容 |
|------|------|
| `crates/muskitty-layout/Cargo.toml` | 添加 rayon 依赖 |
| `crates/muskitty-layout/src/lib.rs` | 并行布局入口 |
| `crates/muskitty-layout/src/parallel.rs` | 并行调度逻辑 |
| `crates/muskitty-cascade/Cargo.toml` | 添加 rayon 依赖 |
| `crates/muskitty-cascade/src/parallel.rs` | 并行选择器匹配 |
| 未来 `muskitty-shell/src/window.rs` | `PlatformWindow` trait |
| 未来 `muskitty-shell/src/winit_window.rs` | `WinitWindow` 实现 |

---

## 7. 依赖关系

```
muskitty-shell (未来浏览器外壳)
├── src/window.rs              # PlatformWindow trait
├── src/winit_window.rs        # WinitWindow: PlatformWindow
├── src/headless_window.rs     # HeadlessWindow: PlatformWindow
├── src/input.rs               # InputEvent / Key / Modifiers
├── src/app.rs                 # ApplicationHandler
└── main.rs

muskitty-renderer (纯渲染库)
├── src/backend/mod.rs         # Backend trait
├── src/backend/tiny_skia.rs   # TinySkiaBackend
└── src/backend/mock.rs        # MockBackend

muskitty-layout (布局引擎)
├── src/lib.rs                 # 布局入口（并行）
├── src/parallel.rs            # Rayon 并行调度
└── Cargo.toml                 # + rayon 依赖

muskitty-cascade (样式层叠)
├── src/lib.rs                 # Cascade 入口（并行）
├── src/parallel.rs            # Rayon 并行选择器匹配
└── Cargo.toml                 # + rayon 依赖
```

**依赖方向**：`muskitty-shell` → `muskitty-renderer` → `muskitty-layout` → `muskitty-cascade` → ...（单向依赖）

---

## 8. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 并行布局结果不一致 | 严格的 PositioningContext 隔离 + 回归测试 |
| Rayon 开销大于收益 | 自适应启发式，小树回退串行 |
| 浮动/计数器破坏并行 | 强制串行回退（Servo 已验证模式） |
| Rayon 线程池配置 | 可配置 `parallelism_job_count_minimum` 和 `parallelism_job_size_minimum` |
| 过早抽象 | 只抽象当前需要的接口，不预设未来需求 |
| DPI 处理复杂 | 先支持整数倍缩放（1x/2x），小数倍延后 |

---

## 9. 总结

Servo 对 Muskitty 最大的价值是两个方面：

### 并行管线（高优先级）
1. **Rayon 并行布局**：block/flex/table 子元素并行布局，`PositioningContext` 隔离消除竞争
2. **LayoutRoot 增量布局**：精细度子树隔离重布局
3. **并行样式重算**：选择器匹配是"尴尬并行"
4. **自适应调度**：根据作业数量和子树大小自动选择并行/串行

### 窗口层（中优先级）
1. **`PlatformWindow` trait 抽象**：解耦窗口操作与具体实现
2. **DPI 缩放处理**：支持 HiDPI 显示

Muskitty 作为纯 Rust 项目，天然适合借鉴 Servo 的并行模式。建议按阶段 1→2→3→4 逐步演进，每个阶段独立 commit + 测试通过后推进下一阶段。
