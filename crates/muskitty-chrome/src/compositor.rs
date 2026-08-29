//! 帧合成：页面像素 + chrome → 全窗口 RGBA。
//!
//! Chromium Views"chrome 即合成像素"的最小实现：先白底全窗口，把页面
//! 物理像素 blit 到页面视口位置，再在其上绘制 chrome（chrome 条不与页面
//! 重叠，但绘制顺序仍保证 chrome 在上）。输入输出都是纯像素数据，
//! 无外部依赖类型（tiny-skia 仅内部使用，decoupling ADR）。

use crate::chrome::model::{ChromeRects, ChromeState, Rect};
use crate::chrome::paint::{paint_chrome, ChromeAssets};
use tiny_skia::Pixmap;

/// 合成一帧全窗口像素。
///
/// - `window_width × window_height`：窗口客户区物理尺寸；
/// - `page`：active 标签最近一帧 `(RGBA 数据, 宽, 高)`（物理 px，
///   `page::render_page` 输出；空数据跳过 blit，如尚未渲染）；
/// - `viewport`：页面视口矩形（`rects.page_viewport`）；
/// - 页面比视口小（如窗口刚放大、重渲染前）时按左上角对齐，其余露白。
#[allow(clippy::too_many_arguments)]
pub fn compose_frame(
    window_width: u32,
    window_height: u32,
    page: (&[u8], u32, u32),
    viewport: Rect,
    rects: &ChromeRects,
    state: &ChromeState,
    tab_titles: &[&str],
    active_tab: usize,
    assets: &mut ChromeAssets,
) -> Option<Pixmap> {
    let mut frame = Pixmap::new(window_width, window_height)?;
    // 全窗口白底（Pixmap::new 初始为透明黑，直接 present 会是黑屏）。
    frame.fill(tiny_skia::Color::WHITE);

    // 页面 blit（页面数据来自 tiny-skia Pixmap，本就是预乘 RGBA，
    // from_vec 语义一致）。
    let (data, pw, ph) = page;
    if !data.is_empty() && pw > 0 && ph > 0 {
        if let Some(size) = tiny_skia::IntSize::from_wh(pw, ph) {
            if let Some(page_pixmap) = Pixmap::from_vec(data.to_vec(), size) {
                frame.draw_pixmap(
                    viewport.x.round() as i32,
                    viewport.y.round() as i32,
                    page_pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
        }
    }

    // chrome 绘制在页面之上。
    paint_chrome(&mut frame, rects, state, tab_titles, active_tab, assets);

    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::model::{layout_chrome, ChromeState};

    const HTML: &str = r#"<!doctype html><html><body><div style="width:100px;height:50px;background-color:#ff0000"></div></body></html>"#;
    const CSS: &str = "div { display: block; } body { margin: 0; }";

    fn page() -> (Vec<u8>, u32, u32) {
        let out = muskitty_shell::page::render_page(HTML, CSS, 200, 100, 1.0);
        let muskitty_renderer::RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        else {
            panic!("expected Pixels");
        };
        (data, width, height)
    }

    #[test]
    fn compose_frame_blits_page_below_chrome() {
        let rects = layout_chrome(800, 600, 1.0, 1, &ChromeState::default());
        let state = ChromeState::default();
        let mut assets = ChromeAssets::new();
        let (data, pw, ph) = page();
        let frame = compose_frame(
            800,
            600,
            (&data, pw, ph),
            rects.page_viewport,
            &rects,
            &state,
            &["Demo"],
            0,
            &mut assets,
        )
        .expect("frame");
        assert_eq!((frame.width(), frame.height()), (800, 600));

        let d = frame.data();
        let pixel = |x: u32, y: u32| {
            let i = ((y * 800 + x) * 4) as usize;
            (d[i], d[i + 1], d[i + 2])
        };
        // 页面红块：视口 y=80 起，页面内 (10,10) → 窗口 (10,90)。
        assert_eq!(pixel(10, 90), (255, 0, 0));
        // chrome 区域不透页面：标签条空白处灰。
        assert_eq!(pixel(400, 4), (0xde, 0xe1, 0xe6));
        // 页面之外视口区域露白（页面 200 宽 < 视口 800 宽）。
        assert_eq!(pixel(700, 90), (255, 255, 255));
    }

    #[test]
    fn compose_frame_without_page_is_chrome_only() {
        let rects = layout_chrome(800, 600, 1.0, 1, &ChromeState::default());
        let state = ChromeState::default();
        let mut assets = ChromeAssets::new();
        let frame = compose_frame(
            800,
            600,
            (&[], 0, 0),
            rects.page_viewport,
            &rects,
            &state,
            &["Demo"],
            0,
            &mut assets,
        )
        .expect("frame");
        let d = frame.data();
        let i = ((90 * 800 + 10) * 4) as usize;
        // 无页面 → 视口位置露白底。
        assert_eq!((d[i], d[i + 1], d[i + 2]), (255, 255, 255));
    }
}
