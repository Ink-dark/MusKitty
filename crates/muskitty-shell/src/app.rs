//! 应用入口：winit 事件循环 + 窗口 + 渲染管线。
//!
//! [`App`] 实现 winit `ApplicationHandler`，管理窗口生命周期：
//! 窗口创建 / 重绘（渲染当前页）/ 尺寸变化重渲染 / 关闭退出。
//! 页面内容通过 [`App::run`] 传入（W-1 硬编码 HTML+CSS，后续接加载器）。
//!
//! 本模块仅在 `winit-backend` feature 下编译（无头场景用
//! [`crate::page::render_page`] 即可，无需事件循环）。

use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use muskitty_renderer::RenderOutput;

use crate::page::render_page;
use crate::window::PlatformWindow;
use crate::winit_window::WinitWindow;

/// 初始窗口尺寸（逻辑 px；W-1 阶段 scale=1，即物理 px）。
pub const DEFAULT_W: u32 = 800;
pub const DEFAULT_H: u32 = 600;

/// 应用状态：窗口 + 当前渲染结果。
///
/// `pixels` / `width` / `height` 为最近一次渲染的 RGBA 输出，尺寸变化时
/// 重渲染（脏尺寸检查，避免每帧全量渲染）。
pub struct App {
    window: Option<Rc<Window>>,
    winit_window: Option<WinitWindow>,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    html: &'static str,
    css: &'static str,
}

impl App {
    /// 构造应用状态（尚未创建窗口，`resumed` 时创建）。
    pub fn new(html: &'static str, css: &'static str) -> Self {
        Self {
            window: None,
            winit_window: None,
            pixels: Vec::new(),
            width: 0,
            height: 0,
            html,
            css,
        }
    }

    /// 以给定页面内容启动窗口应用（阻塞直到窗口关闭）。
    ///
    /// 便捷入口：创建事件循环 + `App` 并运行。
    pub fn run(html: &'static str, css: &'static str) {
        let event_loop = EventLoop::new().expect("event loop");
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = App::new(html, css);
        event_loop.run_app(&mut app).expect("run app");
    }

    /// 以指定尺寸重新渲染页面，结果存入 `pixels`。
    fn render_at(&mut self, width: u32, height: u32) {
        // W-2：C-2 起 render_page 带 scale；窗口流在 C-3 接入 hidpi 真实值。
        let out = render_page(self.html, self.css, width, height, 1.0);
        if let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        {
            self.width = width;
            self.height = height;
            self.pixels = data;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Rc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("MusKitty")
                            .with_inner_size(LogicalSize::new(DEFAULT_W as f64, DEFAULT_H as f64)),
                    )
                    .expect("create window"),
            );
            let ww = WinitWindow::new(1, window.clone()).expect("init winit window");
            self.window = Some(window);
            self.winit_window = Some(ww);
            self.winit_window
                .as_ref()
                .expect("just set")
                .request_repaint();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::RedrawRequested => {
                // 尺寸变化时先重渲染（脏尺寸检查），再提交像素。
                let (w, h) = match &self.winit_window {
                    Some(ww) => {
                        let g = ww.geometry();
                        (g.width.max(1), g.height.max(1))
                    }
                    None => return,
                };
                if (w, h) != (self.width, self.height) {
                    self.render_at(w, h);
                }
                if let Some(ww) = &mut self.winit_window {
                    ww.present(&self.pixels, self.width, self.height);
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(ww) = &self.winit_window {
                    ww.request_repaint();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}
