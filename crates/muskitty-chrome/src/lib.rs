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
//! 页面渲染管线（`page`）、标签状态（`webview`）与 shell 快捷键
//! （`shortcut`）自 muskitty-shell 迁入（shell crate 已退役，其窗口
//! 职责由本 crate 全面取代；见 git 历史与 W-1~W-5 规划记录）。
//!
//! 地址栏导航（Phase 5 接驳）在 `navigation`：http/https 抓取在独立
//! 线程完成（结果经 channel 回填 `webview`），渲染管线入口仍是
//! `page::render_page`。
//!
//! 规划见 `docs/plans/2026-08-29-chrome-window-layer.md`。

pub mod chrome;
pub mod compositor;
pub mod headless;
pub mod navigation;
pub mod page;
pub mod shortcut;
pub mod webview;

/// 浏览器窗口应用（winit 事件循环 + chrome 合成呈现，`winit-backend`
/// feature 门控）。
#[cfg(feature = "winit-backend")]
pub mod app;
