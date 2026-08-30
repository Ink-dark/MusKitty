//! 无窗口环境渲染测试（chrome 层 W-4 价值平移）。
//!
//! 不创建任何 OS 窗口：`render_page_to_png` / `render_window_to_png`
//! 纯内存合成 → PNG。CI（无 winit/softbuffer）可跑本文件全部测试，
//! 包括 `--no-default-features`。

use muskitty_chrome::chrome::model::ChromeState;
use muskitty_chrome::headless::{render_page_to_png, render_window_to_png};
use tiny_skia::Pixmap;

const HTML: &str = r#"<!doctype html><html><body><div style="width:100px;height:50px;background-color:#ff0000"></div></body></html>"#;
const CSS: &str = "div { display: block; } body { margin: 0; }";

#[test]
fn render_page_to_png_matches_direct_render() {
    let path = std::env::temp_dir().join("muskitty_chrome_page_test.png");
    render_page_to_png(HTML, CSS, 200, 100, 1.0, &path).expect("render");

    let pixmap = Pixmap::load_png(&path).expect("decode");
    assert_eq!((pixmap.width(), pixmap.height()), (200, 100));
    // 红块 (10,10)。
    let d = pixmap.data();
    let i = ((10 * 200 + 10) * 4) as usize;
    assert_eq!((d[i], d[i + 1], d[i + 2]), (255, 0, 0));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn render_window_to_png_composes_page_and_chrome() {
    let path = std::env::temp_dir().join("muskitty_chrome_window_test.png");
    let state = ChromeState::default();
    render_window_to_png(
        HTML,
        CSS,
        800,
        600,
        1.0,
        2,
        &["Demo", "Tab 2"],
        0,
        &state,
        &path,
    )
    .expect("render");

    let pixmap = Pixmap::load_png(&path).expect("decode");
    assert_eq!((pixmap.width(), pixmap.height()), (800, 600));
    let d = pixmap.data();
    let pixel = |x: u32, y: u32| {
        let i = ((y * 800 + x) * 4) as usize;
        (d[i], d[i + 1], d[i + 2])
    };
    // chrome 标签条背景（两标签共 220*2+8 内边 = 448 之后、+ 按钮 764 之前的空白）。
    assert_eq!(pixel(460, 4), (0xde, 0xe1, 0xe6));
    // 页面红块：视口 y=80 起，页面内 (10,10) → 窗口 (10,90)。
    assert_eq!(pixel(10, 90), (255, 0, 0));
    // 工具栏底部分隔线（y=79）。
    assert_eq!(pixel(400, 79), (0xcf, 0xd4, 0xda));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn render_window_to_png_scale_2_resolution() {
    let path = std::env::temp_dir().join("muskitty_chrome_scale2_test.png");
    let state = ChromeState::default();
    render_window_to_png(HTML, CSS, 400, 300, 2.0, 1, &["Demo"], 0, &state, &path).expect("render");
    let pixmap = Pixmap::load_png(&path).expect("decode");
    assert_eq!((pixmap.width(), pixmap.height()), (400, 300));
    // scale=2：chrome 高 160 物理px；页面视口 (0,160)-(400,300)。
    let d = pixmap.data();
    let i = ((170 * 400 + 10) * 4) as usize;
    assert_eq!((d[i], d[i + 1], d[i + 2]), (255, 0, 0));
    let _ = std::fs::remove_file(&path);
}
