//! 渲染后端抽象。
//!
//! [`Backend`] trait 抽象具体渲染实现（tiny-skia / vello / GPUI / mock），
//! paint 阶段产出的 `Vec<RenderCommand>` 经由 [`Backend::render`] 栅格化为像素。
//!
//! # 当前后端
//!
//! - **B-1**：`MockBackend`（仅记录命令，用于测试）
//! - **B-3**：`TinySkiaBackend`（纯 Rust CPU 渲染，输出 PNG）
//!
//! GPUI / vello 作为未来可选后端（feature flag），见
//! `docs/research/gpui-integration.md`。

use crate::command::RenderCommand;

pub mod mock;
#[cfg(feature = "backend-tiny-skia")]
pub mod tiny_skia;

/// 渲染后端 trait。
///
/// 消费 `RenderCommand` 列表，输出像素到目标（PNG 文件 / 窗口 / buffer）。
pub trait Backend {
    /// 渲染给定指令列表到后端目标。
    ///
    /// `width` / `height` 为画布尺寸（px）。
    fn render(&mut self, commands: &[RenderCommand], width: u32, height: u32);
}
