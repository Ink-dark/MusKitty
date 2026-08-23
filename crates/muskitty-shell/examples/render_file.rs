//! 渲染自包含 HTML 文件（含 `<style>`）到 PNG —— 渲染检测工具。
//!
//! 把纯 HTML+CSS 检测页（`examples/assets/rendering-test.html`）经 shell 的
//! [`muskitty_shell::page::render_html_file`] 全管线渲染为 PNG，输出到当前
//! 目录，方便与浏览器截图逐块对照（背景色 / 边框 / box model / 文本 /
//! flex / display:none / overflow 裁剪）。
//!
//! 运行：
//! ```text
//! cargo run -p muskitty-shell --example render_file
//! cargo run -p muskitty-shell --example render_file -- <输入.html> <输出.png> [宽] [高]
//! ```

use muskitty_renderer::RenderOutput;
use muskitty_shell::page::render_html_file;
use tiny_skia::Pixmap;

// CARGO_MANIFEST_DIR 编译期指向 crate 根，不随运行 CWD 变化。
const DEFAULT_HTML: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/assets/rendering-test.html"
);
const DEFAULT_PNG: &str = "rendering-test.png";
const DEFAULT_WIDTH: u32 = 800;
const DEFAULT_HEIGHT: u32 = 2200;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (html_path, png_path, width, height) = match args.len() {
        0 => (DEFAULT_HTML, DEFAULT_PNG, DEFAULT_WIDTH, DEFAULT_HEIGHT),
        1 => (args[0].as_str(), DEFAULT_PNG, DEFAULT_WIDTH, DEFAULT_HEIGHT),
        2 => (
            args[0].as_str(),
            args[1].as_str(),
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
        ),
        3 => (
            args[0].as_str(),
            args[1].as_str(),
            args[2].parse().expect("width must be integer px"),
            DEFAULT_HEIGHT,
        ),
        4 => (
            args[0].as_str(),
            args[1].as_str(),
            args[2].parse().expect("width must be integer px"),
            args[3].parse().expect("height must be integer px"),
        ),
        _ => panic!("usage: render_file [<输入.html> <输出.png> [宽] [高]]"),
    };

    let out = render_html_file(html_path, width, height)
        .unwrap_or_else(|e| panic!("render {html_path} failed: {e}"));

    let RenderOutput::Pixels {
        width: w,
        height: h,
        data,
    } = out
    else {
        panic!("expected Pixels");
    };

    let mut pixmap = Pixmap::new(w, h).expect("alloc pixmap");
    pixmap.data_mut().copy_from_slice(&data);
    pixmap
        .save_png(png_path)
        .unwrap_or_else(|e| panic!("save {png_path} failed: {e}"));

    println!("rendered {html_path} -> {png_path} ({w}x{h})");
}
