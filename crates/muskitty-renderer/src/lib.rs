//! MusKitty Renderer — CSS 渲染层。
//!
//! 将 [`muskitty_layout::LayoutResult`] + 每元素的 [`ComputedStyle`]
//! 转换为绘制指令列表（[`RenderCommand`]），再交给后端
//! （[`Backend`] trait，如 tiny-skia）栅格化为像素。
//!
//! # 数据流
//!
//! ```text
//! muskitty-layout 的 LayoutResult (per-element x/y/width/height)
//! + per-element ComputedStyle (background-color / border / ...)
//! + DOM 树 (用于遍历元素)
//!     │  paint
//!     ▼
//! RenderCommand[] (Rect / Text / Clip / ...)
//!     │  Backend::render
//!     ▼
//! 像素输出 (PNG / 窗口 / ...)
//! ```
//!
//! # 规范依据
//!
//! - CSS Color Level 4: 颜色解析（named / hex / rgb 子集）
//! - CSS Backgrounds Level 3: background-color 绘制
//! - CSS Box Model Level 3: border 绘制
//!
//! # 当前后端
//!
//! Phase 4 B-0 调研结论（见 `docs/research/gpui-integration.md`）：
//! GPUI 发布版仅支持 macOS/Linux，Windows 不可用。主后端采用
//! **tiny-skia**（纯 Rust、CPU 渲染、跨平台、PNG 内置）。
//!
//! [`ComputedStyle`]: muskitty_cascade::ComputedStyle

pub mod backend;
pub mod color;
pub mod command;
pub mod paint;
pub mod render_tree;

#[cfg(feature = "backend-tiny-skia")]
pub use backend::tiny_skia::TinySkiaBackend;
pub use backend::{mock::MockBackend, Backend};
pub use color::Color;
pub use command::{Border, BorderStyle, RenderCommand};
pub use paint::{paint, PaintInput};
pub use render_tree::{extract_background_color, extract_border};
