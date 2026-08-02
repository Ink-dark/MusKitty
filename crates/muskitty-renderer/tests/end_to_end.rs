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
    apply_defaulting, cascade_for_element, cascade_winner, collect_custom_properties,
    collect_declared_values, compute_value, ComputeContext, ComputedStyle, ComputedValue,
    BUILTIN_PROPERTIES,
};
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::{build_layout_tree, compute_layout};
use muskitty_renderer::{paint, Backend, PaintInput, TinySkiaBackend};
use muskitty_selectors::matching::{DomElement, Element as _};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 递归计算每个元素的 ComputedStyle。
fn compute_styles_recursive(
    node: &Rc<RefCell<Node>>,
    sheets: &[muskitty_cssom::CssStyleSheet],
    parent_props: Option<&HashMap<String, Vec<muskitty_css::parser::ComponentValue>>>,
    parent_style: Option<&ComputedStyle>,
    styles: &mut HashMap<usize, ComputedStyle>,
) {
    let is_element = matches!(node.borrow().kind, NodeKind::Element(_));
    let addr = Rc::as_ptr(node) as usize;
    let empty_props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    let parent_props = parent_props.unwrap_or(&empty_props);
    let mut props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    if is_element {
        let element = DomElement::new(Rc::clone(node));
        props = collect_custom_properties(&element, sheets, parent_props);
        let ctx = ComputeContext::new(&props);
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
                ComputedValue::Raw(cvs) => compute_value(property, cvs, &ctx),
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
                    ComputedValue::Raw(cvs) => compute_value(prop_def.name, cvs, &ctx),
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
        compute_styles_recursive(child, sheets, Some(&props), parent_cs.as_ref(), styles);
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
    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    compute_styles_recursive(&dom, &[sheet], None, None, &mut styles);

    let mut tree = build_layout_tree(&dom, &styles);
    let layout = compute_layout(&mut tree, vw, vh).expect("layout should succeed");

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
    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    compute_styles_recursive(&dom, &[sheet], None, None, &mut styles);
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
    match cs.get("color") {
        Some(ComputedValue::Resolved(cvs)) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(
                muskitty_css::tokenizer::Token::Ident(s),
            ) => assert_eq!(s, expected),
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved color, got {:?}", other),
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
