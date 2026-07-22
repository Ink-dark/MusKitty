//! var() 引用解析测试 — CSS Variables Level 1 §3。

#![cfg(test)]

use muskitty_css_values::var::VarReference;

#[test]
fn var_simple() {
    let v = VarReference::parse("var(--foo)").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_none());
}

#[test]
fn var_case_insensitive_function_name() {
    // var() 函数名 ASCII case-insensitive
    let v = VarReference::parse("VAR(--foo)").unwrap();
    assert_eq!(v.name, "--foo");
}

#[test]
fn var_with_whitespace_around_name() {
    let v = VarReference::parse("var( --foo )").unwrap();
    assert_eq!(v.name, "--foo");
}

#[test]
fn var_with_fallback() {
    let v = VarReference::parse("var(--foo, 10px)").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_some());
    // fallback 不为空
    assert!(!v.fallback.as_ref().unwrap().is_empty());
}

#[test]
fn var_empty_fallback_bare_comma() {
    // §3: bare comma with nothing following is valid (empty fallback)
    let v = VarReference::parse("var(--foo,)").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_some());
    assert!(v.fallback.as_ref().unwrap().is_empty());
}

#[test]
fn var_fallback_with_whitespace_only() {
    // 逗号后只有 whitespace 也算 fallback（保留 whitespace）
    let v = VarReference::parse("var(--foo, )").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_some());
}

#[test]
fn var_nested_in_fallback() {
    // fallback 内可嵌套 var()，本阶段不递归解析，只保留 component values
    let v = VarReference::parse("var(--foo, var(--bar, 10px))").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_some());
    // fallback 应包含 var(--bar, 10px) 的 component values
    assert!(!v.fallback.as_ref().unwrap().is_empty());
}

#[test]
fn var_fallback_preserves_complex_value() {
    let v = VarReference::parse("var(--foo, 10px solid red)").unwrap();
    assert_eq!(v.name, "--foo");
    let fb = v.fallback.unwrap();
    // fallback 包含 10px + solid + red（以及中间的 whitespace tokens）
    assert!(fb.len() >= 5);
}

#[test]
fn var_rejects_no_args() {
    assert!(VarReference::parse("var()").is_err());
}

#[test]
fn var_rejects_non_custom_property_name() {
    assert!(VarReference::parse("var(foo)").is_err());
    assert!(VarReference::parse("var(-foo)").is_err()); // 单 dash 不是 custom property
    assert!(VarReference::parse("var(--)").is_err()); // 只有 -- 不算
}

#[test]
fn var_rejects_non_function_input() {
    assert!(VarReference::parse("--foo").is_err());
    assert!(VarReference::parse("10px").is_err());
}

#[test]
fn var_rejects_wrong_function_name() {
    assert!(VarReference::parse("calc(--foo)").is_err());
    assert!(VarReference::parse("url(--foo)").is_err());
}
