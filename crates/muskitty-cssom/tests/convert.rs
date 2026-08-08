//! OM-4 端到端转换测试：parse CSS → convert → verify CssStyleSheet 结构。

use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, from_stylesheet_with_origin, CssRule, Origin};

#[test]
fn empty_stylesheet() {
    let ss = parse_stylesheet("");
    let om = from_stylesheet(&ss);
    assert!(om.is_empty());
    assert!(om.css_rules.is_empty());
}

#[test]
fn from_stylesheet_with_origin_sets_origin() {
    // P2-15: 显式指定 origin（from_stylesheet 默认 Author）
    let ss = parse_stylesheet("a { color: red; }");
    let om = from_stylesheet_with_origin(&ss, Origin::UserAgent);
    assert_eq!(om.origin, Origin::UserAgent);
    assert_eq!(om.len(), 1);
    // from_stylesheet 保持 Author 兼容
    let author = from_stylesheet(&ss);
    assert_eq!(author.origin, Origin::Author);
}

#[test]
fn custom_property_original_text_preserved() {
    // P2-16: custom property 的 original_text 透传（var() 源文本用），
    // 普通属性不设。
    let ss = parse_stylesheet(":root { --x: red; color: blue; }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Style(r) => {
            let custom = r.style.get_property("--x").expect("--x decl");
            assert!(
                custom.original_text.is_some(),
                "custom property keeps original_text"
            );
            let normal = r.style.get_property("color").expect("color decl");
            assert!(
                normal.original_text.is_none(),
                "regular property has no original_text"
            );
        }
        other => panic!("expected Style, got {:?}", other),
    }
}

#[test]
fn single_style_rule() {
    let ss = parse_stylesheet("a { color: red; }");
    let om = from_stylesheet(&ss);
    assert_eq!(om.len(), 1);
    match &om.css_rules[0] {
        CssRule::Style(r) => {
            assert!(!r.selectors.is_empty());
            assert_eq!(r.style.len(), 1);
            assert_eq!(r.style.get_property("color").unwrap().name, "color");
            assert!(r.css_rules.is_empty());
        }
        other => panic!("expected Style, got {:?}", other),
    }
}

#[test]
fn style_rule_with_important() {
    let ss = parse_stylesheet("a { color: red !important; }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Style(r) => {
            let decl = r.style.get_property("color").unwrap();
            assert!(decl.important);
        }
        _ => panic!(),
    }
}

#[test]
fn multiple_declarations() {
    let ss = parse_stylesheet("a { color: red; font-size: 16px; }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Style(r) => {
            assert_eq!(r.style.len(), 2);
            assert!(r.style.get_property("color").is_some());
            assert!(r.style.get_property("font-size").is_some());
        }
        _ => panic!(),
    }
}

#[test]
fn media_rule() {
    let ss = parse_stylesheet("@media print { a { color: black; } }");
    let om = from_stylesheet(&ss);
    assert_eq!(om.len(), 1);
    match &om.css_rules[0] {
        CssRule::Media(r) => {
            assert!(!r.condition.is_empty());
            assert_eq!(r.css_rules.len(), 1);
            match &r.css_rules[0] {
                CssRule::Style(inner) => {
                    assert!(inner.style.get_property("color").is_some());
                }
                _ => panic!(),
            }
        }
        other => panic!("expected Media, got {:?}", other),
    }
}

#[test]
fn at_rule_name_case_insensitive() {
    // P1-6: at-rule 名大小写不敏感（CSS Syntax §6.3.4）
    let ss = parse_stylesheet("@MEDIA print { a { color: black; } }");
    let om = from_stylesheet(&ss);
    assert_eq!(om.len(), 1);
    assert!(
        matches!(&om.css_rules[0], CssRule::Media(_)),
        "expected Media, got {:?}",
        om.css_rules[0]
    );
}

#[test]
fn import_rule_with_string() {
    let ss = parse_stylesheet("@import \"style.css\";");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Import(r) => {
            assert_eq!(r.href, "style.css");
            assert!(r.media.is_empty());
        }
        _ => panic!(),
    }
}

