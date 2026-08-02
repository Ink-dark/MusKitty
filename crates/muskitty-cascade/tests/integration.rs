//! CC-7 端到端集成测试：parse CSS → DOM → cascade → computed value。
//!
//! 验证完整数据流：
//! ```text
//! CssStyleSheet[] + DomElement
//!     → collect_declared_values (§5 Filtering)
//!     → cascade_for_element (§6.1 Cascade 排序)
//!     → cascade_winner (取首项)
//!     → apply_defaulting (§7 Defaulting)
//!     → compute_value (§4.4 Computed Value)
//! ```

use muskitty_cascade::{
    apply_defaulting, cascade_for_element, cascade_winner, collect_declared_values, compute_value,
    ComputeContext, ComputedValue,
};
use muskitty_css::parse_stylesheet;
use muskitty_css::tokenizer::Token;
use muskitty_cssom::{from_stylesheet, Origin};
use muskitty_dom::{Attribute, Node};
use muskitty_selectors::matching::DomElement;
use std::collections::HashMap;

// —— 辅助函数 ——

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

/// 完整 pipeline：对单个属性，从 DOM + CSS 计算出 computed value。
fn compute_property(
    element: &DomElement,
    sheets: &[muskitty_cssom::CssStyleSheet],
    property: &str,
    parent_computed: Option<&ComputedValue>,
    ctx: &ComputeContext,
) -> ComputedValue {
    let declared = collect_declared_values(element, sheets);
    let groups = cascade_for_element(declared);
    let group: &[muskitty_cascade::DeclaredValue] =
        groups.get(property).map(|g| g.as_slice()).unwrap_or(&[]);
    let winner = cascade_winner(group);
    let cascaded = winner.map(|w| w.value.as_slice());
    let specified = apply_defaulting(property, cascaded, parent_computed);
    match &specified {
        ComputedValue::Raw(cvs) => compute_value(property, cvs, ctx),
        _ => specified,
    }
}

fn default_ctx() -> ComputeContext<'static> {
    static EMPTY: std::sync::OnceLock<HashMap<String, Vec<muskitty_css::parser::ComponentValue>>> =
        std::sync::OnceLock::new();
    let props = EMPTY.get_or_init(HashMap::new);
    ComputeContext::new(props)
}

// —— 基础 cascade ——

#[test]
fn single_rule_single_property() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: red; }", Origin::Author);
    let ctx = default_ctx();

    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => {
            assert_eq!(cvs.len(), 1);
            match &cvs[0] {
                muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                    assert_eq!(s, "red");
                }
                other => panic!("expected Ident, got {:?}", other),
            }
        }
        other => panic!("expected Resolved, got {:?}", other),
    }
}

#[test]
fn higher_specificity_wins() {
    let element = make_element("div", &[("id", "main")]);
    let sheet = make_sheet("div { color: red; } #main { color: blue; }", Origin::Author);
    let ctx = default_ctx();

    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "blue"); // #main 的特异性更高
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

#[test]
fn important_beats_normal() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "div { color: red; } div { color: blue !important; }",
        Origin::Author,
    );
    let ctx = default_ctx();

    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "blue"); // !important 胜出
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

#[test]
fn later_declaration_wins_on_tie() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: red; } div { color: green; }", Origin::Author);
    let ctx = default_ctx();

    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "green"); // 后出现的胜出
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

// —— Defaulting ——

#[test]
fn no_declaration_uses_initial_value() {
    // div 无 color 声明 → 非根元素也无父 → 初始值 "black"
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { font-size: 16px; }", Origin::Author);
    let ctx = default_ctx();

    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Keyword(s) => assert_eq!(s, "black"),
        other => panic!("expected Keyword 'black', got {:?}", other),
    }
}

#[test]
fn inherited_property_inherits_from_parent() {
    // color 是继承属性，无声明时从父继承
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { font-size: 16px; }", Origin::Author);
    let ctx = default_ctx();
    let parent_color = ComputedValue::Keyword("red".to_string());

    let result = compute_property(&element, &[sheet], "color", Some(&parent_color), &ctx);
    match result {
        ComputedValue::Keyword(s) => assert_eq!(s, "red"),
        other => panic!("expected Keyword 'red', got {:?}", other),
    }
}

