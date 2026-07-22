//! 集成测试 — Grammar trait 接入 + 序列化 roundtrip。

#![cfg(test)]

use muskitty_css::parser::parse_a_grammar;
use muskitty_css_values::grammar::{CssValue, ValueKind, ValuesGrammar};
use muskitty_css_values::serialize::ToCss;

// ── Grammar 入口测试 ───────────────────────────────────────────────

#[test]
fn parse_length_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Length,
    };
    let v = parse_a_grammar("10px", &g).unwrap();
    assert!(matches!(v, CssValue::Length(_)));
}

#[test]
fn parse_percentage_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Percentage,
    };
    let v = parse_a_grammar("50%", &g).unwrap();
    assert!(matches!(v, CssValue::Percentage(_)));
}

#[test]
fn parse_number_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Number,
    };
    let v = parse_a_grammar("42", &g).unwrap();
    assert!(matches!(v, CssValue::Number(_)));
}

#[test]
fn parse_integer_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Integer,
    };
    let v = parse_a_grammar("7", &g).unwrap();
    assert!(matches!(v, CssValue::Integer(_)));
}

#[test]
fn parse_angle_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Angle,
    };
    let v = parse_a_grammar("45deg", &g).unwrap();
    assert!(matches!(v, CssValue::Angle(_)));
}

#[test]
fn parse_keyword_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Keyword,
    };
    let v = parse_a_grammar("auto", &g).unwrap();
    assert!(matches!(v, CssValue::Keyword(_)));
}

#[test]
fn parse_dashed_ident_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::DashedIdent,
    };
    let v = parse_a_grammar("--foo", &g).unwrap();
    assert!(matches!(v, CssValue::DashedIdent(_)));
}

#[test]
fn parse_string_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::String,
    };
    let v = parse_a_grammar("\"hello\"", &g).unwrap();
    assert!(matches!(v, CssValue::String(_)));
}

#[test]
fn parse_url_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Url,
    };
    let v = parse_a_grammar("url(image.png)", &g).unwrap();
    assert!(matches!(v, CssValue::Url(_)));
}

#[test]
fn parse_calc_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Calc,
    };
    let v = parse_a_grammar("calc(10px + 5px)", &g).unwrap();
    assert!(matches!(v, CssValue::Calc(_)));
}

#[test]
fn parse_min_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Calc,
    };
    let v = parse_a_grammar("min(10px, 20px)", &g).unwrap();
    assert!(matches!(v, CssValue::Calc(_)));
}

#[test]
fn parse_var_via_grammar() {
    let g = ValuesGrammar {
        kind: ValueKind::Var,
    };
    let v = parse_a_grammar("var(--foo, 10px)", &g).unwrap();
    assert!(matches!(v, CssValue::Var(_)));
}

#[test]
fn grammar_returns_error_on_mismatch() {
    let g = ValuesGrammar {
        kind: ValueKind::Length,
    };
    assert!(parse_a_grammar("auto", &g).is_err());
    assert!(parse_a_grammar("45deg", &g).is_err());
}

// ── 序列化测试 ─────────────────────────────────────────────────────

#[test]
fn serialize_length() {
    let l = muskitty_css_values::numeric::Length::parse("10px").unwrap();
    assert_eq!(l.to_css_string(), "10px");
}

#[test]
fn serialize_negative_length() {
    let l = muskitty_css_values::numeric::Length::parse("-5px").unwrap();
    assert_eq!(l.to_css_string(), "-5px");
}

#[test]
fn serialize_float_length() {
    let l = muskitty_css_values::numeric::Length::parse("1.5em").unwrap();
    assert_eq!(l.to_css_string(), "1.5em");
}

#[test]
fn serialize_percentage() {
    let p = muskitty_css_values::numeric::Percentage::parse("50%").unwrap();
    assert_eq!(p.to_css_string(), "50%");
}

#[test]
fn serialize_integer() {
    let i = muskitty_css_values::numeric::Integer::parse("42").unwrap();
    assert_eq!(i.to_css_string(), "42");
}

#[test]
fn serialize_keyword() {
    let k = muskitty_css_values::textual::Keyword::parse("auto").unwrap();
    assert_eq!(k.to_css_string(), "auto");
}

#[test]
fn serialize_string() {
    let s = muskitty_css_values::textual::CssString::parse("\"hello\"").unwrap();
    assert_eq!(s.to_css_string(), "\"hello\"");
}

#[test]
fn serialize_string_with_escape() {
    let s = muskitty_css_values::textual::CssString::parse(r#""a\"b""#).unwrap();
    assert_eq!(s.to_css_string(), "\"a\\\"b\"");
}

#[test]
fn serialize_url() {
    let u = muskitty_css_values::textual::Url::parse("url(image.png)").unwrap();
    assert_eq!(u.to_css_string(), "url(\"image.png\")");
}

#[test]
fn serialize_calc_sum() {
    let expr = muskitty_css_values::math::parse_calc("calc(10px + 5px)").unwrap();
    assert_eq!(expr.to_css_string(), "10px + 5px");
}

#[test]
fn serialize_calc_product() {
    let expr = muskitty_css_values::math::parse_calc("calc(10px * 2)").unwrap();
    assert_eq!(expr.to_css_string(), "10px * 2");
}

#[test]
fn serialize_calc_quotient() {
    let expr = muskitty_css_values::math::parse_calc("calc(100px / 2)").unwrap();
    assert_eq!(expr.to_css_string(), "100px / 2");
}

#[test]
fn serialize_calc_subtraction() {
    // calc(10px - 5px) → Sum(10px, Negate(5px)) → "10px + (-1 * 5px)"
    let expr = muskitty_css_values::math::parse_calc("calc(10px - 5px)").unwrap();
    assert_eq!(expr.to_css_string(), "10px + (-1 * 5px)");
}

#[test]
fn serialize_min() {
    let expr = muskitty_css_values::math::parse_math_function("min(10px, 20px)").unwrap();
    assert_eq!(expr.to_css_string(), "min(10px, 20px)");
}

#[test]
fn serialize_clamp() {
    let expr = muskitty_css_values::math::parse_math_function("clamp(10px, 50px, 100px)").unwrap();
    assert_eq!(expr.to_css_string(), "clamp(10px, 50px, 100px)");
}

#[test]
fn serialize_var_no_fallback() {
    let v = muskitty_css_values::var::VarReference::parse("var(--foo)").unwrap();
    assert_eq!(v.to_css_string(), "var(--foo)");
}

#[test]
fn serialize_var_with_fallback() {
    let v = muskitty_css_values::var::VarReference::parse("var(--foo, 10px)").unwrap();
    assert_eq!(v.to_css_string(), "var(--foo, 10px)");
}

#[test]
fn serialize_var_with_complex_fallback() {
    let v = muskitty_css_values::var::VarReference::parse("var(--foo, 10px solid red)").unwrap();
    assert_eq!(v.to_css_string(), "var(--foo, 10px solid red)");
}

#[test]
fn serialize_ratio_single() {
    let r = muskitty_css_values::numeric::Ratio::parse("16").unwrap();
    assert_eq!(r.to_css_string(), "16");
}

#[test]
fn serialize_ratio_two_numbers() {
    let r = muskitty_css_values::numeric::Ratio::parse("16 / 9").unwrap();
    assert_eq!(r.to_css_string(), "16 / 9");
}
