//! B-4 集成测试：端到端 demo（HTML + CSS → cascade → layout → paint → PNG）。
//!
//! 验证完整 pipeline：
//! 1. 解析 HTML + CSS
//! 2. cascade + compute → ComputedStyle
//! 3. layout → LayoutResult
//! 4. paint → RenderCommand[]
//! 5. render via TinySkiaBackend → PNG bytes
//!
//! 这是 muskitty-renderer 的「smoke test」：证明 DOM→CSS→Layout→Render 全链路打通。

use muskitty_cascade::{compute_styles as compute_styles_tree, ComputedStyle, StyleTreeOptions};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{no_images, paint, Backend, PaintInput, RenderOutput, TinySkiaBackend};
use muskitty_selectors::matching::{DomElement, Element as _};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 运行完整 pipeline 并编码为 PNG。
fn render_to_png(html: &str, css: &str, vw: f32, vh: f32) -> Vec<u8> {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet(css);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());

    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, vw, vh).expect("layout should succeed");

    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images: no_images(),
    };
    let commands = paint(&input);

    let mut backend = TinySkiaBackend::new();
    // P2-18：render 返回像素输出，测试消费返回值确认尺寸与数据长度。
    let output = backend.render(&commands, vw as u32, vh as u32, 1.0);
    match output {
        RenderOutput::Pixels {
            width,
            height,
            data,
        } => {
            assert_eq!(width, vw as u32, "pixel buffer width");
            assert_eq!(height, vh as u32, "pixel buffer height");
            assert_eq!(
                data.len(),
                vw as usize * vh as usize * 4,
                "RGBA buffer length"
            );
        }
        other => panic!("expected Pixels from tiny-skia, got {:?}", other),
    }
    backend
        .encode_png()
        .expect("PNG encoding should succeed after render")
}

