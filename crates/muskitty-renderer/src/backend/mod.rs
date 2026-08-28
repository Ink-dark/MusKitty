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

/// 后端渲染输出（P2-18）。
///
/// [`Backend::render`] 将指令栅格化后返回结构化输出，而不是仅写入
/// 内部状态——调用方据此消费像素或指令，无需依赖后端的具体形态。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutput {
    /// 已栅格化的像素（RGBA 8-bit/channel，行长 = `width * 4`）。
    Pixels {
        /// 画布宽度（物理 px，= 逻辑宽 × scale，W-2）。
        width: u32,
        /// 画布高度（物理 px，= 逻辑高 × scale，W-2）。
        height: u32,
        /// RGBA 像素数据（长度 = `width * height * 4`）。
        data: Vec<u8>,
    },
    /// 绘制指令（后端未栅格化，如 Mock）。
    Commands(Vec<RenderCommand>),
    /// 无输出（后端未产生可消费结果，如未来窗口后端）。
    None,
}

/// 渲染后端 trait。
///
/// 消费 `RenderCommand` 列表，输出像素到目标（PNG 文件 / 窗口 / buffer）。
pub trait Backend {
    /// 渲染给定指令列表到后端目标。
    ///
    /// `width` / `height` 为**逻辑**画布尺寸（CSS px，即布局视口）；
    /// `scale` 为 HiDPI 缩放因子（物理像素 ÷ 逻辑像素，W-2）。后端以
    /// `round(width × scale) × round(height × scale)` 的物理分辨率栅格化，
    /// 指令坐标保持逻辑 px，绘制时统一应用 scale transform（向量缩放，
    /// 非模糊放大）。返回本次渲染的输出（[`RenderOutput`]），调用方可按需消费。
    fn render(
        &mut self,
        commands: &[RenderCommand],
        width: u32,
        height: u32,
        scale: f32,
    ) -> RenderOutput;
}