#[test]
fn import_rule_with_url_function() {
    let ss = parse_stylesheet("@import url(style.css);");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Import(r) => assert_eq!(r.href, "style.css"),
        _ => panic!(),
    }
}

#[test]
fn import_rule_with_quoted_url_function() {
    // P1-4: `url("...")`（带引号）在 CSS Syntax §4.3.8 下是 Function
    // 形态而非 Url token，需从 Function 里提取 href。
    let ss = parse_stylesheet("@import url(\"style.css\");");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Import(r) => assert_eq!(r.href, "style.css"),
        _ => panic!(),
    }
}

#[test]
fn import_rule_with_media() {
    let ss = parse_stylesheet("@import \"style.css\" screen and (min-width: 100px);");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Import(r) => {
            assert_eq!(r.href, "style.css");
            assert!(!r.media.is_empty());
        }
        _ => panic!(),
    }
}

#[test]
fn namespace_rule_default() {
    let ss = parse_stylesheet("@namespace \"http://www.w3.org/2000/svg\";");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Namespace(r) => {
            assert_eq!(r.namespace_uri, "http://www.w3.org/2000/svg");
            assert!(r.prefix.is_none());
        }
        _ => panic!(),
    }
}

#[test]
fn namespace_rule_with_prefix() {
    let ss = parse_stylesheet("@namespace svg \"http://www.w3.org/2000/svg\";");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Namespace(r) => {
            assert_eq!(r.namespace_uri, "http://www.w3.org/2000/svg");
            assert_eq!(r.prefix.as_deref(), Some("svg"));
        }
        _ => panic!(),
    }
}