#[test]
fn end_to_end_single_red_box_to_png() {
    let png = render_to_png(
        r#"<div style="background-color: red; width: 100px; height: 50px"></div>"#,
        "",
        200.0,
        200.0,
    );
    // PNG magic header
    assert!(png.len() > 64, "PNG should be non-trivial size");
    assert_eq!(
        &png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
}

#[test]
fn end_to_end_nested_boxes_to_png() {
    let html = r#"
    <div style="background-color: #2196f3; width: 200px; height: 200px; border-width: 4px; border-style: solid; border-color: #0d47a1">
      <div style="background-color: #ffeb3b; width: 100px; height: 100px"></div>
    </div>
    "#;
    let css = "div { display: block; }";
    let png = render_to_png(html, css, 400.0, 400.0);
    assert!(png.len() > 1024, "PNG with multiple rects should be > 1KB");
    assert_eq!(
        &png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
}

#[test]
fn end_to_end_empty_document_produces_valid_png() {
    let png = render_to_png("<html></html>", "", 100.0, 100.0);
    // 即便无内容，也应产出合法 PNG（透明画布）
    assert_eq!(
        &png[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    );
    assert!(png.len() > 64);
}

// —— H-4: var() 端到端（cascade 收集自定义属性 → computed value）——

/// 运行 HTML+CSS 全链路（不含 layout/paint），返回每元素 ComputedStyle。
fn compute_styles(html: &str, css: &str) -> (Rc<RefCell<Node>>, HashMap<usize, ComputedStyle>) {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet(css);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());
    (dom, styles)
}

/// 按 id 查找元素。
fn find_element_by_id(node: &Rc<RefCell<Node>>, id: &str) -> Option<DomElement> {
    if matches!(&node.borrow().kind, NodeKind::Element(_)) {
        let el = DomElement::new(Rc::clone(node));
        if el.get_attribute("id").as_deref() == Some(id) {
            return Some(el);
        }
    }
    for child in node.borrow().child_nodes() {
        if let Some(found) = find_element_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

/// 断言某元素的 color 计算值为指定 ident。
fn assert_color_ident(
    dom: &Rc<RefCell<Node>>,
    styles: &HashMap<usize, ComputedStyle>,
    id: &str,
    expected: &str,
) {
    let el = find_element_by_id(dom, id).unwrap_or_else(|| panic!("element #{id} not found"));
    let addr = Rc::as_ptr(el.inner()) as usize;
    let cs = &styles[&addr];
    // 单态化（P2-20）：关键字/解析值统一为 token 序列，直接取首个 token。
    let cvs = cs.get("color").expect("color not in style").tokens();
    match &cvs[0] {
        muskitty_css::parser::ComponentValue::PreservedToken(
            muskitty_css::tokenizer::Token::Ident(s),
        ) => assert_eq!(s, expected),
        other => panic!("expected Ident, got {:?}", other),
    }
}

#[test]
fn end_to_end_var_root_custom_property_colors_div() {
    // :root { --brand: red } div { color: var(--brand) } → div 的 color = red
    let (dom, styles) = compute_styles(
        r#"<html><body><div id="a"></div></body></html>"#,
        ":root { --brand: red; } div { color: var(--brand); }",
    );
    assert_color_ident(&dom, &styles, "a", "red");
}

#[test]
fn end_to_end_var_inherits_and_overrides() {
    // :root { --brand: red } .child { --brand: blue } .child .grand { color: var(--brand) }
    // → grand 继承 child 的 blue
    let (dom, styles) = compute_styles(
        r#"<html><body>
            <div class="child" id="c"><span class="grand" id="g"></span></div>
        </body></html>"#,
        ":root { --brand: red; } .child { --brand: blue; } .child .grand { color: var(--brand); }",
    );
    assert_color_ident(&dom, &styles, "g", "blue");
}

#[test]
fn end_to_end_var_chained_resolves() {
    // :root { --x: var(--y); --y: green } p { color: var(--x) } → green
    let (dom, styles) = compute_styles(
        r#"<html><body><p id="p"></p></body></html>"#,
        ":root { --x: var(--y); --y: green; } p { color: var(--x); }",
    );
    assert_color_ident(&dom, &styles, "p", "green");
}

#[test]
fn end_to_end_var_missing_uses_fallback_or_empty() {
    // 父级未声明 → var(--missing, orange) 命中 fallback → orange
    let (dom, styles) = compute_styles(
        r#"<html><body><p id="p"></p></body></html>"#,
        "p { color: var(--missing, orange); }",
    );
    assert_color_ident(&dom, &styles, "p", "orange");
}

#[test]
fn end_to_end_text_produces_ink_pixels() {
    // T-2 完整链路：HTML 文本 → layout 测量 → paint Text 命令 → tiny-skia
    // 光栅化。验证 (a) 生成了 Text 命令，(b) 渲染像素中有文字墨迹。
    let dom = muskitty_html5_parser::parse(r#"<p style="font-size: 24px; color: black">Hi</p>"#);
    let parsed = parse_stylesheet("p { color: black; font-size: 24px; }");
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, 200.0, 100.0).expect("layout ok");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images: no_images(),
    };
    let commands = paint(&input);
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, muskitty_renderer::RenderCommand::Text { .. })),
        "paint 应为文本节点生成 Text 命令"
    );

    let mut backend = TinySkiaBackend::new();
    let data = match backend.render(&commands, 200, 100, 1.0) {
        RenderOutput::Pixels { data, .. } => data,
        other => panic!("expected Pixels, got {:?}", other),
    };
    let ink = data
        .chunks_exact(4)
        .filter(|px| px[0] < 200 || px[1] < 200 || px[2] < 200)
        .count();
    assert!(ink > 0, "文字应产生非白（墨迹）像素");
}

#[test]
fn end_to_end_overflow_hidden_emits_clip() {
    // L-2：overflow: hidden 的元素为子内容生成 Clip/EndClip 命令。
    let dom = muskitty_html5_parser::parse(
        r#"<div style="overflow: hidden; width: 50px; height: 50px"><div style="width: 100px; height: 100px; background-color: red"></div></div>"#,
    );
    let parsed = parse_stylesheet("div { display: block; } body { margin: 0; }");
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, 200.0, 200.0).expect("layout ok");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images: no_images(),
    };
    let commands = paint(&input);
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, muskitty_renderer::RenderCommand::Clip { .. })),
        "overflow:hidden 应生成 Clip 命令"
    );
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, muskitty_renderer::RenderCommand::EndClip)),
        "overflow:hidden 应生成 EndClip 命令"
    );
}

