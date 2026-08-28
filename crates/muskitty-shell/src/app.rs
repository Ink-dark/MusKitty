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

/// 初始窗口尺寸（逻辑 px；W-2 起布局视口为逻辑 CSS px，物理分辨率 ×scale）。
pub const DEFAULT_W: u32 = 800;
pub const DEFAULT_H: u32 = 600;

/// 应用状态：窗口 + 当前渲染结果。
///
/// `pixels` / `width` / `height` 为最近一次渲染的 RGBA 输出（物理分辨率，
/// `round(逻辑 × scale)`，present 用）；`logical_width` / `logical_height` /
/// `scale` 为最近一次渲染的布局状态（脏检查用，避免每帧全量渲染）。
pub struct App {
    window: Option<Rc<Window>>,
    winit_window: Option<WinitWindow>,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    logical_width: u32,
    logical_height: u32,
    scale: f32,
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
            logical_width: 0,
            logical_height: 0,
            scale: 1.0,
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

    /// 以逻辑尺寸 + scale 重新渲染页面，结果存入 `pixels`（物理分辨率）。
    fn render_at(&mut self, logical_width: u32, logical_height: u32, scale: f32) {
        // 布局用逻辑尺寸（CSS px），栅格化按 scale 输出物理分辨率（W-2）。
        let out = render_page(self.html, self.css, logical_width, logical_height, scale);
        if let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        {
            self.logical_width = logical_width;
            self.logical_height = logical_height;
            self.scale = scale;
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
                // 布局用窗口逻辑尺寸 + 当前 scale（W-2）；脏检查含 scale，
                // 任一变化才重渲染，再提交物理分辨率像素。
                let (w, h, scale) = match &self.winit_window {
                    Some(ww) => {
                        let g = ww.geometry();
                        (g.width.max(1), g.height.max(1), ww.hidpi_scale_factor())
                    }
                    None => return,
                };
                if (w, h, scale) != (self.logical_width, self.logical_height, self.scale) {
                    self.render_at(w, h, scale);
                }
                if let Some(ww) = &mut self.winit_window {
                    // present 用渲染输出的物理尺寸（self.width/height）。
                    ww.present(&self.pixels, self.width, self.height);
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(ww) = &self.winit_window {
                    ww.request_repaint();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                // scale 变化（如窗口拖到 HiDPI 显示器）→ 请求重绘，
                // RedrawRequested 按新逻辑尺寸+scale 重新布局渲染。
                if let Some(ww) = &self.winit_window {
                    ww.request_repaint();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}
