//! W-1 窗口 demo：HTML + CSS → 渲染 → winit 真窗口显示。
//!
//! 运行：
//! ```text
//! cargo run -p muskitty-renderer --example window_demo
//! ```
//!
//! tiny-skia 软件渲染到 RGBA 像素，转成 softbuffer 的 0RGB 格式显示到
//! winit 窗口。窗口可缩放、可关闭（Esc / 关闭按钮）。

use std::num::NonZeroU32;
use std::rc::Rc;

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use muskitty_cascade::{compute_styles, StyleTreeOptions};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{paint, Backend, PaintInput, TinySkiaBackend};

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

/// 渲染 HTML+CSS 到 softbuffer 的 0RGB 像素（u32，row-major）。
fn render_page(width: u32, height: u32) -> Vec<u32> {
    let dom = muskitty_html5_parser::parse(HTML);
    let parsed = parse_stylesheet(CSS);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles(&dom, &[sheet], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, width as f32, height as f32).expect("layout failed");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
    };
    let commands = paint(&input);
    let mut backend = TinySkiaBackend::new();
    backend.render(&commands, width, height);
    let pixmap = backend.take_pixmap().expect("pixmap after render");

    // RGBA8 → softbuffer 0RGB u32。
    let mut out = Vec::with_capacity((width * height) as usize);
    for px in pixmap.data().chunks_exact(4) {
        out.push(((px[2] as u32) << 16) | ((px[1] as u32) << 8) | (px[0] as u32));
    }
    out
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    pixels: Vec<u32>,
    width: u32,
    height: u32,
}

impl App {
    fn new() -> Self {
        let pixels = render_page(VIEWPORT_W, VIEWPORT_H);
        Self {
            window: None,
            surface: None,
            pixels,
            width: VIEWPORT_W,
            height: VIEWPORT_H,
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
                                self.width as f64,
                                self.height as f64,
                            )),
                    )
                    .expect("create window"),
            );
            let context = Context::new(window.clone()).expect("softbuffer context");
            let surface = Surface::new(&context, window.clone()).expect("softbuffer surface");
            self.window = Some(window);
            self.surface = Some(surface);
            self.window.as_ref().unwrap().request_redraw();
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
                if let (Some(surface), Some(window)) = (&mut self.surface, &self.window) {
                    let size = window.inner_size();
                    let (w, h) = (size.width.max(1), size.height.max(1));
                    surface
                        .resize(
                            NonZeroU32::new(w).expect("w > 0"),
                            NonZeroU32::new(h).expect("h > 0"),
                        )
                        .expect("surface resize");
                    // 尺寸变化时按新尺寸重新渲染。
                    if w != self.width || h != self.height {
                        self.width = w;
                        self.height = h;
                        self.pixels = render_page(w, h);
                    }
                    let mut buffer = surface.buffer_mut().expect("softbuffer buffer");
                    buffer.copy_from_slice(&self.pixels);
                    buffer.present().expect("present frame");
                }
            }
            WindowEvent::Resized(_) => {
                self.window.as_ref().unwrap().request_redraw();
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