#[test]
fn initial_keyword_resets_to_initial() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: initial; }", Origin::Author);
    let ctx = default_ctx();
    let parent_color = ComputedValue::Keyword("red".to_string());

    let result = compute_property(&element, &[sheet], "color", Some(&parent_color), &ctx);
    match result {
        ComputedValue::Keyword(s) => assert_eq!(s, "black"),
        other => panic!("expected Keyword 'black', got {:?}", other),
    }
}

#[test]
fn inherit_keyword_explicitly_inherits() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: inherit; }", Origin::Author);
    let ctx = default_ctx();
    let parent_color = ComputedValue::Keyword("blue".to_string());

    let result = compute_property(&element, &[sheet], "color", Some(&parent_color), &ctx);
    match result {
        ComputedValue::Keyword(s) => assert_eq!(s, "blue"),
        other => panic!("expected Keyword 'blue', got {:?}", other),
    }
}

// —— 相对单位解析 ——

#[test]
fn em_resolves_in_full_pipeline() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { margin-top: 2em; }", Origin::Author);
    let ctx = ComputeContext {
        parent_font_size: 20.0,
        ..default_ctx()
    };

    let result = compute_property(&element, &[sheet], "margin-top", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 40.0); // 2em * 20px = 40px
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

#[test]
fn font_size_percentage_resolves_in_full_pipeline() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { font-size: 150%; }", Origin::Author);
    let ctx = ComputeContext {
        parent_font_size: 20.0,
        ..default_ctx()
    };

    let result = compute_property(&element, &[sheet], "font-size", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 30.0); // 150% * 20px = 30px
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

// —— 多 origin ——

#[test]
fn author_beats_user_agent() {
    let element = make_element("div", &[]);
    let ua_sheet = make_sheet("div { color: gray; }", Origin::UserAgent);
    let author_sheet = make_sheet("div { color: red; }", Origin::Author);
    let ctx = default_ctx();

    let result = compute_property(&element, &[ua_sheet, author_sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "red"); // Author 胜出
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

#[test]
fn important_ua_beats_important_author() {
    let element = make_element("div", &[]);
    let ua_sheet = make_sheet("div { color: gray !important; }", Origin::UserAgent);
    let author_sheet = make_sheet("div { color: red !important; }", Origin::Author);
    let ctx = default_ctx();

    let result = compute_property(&element, &[ua_sheet, author_sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "gray"); // Important UA 胜出
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

// —— 多属性 ——

#[test]
fn multiple_properties_computed() {
    let element = make_element("div", &[]);
    let sheet = make_sheet(
        "div { color: red; font-size: 16px; display: block; }",
        Origin::Author,
    );
    let ctx = default_ctx();
    let sheets = [sheet];

    // color
    let color = compute_property(&element, &sheets, "color", None, &ctx);
    match color {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "red");
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }

    // font-size
    let font_size = compute_property(&element, &sheets, "font-size", None, &ctx);
    match font_size {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 16.0);
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }

    // display
    let display = compute_property(&element, &sheets, "display", None, &ctx);
    match display {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "block");
            }
            other => panic!("expected Ident, got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

// —— var() 全链路 ——

#[test]
fn var_in_full_pipeline() {
    let element = make_element("div", &[]);
    let sheet = make_sheet("div { color: var(--main); }", Origin::Author);
    let mut props: HashMap<String, Vec<muskitty_css::parser::ComponentValue>> = HashMap::new();
    props.insert(
        "--main".to_string(),
        vec![muskitty_css::parser::ComponentValue::PreservedToken(
            Token::Ident("blue".to_string()),
        )],
    );
    let ctx = ComputeContext::new(&props);

    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Resolved(cvs) => match &cvs[0] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "blue");
            }
            other => panic!("expected Ident 'blue', got {:?}", other),
        },
        other => panic!("expected Resolved, got {:?}", other),
    }
}

// —— 非匹配选择器 → defaulting ——

#[test]
fn non_matching_selector_falls_back_to_initial() {
    let element = make_element("span", &[]);
    let sheet = make_sheet("div { color: red; }", Origin::Author);
    let ctx = default_ctx();

    // span 不匹配 div 选择器 → 无声明 → 初始值 "black"
    let result = compute_property(&element, &[sheet], "color", None, &ctx);
    match result {
        ComputedValue::Keyword(s) => assert_eq!(s, "black"),
        other => panic!("expected Keyword 'black', got {:?}", other),
    }
}