#[test]
fn end_to_end_text_align_center() {
    // T-3：text-align 继承传递，Text 命令携带正确对齐。
    let dom =
        muskitty_html5_parser::parse(r#"<div style="text-align: center; width: 200px">Hi</div>"#);
    let parsed = parse_stylesheet("div { display: block; } body { margin: 0; }");
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, 200.0, 100.0).expect("layout ok");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images: no_images(),
    };
    let commands = paint(&input);
    let align = commands
        .iter()
        .find_map(|c| match c {
            muskitty_renderer::RenderCommand::Text { text_align, .. } => Some(*text_align),
            _ => None,
        })
        .expect("text command should exist");
    assert_eq!(
        align,
        muskitty_renderer::TextAlign::Center,
        "text-align: center 应传递为 Center"
    );
}

// —— T-3: 换行 + 字体属性端到端 ——

/// 全链路（HTML → cascade → layout → paint → render），返回绘制指令与
/// 含墨迹的扫描行数（该行内存在非白像素）。
fn render_text_case(
    html: &str,
    vw: f32,
    vh: f32,
) -> (Vec<muskitty_renderer::RenderCommand>, usize) {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet("div { display: block; } body { margin: 0; }");
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, vw, vh).expect("layout ok");
    let commands = paint(&PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images: no_images(),
    });

    let mut backend = TinySkiaBackend::new();
    let data = match backend.render(&commands, vw as u32, vh as u32, 1.0) {
        RenderOutput::Pixels { data, .. } => data,
        other => panic!("expected Pixels, got {:?}", other),
    };
    let ink_rows = (0..vh as usize)
        .filter(|&y| {
            (0..vw as usize).any(|x| {
                let i = (y * vw as usize + x) * 4;
                data[i] < 200 || data[i + 1] < 200 || data[i + 2] < 200
            })
        })
        .count();
    (commands, ink_rows)
}

#[test]
fn end_to_end_wrapped_text_renders_multiple_lines() {
    // T-3：窄容器长文本换行 —— Text 命令宽度 = 容器宽，墨迹扫描行数
    // 明显多于单行文本（多行渲染）。
    let (long_cmds, long_ink) = render_text_case(
        r#"<div style="width: 100px">The quick brown fox jumps over the lazy dog again and again</div>"#,
        100.0,
        200.0,
    );
    let (_single_cmds, single_ink) =
        render_text_case(r#"<div style="width: 100px">Hi</div>"#, 100.0, 200.0);

    let long_width = long_cmds
        .iter()
        .find_map(|c| match c {
            muskitty_renderer::RenderCommand::Text { width, .. } => Some(*width),
            _ => None,
        })
        .expect("long text command should exist");
    assert!(
        (long_width - 100.0).abs() < 1.0,
        "wrapped text command width should fill container, got {long_width}"
    );
    assert!(
        long_ink > single_ink * 2,
        "wrapped text should ink more scanlines than single line, long={long_ink} single={single_ink}"
    );
}

#[test]
fn end_to_end_font_weight_and_size_reach_text_command() {
    // T-3：font-weight: bold / font-size: 32px 经继承传入 Text 命令，
    // 且 32px 的墨迹行数多于 16px（字号影响渲染）。
    let (bold_cmds, bold_ink) = render_text_case(
        r#"<p style="font-weight: bold; font-size: 32px; color: black">Hi</p>"#,
        200.0,
        120.0,
    );
    let (_normal_cmds, normal_ink) = render_text_case(
        r#"<p style="font-weight: normal; font-size: 16px; color: black">Hi</p>"#,
        200.0,
        120.0,
    );

    let (weight, size) = bold_cmds
        .iter()
        .find_map(|c| match c {
            muskitty_renderer::RenderCommand::Text {
                font_weight,
                font_size,
                ..
            } => Some((*font_weight, *font_size)),
            _ => None,
        })
        .expect("bold text command should exist");
    assert_eq!(weight, 700, "font-weight: bold 应传递为 700");
    assert!(
        (size - 32.0).abs() < f32::EPSILON,
        "font-size: 32px 应传递为 32.0, got {size}"
    );
    assert!(
        bold_ink > normal_ink,
        "32px text should ink more scanlines than 16px, bold={bold_ink} normal={normal_ink}"
    );
}

// —— M-3 batch 2: 方向性边框 / 轮廓的全链路像素验证 ——

/// 全链路渲染并返回原始 RGBA 像素（与 [`render_to_png`] 同管线，跳过 PNG 编码）。
fn render_raw_pixels(html: &str, css: &str, vw: u32, vh: u32) -> (u32, Vec<u8>) {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet(css);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let styles = compute_styles_tree(&dom, &[sheet], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, vw as f32, vh as f32).expect("layout should succeed");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images: no_images(),
    };
    let commands = paint(&input);
    let mut backend = TinySkiaBackend::new();
    match backend.render(&commands, vw, vh, 1.0) {
        RenderOutput::Pixels { width, data, .. } => (width, data),
        other => panic!("expected Pixels, got {other:?}"),
    }
}

