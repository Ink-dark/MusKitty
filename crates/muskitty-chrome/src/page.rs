//! 渲染管线：HTML + CSS → 像素。
//!
//! 把 DOM→CSS→Layout→Render 全链路串起来（浏览器外壳的核心职责），
//! 产出 [`muskitty_renderer::RenderOutput`]。renderer 只负责
//! `LayoutResult → RenderCommand[] → 像素`，本模块负责驱动整个管线。
//! 管线逻辑从 renderer 的 `window_demo` 抽出，供真窗口 / Headless 复用。

use muskitty_cascade::{compute_styles, StyleTreeOptions};
use muskitty_cssom::CssStyleSheet;
use muskitty_layout::{build_layout_tree_with_fonts, compute_layout, SharedFontSystem};
use muskitty_renderer::{
    no_images, paint, Backend, ImageBits, PaintInput, RenderOutput, TinySkiaBackend,
};
use std::collections::HashMap;

use crate::images::{load_images, ImageLoadOptions};
use crate::stylesheets::{load_stylesheets, DocumentFetcher, LoadOptions};

pub use crate::stylesheets::author_sheet;

thread_local! {
    /// LAY-2：会话级共享字体系统。
    ///
    /// `FontSystem::new()` 枚举系统字体需 50–300 ms；render_page 每次调用
    ///（真窗口每帧 resize / 文件热重载）此前都随建树重复支付。线程级持有一
    /// 份（渲染单线程，Rc 语义即可），经 [`build_layout_tree_with_fonts`]
    /// 注入每次建树。
    static FONT_SYSTEM: SharedFontSystem = SharedFontSystem::new();
}

