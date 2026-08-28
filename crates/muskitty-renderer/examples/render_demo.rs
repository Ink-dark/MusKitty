//! B-4 端到端 demo：HTML + CSS → cascade → layout → paint → PNG。
//!
//! 运行：
//! ```text
//! cargo run --example render_demo
//! ```
//!
//! 输出 `render_demo.png` 到当前工作目录。

use muskitty_cascade::{compute_styles, StyleTreeOptions};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{paint, Backend, PaintInput, TinySkiaBackend};

const VIEWPORT_W: f32 = 800.0;
const VIEWPORT_H: f32 = 600.0;

const HTML: &str = r#"
<!doctype html>
<html>
  <body>
    <div style="background-color: #2196f3; width: 600px; height: 400px; border-width: 4px; border-style: solid; border-color: #0d47a1">
      <div style="background-color: #ffeb3b; width: 200px; height: 200px; border-width: 2px; border-style: solid; border-color: #f57f17"></div>
      <div style="background-color: #f44336; width: 200px; height: 150px; border-width: 2px; border-style: solid; border-color: #b71c1c"></div>
    </div>
    <p style="font-size: 32px; color: #212121">Hello MusKitty</p>
  </body>
</html>
"#;

const CSS: &str = r#"
div { display: block; }
body { margin: 0; }
"#;

fn main() {
    // 1. 解析 HTML → DOM
    let dom = muskitty_html5_parser::parse(HTML);

    // 2. 解析 CSS → CssStyleSheet
    let parsed = parse_stylesheet(CSS);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };

    // 3. cascade + compute → 每元素 ComputedStyle
    let styles = compute_styles(&dom, &[sheet], &StyleTreeOptions::default());

    // 4. layout → LayoutResult
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, VIEWPORT_W, VIEWPORT_H).expect("layout failed");

    // 5. paint → RenderCommand[]
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
    };
    let commands = paint(&input);

    println!("paint produced {} render commands", commands.len());
    for (i, cmd) in commands.iter().enumerate() {
        println!("  [{}] {:?}", i, cmd);
    }

    // 6. render → PNG via tiny-skia
    let mut backend = TinySkiaBackend::new();
    // P2-18：消费返回的像素输出（此处仅确认形状，PNG 编码走内部 pixmap）。
    // W-2：demo 默认 1x（逻辑 = 物理）。
    let output = backend.render(&commands, VIEWPORT_W as u32, VIEWPORT_H as u32, 1.0);
    if let muskitty_renderer::RenderOutput::Pixels {
        width,
        height,
        data,
    } = output
    {
        println!(
            "  Pixel buffer: {}x{} ({} bytes)",
            width,
            height,
            data.len()
        );
    }

    let out_path = "render_demo.png";
    backend
        .save_png(out_path)
        .unwrap_or_else(|e| panic!("failed to save PNG to {}: {}", out_path, e));

    println!();
    println!("✓ Rendered to {}", out_path);
    println!("  Viewport: {}x{}", VIEWPORT_W, VIEWPORT_H);
    println!("  Commands: {}", commands.len());
}