/// 读取像素为 `(r, g, b, a)`。
fn pixel_at(data: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
    let i = ((y * width + x) * 4) as usize;
    (data[i], data[i + 1], data[i + 2], data[i + 3])
}

#[test]
fn end_to_end_directional_border_only_that_side() {
    // `border-left` 走完 cascade 简写展开 → layout 盒模型 → paint → 像素：
    // 左边 6px 红条，其余三边与盒内保持白底
    let (width, data) = render_raw_pixels(
        r#"<div style="border-left: 6px solid red; width: 40px; height: 20px"></div>"#,
        "",
        60,
        40,
    );
    assert_eq!(
        pixel_at(&data, width, 2, 10),
        (255, 0, 0, 255),
        "left border"
    );
    assert_eq!(
        pixel_at(&data, width, 6, 10),
        (255, 255, 255, 255),
        "inside the box (right of the 6px border)"
    );
    assert_eq!(
        pixel_at(&data, width, 30, 10),
        (255, 255, 255, 255),
        "no right border"
    );
    assert_eq!(
        pixel_at(&data, width, 30, 25),
        (255, 255, 255, 255),
        "below the box"
    );
}

#[test]
fn end_to_end_border_box_grows_with_border() {
    // 盒模型：content-box 下 border 使 border box 变大 → 边框外侧属于盒子
    // （旧行为下 border 不占空间，同一像素位置不会被边框覆盖）
    let (width, data) = render_raw_pixels(
        r#"<div style="border: 5px solid red; width: 20px; height: 20px"></div>"#,
        "",
        40,
        40,
    );
    // 垂直中线 y=15 处：x ∈ [0,5) 左边框，x ∈ [5,25) 内部（无背景 → 白），
    // x ∈ [25,30) 右边框
    assert_eq!(
        pixel_at(&data, width, 2, 15),
        (255, 0, 0, 255),
        "left border"
    );
    assert_eq!(
        pixel_at(&data, width, 15, 15),
        (255, 255, 255, 255),
        "content"
    );
    assert_eq!(
        pixel_at(&data, width, 27, 15),
        (255, 0, 0, 255),
        "right border"
    );
    assert_eq!(
        pixel_at(&data, width, 32, 15),
        (255, 255, 255, 255),
        "outside the 30px border box"
    );
}

#[test]
fn end_to_end_outline_drawn_outside_box() {
    // outline 画在 border box 之外、且不影响布局（margin:10px 让轮廓可见）
    let (width, data) = render_raw_pixels(
        r#"<div style="margin: 10px; width: 40px; height: 20px; outline: 3px solid blue"></div>"#,
        "",
        80,
        60,
    );
    // border box = (10,10)-(50,30)；轮廓在其外侧 3px
    assert_eq!(
        pixel_at(&data, width, 30, 8),
        (0, 0, 255, 255),
        "outline top"
    );
    assert_eq!(
        pixel_at(&data, width, 8, 20),
        (0, 0, 255, 255),
        "outline left"
    );
    assert_eq!(
        pixel_at(&data, width, 52, 20),
        (0, 0, 255, 255),
        "outline right"
    );
    assert_eq!(
        pixel_at(&data, width, 30, 32),
        (0, 0, 255, 255),
        "outline bottom"
    );
    assert_eq!(
        pixel_at(&data, width, 30, 20),
        (255, 255, 255, 255),
        "interior"
    );
    assert_eq!(
        pixel_at(&data, width, 30, 6),
        (255, 255, 255, 255),
        "outside outline"
    );
}

// —— M-3 batch 3: line-height / text-transform 的全链路像素验证 ——

