//! OM-5 序列化 roundtrip 集成测试：parse → convert → serialize → 验证结构。

use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet, ToCss};

#[test]
fn roundtrip_simple_style_rule() {
    let css = "a { color: red; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    // 序列化后应包含 selector 和 declaration
    assert!(out.contains("a"), "output: {}", out);
    assert!(out.contains("color"), "output: {}", out);
    assert!(out.contains("red"), "output: {}", out);
    assert!(out.contains("{") && out.contains("}"), "output: {}", out);
}

#[test]
fn roundtrip_multiple_declarations() {
    let css = "a { color: red; font-size: 16px; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("color: red"), "output: {}", out);
    assert!(out.contains("font-size"), "output: {}", out);
    assert!(out.contains("16px"), "output: {}", out);
}

#[test]
fn roundtrip_important() {
    let css = "a { color: red !important; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("!important"), "output: {}", out);
}

#[test]
fn roundtrip_media_rule() {
    let css = "@media print { a { color: black; } }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@media"), "output: {}", out);
    assert!(out.contains("print"), "output: {}", out);
    assert!(out.contains("color: black"), "output: {}", out);
}

#[test]
fn roundtrip_import_rule() {
    let css = "@import \"style.css\";";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@import"), "output: {}", out);
    assert!(out.contains("style.css"), "output: {}", out);
    assert!(out.ends_with(';'), "output: {}", out);
}

#[test]
fn roundtrip_import_with_media() {
    let css = "@import \"style.css\" screen;";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("screen"), "output: {}", out);
}

#[test]
fn roundtrip_namespace_rule() {
    let css = "@namespace svg \"http://www.w3.org/2000/svg\";";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@namespace"), "output: {}", out);
    assert!(out.contains("svg"), "output: {}", out);
    assert!(
        out.contains("http://www.w3.org/2000/svg"),
        "output: {}",
        out
    );
}

#[test]
fn roundtrip_supports_rule() {
    let css = "@supports (display: grid) { a { color: red; } }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@supports"), "output: {}", out);
    assert!(out.contains("display: grid"), "output: {}", out);
}

#[test]
fn roundtrip_layer_block() {
    let css = "@layer base { a { color: red; } }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@layer base"), "output: {}", out);
    assert!(out.contains("color: red"), "output: {}", out);
}

#[test]
fn roundtrip_layer_statement() {
    let css = "@layer base, theme;";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert_eq!(out, "@layer base, theme;");
}

#[test]
fn roundtrip_container_rule() {
    let css = "@container (min-width: 100px) { a { color: red; } }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@container"), "output: {}", out);
    assert!(out.contains("min-width"), "output: {}", out);
}

#[test]
fn roundtrip_multiple_rules() {
    let css = "a { color: red; } b { color: blue; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    // 两个 rule 应该用换行分隔
    assert!(out.contains("\n"), "output: {}", out);
    assert!(
        out.contains("a {") && out.contains("b {"),
        "output: {}",
        out
    );
}

#[test]
fn roundtrip_nested_rules() {
    let css = "a { color: red; &:hover { color: blue; } }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("color: red"), "output: {}", out);
    assert!(out.contains("color: blue"), "output: {}", out);
    assert!(out.contains("&:hover"), "output: {}", out);
}

#[test]
fn roundtrip_empty_stylesheet() {
    let css = "";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    assert_eq!(om.to_css_string(), "");
}

#[test]
fn roundtrip_dimension_value() {
    let css = "a { margin: 10px; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("10px"), "output: {}", out);
}

#[test]
fn roundtrip_percentage_value() {
    let css = "a { width: 50%; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("50%"), "output: {}", out);
}

#[test]
fn roundtrip_function_value() {
    let css = "a { color: rgb(255, 0, 0); }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("rgb("), "output: {}", out);
    assert!(out.contains("255"), "output: {}", out);
}

#[test]
fn roundtrip_hash_color() {
    let css = "a { color: #ff0000; }";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.contains("#ff0000"), "output: {}", out);
}

#[test]
fn roundtrip_other_at_rule_statement() {
    let css = "@charset \"UTF-8\";";
    let ss = parse_stylesheet(css);
    let om = from_stylesheet(&ss);
    let out = om.to_css_string();
    assert!(out.starts_with("@charset"), "output: {}", out);
    assert!(out.contains("UTF-8"), "output: {}", out);
    assert!(out.ends_with(';'), "output: {}", out);
}
