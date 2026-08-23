# Servo 窗口层架构分析与 Muskitty 演进指导

> **日期**：2026-08-23
> **参考代码**：servo/servo (2026-08-23 shallow clone)
> **关联 Muskitty 文件**：`crates/muskitty-renderer/examples/window_demo.rs`

## 1. Servo 窗口层架构

### 1.1 核心文件

| 文件 | 职责 |
|------|------|
| `ports/servoshell/window.rs` | `PlatformWindow` trait 抽象 + `ServoShellWindow` 多 WebView 管理 |
| `ports/servoshell/desktop/headed_window.rs` | winit 窗口实现（~1200 行）：输入/IME/对话框/渲染上下文 |
| `ports/servoshell/desktop/headless_window.rs` | 无头渲染（`SoftwareRenderingContext`） |
| `ports/servoshell/desktop/gui.rs` | egui 工具栏 UI + 与 Servo 内容合成 |
| `components/shared/paint/rendering_context.rs` | `RenderingContext` trait + surfman OpenGL 封装 |

### 1.2 `PlatformWindow` trait（window.rs:380-462）

Servo 将窗口操作抽象为 trait，解耦具体实现：

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
    fn update_user_interface_state(&self, _: &RunningAppState, _: &ServoShellWindow) -> bool { false }
    // ... 对话框/IME/无障碍等
}
```

两个实现：
- `HeadedWindow`（winit + egui）→ 有 GUI 的桌面窗口
- `HeadlessWindow`（SoftwareRenderingContext）→ 无头测试

### 1.3 双 RenderingContext 合成模式

```
┌─────────────────────────────────────────┐
│  WindowRenderingContext (OS 窗口表面)    │
│  ┌───────────────────────────────────┐  │
│  │  OffscreenRenderingContext (FBO)   │  │
│  │  Servo 渲染 web 内容              │  │
│  └───────────────────────────────────┘  │
│  egui 渲染工具栏/UI                      │
│  glBlitFramebuffer 合成                  │
└─────────────────────────────────────────┘
```

- `WindowRenderingContext`：通过 surfman 绑定 OS 原生窗口表面，`present()` 调用 swap buffers
- `OffscreenRenderingContext`：离屏 FBO，`present()` 是 no-op，通过 `render_to_parent_callback()` 返回 `glBlitFramebuffer` 闭包合成到父上下文

### 1.4 DPI / HiDPI 处理

```rust
// headed_window.rs:827-835
fn hidpi_scale_factor(&self) -> Scale<f32, DIP, DP> {
    self.device_pixel_ratio_override
        .map(Scale::new)
        .unwrap_or_else(|| self.device_hidpi_scale_factor())
}

// ScaleFactorChanged 事件处理
WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
    let effective_egui_zoom = desired_scale_factor / scale_factor as f32;
    self.gui.borrow().set_zoom_factor(effective_egui_zoom);
    window.hidpi_scale_factor_changed();  // 通知所有 WebView
    self.winit_window.request_redraw();
}
```

### 1.5 输入事件分层处理

```rust
// headed_window.rs:330-435
fn handle_intercepted_key_bindings(...) -> bool {
    ShortcutMatcher::from_event(key_event.event.clone())
        .shortcut(CMD_OR_CONTROL, 'W', || window.close_webview(...))
        .shortcut(CMD_OR_CONTROL, 'T', || window.create_and_activate_toplevel_webview(...))
        .shortcut(CMD_OR_CONTROL, 'Q', || state.schedule_exit())
        .otherwise(|| handled = false);
    handled  // true = 已处理，不转发给 WebView
}
```

先检查 servoshell 快捷键，未处理的事件才转发给 WebView。

### 1.6 多 WebView 管理

`ServoShellWindow` 内含 `WebViewCollection`：
- `add_webview()` / `close_webview()` / `activate_webview()`
- 标签页切换（Ctrl+1~9 / Ctrl+PageUp/Down）
- favicon 缓存管理

### 1.7 窗口状态管理

```rust
pub struct ServoShellWindow {
    close_scheduled: Cell<bool>,        // 安全关闭标志
    needs_update: Cell<bool>,           // UI 状态需要更新
    needs_repaint: Cell<bool>,          // 渲染内容需要重绘
    pending_commands: RefCell<Vec<UserInterfaceCommand>>,  // 命令队列
    pending_favicon_loads: RefCell<Vec<WebViewId>>,       // favicon 加载队列
}
```

延迟更新模式：标记脏位，下一次事件循环统一处理。

---

## 2. Muskitty 当前窗口实现

### 2.1 现状（`window_demo.rs`）

```rust
// 167 行，硬编码 winit + softbuffer
fn render_page(width: u32, height: u32) -> Vec<u32> {
    // DOM → CSS → Layout → Render → RGBA
    // RGBA8 → softbuffer 0RGB u32
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pixels: Vec<u32>,
    width: u32,
    height: u32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) { /* 创建窗口 */ }
    fn window_event(&mut self, ..., event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => { /* 复制像素 */ }
            WindowEvent::Resized(_) => { /* 重渲染 */ }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}
