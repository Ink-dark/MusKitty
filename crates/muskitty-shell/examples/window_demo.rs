//! W-1 窗口 demo：HTML + CSS → 渲染 → winit 真窗口显示。
//!
//! 直接通过 [`PlatformWindow`] trait 构造窗口（不经 [`App`] 便捷入口），
//! 演示窗口抽象的解耦：`render_page` 产 RGBA 像素 → `present` 显示。
//! 与 renderer 的 `window_demo` 行为一致（可缩放、可关闭）。
//!
//! 运行：
//! ```text
//! cargo run -p muskitty-shell --example window_demo
//! ```

use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use muskitty_renderer::RenderOutput;
use muskitty_shell::page::render_page;
use muskitty_shell::window::PlatformWindow;
use muskitty_shell::winit_window::WinitWindow;

const VIEWPORT_W: u32 = 800;
const VIEWPORT_H: u32 = 600;

const HTML: &str = r#"
<!doctype html>
<html>
  <body>
    <div style="background-color: #2196f3; width: 600px; height: 300px; border-width: 4px; border-style: solid; border-color: #0d47a1">
      <div style="background-color: #ffeb3b; width: 200px; height: 120px; border-width: 2px; border-style: solid; border-color: #f57f17"></div>
    </div>
    <p style="font-size: 32px; color: #212121">Hello MusKitty</p>
    <p style="font-size: 20px; color: #757575">DOM → CSS → Layout → Render</p>
  </body>
</html>
"#;

const CSS: &str = r#"
div { display: block; }
body { margin: 0; }
"#;

struct App {
    window: Option<Box<dyn PlatformWindow>>,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            pixels: Vec::new(),
            width: 0,
            height: 0,
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
                            .with_inner_size(LogicalSize::new(
                                VIEWPORT_W as f64,
                                VIEWPORT_H as f64,
                            )),
                    )
                    .expect("create window"),
            );
            // WinitWindow 经 trait 对象使用，上层只依赖 PlatformWindow。
            let ww = WinitWindow::new(1, window.clone()).expect("init winit window");
            self.window = Some(Box::new(ww));
            self.window.as_ref().expect("just set").request_repaint();
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
                let (w, h, needs_render) = match &self.window {
                    Some(win) => {
                        let g = win.geometry();
                        let (w, h) = (g.width.max(1), g.height.max(1));
                        (w, h, (w, h) != (self.width, self.height))
                    }
                    None => return,
                };
                if needs_render {
                    if let RenderOutput::Pixels { data, .. } = render_page(HTML, CSS, w, h) {
                        self.pixels = data;
                        self.width = w;
                        self.height = h;
                    }
                }
                if let Some(win) = self.window.as_mut() {
                    win.present(&self.pixels, self.width, self.height);
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(win) = &self.window {
                    win.request_repaint();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run app");
}
