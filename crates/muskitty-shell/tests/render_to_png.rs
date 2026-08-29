//! 无窗口环境渲染测试（W-4 C-3）。
//!
//! 不创建任何 OS 窗口：`render_page` / `render_to_png` /
//! `HeadlessWindow` 纯内存渲染 + PNG 编码出口。CI（无 winit/softbuffer
//! 系统依赖）可跑本文件全部测试。
//!
//! 一致性判据（goal.md W-4 退出条件）：`render_to_png` 写出的 PNG 解码
//! 后像素与 [`render_page`] 直接渲染的像素完全一致；`HeadlessWindow`
//! 经 `present` 保存的帧与直接渲染一致。

use muskitty_renderer::RenderOutput;
use muskitty_shell::headless_window::HeadlessWindow;
use muskitty_shell::page::render_page;
use muskitty_shell::{render_to_png, window::PlatformWindow};

const HTML: &str = r#"<!doctype html><html><body><div style="width:100px;height:50px;background-color:#ff0000"><p style="font-size:20px;color:#000000">Hi</p></div></body></html>"#;
const CSS: &str = "div { display: block; } body { margin: 0; }";

fn render_direct(width: u32, height: u32, scale: f32) -> (Vec<u8>, u32, u32) {
    let RenderOutput::Pixels {
        width,
        height,
        data,
    } = render_page(HTML, CSS, width, height, scale)
    else {
        panic!("expected Pixels");
    };
    (data, width, height)
}

#[test]
fn render_to_png_pixels_match_direct_render() {
    let (direct, w, h) = render_direct(200, 100, 1.0);

    let path = std::env::temp_dir().join("muskitty_w4_render_to_png_test.png");
    render_to_png(HTML, CSS, 200, 100, 1.0, &path).expect("render_to_png");

    // 解码 PNG 回读（与 renderer 同一 tiny-skia 后端）。
    let pixmap = tiny_skia::Pixmap::load_png(&path).expect("decode png");
    assert_eq!((pixmap.width(), pixmap.height()), (w, h));
    assert_eq!(
        pixmap.data().to_vec(),
        direct,
        "PNG pixels must equal direct render"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn headless_window_frame_matches_direct_render() {
    let (direct, w, h) = render_direct(200, 100, 1.0);

    // 走 PlatformWindow trait 的窗口路径：present 保存帧。
    let mut win = HeadlessWindow::new(200, 100);
    win.present(&direct, w, h);
    let (frame, fw, fh) = win.frame().expect("frame after present");
    assert_eq!((fw, fh), (w, h));
    assert_eq!(frame, &direct[..]);

    // 帧经 save_png 编码后与 render_to_png 产物一致（同一编码出口）。
    let path_a = std::env::temp_dir().join("muskitty_w4_headless_save.png");
    let path_b = std::env::temp_dir().join("muskitty_w4_render_to_png.png");
    win.save_png(&path_a).expect("save_png");
    render_to_png(HTML, CSS, 200, 100, 1.0, &path_b).expect("render_to_png");
    assert_eq!(
        std::fs::read(&path_a).unwrap(),
        std::fs::read(&path_b).unwrap()
    );

    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}

#[test]
fn render_to_png_scale_2_resolution_matches_direct() {
    let (direct, w, h) = render_direct(200, 100, 2.0);
    assert_eq!((w, h), (400, 200), "scale=2 doubles physical resolution");

    let path = std::env::temp_dir().join("muskitty_w4_scale2_test.png");
    render_to_png(HTML, CSS, 200, 100, 2.0, &path).expect("render_to_png");
    let pixmap = tiny_skia::Pixmap::load_png(&path).expect("decode png");
    assert_eq!((pixmap.width(), pixmap.height()), (w, h));
    assert_eq!(pixmap.data().to_vec(), direct);

    let _ = std::fs::remove_file(&path);
}