```

**注意**：`winit` 和 `softbuffer` 位于 `[dev-dependencies]`，窗口逻辑仅在 examples 中使用，核心 renderer crate 与窗口系统解耦。这符合 Muskitty 的设计原则：renderer 只负责 `LayoutResult → RenderCommand[] → backend pixels`，窗口集成由上层（未来的浏览器外壳）负责。

### 2.2 存在的问题

| 问题 | 影响 |
|------|------|
| 无 DPI 处理 | HiDPI 屏幕模糊 |
| 硬编码 winit | 无法切换窗口后端或 headless 测试 |
| 无输入事件抽象 | 未来添加键盘/鼠标交互困难 |
| 每次 resize 全量重渲染 | 性能差 |
| 无窗口状态管理 | 无法支持多标签/全屏等 |

---

## 3. 可借鉴部分（按价值排序）

### 3.1 `PlatformWindow` trait 抽象 — ⭐⭐⭐⭐⭐

**价值**：解耦窗口操作与具体实现，支持多种后端。

**建议实现**：

```rust
// crates/muskitty-renderer/src/window.rs
pub trait PlatformWindow {
    fn id(&self) -> u64;
    fn hidpi_scale_factor(&self) -> f32;
    fn request_repaint(&self);
    fn set_cursor(&self, cursor: Cursor);
    fn set_fullscreen(&self, state: bool);
    fn window_rect(&self) -> (i32, i32, u32, u32);
    fn rendering_context(&self) -> &dyn RenderingContext;
}

pub enum Cursor {
    Default, Pointer, Text, Wait, ...
}
```

### 3.2 DPI 缩放支持 — ⭐⭐⭐⭐

**价值**：正确处理 HiDPI 显示。

**实现要点**：
- `render_page` 接受 scale factor 参数
- `TinySkiaBackend` 渲染时输出 `width * scale × height * scale` 像素
- 处理 `ScaleFactorChanged` 事件

### 3.3 输入事件抽象 — ⭐⭐⭐⭐

**价值**：为交互功能做准备。

**建议枚举**：

```rust
pub enum InputEvent {
    Keyboard { key: Key, state: ElementState, modifiers: Modifiers },
    MouseButton { button: MouseButton, state: ElementState, position: Point },
    MouseMove { position: Point },
    MouseWheel { delta: (f32, f32) },
    Touch { id: u32, phase: TouchPhase, position: Point },
}
```

### 3.4 窗口状态管理 — ⭐⭐⭐

**价值**：避免不必要的重渲染。

**实现**：
- `needs_repaint` / `needs_update` 脏位标记
- `close_scheduled` 安全关闭
- 命令队列延迟处理

### 3.5 Headless 后端 — ⭐⭐⭐

**价值**：支持自动化测试。

**实现**：
- `HeadlessWindow` 实现 `PlatformWindow`
- 直接写 PNG 文件，不创建窗口
- 用于 WPT 测试和 CI

### 3.6 双 RenderingContext 合成 — ⭐⭐（当前低价值）

**评估**：Muskitty 使用 tiny-skia 软件渲染 + softbuffer，不需要 GPU 合成。未来切换到 GPU 渲染（wgpu/vulkan）时再考虑。

---

## 4. Muskitty 窗口层演进路径

> **架构约束**：`muskitty-renderer` 是纯渲染库，不依赖 winit/softbuffer（仅 dev-dependencies）。窗口集成逻辑应放在未来的浏览器外壳 crate（如 `muskitty-shell`）中。

### 阶段 1：定义窗口抽象接口（立即可做）

**目标**：为浏览器外壳定义窗口操作接口。

**步骤**：
1. 在 `docs/` 或新 crate 中定义 `PlatformWindow` trait（纯接口，无实现）
2. 定义 `Cursor` / `InputEvent` / `Modifiers` 等枚举
3. 在 `window_demo.rs` 中实现 `WinitWindow: PlatformWindow`

**退出条件**：
- trait 定义完成，无外部依赖类型泄漏
- `window_demo` 功能不变

### 阶段 2：DPI 支持（紧随其后）

**目标**：正确处理 HiDPI 显示。

**步骤**：
1. `render_page` 接受 scale factor 参数
2. `TinySkiaBackend` 渲染时输出 `width * scale × height * scale` 像素
3. 处理 `ScaleFactorChanged` 事件

**退出条件**：
- HiDPI 屏幕上窗口内容清晰
- 缩放变化时正确重渲染

### 阶段 3：输入事件抽象（布局增强后）

**目标**：支持键盘/鼠标交互。

**步骤**：
1. 在 `PlatformWindow` 中添加 `handle_event()` 方法
2. 实现基础快捷键（Esc 关闭、Ctrl+R 刷新）
3. 事件分层处理（servoshell 快捷键 vs 页面事件）

**退出条件**：
- Esc 可关闭窗口
- 事件正确分发

### 阶段 4：Headless 后端（测试需要）

**目标**：支持无头测试。

**步骤**：
1. 实现 `HeadlessWindow`，直接写 PNG 文件
2. 提供 `render_to_png(html, css, path)` 便捷函数
3. 集成到现有测试流程

**退出条件**：
- `cargo test` 中可通过 headless 模式验证渲染结果
- CI 无需窗口环境即可运行渲染测试

### 阶段 5：窗口状态管理（多标签前）

**目标**：支持多标签页。

**步骤**：
1. `ServoShellWindow` 管理多个 `WebView`
2. 标签页切换（Ctrl+T/W/1~9）
3. favicon 缓存管理

**退出条件**：
- 可创建/关闭多个标签页
- 标签页切换正确刷新

---

## 5. 关键代码位置参考

### Servo 参考

| 文件 | 行号 | 内容 |
|------|------|------|
| `ports/servoshell/window.rs` | 380-462 | `PlatformWindow` trait 定义 |
| `ports/servoshell/desktop/headed_window.rs` | 106-218 | 窗口创建 + DPI + RenderingContext 初始化 |
| `ports/servoshell/desktop/headed_window.rs` | 520-767 | `handle_winit_window_event` 事件分发 |
| `ports/servoshell/desktop/headed_window.rs` | 800-1000 | `PlatformWindow` 实现 |
| `components/shared/paint/rendering_context.rs` | 35-400 | `RenderingContext` trait |
| `ports/servoshell/desktop/gui.rs` | 188-237 | egui 初始化 + RenderingContext 集成 |

### Muskitty 目标（浏览器外壳 crate）

```
未来 muskitty-shell/（或 muskitty-browser/）
├── src/
│   ├── window.rs              # PlatformWindow trait 定义
│   ├── winit_window.rs        # WinitWindow 实现
│   ├── headless_window.rs     # HeadlessWindow 实现
│   ├── input.rs               # InputEvent / Key / Modifiers 枚举
│   ├── app.rs                 # ApplicationHandler 实现
│   └── main.rs                # 入口
└── examples/
    └── window_demo.rs         # 使用 PlatformWindow 的 demo
