//! 无窗口渲染出口（W-4 价值平移）。
//!
//! 不创建 OS 窗口：页面渲染管线 + chrome 合成 → PNG 文件。CI（无
//! winit/softbuffer 系统依赖）可跑 `cargo test -p muskitty-chrome
//! --no-default-features`。PNG 编码在 shell 侧同款 tiny-skia 路径。

use crate::chrome::model::{chrome_height, layout_chrome, ChromeState};
use crate::chrome::paint::ChromeAssets;
use crate::compositor::compose_frame;
use std::path::Path;

/// 渲染 HTML + CSS（纯页面，无 chrome）到 PNG。
///
/// 管线与真窗口的页面渲染一致（[`crate::page::render_page`]）。
pub fn render_page_to_png(
    html: &str,
    css: &str,
    width: u32,
    height: u32,
    scale: f32,
    path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = crate::page::render_page(html, css, width, height, scale)?;
    let muskitty_renderer::RenderOutput::Pixels {
        width,
        height,
        data,
    } = out
    else {
        return Err("render_page: expected Pixels".into());
    };
    std::fs::write(path, crate::page::encode_png(&data, width, height)?)?;
    Ok(())
}

/// 渲染**完整浏览器帧**（页面 + chrome 合成）到 PNG。
///
/// 与真窗口同一条合成路径（[`compose_frame`]）：active 标签按页面视口
/// 逻辑尺寸渲染，chrome 按窗口物理尺寸布局绘制。无窗口环境的像素级
/// 回归出口（CI）。
#[allow(clippy::too_many_arguments)]
pub fn render_window_to_png(
    html: &str,
    css: &str,
    window_width: u32,
    window_height: u32,
    scale: f32,
    tab_count: usize,
    tab_titles: &[&str],
    active_tab: usize,
    chrome_state: &ChromeState,
    path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rects = layout_chrome(window_width, window_height, scale, tab_count, chrome_state);
    let viewport_logical_w = ((window_width as f32) / scale).max(1.0) as u32;
    let viewport_logical_h =
        (((window_height as f32) - chrome_height(scale)) / scale).max(1.0) as u32;
    let out = crate::page::render_page(html, css, viewport_logical_w, viewport_logical_h, scale)?;
    let (page_data, page_w, page_h) = match out {
        muskitty_renderer::RenderOutput::Pixels {
            width,
            height,
            data,
        } => (data, width, height),
        _ => return Err("render_page: expected Pixels".into()),
    };
    let mut assets = ChromeAssets::new();
    let frame = compose_frame(
        window_width,
        window_height,
        (&page_data, page_w, page_h),
        rects.page_viewport,
        &rects,
        chrome_state,
        tab_titles,
        active_tab,
        &mut assets,
    )
    .ok_or("compose_frame failed")?;
    std::fs::write(
        path,
        crate::page::encode_png(frame.data(), frame.width(), frame.height())?,
    )?;
    Ok(())
}
