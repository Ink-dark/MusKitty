//! MusKitty Browser Shell — 窗口层。
//!
//! 浏览器外壳 crate：把 HTML + CSS 渲染成像素，再显示到具体窗口目标
//! （winit + softbuffer 真窗口 / 无头 PNG）。与 [`muskitty-renderer`]
//! 的分工是：renderer 只负责 `LayoutResult → RenderCommand[] → 像素`
//! （纯渲染库），本 crate 负责把 DOM→CSS→Layout→Render 全管线串起来，
//! 并通过 [`PlatformWindow`] trait 抽象"如何显示像素"，与具体窗口后端解耦。
//!
//! # 数据流
//!
//! ```text
//! HTML + CSS
//!     │  page::render_page
//!     ▼
//! muskitty-renderer::RenderOutput::Pixels { width, height, data (RGBA) }
//!     │  PlatformWindow::present
//!     ▼
//! 窗口目标（winit+softbuffer 真窗口 / Headless 写 PNG）
//! ```
//!
//! # 架构约束
//!
//! 公共 API（[`PlatformWindow`] 等）只暴露本 crate 自身抽象类型，
//! winit / softbuffer 等外部依赖类型不出现在 `pub` 导出中（对齐
//! `docs/decisions/2026-08-16-external-dependency-decoupling.md`）。
//! winit 后端由 `winit-backend` feature 门控，`--no-default-features`
//! 下仍可编译无头渲染。
//!
//! 规划见 `docs/plans/2026-08-23-windowing.md`。

pub mod headless_window;
pub mod input;
pub mod page;
pub mod window;

/// winit + softbuffer 真窗口后端（`winit-backend` feature 门控）。
///
/// `pub(crate)`：窗口创建完全封装在 crate 内部（经 `app::App` 暴露），
/// 公共 API 不泄漏 winit/softbuffer 类型（对齐 decoupling ADR）。直接
/// 构造 `PlatformWindow` 的演示由 W-4 的 `HeadlessWindow`（可无参构造）
/// 承担。
#[cfg(feature = "winit-backend")]
pub(crate) mod winit_window;

/// 应用入口（事件循环 + 窗口生命周期，`winit-backend` feature 门控）。
#[cfg(feature = "winit-backend")]
pub mod app;
