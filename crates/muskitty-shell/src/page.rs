//! 渲染管线：HTML + CSS → 像素。
//!
//! 把 DOM→CSS→Layout→Render 全链路串起来（浏览器外壳的核心职责），
//! 产出 [`muskitty_renderer::RenderOutput`]。renderer 只负责
//! `LayoutResult → RenderCommand[] → 像素`，本模块负责驱动整个管线。
//! 管线逻辑从 renderer 的 `window_demo` 抽出，供真窗口 / Headless 复用。

use muskitty_cascade::{compute_styles, StyleTreeOptions};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{paint, Backend, PaintInput, RenderOutput, TinySkiaBackend};

/// 渲染 HTML + CSS 到 RGBA 像素（[`RenderOutput::Pixels`]）。
///
/// `width` / `height` 为**逻辑**画布尺寸（CSS px，即布局视口）；
/// `scale` 为 HiDPI 缩放因子（物理像素 ÷ 逻辑像素，W-2）。布局与
/// 指令坐标均保持逻辑 px，输出分辨率为 `round(width×scale) ×
/// round(height×scale)`（物理 px）。
///
/// 管线步骤：
/// 1. HTML → DOM（muskitty-html5-parser）
/// 2. CSS → CssStyleSheet（Author origin）
/// 3. cascade + compute → 每元素 ComputedStyle
/// 4. layout → LayoutResult（视口 = width × height）
/// 5. paint → RenderCommand[]
/// 6. TinySkiaBackend::render → `RenderOutput::Pixels`（RGBA8，物理分辨率）
pub fn render_page(html: &str, css: &str, width: u32, height: u32, scale: f32) -> RenderOutput {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet(css);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    // media 视口 = 布局视口（逻辑 CSS px）；与 layout 用同一 width/height。
    let opts = StyleTreeOptions {
        viewport_width: width as f64,
        viewport_height: height as f64,
    };
    let styles = compute_styles(&dom, &[sheet], &opts);
    let mut tree = build_layout_tree(&dom, &styles);
    // 布局用逻辑尺寸（CSS px）；scale 只影响栅格化，不改变布局。
    let layout = compute_layout(&mut tree, width as f32, height as f32).expect("layout failed");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
    };
    let commands = paint(&input);
    let mut backend = TinySkiaBackend::new();
    backend.render(&commands, width, height, scale)
}

/// 渲染自包含 HTML 文件（含 `<style>`）到 RGBA 像素。
///
/// 读取 `path` 指向的 HTML 文件，用 [`extract_inline_style`] 提取其中
/// `<style>` 块作为 Author CSS，再走 [`render_page`] 全管线。用于
/// 渲染检测页（纯 HTML+CSS fixture）→ 与浏览器对照。
pub fn render_html_file(
    path: &str,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<RenderOutput, Box<dyn std::error::Error>> {
    let html = std::fs::read_to_string(path)?;
    let css = extract_inline_style(&html);
    Ok(render_page(&html, &css, width, height, scale))
}

/// 从自包含 HTML 提取所有 `<style>...</style>` 块内容，拼接为 CSS。
///
/// 标签名与属性大小写不敏感（HTML），但当前按 ASCII 小写匹配即可覆盖
/// 常见写法（`<style>`、`<style type="text/css">`）。无 `<style>` 时返回空串。
pub(crate) fn extract_inline_style(html: &str) -> String {
    const OPEN_TAG: &str = "<style";
    const CLOSE_TAG: &str = "</style>";
    let lower = html.to_ascii_lowercase();
    let mut css = String::new();
    let mut search_from = 0usize;
    while let Some(i) = lower[search_from..].find(OPEN_TAG) {
        let open = search_from + i;
        // 跳过开始标签本身（含属性），定位到 '>'.
        let Some(gt) = lower[open..].find('>') else {
            break;
        };
        let gt = open + gt + 1;
        let Some(close) = lower[gt..].find(CLOSE_TAG) else {
            break;
        };
        let close = gt + close;
        css.push_str(&html[gt..close]);
        css.push('\n');
        search_from = close + CLOSE_TAG.len();
    }
    css
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
        );
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
        let out = render_page(html, "body { margin: 0; }", 200, 80, 1.0);
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
    fn extract_inline_style_single_block() {
        let html = r#"<!doctype html><html><head><style>body{margin:0}</style></head><body></body></html>"#;
        assert_eq!(extract_inline_style(html), "body{margin:0}\n");
    }

    #[test]
    fn extract_inline_style_with_attributes_and_multiple_blocks() {
        let html = "<style type=\"text/css\">a{color:red}</style><body></body><style>b{display:block}</style>";
        assert_eq!(
            extract_inline_style(html),
            "a{color:red}\nb{display:block}\n"
        );
    }

    #[test]
    fn extract_inline_style_missing_is_empty() {
        assert_eq!(extract_inline_style("<body></body>"), "");
    }

    #[test]
    fn extract_inline_style_keeps_css_semicolons_and_braces() {
        let html = "<style>.a{border-top-width:1px;border-top-style:solid}</style>";
        assert_eq!(
            extract_inline_style(html),
            ".a{border-top-width:1px;border-top-style:solid}\n"
        );
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
        );
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
