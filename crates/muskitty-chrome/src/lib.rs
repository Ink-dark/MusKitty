//! MusKitty Browser Chrome — 浏览器 chrome 窗口层。
//!
//! 自绘非原生 UI 的浏览器外壳（决策见
//! `docs/decisions/2026-08-29-chrome-window-layer.md`）：标签栏（标签标题 +
//! 关闭 + 新建）、工具栏（后退/前进/刷新 + 圆角地址栏）由本 crate 直接
//! 绘制为像素并与页面视口**同帧合成**（Chromium Views"chrome 即合成像素"
//! 思路），不使用系统原生控件，也不引入 egui/iced 等 UI 框架。
//!
//! # 分层（chrome 自绘的纯函数管线，全部可无窗口测试）
//!
//! ```text
//! ChromeState + 窗口几何
//!     │  chrome::model::layout_chrome      （布局 → ChromeRects）
//!     ▼
//! ChromeRects
//!     ├─ chrome::paint::paint_chrome       （绘制 → chrome 像素）
//!     └─ chrome::input::hit_test / apply   （命中测试 → ChromeEffect）
//!
//! 页面 RGBA（page::render_page）+ chrome 像素
//!     │  compositor::compose
//!     ▼
//! 全窗口 RGBA → winit+softbuffer present（`winit-backend` 门控）
//! ```
//!
//! # 架构约束
//!
//! 公共 API 只暴露本 crate 自身类型；winit / softbuffer / tiny-skia /
//! cosmic-text 类型不出现在 `pub` 导出（对齐
//! `docs/decisions/2026-08-16-external-dependency-decoupling.md`）。
//! `winit-backend` feature 门控真窗口（默认开）；`--no-default-features`
//! 下纯函数层 + 无头渲染照常编译测试（CI 无窗口可跑）。
//!
//! 规划见 `docs/plans/2026-08-29-chrome-window-layer.md`。

pub mod chrome;
pub mod compositor;

/// 浏览器窗口应用（winit 事件循环 + chrome 合成呈现，`winit-backend`
/// feature 门控）。
#[cfg(feature = "winit-backend")]
pub mod app;