/// 全链路渲染并返回 `(width, 墨迹行号集合, 墨迹像素数)`。
///
/// 墨迹 = 任一通道 < 200 的像素（白底画布 + 黑字）。
fn render_text_ink(html: &str, vw: u32, vh: u32) -> (u32, Vec<u32>, usize) {
    let (width, data) = render_raw_pixels(html, "body { margin: 0 }", vw, vh);
    let mut rows = Vec::new();
    let mut ink = 0usize;
    for y in 0..vh {
        let mut row_has_ink = false;
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            if data[i] < 200 || data[i + 1] < 200 || data[i + 2] < 200 {
                row_has_ink = true;
                ink += 1;
            }
        }
        if row_has_ink {
            rows.push(y);
        }
    }
    (width, rows, ink)
}

#[test]
fn end_to_end_line_height_moves_second_line_down() {
    // 同一段换行文本：line-height 60px 时后续行明显下移，默认 1.2em=19.2px
    // 更紧凑。换行行数与行高无关，故最底墨迹行号的差值直接反映行高注入
    // （layout 测量与 renderer 绘制都取自同一份使用值）。
    // 画布足够高（500px）让两种情况下所有行都完整落在画布内——否则高行高
    // 的末行会被画布裁掉，墨迹像素数比较就失去意义。
    let text = "The quick brown fox jumps over the lazy dog";
    let (_, rows_default, ink_default) = render_text_ink(
        &format!(r#"<div style="width: 100px">{text}</div>"#),
        100,
        500,
    );
    let (_, rows_lh60, ink_lh60) = render_text_ink(
        &format!(r#"<div style="width: 100px; line-height: 60px">{text}</div>"#),
        100,
        500,
    );
    let last_default = *rows_default.last().expect("default text must ink");
    let last_lh60 = *rows_lh60.last().expect("line-height text must ink");
    assert!(
        rows_default.len() > 5 && rows_lh60.len() > 5,
        "both cases should wrap into several lines: default={} lh60={}",
        rows_default.len(),
        rows_lh60.len()
    );
    assert!(
        last_lh60 > last_default + 40,
        "60px line-height must push the last line well below the 19.2px default: \
         last_lh60={last_lh60} last_default={last_default}"
    );
    // 同样的文本、同样的字形量、同样多的有效行 → 墨迹像素量接近
    // （行高只挪位置，不改内容与字形）
    let ratio = ink_lh60 as f64 / ink_default as f64;
    assert!(
        (0.9..1.1).contains(&ratio),
        "line-height must move glyphs, not restyle them: ink ratio={ratio}"
    );
}

#[test]
fn end_to_end_text_transform_changes_rendered_glyphs() {
    // uppercase 改变用于排版与绘制的文本 → 同串的墨迹不同（不同字形）。
    // 不断言墨迹多少/行高方向：那取决于字体对大小写的设计（goal.md 已记
    // 「像素断言只用不等性与位置，不用字体相关的量值比较」）。
    let (_, _rows_none, ink_none) = render_text_ink(
        r#"<div style="width: 300px; font-size: 32px">hello world</div>"#,
        300,
        80,
    );
    let (_, _rows_upper, ink_upper) = render_text_ink(
        r#"<div style="width: 300px; font-size: 32px; text-transform: uppercase">hello world</div>"#,
        300,
        80,
    );
    assert!(ink_none > 0 && ink_upper > 0, "both cases must ink");
    assert_ne!(
        ink_none, ink_upper,
        "uppercase glyphs must not produce pixel-identical ink (none={ink_none}, upper={ink_upper})"
    );
}

// —— M-3 batch 3c: white-space 全链路像素验证 ——

#[test]
fn end_to_end_collapsed_source_inks_like_the_literal_line() {
    // normal 折叠是布局前语义：含源码换行缩进的文本与手写单行文本在同一
    // 容器下测出同样的排版 → 墨迹行号逐行一致（等值断言，非字体量值比较）。
    let source = "the quick   brown fox
        jumps over
        the lazy dog";
    let (_, rows_source, ink_source) = render_text_ink(
        &format!(r#"<div style="width: 80px">{source}</div>"#),
        80,
        400,
    );
    let (_, rows_literal, ink_literal) = render_text_ink(
        r#"<div style="width: 80px">the quick brown fox jumps over the lazy dog</div>"#,
        80,
        400,
    );
    assert_eq!(
        rows_source, rows_literal,
        "collapsed source text must render the exact same ink rows as the literal text"
    );
    assert_eq!(
        ink_source, ink_literal,
        "collapsed folding must not change glyph output"
    );
}

#[test]
fn end_to_end_pre_keeps_line_breaks() {
    // pre：三个源行保留为 3 行墨迹块（行块之间有空白间隔），且总墨迹范围
    // 明显高于折叠成单行的对照组。
    let source = "line one
line two
line three";
    let (_, rows_pre, _) = render_text_ink(
        &format!(r#"<div style="white-space: pre">{source}</div>"#),
        300,
        200,
    );
    let (_, rows_normal, _) = render_text_ink(&format!(r#"<div>{source}</div>"#), 300, 200);
    // normal 折叠 → 一行（墨迹行少且连续）；pre → 三行块。
    assert!(
        rows_normal.len() < 24,
        "collapsed text should be a single line block, got {} ink rows",
        rows_normal.len()
    );
    let gaps = rows_pre.windows(2).filter(|w| w[1] - w[0] > 3).count();
    assert!(
        gaps >= 2,
        "pre must keep the two forced line breaks (3 ink blocks), got {gaps} gaps: rows={rows_pre:?}"
    );
}

#[test]
fn end_to_end_nowrap_overflows_without_wrapping() {
    // nowrap：同一段文本在窄容器里，normal 折多行（最底墨迹行远低于首行），
    // nowrap 单行溢出（墨迹只出现在首行高度附近）。
    let text = "The quick brown fox jumps over the lazy dog";
    let (_, rows_normal, _) = render_text_ink(
        &format!(r#"<div style="width: 100px">{text}</div>"#),
        100,
        300,
    );
    let (_, rows_nowrap, _) = render_text_ink(
        &format!(r#"<div style="width: 100px; white-space: nowrap">{text}</div>"#),
        100,
        300,
    );
    let last_normal = *rows_normal.last().expect("normal must ink");
    let last_nowrap = *rows_nowrap.last().expect("nowrap must ink");
    assert!(last_normal > last_nowrap + 10,
        "normal wraps (last ink row {last_normal}); nowrap must stay on one line (last ink row {last_nowrap})"
    );
    // nowrap 的墨迹垂直范围不超过两倍行高（单行，1.2em ≈ 19.2px → <40px）。
    let span_nowrap = last_nowrap - rows_nowrap[0];
    assert!(
        span_nowrap < 40,
        "nowrap ink must stay within one line height, got span={span_nowrap}"
    );
}

// —— BG-1: background-image 全链路像素验证 ——

/// 1x1 纯色 PNG（测试内编码）。
fn one_pixel_png(r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(1, 1).unwrap();
    let c = tiny_skia::Color::from_rgba8(r, g, b, 255);
    let u8c = c.premultiply().to_color_u8();
    pixmap.pixels_mut()[0] =
        tiny_skia::PremultipliedColorU8::from_rgba(u8c.red(), u8c.green(), u8c.blue(), u8c.alpha())
            .unwrap();
    pixmap.encode_png().unwrap()
}

/// 全链路渲染（含图像资源表）并返回 RGBA。
fn render_with_images(
    html: &str,
    images: &HashMap<String, muskitty_renderer::ImageBits>,
    vw: u32,
    vh: u32,
) -> (u32, Vec<u8>) {
    let dom = muskitty_html5_parser::parse(html);
    let styles = compute_styles_tree(&dom, &[], &StyleTreeOptions::default());
    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, vw as f32, vh as f32).expect("layout ok");
    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
        viewport: None,
        images,
    };
    let commands = paint(&input);
    let mut backend = TinySkiaBackend::new();
    match backend.render(&commands, vw, vh, 1.0) {
        RenderOutput::Pixels { width, data, .. } => (width, data),
        other => panic!("expected Pixels, got {other:?}"),
    }
}

#[test]
fn end_to_end_background_image_repeats_across_the_box() {
    // 1x1 蓝图 + repeat（初始值）→ 整个盒被平铺成蓝色（含右下角，远超
    // 一个图像像素的范围，证明平铺而非单次贴图）。
    let img = muskitty_renderer::ImageBits::from_png(&one_pixel_png(0, 0, 255)).unwrap();
    let mut images = HashMap::new();
    images.insert("https://example.com/tile.png".to_string(), img);
    let (width, data) = render_with_images(
        r#"<div style="width: 60px; height: 40px; background-image: url('https://example.com/tile.png')"></div>"#,
        &images,
        80,
        60,
    );
    assert_eq!(pixel_at(&data, width, 2, 2), (0, 0, 255, 255), "top-left");
    assert_eq!(
        pixel_at(&data, width, 50, 30),
        (0, 0, 255, 255),
        "bottom-right must be tiled too"
    );
    // 盒外仍是白画布。
    assert_eq!(pixel_at(&data, width, 75, 50), (255, 255, 255, 255));
}

#[test]
fn end_to_end_background_image_draws_over_color_and_under_border() {
    // 绘制顺序（CSS Backgrounds L3 §2）：color 在下、image 在上、border 最上。
    // 1x1 绿图 + 红底色 + 6px 黑左边框 → 盒内非边框处为绿（图盖住红），
    // 左边框区为黑（边框盖住图）。
    let img = muskitty_renderer::ImageBits::from_png(&one_pixel_png(0, 255, 0)).unwrap();
    let mut images = HashMap::new();
    images.insert("https://example.com/g.png".to_string(), img);
    let (width, data) = render_with_images(
        r#"<div style="width: 40px; height: 20px; background-color: #ff0000; background-image: url('https://example.com/g.png'); border-left: 6px solid black"></div>"#,
        &images,
        60,
        40,
    );
    assert_eq!(
        pixel_at(&data, width, 2, 10),
        (0, 0, 0, 255),
        "border paints on top"
    );
    assert_eq!(
        pixel_at(&data, width, 20, 10),
        (0, 255, 0, 255),
        "image paints over the background color"
    );
}

#[test]
fn end_to_end_missing_background_image_keeps_page() {
    // 资源缺失（抓取失败等价）→ 该元素不画背景图，其余声明（背景色）照常。
    let (width, data) = render_with_images(
        r#"<div style="width: 40px; height: 20px; background-color: #ff0000; background-image: url('https://example.com/missing.png')"></div>"#,
        no_images(),
        60,
        40,
    );
    assert_eq!(
        pixel_at(&data, width, 20, 10),
        (255, 0, 0, 255),
        "background-color must still paint when the image cannot be resolved"
    );
}

// —— BG-1 收尾：background-repeat / position / size 全链路像素验证 ——

#[test]
fn end_to_end_background_no_repeat_paints_single_tile() {
    // background-repeat: no-repeat → 1x1 图只在起点画一块，其余盒内为白画布。
    let img = muskitty_renderer::ImageBits::from_png(&one_pixel_png(0, 0, 255)).unwrap();
    let mut images = HashMap::new();
    images.insert("https://example.com/single.png".to_string(), img);
    let (width, data) = render_with_images(
        r#"<div style="width: 60px; height: 40px; background-image: url('https://example.com/single.png'); background-repeat: no-repeat"></div>"#,
        &images,
        80,
        60,
    );
    assert_eq!(
        pixel_at(&data, width, 0, 0),
        (0, 0, 255, 255),
        "single tile top-left"
    );
    assert_eq!(
        pixel_at(&data, width, 10, 10),
        (255, 255, 255, 255),
        "not tiled in the middle"
    );
    assert_eq!(
        pixel_at(&data, width, 50, 30),
        (255, 255, 255, 255),
        "not tiled at bottom-right"
    );
}

#[test]
fn end_to_end_background_position_center_centers_single_tile() {
    // background-position: center + no-repeat → 1x1 图居中放在 (30,20)，
    // 四周是非图区域（证明偏移生效）。
    let img = muskitty_renderer::ImageBits::from_png(&one_pixel_png(0, 0, 255)).unwrap();
    let mut images = HashMap::new();
    images.insert("https://example.com/c.png".to_string(), img);
    let (width, data) = render_with_images(
        r#"<div style="width: 60px; height: 40px; background-image: url('https://example.com/c.png'); background-repeat: no-repeat; background-position: center"></div>"#,
        &images,
        80,
        60,
    );
    assert_eq!(
        pixel_at(&data, width, 30, 20),
        (0, 0, 255, 255),
        "centered tile"
    );
    assert_eq!(
        pixel_at(&data, width, 29, 20),
        (255, 255, 255, 255),
        "left blank"
    );
    assert_eq!(
        pixel_at(&data, width, 30, 19),
        (255, 255, 255, 255),
        "top blank"
    );
    assert_eq!(
        pixel_at(&data, width, 0, 0),
        (255, 255, 255, 255),
        "corner blank"
    );
}

#[test]
fn end_to_end_background_size_cover_scales_to_fill_box() {
    // background-size: cover → 1x1 图等比缩放到铺满盒（超出部分被盒裁剪），
    // 整盒均被覆盖；而 no-repeat/初始正值只画右上角单块（回归对照见
    // end_to_end_background_image_repeats_across_the_box）。
    let img = muskitty_renderer::ImageBits::from_png(&one_pixel_png(0, 0, 255)).unwrap();
    let mut images = HashMap::new();
    images.insert("https://example.com/cover.png".to_string(), img);
    let (width, data) = render_with_images(
        r#"<div style="width: 60px; height: 40px; background-image: url('https://example.com/cover.png'); background-size: cover"></div>"#,
        &images,
        80,
        60,
    );
    assert_eq!(
        pixel_at(&data, width, 30, 20),
        (0, 0, 255, 255),
        "box center covered"
    );
    assert_eq!(
        pixel_at(&data, width, 59, 39),
        (0, 0, 255, 255),
        "bottom-right covered"
    );
    assert_eq!(
        pixel_at(&data, width, 75, 50),
        (255, 255, 255, 255),
        "outside the box stays white"
    );
}

// —— IS-V: inline style 属性的全链路像素验证（§6.1 准则 3/4）——

#[test]
fn end_to_end_inline_style_beats_same_specificity_stylesheet() {
    // 内联声明胜过等特异性（乃至更高特异性）的作者规则——层叠准则 4 在
    // 整条链路上生效：cascade 收集 → 排序 → computed → paint → 像素。
    // CSS 里用 #id 提高特异性，内联仍应胜出。
    let (width, data) = render_raw_pixels(
        r#"<div id="box" style="background-color: blue; width: 40px; height: 20px"></div>"#,
        "#box { background-color: red; }",
        60,
        40,
    );
    assert_eq!(
        pixel_at(&data, width, 20, 10),
        (0, 0, 255, 255),
        "inline declaration must beat a higher-specificity author rule"
    );
}

#[test]
fn end_to_end_important_stylesheet_beats_inline_normal() {
    // §6.1 准则 1 先于准则 4：作者的 `!important` 声明胜过内联普通声明
    // （内联只在**同等重要性**下凭准则 4 胜出）。
    let (width, data) = render_raw_pixels(
        r#"<div id="box" style="background-color: blue; width: 40px; height: 20px"></div>"#,
        "div { background-color: red !important; }",
        60,
        40,
    );
    assert_eq!(
        pixel_at(&data, width, 20, 10),
        (255, 0, 0, 255),
        "author !important must beat the inline normal declaration"
    );
}

#[test]
fn end_to_end_inline_shorthand_expands_like_a_stylesheet() {
    // 内联走同一个 §5.4.5 解析入口：简写展开（这里用 border 简写）与样式表
    // 声明块行为一致 → 四边都画出来。
    let (width, data) = render_raw_pixels(
        r#"<div style="border: 4px solid red; width: 20px; height: 20px"></div>"#,
        "",
        40,
        40,
    );
    // 盒 = 内容 20 + 边框 4×2 = 28px；四条边分别在 0..4 与 24..28 区间。
    assert_eq!(
        pixel_at(&data, width, 10, 1),
        (255, 0, 0, 255),
        "top border"
    );
    assert_eq!(
        pixel_at(&data, width, 1, 10),
        (255, 0, 0, 255),
        "left border"
    );
    assert_eq!(
        pixel_at(&data, width, 10, 26),
        (255, 0, 0, 255),
        "bottom border"
    );
    assert_eq!(
        pixel_at(&data, width, 26, 10),
        (255, 0, 0, 255),
        "right border"
    );
}

#[test]
fn end_to_end_inline_important_beats_stylesheet_important_of_lower_specificity() {
    // 同等重要性下的第二种对照：内联 `!important` 对作者 `!important`
    // 仍按准则 4 胜出（此处作者规则特异性更高，仍应落败）。
    let (width, data) = render_raw_pixels(
        r#"<div id="box" style="background-color: blue !important; width: 40px; height: 20px"></div>"#,
        "#box { background-color: red !important; }",
        60,
        40,
    );
    assert_eq!(
        pixel_at(&data, width, 20, 10),
        (0, 0, 255, 255),
        "inline !important must beat author !important (criterion 4)"
    );
}
