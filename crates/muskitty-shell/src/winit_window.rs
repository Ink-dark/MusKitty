//! winit + softbuffer 真窗口后端（`winit-backend` feature 门控）。
//!
//! [`WinitWindow`] 实现 [`PlatformWindow`]：winit 创建 OS 窗口，softbuffer
//! 管理像素表面。[`present`](Self::present) 接收 renderer 产出的 RGBA 像素，
//! 转为 softbuffer 的 0RGB u32 后提交。像素格式转换抽为
//! [`rgba_to_0rgb`] 纯函数，便于无窗口单测。
//!
//! 本模块为 **crate 内部实现**（`pub(crate)`）：winit 窗口必须经事件循环
//! 创建，无法像 `ReqwestFetcher` 那样内部自建资源，因此构造参数天然含
//! winit 类型。为避免泄漏进公共 API（decoupling ADR），窗口创建由
//! [`crate::app::App`] 封装，本模块不对外暴露。`--no-default-features`
//! 时本模块不编译，shell 仍可无窗口渲染（见 [`crate::page`]）。

use std::num::NonZeroU32;
use std::rc::Rc;

use softbuffer::{Context, Surface};
use winit::window::{CursorIcon, Fullscreen, Window};

use crate::window::{Cursor, PlatformWindow, WindowGeometry};

/// RGBA8 → softbuffer 0RGB u32（row-major）。
///
/// softbuffer 的 buffer 为 u32 大端 0RGB（红在最低字节）。每个 RGBA 像素
/// 4 字节重排为 u32：`(b << 16) | (g << 8) | r`。从 renderer 的
/// `window_demo` 抽出，独立纯函数以便无窗口单测。
///
/// `pub(crate)`：softbuffer 格式转换属后端内部实现，不外泄。
pub(crate) fn rgba_to_0rgb(data: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(data.len() / 4);
    for px in data.chunks_exact(4) {
        out.push(((px[2] as u32) << 16) | ((px[1] as u32) << 8) | (px[0] as u32));
    }
    out
}

/// winit + softbuffer 真窗口实现。
///
/// 持有 winit 窗口 + softbuffer 表面。softbuffer `Context` 在 `Surface::new`
/// 后即不再需要（0.4 的 `Surface` 不借用 Context，见 softbuffer 源码
/// `Surface` 构造），故不持有，避免 dead_code 字段。
///
/// `pub(crate)`：构造参数含 winit 类型，仅 `app::App` 内部创建，不对外
/// 暴露（decoupling ADR）。
pub(crate) struct WinitWindow {
    id: u64,
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

impl WinitWindow {
    /// 基于已创建的 winit 窗口构造。
    ///
    /// `id` 由调用方分配（如窗口创建序号），供上层区分多窗口。
    pub(crate) fn new(id: u64, window: Rc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Context::new(window.clone())?;
        let surface = Surface::new(&context, window.clone())?;
        Ok(Self {
            id,
            window,
            surface,
        })
    }
}

impl PlatformWindow for WinitWindow {
    fn id(&self) -> u64 {
        self.id
    }

    fn hidpi_scale_factor(&self) -> f32 {
        // winit 返回 f64，取整为 f32（W-2 起参与渲染缩放与逻辑换算）。
        self.window.scale_factor() as f32
    }

    fn request_repaint(&self) {
        self.window.request_redraw();
    }

    fn geometry(&self) -> WindowGeometry {
        // W-2：winit 的 inner_size / outer_position 返回物理像素；WindowGeometry
        // 语义为逻辑 px，用窗口 scale_factor（f64 保精度）换算。布局按逻辑尺寸，
        // 栅格化再乘 scale（见 app.rs 的 render_at）。
        let scale = self.window.scale_factor();
        // outer_position 在部分平台（如 Wayland）返回 Err，回退到默认。
        let pos = self
            .window
            .outer_position()
            .unwrap_or_default()
            .to_logical::<i32>(scale);
        let size = self.window.inner_size().to_logical::<u32>(scale);
        WindowGeometry::new(pos.x, pos.y, size.width.max(1), size.height.max(1))
    }

    fn set_cursor(&self, cursor: Cursor) {
        let icon = match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::Wait => CursorIcon::Wait,
            // cursor-icon 1.2 无 Hand 变体；张开的手 = Grab（可拖动）。
            Cursor::Hand => CursorIcon::Grab,
        };
        self.window.set_cursor(icon);
    }

    fn set_fullscreen(&self, state: bool) {
        if state {
            self.window
                .set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            self.window.set_fullscreen(None);
        }
    }

    fn present(&mut self, data: &[u8], width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        // surface 与像素尺寸同步（softbuffer 建议 buffer 填满窗口；尺寸
        // 不匹配时绘在左上角）。
        self.surface
            .resize(
                NonZeroU32::new(width).expect("width > 0"),
                NonZeroU32::new(height).expect("height > 0"),
            )
            .expect("surface resize");
        let pixels = rgba_to_0rgb(data);
        let mut buffer = self.surface.buffer_mut().expect("surface buffer");
        buffer.copy_from_slice(&pixels);
        buffer.present().expect("present frame");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_to_0rgb_byte_order() {
        // 每像素 RGBA8 → 0RGB u32（红在低字节）：
        // 红 (255,0,0) → 0x0000FF；绿 (0,255,0) → 0x00FF00；蓝 (0,0,255) → 0xFF0000。
        let data = [
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
        ];
        let out = rgba_to_0rgb(&data);
        assert_eq!(out, vec![0x0000FF, 0x00FF00, 0xFF0000]);
    }

    #[test]
    fn rgba_to_0rgb_length_matches_pixel_count() {
        let data = [10u8, 20, 30, 255, 40, 50, 60, 255];
        let out = rgba_to_0rgb(&data);
        assert_eq!(out.len(), 2);
    }
}
