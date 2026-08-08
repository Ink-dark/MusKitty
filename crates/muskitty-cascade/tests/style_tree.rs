//! B3 集成测试：`compute_styles` 整树样式计算 + font-size 传播（P0-1）。
//!
//! 验证两步 font-size 算法：
//! 1. font-size 用父 font-size 作 em/百分比基准解析；
//! 2. 其余属性用元素自身 font-size 作 em 基准。
//!
//! rem 用根元素 font-size 作基准，自根向下传播。

use muskitty_cascade::{compute_styles, ComputedStyle, ComputedValue, StyleTreeOptions};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Node, NodeKind};
use muskitty_selectors::matching::{DomElement, Element as _};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn parse_dom(html: &str) -> Rc<RefCell<Node>> {
    muskitty_html5_parser::parse(html)
}

fn author_sheet(css: &str) -> muskitty_cssom::CssStyleSheet {
    let parsed = muskitty_css::parse_stylesheet(css);
    let mut s = from_stylesheet(&parsed);
    s.origin = Origin::Author;
    s
}

fn find_element(
    node: &Rc<RefCell<Node>>,
    predicate: &dyn Fn(&DomElement) -> bool,
) -> Option<DomElement> {
    if matches!(&node.borrow().kind, NodeKind::Element(_)) {
        let el = DomElement::new(Rc::clone(node));
        if predicate(&el) {
            return Some(el);
        }
    }
    for child in node.borrow().child_nodes() {
        if let Some(found) = find_element(child, predicate) {
            return Some(found);
        }
    }
    None
}

fn element_with_id(node: &Rc<RefCell<Node>>, id: &str) -> DomElement {
    find_element(node, &|el| el.get_attribute("id").as_deref() == Some(id))
        .unwrap_or_else(|| panic!("element #{id} not found"))
}

fn addr(el: &DomElement) -> usize {
    Rc::as_ptr(el.inner()) as usize
}

/// 从 ComputedStyle 提取某属性的第一个 px Dimension 数值。
fn style_px(cs: &ComputedStyle, prop: &str) -> f64 {
    let cv = cs
        .get(prop)
        .unwrap_or_else(|| panic!("{prop} not in style"));
    match cv {
        ComputedValue::Resolved(cvs) => {
            for v in cvs {
                if let ComponentValue::PreservedToken(Token::Dimension(n, u)) = v {
                    assert_eq!(u, "px", "expected px for {prop}");
                    return n.value;
                }
            }
            panic!("{prop} has no px dimension in {:?}", cvs);
        }
        other => panic!("expected Resolved for {prop}, got {:?}", other),
    }
}

#[test]
fn font_size_inherits_to_child() {
    // 父 div font-size:28px，子 span 未声明 → 继承 28px
    let dom = parse_dom(
        r#"<html><body><div id="a" style="font-size: 28px"><span id="b"></span></div></body></html>"#,
    );
    let styles = run_from_dom(&dom, "");
    let a = element_with_id(&dom, "a");
    let b = element_with_id(&dom, "b");
    assert_eq!(style_px(styles.get(&addr(&a)).unwrap(), "font-size"), 28.0);
    assert_eq!(style_px(styles.get(&addr(&b)).unwrap(), "font-size"), 28.0);
}

#[test]
fn font_size_percentage_of_parent() {
    // 父 font-size:32px，子 font-size:200% → 64px
    let dom = parse_dom(
        r#"<html><body><div id="a" style="font-size: 32px"><span id="b" style="font-size: 200%"></span></div></body></html>"#,
    );
    let styles = run_from_dom(&dom, "");
    let b = element_with_id(&dom, "b");
    assert_eq!(style_px(styles.get(&addr(&b)).unwrap(), "font-size"), 64.0);
}

#[test]
fn em_in_margin_uses_own_font_size() {
    // P0-1 核心回归：div 自身 font-size:32px，margin-top:2em → 64px
    // （em 语义 = 元素自身 font-size，而非父 font-size 16px）。
    let dom = parse_dom(
        r#"<html><body><div id="a" style="font-size: 32px; margin-top: 2em"></div></body></html>"#,
    );
    let styles = run_from_dom(&dom, "");
    let a = element_with_id(&dom, "a");
    assert_eq!(style_px(styles.get(&addr(&a)).unwrap(), "margin-top"), 64.0);
}

#[test]
fn em_in_child_margin_uses_inherited_font_size() {
    // 父 font-size:32px，子 span margin-left:2em → 64px
    // （span 继承 32px，em 按自身 32px 计算）。
    let dom = parse_dom(
        r#"<html><body><div id="a" style="font-size: 32px"><span id="b" style="margin-left: 2em"></span></div></body></html>"#,
    );
    let styles = run_from_dom(&dom, "");
    let b = element_with_id(&dom, "b");
    assert_eq!(
        style_px(styles.get(&addr(&b)).unwrap(), "margin-left"),
        64.0
    );
}

#[test]
fn rem_uses_root_font_size() {
    // 根元素（html）font-size:20px，后代 margin-left:2rem → 40px
    let dom = parse_dom(
        r#"<html style="font-size: 20px"><body><div id="a" style="margin-left: 2rem"></div></body></html>"#,
    );
    let styles = run_from_dom(&dom, "");
    let a = element_with_id(&dom, "a");
    assert_eq!(
        style_px(styles.get(&addr(&a)).unwrap(), "margin-left"),
        40.0
    );
}

#[test]
fn root_font_size_defaults_to_16() {
    // 未声明 font-size → 根默认 16px；子元素继承 16px
    let dom = parse_dom(r#"<html><body><div id="a" style="margin-top: 1em"></div></body></html>"#);
    let styles = run_from_dom(&dom, "");
    let a = element_with_id(&dom, "a");
    assert_eq!(style_px(styles.get(&addr(&a)).unwrap(), "font-size"), 16.0);
    // 1em = 自身 font-size 16px
    assert_eq!(style_px(styles.get(&addr(&a)).unwrap(), "margin-top"), 16.0);
}

/// 与 run 相同，但复用已解析的 DOM（避免重复解析）。
fn run_from_dom(dom: &Rc<RefCell<Node>>, css: &str) -> HashMap<usize, ComputedStyle> {
    let sheet = author_sheet(css);
    compute_styles(dom, &[sheet], &StyleTreeOptions::default())
}
