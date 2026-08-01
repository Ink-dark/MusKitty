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

use muskitty_cascade::{
    apply_defaulting, cascade_for_element, cascade_winner, collect_declared_values, compute_value,
    ComputeContext, ComputedStyle, ComputedValue, BUILTIN_PROPERTIES,
};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{paint, Backend, PaintInput, TinySkiaBackend};
use muskitty_selectors::matching::DomElement;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 递归计算每个元素的 ComputedStyle。
fn compute_styles_recursive(
    node: &Rc<RefCell<Node>>,
    sheets: &[muskitty_cssom::CssStyleSheet],
    ctx: &ComputeContext,
    parent_style: Option<&ComputedStyle>,
    styles: &mut HashMap<usize, ComputedStyle>,
) {
    let is_element = matches!(node.borrow().kind, NodeKind::Element(_));
    let addr = Rc::as_ptr(node) as usize;
    if is_element {
        let element = DomElement::new(Rc::clone(node));
        let declared = collect_declared_values(&element, sheets);
        let groups = cascade_for_element(declared);
        let mut cs = ComputedStyle::new();
        for (property, group) in &groups {
            let winner = cascade_winner(group);
            let cascaded = winner.map(|w| w.value.as_slice());
            let specified = apply_defaulting(
                property,
                cascaded,
                parent_style.and_then(|ps| ps.get(property)),
            );
            let computed = match &specified {
                ComputedValue::Raw(cvs) => compute_value(property, cvs, ctx),
                _ => specified,
            };
            cs.set(property.clone(), computed);
        }
        for prop_def in BUILTIN_PROPERTIES.iter() {
            if !cs.properties.contains_key(prop_def.name) {
                let specified = apply_defaulting(
                    prop_def.name,
                    None,
                    parent_style.and_then(|ps| ps.get(prop_def.name)),
                );
                let computed = match &specified {
                    ComputedValue::Raw(cvs) => compute_value(prop_def.name, cvs, ctx),
                    _ => specified,
                };
                cs.set(prop_def.name.to_string(), computed);
            }
        }
        styles.insert(addr, cs);
    }

    let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
    let parent_cs = styles.get(&addr).cloned();
    for child in &children {
        compute_styles_recursive(child, sheets, ctx, parent_cs.as_ref(), styles);
    }
}

/// 运行完整 pipeline 并编码为 PNG。
fn render_to_png(html: &str, css: &str, vw: f32, vh: f32) -> Vec<u8> {
    let dom = muskitty_html5_parser::parse(html);
    let parsed = parse_stylesheet(css);
    let sheet = {
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    };
    let empty_props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    let ctx = ComputeContext::new(&empty_props);
    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    compute_styles_recursive(&dom, &[sheet], &ctx, None, &mut styles);

    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, vw, vh);

    let input = PaintInput {
        dom: &dom,
        styles: &styles,
        layout: &layout,
    };
    let commands = paint(&input);

    let mut backend = TinySkiaBackend::new();
    backend.render(&commands, vw as u32, vh as u32);
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
