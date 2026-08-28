//! 平台窗口抽象。
//!
//! [`PlatformWindow`] trait 定义窗口操作的最小接口，与具体窗口后端
//! （winit + softbuffer 真窗口 / Headless 写 PNG）解耦。公共 API 只暴露
//! 本 crate 自身类型（[`Cursor`] / [`WindowGeometry`]），不泄漏 winit /
//! softbuffer 等外部依赖类型（对齐
//! `docs/decisions/2026-08-16-external-dependency-decoupling.md`）。
//!
//! 参照 Servo `ports/servoshell/window.rs` 的 `PlatformWindow` trait
//! （见 `docs/research/2026-08-23-servo-window-layer-analysis.md` §1.2），
//! 但裁剪掉 Muskitty 软件渲染不需要的部分（GPU RenderingContext /
//! 对话框 / IME / 无障碍）。

use crate::input::InputEvent;

/// 鼠标光标形状。
///
/// 与 winit `CursorIcon` 的子集对应，按需扩展。仅在 `winit-backend`
/// 下映射到具体图标；Headless 后端忽略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// 默认箭头。
    Default,
    /// 手型（可点击元素）。
    Pointer,
    /// I 型（可编辑文本）。
    Text,
    /// 等待（沙漏/转圈）。
    Wait,
    /// 抓取/移动。
    Hand,
}

/// 窗口几何信息（逻辑像素）。
///
/// `width` / `height` 为窗口客户区尺寸（不含标题栏等装饰），与
/// winit 的 `inner_size` 对应。所有值均为逻辑像素，物理分辨率需乘以
/// [`PlatformWindow::hidpi_scale_factor`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowGeometry {
    /// 窗口左上角屏幕 x（逻辑 px）。
    pub x: i32,
    /// 窗口左上角屏幕 y（逻辑 px）。
    pub y: i32,
    /// 客户区宽度（逻辑 px）。
    pub width: u32,
    /// 客户区高度（逻辑 px）。
    pub height: u32,
}

impl WindowGeometry {
    /// 构造几何信息。
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// 平台窗口抽象。
///
/// 定义"如何显示像素"与窗口级操作的接口。窗口后端实现本 trait，上层
/// 只依赖本 trait 不关心具体实现——替换窗口后端（winit / headless /
/// 未来其他）不影响上层。
///
/// 方法语义：
/// - 只读查询（[`id`](Self::id) / [`hidpi_scale_factor`](Self::hidpi_scale_factor) /
///   [`geometry`](Self::geometry)）取 `&self`；
/// - 状态变更（[`set_cursor`](Self::set_cursor) / [`set_fullscreen`](Self::set_fullscreen) /
///   [`request_repaint`](Self::request_repaint)）取 `&self`（各后端内部可用
///   内部可变性）；
/// - [`present`](Self::present) 提交一帧像素，需独占借用（softbuffer
///   表面需 `&mut` 取 buffer）；
/// - [`handle_event`](Self::handle_event) 接收输入事件（页面层入口），
///   需独占借用（事件分发可能变更窗口/页面状态）。
pub trait PlatformWindow {
    /// 窗口唯一标识。
    fn id(&self) -> u64;

    /// HiDPI 缩放因子（物理像素 ÷ 逻辑像素）。
    ///
    /// W-2 起 winit 后端返回窗口实际值（物理↔逻辑换算与渲染缩放统一用它）；
    /// Headless 后端返回 1.0（逻辑 = 物理）。
    fn hidpi_scale_factor(&self) -> f32;

    /// 请求一次重绘（异步，事件循环下一帧处理）。
    ///
    /// 对应 winit 的 `Window::request_redraw`；Headless 后端为 no-op。
    fn request_repaint(&self);

    /// 当前窗口几何信息（逻辑 px）。
    fn geometry(&self) -> WindowGeometry;

    /// 设置鼠标光标形状。
    fn set_cursor(&self, cursor: Cursor);

    /// 设置全屏状态。
    fn set_fullscreen(&self, state: bool);

    /// 显示一帧像素（RGBA8，行长 = `width * 4`）。
    ///
    /// `data` 为 renderer 输出的非预乘 RGBA 像素（长度 =
    /// `width * height * 4`）。各实现负责转成自身显示格式（如 softbuffer
    /// 的 0RGB u32）并提交。
    fn present(&mut self, data: &[u8], width: u32, height: u32);

    /// 页面层输入入口：把已转成 [`InputEvent`] 的事件交给窗口/页面处理。
    ///
    /// 返回 `true` 表示页面消费了该事件（不应继续转发）；`false` 表示未消费。
    /// W-3 无页面级命中测试，所有后端恒返回 `false`，仅建立事件分发结构——
    /// 后续命中测试阶段（事件 → 具体元素）在本方法内实现。
    /// shell 快捷键（Esc 关闭 / Ctrl+R 刷新）**不经过本方法**：在
    /// `crate::app::App::dispatch_input` 中先于本方法处理（对齐
    /// `docs/plans/2026-08-23-windowing.md` §W-3 事件分层）。
    fn handle_event(&mut self, event: InputEvent) -> bool;
}
