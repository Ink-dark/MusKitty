//! 文本类型测试 — CSS Values Level 4 §3。

#![cfg(test)]

use muskitty_css_values::textual::{CssString, CustomIdent, DashedIdent, Keyword, Url};

// ── Keyword ────────────────────────────────────────────────────────

#[test]
fn parse_keyword() {
    assert_eq!(Keyword::parse("auto").unwrap().value, "auto");
    assert_eq!(Keyword::parse("block").unwrap().value, "block");
}

#[test]
fn parse_keyword_preserves_case() {
    // 关键字保留原始大小写供序列化（比较时由调用方做 case-insensitive）
    let kw = Keyword::parse("Auto").unwrap();
    assert_eq!(kw.value, "Auto");
}

#[test]
fn parse_keyword_with_whitespace() {
    assert_eq!(Keyword::parse("  auto  ").unwrap().value, "auto");
}

#[test]
fn reject_dimension_as_keyword() {
    assert!(Keyword::parse("10px").is_err());
}

// ── CustomIdent ────────────────────────────────────────────────────

#[test]
fn parse_custom_ident() {
    let id = CustomIdent::parse("my-anim").unwrap();
    assert_eq!(id.value, "my-anim");
}

#[test]
fn parse_custom_ident_with_underscore() {
    let id = CustomIdent::parse("my_var_1").unwrap();
    assert_eq!(id.value, "my_var_1");
}

#[test]
fn custom_ident_rejects_css_wide_keywords() {
    // §3.2 L676-678: CSS-wide keywords + default 不是合法 custom-ident
    assert!(CustomIdent::parse("initial").is_err());
    assert!(CustomIdent::parse("inherit").is_err());
    assert!(CustomIdent::parse("unset").is_err());
    assert!(CustomIdent::parse("default").is_err());
    assert!(CustomIdent::parse("none").is_err());
}

#[test]
fn custom_ident_rejects_case_variants_of_wide_keywords() {
    // §3.2 L682: Excluded keywords are excluded in all ASCII case permutations.
    assert!(CustomIdent::parse("INITIAL").is_err());
    assert!(CustomIdent::parse("Inherit").is_err());
    assert!(CustomIdent::parse("UNSET").is_err());
}

#[test]
fn custom_ident_case_sensitive() {
    // §3.2 L671-674: custom-ident 是大小写敏感的
    let a = CustomIdent::parse("example").unwrap();
    let b = CustomIdent::parse("EXAMPLE").unwrap();
    assert_ne!(a.value, b.value);
}

#[test]
fn reject_string_as_custom_ident() {
    assert!(CustomIdent::parse("\"hello\"").is_err());
}

// ── DashedIdent ────────────────────────────────────────────────────

#[test]
fn parse_dashed_ident() {
    let id = DashedIdent::parse("--my-var").unwrap();
    assert_eq!(id.value, "--my-var");
}

#[test]
fn parse_dashed_ident_complex() {
    let id = DashedIdent::parse("--foo-bar-baz_123").unwrap();
    assert_eq!(id.value, "--foo-bar-baz_123");
}

#[test]
fn dashed_ident_must_start_with_double_dash() {
    assert!(DashedIdent::parse("my-var").is_err());
    assert!(DashedIdent::parse("-my-var").is_err());
}

#[test]
fn dashed_ident_rejects_only_double_dash() {
    // `--` alone 不算合法 dashed-ident（长度必须 > 2）
    assert!(DashedIdent::parse("--").is_err());
}

// ── String ─────────────────────────────────────────────────────────

#[test]
fn parse_double_quoted_string() {
    assert_eq!(CssString::parse("\"hello\"").unwrap().value, "hello");
}

#[test]
fn parse_single_quoted_string() {
    assert_eq!(CssString::parse("'world'").unwrap().value, "world");
}

#[test]
fn parse_empty_string() {
    assert_eq!(CssString::parse("\"\"").unwrap().value, "");
    assert_eq!(CssString::parse("''").unwrap().value, "");
}

#[test]
fn parse_string_with_escape() {
    // \\22 是 " 的转义
    let s = CssString::parse(r#""a\"b""#).unwrap();
    assert_eq!(s.value, "a\"b");
}

#[test]
fn reject_unquoted_string() {
    assert!(CssString::parse("hello").is_err());
}

// ── Url ────────────────────────────────────────────────────────────

#[test]
fn parse_unquoted_url() {
    let url = Url::parse("url(image.png)").unwrap();
    assert_eq!(url.value, "image.png");
}

#[test]
fn parse_double_quoted_url() {
    let url = Url::parse("url(\"path/to/img.png\")").unwrap();
    assert_eq!(url.value, "path/to/img.png");
}

#[test]
fn parse_single_quoted_url() {
    let url = Url::parse("url('path/to/img.png')").unwrap();
    assert_eq!(url.value, "path/to/img.png");
}

#[test]
fn parse_url_case_insensitive_function_name() {
    // url() 函数名 ASCII case-insensitive
    let url = Url::parse("URL(image.png)").unwrap();
    assert_eq!(url.value, "image.png");
}

#[test]
fn parse_empty_url() {
    let url = Url::parse("url()").unwrap();
    assert_eq!(url.value, "");
}

#[test]
fn reject_non_url_function() {
    assert!(Url::parse("foo(image.png)").is_err());
}
