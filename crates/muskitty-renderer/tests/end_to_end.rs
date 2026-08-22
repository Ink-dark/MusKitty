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
use muskitty_renderer::{paint, Backend, PaintInput, RenderOutput, TinySkiaBackend};
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
    };
    let commands = paint(&input);

    let mut backend = TinySkiaBackend::new();
    // P2-18：render 返回像素输出，测试消费返回值确认尺寸与数据长度。
    let output = backend.render(&commands, vw as u32, vh as u32);
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
    };
    let commands = paint(&input);
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, muskitty_renderer::RenderCommand::Text { .. })),
        "paint 应为文本节点生成 Text 命令"
    );

    let mut backend = TinySkiaBackend::new();
    let data = match backend.render(&commands, 200, 100) {
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
    });

    let mut backend = TinySkiaBackend::new();
    let data = match backend.render(&commands, vw as u32, vh as u32) {
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
