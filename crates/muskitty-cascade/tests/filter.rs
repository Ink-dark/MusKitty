//! CC-3 Filtering 测试：选择器匹配 → DeclaredValue 收集。

use muskitty_cascade::collect_declared_values;
use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Attribute, Node};
use muskitty_selectors::matching::DomElement;

fn make_element(tag: &str, attrs: &[(&str, &str)]) -> DomElement {
    let doc = Node::new_document();
    let attrs: Vec<Attribute> = attrs.iter().map(|(k, v)| Attribute::new(k, v)).collect();
    let node = Node::new_element_html(tag, attrs, &doc);
    DomElement::new(node)
}

fn make_sheet(css: &str, origin: Origin) -> muskitty_cssom::CssStyleSheet {
    let parsed = parse_stylesheet(css);
    let mut sheet = from_stylesheet(&parsed);
    sheet.origin = origin;
    sheet
}

#[test]
fn simple_type_selector_match() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: red; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
    assert_eq!(declared[0].origin, Origin::Author);
    assert!(!declared[0].important);
    assert!(!declared[0].from_style_attr);
}

#[test]
fn class_selector_match() {
    let element = make_element("div", &[("class", "foo")]);
    let sheet = make_sheet(".foo { color: blue; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn id_selector_match() {
    let element = make_element("div", &[("id", "bar")]);
    let sheet = make_sheet("#bar { color: green; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
}

#[test]
fn non_matching_selector() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("span { color: red; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert!(declared.is_empty());
}

#[test]
fn multiple_declarations_in_one_rule() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: red; font-size: 16px; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 2);
    assert_eq!(declared[0].property, "color");
    assert_eq!(declared[1].property, "font-size");
}

#[test]
fn multiple_rules_matching_same_element() {
    let element = make_element("div", &[("class", "foo")]);
    let sheet = make_sheet("div { color: red; } .foo { color: blue; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 2);
    // order 应递增
    assert!(declared[0].order < declared[1].order);
}

#[test]
fn important_flag_collected() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: red !important; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert!(declared[0].important);
}

#[test]
fn media_rule_collected() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("@media print { div { color: black; } }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    // 简化：无条件收集
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn nested_rules_collected() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "div { color: red; &:hover { color: blue; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    // div 匹配 → color: red
    // &:hover 不匹配（没有 :hover 状态）→ 不收集
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn specificity_recorded() {
    let element = make_element("div", &[("id", "bar"), ("class", "foo")]);
    let sheet = make_sheet(
        "#bar { color: red; } .foo { color: blue; } div { color: green; }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 3);

    // #bar 的 specificity 应该最高 (1,0,0)
    let id_decl = declared.iter().find(|d| d.specificity.a == 1).unwrap();
    assert_eq!(id_decl.property, "color");

    // .foo 的 specificity (0,1,0)
    let class_decl = declared
        .iter()
        .find(|d| d.specificity.b == 1 && d.specificity.a == 0)
        .unwrap();

    // div 的 specificity (0,0,1)
    let type_decl = declared
        .iter()
        .find(|d| d.specificity.c == 1 && d.specificity.b == 0)
        .unwrap();

    // 验证顺序
    assert!(id_decl.order < class_decl.order);
    assert!(class_decl.order < type_decl.order);
}

#[test]
fn origin_recorded() {
    let element = make_element("div", &[]);
    let ua_sheet = make_sheet("div { color: black; }", Origin::UserAgent);
    let author_sheet = make_sheet("div { color: red; }", Origin::Author);

    let declared = collect_declared_values(&element, &[ua_sheet, author_sheet]);
    assert_eq!(declared.len(), 2);

    let ua_decl = declared
        .iter()
        .find(|d| d.origin == Origin::UserAgent)
        .unwrap();
    let author_decl = declared
        .iter()
        .find(|d| d.origin == Origin::Author)
        .unwrap();

    assert!(ua_decl.order < author_decl.order);
}

#[test]
fn layer_block_rules_collected() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("@layer base { div { color: red; } }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
}

#[test]
fn import_and_namespace_skipped() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@import \"style.css\"; @namespace svg \"http://www.w3.org/2000/svg\"; div { color: red; }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn multiple_stylesheets() {
    let element = make_element("div", &[]);
    let sheet1 = make_sheet("div { color: red; }", Origin::Author);
    let sheet2 = make_sheet("div { color: blue; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet1, sheet2]);
    assert_eq!(declared.len(), 2);
    // sheet1 的 order 应小于 sheet2
    assert!(declared[0].order < declared[1].order);
}
