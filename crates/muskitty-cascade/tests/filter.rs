//! CC-3 Filtering 测试：选择器匹配 → DeclaredValue 收集。

use muskitty_cascade::{
    collect_declared_values, collect_declared_values_prepared, prepare_sheets_with_context,
    MediaContext,
};
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
fn media_rule_screen_collected() {
    // P2-6：@media screen 在默认屏幕视口命中 → 收集。
    let element = make_element("div", &[]);
    let sheet = make_sheet("@media screen { div { color: black; } }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
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
fn keyframes_content_does_not_pollute_element_matching() {
    // P2-14: @keyframes 的 from/to 块不是 style rule，不得参与元素匹配。
    // 旧实现把 from 块转成 CssRule::Style，元素 <from> 会被
    // `from { opacity: 0 }` 匹配并收集到 opacity 声明（数据污染）。
    let element = make_element("from", &[]);
    let sheet = make_sheet(
        "@keyframes fade { from { opacity: 0; } to { opacity: 1; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert!(
        declared.is_empty(),
        "keyframe blocks must not produce declared values, got {:?}",
        declared
    );
}

#[test]
fn font_face_and_page_do_not_pollute_element_matching() {
    // @font-face / @page 与元素匹配无关（P2-14 类型化后跳过）。
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@font-face { font-family: X; src: url(x); } @page { margin: 1cm; } div { color: red; }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
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

#[test]
fn style_attr_collected() {
    let element = make_element("div", &[("style", "color: red")]);
    let sheets: Vec<muskitty_cssom::CssStyleSheet> = vec![];

    let declared = collect_declared_values(&element, &sheets);

    let color_decl = declared.iter().find(|d| d.property == "color");
    assert!(
        color_decl.is_some(),
        "style attr 'color: red' should be collected"
    );
    let color_decl = color_decl.unwrap();
    assert!(color_decl.from_style_attr, "from_style_attr should be true");
    assert_eq!(color_decl.origin, Origin::Author);
    assert!(!color_decl.important);
}

#[test]
fn style_attr_multiple_declarations() {
    let element = make_element("div", &[("style", "color: red; margin-top: 10px")]);
    let sheets: Vec<muskitty_cssom::CssStyleSheet> = vec![];

    let declared = collect_declared_values(&element, &sheets);
    assert_eq!(declared.len(), 2);
    assert!(declared.iter().all(|d| d.from_style_attr));
}

#[test]
fn style_attr_with_important() {
    let element = make_element("div", &[("style", "color: red !important")]);
    let sheets: Vec<muskitty_cssom::CssStyleSheet> = vec![];

    let declared = collect_declared_values(&element, &sheets);
    assert_eq!(declared.len(), 1);
    assert!(declared[0].important);
    assert!(declared[0].from_style_attr);
}

#[test]
fn style_attr_combined_with_stylesheet() {
    // div style="color: green" + CSS div { color: red; }
    // style attr 应通过 from_style_attr 标志胜出
    let element = make_element("div", &[("style", "color: green")]);
    let sheet = make_sheet("div { color: red; }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    // 应收集到 2 条声明
    assert_eq!(declared.len(), 2);
    // 验证 style attr 的 order > stylesheet 的 order（后出现）
    let style_decl = declared.iter().find(|d| d.from_style_attr).unwrap();
    let sheet_decl = declared.iter().find(|d| !d.from_style_attr).unwrap();
    assert!(
        style_decl.order > sheet_decl.order,
        "style attr order should be greater"
    );
}

// —— P2-6: @media / @supports 条件评估 ——

#[test]
fn media_print_pruned_on_screen() {
    // @media print 在默认屏幕视口不命中 → 不产生声明。
    let element = make_element("div", &[]);
    let sheet = make_sheet("@media print { div { color: red; } }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert!(
        declared.is_empty(),
        "@media print must not produce declarations on screen, got {:?}",
        declared
    );
}

#[test]
fn media_all_matches() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("@media all { div { color: red; } }", Origin::Author);

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn media_screen_min_width_matches_default_viewport() {
    // 默认视口 1920 宽 → min-width: 100px 命中。
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@media screen and (min-width: 100px) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn media_min_width_pruned_on_narrow_viewport() {
    // 视口宽 50px → min-width: 100px 不命中 → 剪枝。
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@media screen and (min-width: 100px) { div { color: red; } }",
        Origin::Author,
    );

    let prepared = prepare_sheets_with_context(
        &[sheet],
        &MediaContext {
            media_type: "screen",
            viewport_w: 50.0,
            viewport_h: 1080.0,
        },
    );
    let declared = collect_declared_values_prepared(&element, &prepared);
    assert!(
        declared.is_empty(),
        "narrow viewport must prune min-width:100px rule, got {:?}",
        declared
    );
}

#[test]
fn media_unknown_feature_fails_closed() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@media (orientation: portrait) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert!(
        declared.is_empty(),
        "unknown media feature must fail closed, got {:?}",
        declared
    );
}

#[test]
fn supports_display_flex_matches() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@supports (display: flex) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].property, "color");
}

#[test]
fn supports_custom_property_matches() {
    // (--foo: red) → custom property → 支持
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@supports (--foo: red) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
}

#[test]
fn supports_unknown_property_fails_closed() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@supports (bogus-prop: x) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert!(
        declared.is_empty(),
        "unknown property in @supports must fail closed, got {:?}",
        declared
    );
}

#[test]
fn supports_not_inverts() {
    // not (bogus-prop: x) → 未知属性不成立，not 取反成立 → 收集
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@supports not (bogus-prop: x) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
}

#[test]
fn supports_and_chain() {
    // (display: flex) and (color: red) → 两者都支持 → 收集
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@supports (display: flex) and (color: red) { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
}

#[test]
fn media_comma_list_is_or() {
    // @media print, screen → OR，任一命中即收集
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "@media print, screen { div { color: red; } }",
        Origin::Author,
    );

    let declared = collect_declared_values(&element, &[sheet]);
    assert_eq!(declared.len(), 1);
}