/// 渲染 HTML + CSS 文本到 RGBA 像素（[`RenderOutput::Pixels`]）。
///
/// 单表便捷入口：等价于 [`render_page_with_sheets`] 传一张 [`author_sheet`]。
/// 多表（外链 + 内嵌按文档序）请用 [`render_page_with_sheets`]。
pub fn render_page(
    html: &str,
    css: &str,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<RenderOutput, Box<dyn std::error::Error>> {
    render_page_with_sheets(html, &[author_sheet(css)], width, height, scale)
}

/// 渲染 HTML + 已加载样式表（CS-1 主入口）到 RGBA 像素。
///
/// `sheets` 为**文档序**样式表（采集/抓取/`@import` 展开见
/// [`crate::stylesheets`]）；cascade 按 slice 顺序展平，等特异性时后者胜。
///
/// `width` / `height` 为**逻辑**画布尺寸（CSS px，即布局视口）；
/// `scale` 为 HiDPI 缩放因子（物理像素 ÷ 逻辑像素，W-2）。布局与
/// 指令坐标均保持逻辑 px，输出分辨率为 `round(width×scale) ×
/// round(height×scale)`（物理 px）。
///
/// 管线步骤：
/// 1. HTML → DOM（muskitty-html5-parser）
/// 2. cascade + compute → 每元素 ComputedStyle（sheet 级 media/disabled 由 cascade 处理）
/// 3. layout → LayoutResult（视口 = width × height）
/// 4. paint → RenderCommand[]
/// 5. TinySkiaBackend::render → `RenderOutput::Pixels`（RGBA8，物理分辨率）
///
/// F-13（审计 S-7）：layout 失败以 `Err` 上抛而非 `.expect` panic——
/// layout crate 的约定明确要求调用方**不得**跨模块 expect（旧实现任何
/// taffy `Err` 都会 abort 整个浏览器进程）。调用方自行决定降级策略。
///
/// CS-1f：UA 样式表（[`crate::ua`]）自动置于表列表首位——所有渲染入口
/// （真窗口 / 无头 / 测试 / 文件模式）行为一致；cascade 的 origin 权重保证
/// 它低于作者表。
pub fn render_page_with_sheets(
    html: &str,
    sheets: &[CssStyleSheet],
    width: u32,
    height: u32,
    scale: f32,
) -> Result<RenderOutput, Box<dyn std::error::Error>> {
    render_page_with_images(html, sheets, no_images(), width, height, scale)
}

/// 渲染 HTML + 样式表 + 已解码背景图资源（BG-1 主入口）。
///
/// 与 [`render_page_with_sheets`] 同管线，差别只在 `images`：
/// **绝对 URL → 已解码像素**（由 [`crate::images::load_images`] 产出）。
/// 资源缺失的 URL 只是不画背景图，页面其余照常渲染。
pub fn render_page_with_images(
    html: &str,
    sheets: &[CssStyleSheet],
    images: &HashMap<String, ImageBits>,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<RenderOutput, Box<dyn std::error::Error>> {
    let dom = muskitty_html5_parser::parse(html);
    // media 视口 = 布局视口（逻辑 CSS px）；与 layout 用同一 width/height。
    let opts = StyleTreeOptions {
        viewport_width: width as f64,
        viewport_height: height as f64,
    };
    let mut all_sheets = Vec::with_capacity(sheets.len() + 1);
    all_sheets.push(crate::ua::ua_stylesheet());
    all_sheets.extend_from_slice(sheets);
    let styles = compute_styles(&dom, &all_sheets, &opts);
    // LAY-2：注入会话级共享字体系统（系统字体只枚举一次）。
    let mut tree = FONT_SYSTEM.with(|fonts| build_layout_tree_with_fonts(&dom, &styles, fonts));
    // 布局用逻辑尺寸（CSS px）；scale 只影响栅格化，不改变布局。
    let layout = compute_layout(&mut tree, width as f32, height as f32)?;
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images,
    };
    let commands = paint(&input);
    let mut backend = TinySkiaBackend::new();
    Ok(backend.render(&commands, width, height, scale))
}

/// 渲染自包含 HTML 文件（内嵌 + 同目录外链 CSS + 同目录背景图）到 RGBA 像素。
///
/// 读取 `path` 指向的 HTML 文件，以 `file://` URL 为 base 采集/抓取样式表
///（`<style>` + `<link rel=stylesheet href>`，含 `@import`）与样式表引用的
/// `url()` 背景图（BG-1），再走 [`render_page_with_images`] 全管线。
/// 用于渲染检测页（纯 HTML+CSS/图像 fixture）→ 与浏览器对照。
pub fn render_html_file(
    path: &str,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<RenderOutput, Box<dyn std::error::Error>> {
    let html = std::fs::read_to_string(path)?;
    let document_url =
        muskitty_network::url::file_url_from_path(path).ok_or("unmappable file path")?;
    let dom = muskitty_html5_parser::parse(&html);
    let (sheets, images) = {
        let mut fetcher = DocumentFetcher::new(&document_url);
        let (sheets, stats) = load_stylesheets(
            &dom,
            &document_url,
            &mut |url| fetcher.fetch_text(url),
            &LoadOptions::default(),
        );
        report_load_failures(&stats);
        // BG-1：以各表自身 location 为基准解析 url() 并抓取解码。
        let (images, image_stats) = load_images(
            &sheets,
            &document_url,
            &mut |url| fetcher.fetch_bytes(url),
            &ImageLoadOptions::default(),
        );
        report_image_failures(&image_stats);
        (sheets, images)
    };
    render_page_with_images(&html, &sheets, &images, width, height, scale)
}

/// 抓取失败的可观测出口（失败不致命：页面照常渲染，只报一行汇总）。
pub(crate) fn report_load_failures(stats: &crate::stylesheets::LoadStats) {
    if stats.failed > 0 || stats.skipped > 0 {
        eprintln!(
            "muskitty-chrome: stylesheets: {} fetched, {} failed, {} skipped \
             ({} sources, {} imports, {} cache hits)",
            stats.fetched,
            stats.failed,
            stats.skipped,
            stats.sources,
            stats.imports,
            stats.cache_hits
        );
    }
}

/// 图像抓取/解码失败的可观测出口（失败不致命：该元素不画背景图，页面照常）。
pub(crate) fn report_image_failures(stats: &crate::images::ImageLoadStats) {
    if stats.failed > 0 || stats.skipped > 0 {
        eprintln!(
            "muskitty-chrome: images: {} fetched, {} failed, {} skipped \
             ({} references, {} decoded)",
            stats.fetched, stats.failed, stats.skipped, stats.references, stats.decoded
        );
    }
}

/// 把 RGBA8 像素（行长 = `width * 4`）编码为 PNG。
///
/// shell 侧的 PNG 出口（[`crate::render_to_png`] /
/// `HeadlessWindow::save_png`），后端与 renderer 一致（tiny-skia）。
/// tiny-skia 类型只在本函数内部使用，不出现在 pub 签名（对齐
/// decoupling ADR）。
pub(crate) fn encode_png(
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let Some(mut pixmap) = tiny_skia::Pixmap::new(width, height) else {
        return Err(format!("encode_png: invalid dimensions {width}x{height}").into());
    };
    let expected = data.len();
    let buf_len = pixmap.data_mut().len();
    if expected != buf_len {
        return Err(format!(
            "encode_png: data length {expected} does not match {width}x{height} RGBA buffer ({buf_len})"
        )
        .into());
    }
    pixmap.data_mut().copy_from_slice(data);
    Ok(pixmap.encode_png()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 单行无内部空白：空白文本节点在布局中会占位（Muskitty 尚未实现
    // 空白折叠），会破坏 div 位置断言。
    const RED_DIV_HTML: &str = r#"<!doctype html><html><body><div style="width:100px;height:50px;background-color:#ff0000"></div></body></html>"#;

    /// 读取 (x, y) 处 RGBA 像素（8-bit per channel）。
    fn pixel(data: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let i = ((y * width + x) * 4) as usize;
        (data[i], data[i + 1], data[i + 2], data[i + 3])
    }

    #[test]
    fn render_red_div_pixel_and_white_canvas() {
        let out = render_page(
            RED_DIV_HTML,
            "div { display: block; } body { margin: 0; }",
            200,
            100,
            1.0,
        )
        .expect("render_page ok");
        let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        else {
            panic!("expected Pixels");
        };
        assert_eq!(width, 200);
        assert_eq!(height, 100);

        // (10, 10) 在 div 内 → 红（不透明）。
        let (r, g, b, a) = pixel(&data, width, 10, 10);
        assert_eq!((r, g, b, a), (255, 0, 0, 255));
        // (150, 10) 在 div 外 → 白画布（P3-5 白底）。
        let (r, g, b, _) = pixel(&data, width, 150, 10);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    #[test]
    fn render_text_produces_ink() {
        let html = r#"
<!doctype html>
<html><body>
  <p style="font-size:24px;color:#000000">Hello</p>
</body></html>
"#;
        let out = render_page(html, "body { margin: 0; }", 200, 80, 1.0).expect("render_page ok");
        let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        else {
            panic!("expected Pixels");
        };
        assert_eq!(width, 200);
        assert_eq!(height, 80);

        // 统计非白像素（文字墨迹），应存在。
        let mut ink = 0usize;
        for py in 0..height {
            for px in 0..width {
                let (r, g, b, _) = pixel(&data, width, px, py);
                if r < 200 || g < 200 || b < 200 {
                    ink += 1;
                }
            }
        }
        assert!(ink > 0, "text should produce non-white (ink) pixels");
    }

    #[test]
    fn render_html_file_renders_style_driven_page() {
        let dir = std::env::temp_dir();
        let path = dir.join("muskitty_render_html_file_test.html");
        std::fs::write(
            &path,
            r#"<!doctype html><html><head><style>div{display:block;width:100px;height:50px;background-color:#ff0000}</style></head><body><div></div></body></html>"#,
        )
        .unwrap();
        let out = render_html_file(&path.to_string_lossy(), 200, 100, 1.0).expect("render file");
        let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        else {
            panic!("expected Pixels");
        };
        assert_eq!((width, height), (200, 100));
        let (r, g, b, _) = pixel(&data, width, 10, 10);
        assert_eq!((r, g, b), (255, 0, 0));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn render_html_file_follows_external_stylesheet() {
        // CS-1：同目录外链 CSS（相对路径）必须生效。
        let dir = std::env::temp_dir().join("muskitty_ext_css_fixture");
        std::fs::create_dir_all(&dir).unwrap();
        let html_path = dir.join("index.html");
        std::fs::write(
            &html_path,
            r#"<!doctype html><html><head><link rel="stylesheet" href="css/page.css"></head><body><div></div></body></html>"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("css")).unwrap();
        std::fs::write(
            dir.join("css").join("page.css"),
            "body{margin:0} div{display:block;width:100px;height:50px;background-color:#00cc00}",
        )
        .unwrap();

        let out = render_html_file(&html_path.to_string_lossy(), 200, 100, 1.0).expect("render");
        let RenderOutput::Pixels { width, data, .. } = out else {
            panic!("expected Pixels");
        };
        assert_eq!(
            pixel(&data, width, 10, 10),
            (0, 204, 0, 255),
            "external CSS applied"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_page_with_sheets_prefers_later_sheet() {
        // 等特异性：后一张表胜出（cascade 的全局 order 语义）。
        // 不能用 RED_DIV_HTML——它的 background 写在 style 属性里，内联声明
        // 优先于两张表，断言会变成"内联胜出"。
        let html = r#"<!doctype html><html><body><div></div></body></html>"#;
        let first = author_sheet(
            "body{margin:0} div{display:block;width:100px;height:50px;background-color:#ff0000}",
        );
        let second = author_sheet("div{background-color:#0000ff}");
        let out = render_page_with_sheets(html, &[first, second], 200, 100, 1.0).expect("render");
        let RenderOutput::Pixels { width, data, .. } = out else {
            panic!("expected Pixels");
        };
        assert_eq!(pixel(&data, width, 10, 10), (0, 0, 255, 255));
    }

    #[test]
    fn render_page_scale_2_doubles_output_resolution() {
        // W-2 退出条件：逻辑 200×100 布局 + scale=2 → 输出 400×200；
        // 红块（逻辑 100×50，body margin 0 置于 (0,0)）物理坐标 (20,20) 为红，
        // 红块外 (300,20) 仍为白画布（布局不变，仅栅格化放大）。
        let out = render_page(
            RED_DIV_HTML,
            "div { display: block; } body { margin: 0; }",
            200,
            100,
            2.0,
        )
        .expect("render_page ok");
        let RenderOutput::Pixels {
            width,
            height,
            data,
        } = out
        else {
            panic!("expected Pixels");
        };
        assert_eq!((width, height), (400, 200));

        // 逻辑 (10,10) → 物理 (20,20)：红块内。
        let (r, g, b, _) = pixel(&data, width, 20, 20);
        assert_eq!((r, g, b), (255, 0, 0));
        // 逻辑 (150,10) → 物理 (300,20)：红块宽 100 逻辑 → 200 物理，此点在块外。
        let (r, g, b, _) = pixel(&data, width, 300, 20);
        assert_eq!((r, g, b), (255, 255, 255));
    }
}