```

### Muskitty renderer crate（保持不变）

```
crates/muskitty-renderer/
├── src/
│   ├── lib.rs                 # 公共 API
│   ├── backend/
│   │   ├── mod.rs             # Backend trait（已有）
│   │   ├── tiny_skia.rs       # TinySkiaBackend（已有）
│   │   └── mock.rs            # MockBackend（已有）
│   └── ...
└── examples/
    ├── render_demo.rs         # 输出 PNG（保持）
    └── window_demo.rs         # 移到 muskitty-shell
```

---

## 6. 依赖关系

```
muskitty-renderer (纯渲染库，无窗口依赖)
├── src/backend/mod.rs         # Backend trait
├── src/backend/tiny_skia.rs   # TinySkiaBackend
├── src/backend/mock.rs        # MockBackend
└── examples/
    └── render_demo.rs         # 输出 PNG

muskitty-shell (未来浏览器外壳)
├── src/window.rs              # PlatformWindow trait
├── src/winit_window.rs        # WinitWindow: PlatformWindow
├── src/headless_window.rs     # HeadlessWindow: PlatformWindow
├── src/input.rs               # InputEvent / Key / Modifiers
├── src/app.rs                 # ApplicationHandler
└── examples/
    └── window_demo.rs         # 使用 PlatformWindow 的 demo
```

**依赖方向**：`muskitty-shell` → `muskitty-renderer` → `muskitty-layout` → ...（单向依赖）

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 过早抽象 | 只抽象当前需要的接口，不预设未来需求 |
| 破坏现有 demo | 重构 window_demo 时保持功能不变 |
| DPI 处理复杂 | 先支持整数倍缩放（1x/2x），小数倍延后 |
| 输入事件分发性能 | 事件队列 + 脏位标记，避免每帧遍历 |

---

## 8. 总结

Servo 窗口层对 Muskitty 最大的价值是 **`PlatformWindow` trait 抽象**和 **DPI 处理**。Muskitty 当前窗口实现过于简单（硬编码 winit + softbuffer），Servo 的 trait 抽象模式可以帮助 Muskitty 在不重构的情况下支持多种渲染后端和 headless 模式。

建议按阶段 1→2→3→4 逐步演进，每个阶段独立 commit + 测试通过后推进下一阶段。
