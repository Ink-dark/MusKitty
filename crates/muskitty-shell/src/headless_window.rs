//! 无窗口后端（W-4）。
//!
//! [`HeadlessWindow`] 是 [`PlatformWindow`] 的无窗口实现：没有真实 OS
//! 窗口，[`present`](PlatformWindow::present) 把一帧像素保存在内存中，
//! 供测试断言；PNG 编码走 lib 顶层的 [`crate::render_to_png`] 便捷函数
//! （W-4 C-2）。用途：无窗口环境（CI）跑 shell 渲染测试，以及直接构造
//! `PlatformWindow` 的演示用例（W-1 收尾修订迁至此处——`WinitWindow`
//! 构造参数含 winit 类型必须 `pub(crate)`，而 `HeadlessWindow` 无外部
//! 依赖类型，可公开构造）。
//!
//! 本模块零外部依赖类型，`--no-default-features` 下照常编译（对齐
//! `docs/decisions/2026-08-16-external-dependency-decoupling.md`）。
//! 规划见 `docs/plans/2026-08-23-windowing.md` §W-4。

use crate::window::{Cursor, PlatformWindow, WindowGeometry};
use std::cell::Cell;

/// 无窗口 `PlatformWindow` 实现。
///
/// 像素提交（[`PlatformWindow::present`]）保存为最近一帧
/// （[`frame`](Self::frame)），供测试断言或后续编码 PNG。
/// `request_repaint` 为无操作；光标 / 全屏状态仅记录（无窗口可显示）。
#[derive(Debug, Clone)]
pub struct HeadlessWindow {
    id: u64,
    geometry: WindowGeometry,
    scale: f32,
    cursor: Cell<Cursor>,
    fullscreen: Cell<bool>,
    /// 最近一帧 RGBA8 像素（行长 = `width * 4`）。
    frame: Option<(Vec<u8>, u32, u32)>,
}

impl HeadlessWindow {
    /// 构造无头窗口：客户区 `width × height`（逻辑 px），scale = 1.0。
    ///
    /// 与 `WinitWindow` 不同（构造参数含 winit 类型，故为 `pub(crate)`），
    /// 本构造函数不含任何外部依赖类型，可直接使用。
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            id: 0,
            geometry: WindowGeometry::new(0, 0, width, height),
            scale: 1.0,
            cursor: Cell::new(Cursor::Default),
            fullscreen: Cell::new(false),
            frame: None,
        }
    }

    /// 最近一帧像素 `(data, width, height)`（RGBA8），未提交过帧则为 `None`。
    pub fn frame(&self) -> Option<(&[u8], u32, u32)> {
        self.frame.as_ref().map(|(d, w, h)| (d.as_slice(), *w, *h))
    }
}

impl PlatformWindow for HeadlessWindow {
    fn id(&self) -> u64 {
        self.id
    }

    fn hidpi_scale_factor(&self) -> f32 {
        self.scale
    }

    fn request_repaint(&self) {
        // 无窗口：无事件循环，no-op。
    }

    fn geometry(&self) -> WindowGeometry {
        self.geometry
    }

    fn set_cursor(&self, cursor: Cursor) {
        self.cursor.set(cursor);
    }

    fn set_fullscreen(&self, state: bool) {
        self.fullscreen.set(state);
    }

    fn present(&mut self, data: &[u8], width: u32, height: u32) {
        self.frame = Some((data.to_vec(), width, height));
    }

    fn handle_event(&mut self, _event: crate::input::InputEvent) -> bool {
        // W-3：无页面级命中测试，恒未消费。
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputEvent;

    fn red_2x1_frame() -> Vec<u8> {
        // 2×1：左红右白。
        vec![255, 0, 0, 255, 255, 255, 255, 255]
    }

    #[test]
    fn present_stores_last_frame() {
        let mut w = HeadlessWindow::new(2, 1);
        assert!(w.frame().is_none());

        w.present(&red_2x1_frame(), 2, 1);
        let (data, width, height) = w.frame().expect("frame after present");
        assert_eq!((width, height), (2, 1));
        assert_eq!(data, &red_2x1_frame()[..]);

        // 再提交一帧 → 只保留最近帧。
        w.present(&[0, 0, 0, 255, 0, 0, 0, 255], 2, 1);
        let (data, _, _) = w.frame().unwrap();
        assert_eq!(data, &[0, 0, 0, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn default_traits_and_noop_queries() {
        let mut w = HeadlessWindow::new(320, 240);
        assert_eq!(w.id(), 0);
        assert_eq!(w.hidpi_scale_factor(), 1.0);
        assert_eq!(w.geometry(), WindowGeometry::new(0, 0, 320, 240));
        // no-op / 仅记录，不 panic。
        w.request_repaint();
        w.set_cursor(Cursor::Pointer);
        w.set_fullscreen(true);
        assert!(!w.handle_event(InputEvent::MouseMove {
            position: (0.0, 0.0),
            modifiers: Default::default(),
        }));
    }
}