#[test]
fn supports_rule() {
    let ss = parse_stylesheet("@supports (display: grid) { a { color: red; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Supports(r) => {
            assert!(!r.condition.is_empty());
            assert_eq!(r.css_rules.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn layer_block_named() {
    let ss = parse_stylesheet("@layer base { a { color: red; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::LayerBlock(r) => {
            assert_eq!(r.name.as_deref(), Some("base"));
            assert_eq!(r.css_rules.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn layer_block_anonymous() {
    let ss = parse_stylesheet("@layer { a { color: red; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::LayerBlock(r) => {
            assert!(r.name.is_none());
            assert_eq!(r.css_rules.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn layer_block_dotted_name() {
    // P1-7: @layer 名可为点分隔层级（a.b.c），block 形式取首层名
    let ss = parse_stylesheet("@layer a.b.c { a { color: red; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::LayerBlock(r) => {
            assert_eq!(r.name.as_deref(), Some("a.b.c"));
            assert_eq!(r.css_rules.len(), 1);
        }
        other => panic!("expected LayerBlock, got {:?}", other),
    }
}

#[test]
fn layer_statement_dotted_names() {
    // P1-7: statement 形式取全部点分隔层名
    let ss = parse_stylesheet("@layer a.b, c.d;");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::LayerStatement(r) => {
            assert_eq!(r.names, vec!["a.b", "c.d"]);
        }
        other => panic!("expected LayerStatement, got {:?}", other),
    }
}

#[test]
fn layer_statement() {
    let ss = parse_stylesheet("@layer base, theme, utilities;");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::LayerStatement(r) => {
            assert_eq!(r.names, vec!["base", "theme", "utilities"]);
        }
        _ => panic!(),
    }
}

#[test]
fn container_rule() {
    let ss = parse_stylesheet("@container (min-width: 100px) { a { color: red; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Container(r) => {
            assert!(!r.condition.is_empty());
            assert_eq!(r.css_rules.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn other_at_rule_statement() {
    let ss = parse_stylesheet("@charset \"UTF-8\";");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Other(r) => {
            assert_eq!(r.name, "charset");
            assert!(r.declarations.is_none());
            assert!(r.child_rules.is_empty());
        }
        _ => panic!(),
    }
}

#[test]
fn font_face_rule_typed() {
    // P2-14: @font-face 类型化为 CssRule::FontFace（描述符进 declarations）。
    // P1-5: 声明不再丢失。
    let ss = parse_stylesheet("@font-face { font-family: X; src: url(x); }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::FontFace(r) => {
            assert_eq!(r.style.len(), 2);
            let names: Vec<&str> = r
                .style
                .declarations
                .iter()
                .map(|d| d.name.as_str())
                .collect();
            assert_eq!(names, vec!["font-family", "src"]);
        }
        other => panic!("expected FontFace, got {:?}", other),
    }
}

#[test]
fn keyframes_rule_typed() {
    // P2-14: @keyframes 类型化为 CssRule::Keyframes，from/to 块转
    // CssKeyframeRule（不再是 CssRule::Style，避免 cascade 污染）。
    let ss = parse_stylesheet("@keyframes fade { from { opacity: 0; } to { opacity: 1; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Keyframes(r) => {
            assert_eq!(r.name.as_deref(), Some("fade"));
            assert_eq!(r.keyframes.len(), 2);
            let key_texts: Vec<String> = r
                .keyframes
                .iter()
                .map(|kf| {
                    muskitty_cssom::serialize_component_values(&kf.key_text)
                        .trim()
                        .to_string()
                })
                .collect();
            assert_eq!(key_texts, vec!["from", "to"]);
            assert_eq!(r.keyframes[0].style.len(), 1);
            assert_eq!(r.keyframes[0].style.declarations[0].name, "opacity");
        }
        other => panic!("expected Keyframes, got {:?}", other),
    }
}

#[test]
fn page_and_property_typed() {
    let ss = parse_stylesheet("@page { margin: 1cm; } @property --foo { syntax: \"<length>\"; }");
    let om = from_stylesheet(&ss);
    assert!(matches!(om.css_rules[0], CssRule::Page(_)));
    match &om.css_rules[1] {
        CssRule::Property(r) => {
            assert_eq!(r.name, "--foo");
            assert_eq!(r.style.len(), 1);
            assert_eq!(r.style.declarations[0].name, "syntax");
        }
        other => panic!("expected Property, got {:?}", other),
    }
}

#[test]
fn nested_rules() {
    let ss = parse_stylesheet("a { color: red; &:hover { color: blue; } }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Style(r) => {
            assert_eq!(r.style.len(), 1);
            assert_eq!(r.css_rules.len(), 1);
            match &r.css_rules[0] {
                CssRule::Style(nested) => {
                    assert!(nested.style.get_property("color").is_some());
                }
                _ => panic!(),
            }
        }
        _ => panic!(),
    }
}

#[test]
fn nested_declarations_merged_into_parent() {
    // 嵌套裸声明应合并到父 style 块
    let ss = parse_stylesheet("a { color: red; &:hover { color: blue; } font-size: 16px; }");
    let om = from_stylesheet(&ss);
    match &om.css_rules[0] {
        CssRule::Style(r) => {
            // color + font-size 都应在父 style 中
            assert!(r.style.get_property("color").is_some());
            assert!(r.style.get_property("font-size").is_some());
            // 一个嵌套 style rule
            assert_eq!(r.css_rules.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn top_level_declarations_skipped() {
    // 顶层裸声明被跳过（CSS Syntax §5.5.1 视为 parse error）
    let ss = parse_stylesheet("color: red; a { color: blue; }");
    let om = from_stylesheet(&ss);
    // 只应有 1 个 rule（a { ... }），顶层的 color: red 被跳过
    assert_eq!(om.len(), 1);
    match &om.css_rules[0] {
        CssRule::Style(_) => {}
        _ => panic!(),
    }
}

#[test]
fn multiple_rules_at_top_level() {
    let ss = parse_stylesheet(
        "a { color: red; } b { color: blue; } @media print { c { color: green; } }",
    );
    let om = from_stylesheet(&ss);
    assert_eq!(om.len(), 3);
    assert!(matches!(om.css_rules[0], CssRule::Style(_)));
    assert!(matches!(om.css_rules[1], CssRule::Style(_)));
    assert!(matches!(om.css_rules[2], CssRule::Media(_)));
}
