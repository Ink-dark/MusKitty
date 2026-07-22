//! 数学函数解析测试 — CSS Values Level 4 §9。

#![cfg(test)]

use muskitty_css_values::math::{parse_calc, parse_math_function, MathConstant, MathExpression};

// ── calc() 基础 ────────────────────────────────────────────────────

#[test]
fn calc_simple_length() {
    let expr = parse_calc("calc(10px)").unwrap();
    assert!(matches!(expr, MathExpression::Length(_)));
}

#[test]
fn calc_simple_percentage() {
    let expr = parse_calc("calc(50%)").unwrap();
    assert!(matches!(expr, MathExpression::Percentage(_)));
}

#[test]
fn calc_simple_number() {
    let expr = parse_calc("calc(42)").unwrap();
    assert!(matches!(expr, MathExpression::Number(_)));
}

#[test]
fn calc_sum() {
    let expr = parse_calc("calc(10px + 5px)").unwrap();
    assert!(matches!(expr, MathExpression::Sum(_, _)));
}

#[test]
fn calc_subtraction() {
    // calc(10px - 5px) → Sum(10px, Negate(5px))
    let expr = parse_calc("calc(10px - 5px)").unwrap();
    if let MathExpression::Sum(_, right) = &expr {
        assert!(matches!(right.as_ref(), MathExpression::Negate(_)));
    } else {
        panic!("expected Sum, got {:?}", expr);
    }
}

#[test]
fn calc_product() {
    let expr = parse_calc("calc(10px * 2)").unwrap();
    assert!(matches!(expr, MathExpression::Product(_, _)));
}

#[test]
fn calc_quotient() {
    let expr = parse_calc("calc(100px / 2)").unwrap();
    assert!(matches!(expr, MathExpression::Quotient(_, _)));
}

#[test]
fn calc_nested_parens() {
    let expr = parse_calc("calc((10px + 5px) * 2)").unwrap();
    // 顶层应该是 Product((10px+5px), 2)
    assert!(matches!(expr, MathExpression::Product(_, _)));
}

#[test]
fn calc_mixed_operations() {
    // calc(10px + 5px * 2) → 乘法优先于加法 → Sum(10px, Product(5px, 2))
    let expr = parse_calc("calc(10px + 5px * 2)").unwrap();
    if let MathExpression::Sum(_, right) = &expr {
        assert!(matches!(right.as_ref(), MathExpression::Product(_, _)));
    } else {
        panic!("expected Sum at top level, got {:?}", expr);
    }
}

#[test]
fn calc_left_associative_addition() {
    // calc(1px + 2px + 3px) → Sum(Sum(1px, 2px), 3px)（左结合）
    let expr = parse_calc("calc(1px + 2px + 3px)").unwrap();
    if let MathExpression::Sum(left, _) = &expr {
        assert!(matches!(left.as_ref(), MathExpression::Sum(_, _)));
    } else {
        panic!("expected left-associative Sum, got {:?}", expr);
    }
}

#[test]
fn calc_left_associative_multiplication() {
    let expr = parse_calc("calc(2 * 3 * 4)").unwrap();
    if let MathExpression::Product(left, _) = &expr {
        assert!(matches!(left.as_ref(), MathExpression::Product(_, _)));
    } else {
        panic!("expected left-associative Product, got {:?}", expr);
    }
}

// ── calc 常量 ──────────────────────────────────────────────────────

#[test]
fn calc_constant_e() {
    let expr = parse_calc("calc(e)").unwrap();
    assert!(matches!(expr, MathExpression::Constant(MathConstant::E)));
}

#[test]
fn calc_constant_pi() {
    let expr = parse_calc("calc(pi)").unwrap();
    assert!(matches!(expr, MathExpression::Constant(MathConstant::Pi)));
}

#[test]
fn calc_constant_infinity() {
    let expr = parse_calc("calc(infinity)").unwrap();
    assert!(matches!(
        expr,
        MathExpression::Constant(MathConstant::Infinity)
    ));
}

#[test]
fn calc_constant_neg_infinity() {
    let expr = parse_calc("calc(-infinity)").unwrap();
    assert!(matches!(
        expr,
        MathExpression::Constant(MathConstant::NegInfinity)
    ));
}

#[test]
fn calc_constant_nan() {
    let expr = parse_calc("calc(NaN)").unwrap();
    assert!(matches!(expr, MathExpression::Constant(MathConstant::NaN)));
}

#[test]
fn calc_constant_case_insensitive() {
    // §9.3: 这些关键字 ASCII case-insensitive
    let expr = parse_calc("calc(E)").unwrap();
    assert!(matches!(expr, MathExpression::Constant(MathConstant::E)));
    let expr = parse_calc("calc(PI)").unwrap();
    assert!(matches!(expr, MathExpression::Constant(MathConstant::Pi)));
}

#[test]
fn calc_rejects_unknown_identifier() {
    assert!(parse_calc("calc(foo)").is_err());
}

// ── calc 复杂表达式 ────────────────────────────────────────────────

#[test]
fn calc_complex_expression() {
    let expr = parse_calc("calc(100% - 20px)").unwrap();
    assert!(matches!(expr, MathExpression::Sum(_, _)));
}

#[test]
fn calc_nested_calc() {
    // calc(calc(10px + 5px))
    let expr = parse_calc("calc(calc(10px + 5px))").unwrap();
    assert!(matches!(expr, MathExpression::Sum(_, _)));
}

#[test]
fn calc_with_var() {
    // calc(var(--foo) + 10px)
    let expr = parse_calc("calc(var(--foo) + 10px)").unwrap();
    if let MathExpression::Sum(left, _) = &expr {
        assert!(matches!(left.as_ref(), MathExpression::Var(_)));
    } else {
        panic!("expected Sum with Var on left, got {:?}", expr);
    }
}

// ── calc 错误处理 ──────────────────────────────────────────────────

#[test]
fn calc_rejects_empty() {
    assert!(parse_calc("calc()").is_err());
}

#[test]
fn calc_rejects_trailing_operator() {
    assert!(parse_calc("calc(10px +)").is_err());
    assert!(parse_calc("calc(10px -)").is_err());
    assert!(parse_calc("calc(10px *)").is_err());
    assert!(parse_calc("calc(10px /)").is_err());
}

#[test]
fn calc_rejects_leading_operator() {
    // `+` 和 `-` 是二元运算符，不能作为前缀（负数由 dimension token 自带负号）
    assert!(parse_calc("calc(+ 10px)").is_err());
}

#[test]
fn calc_rejects_non_function() {
    assert!(parse_calc("10px").is_err());
}

#[test]
fn calc_rejects_non_length_dimension() {
    // CV-3 只支持 length dimension
    assert!(parse_calc("calc(45deg)").is_err());
    assert!(parse_calc("calc(2s)").is_err());
}

// ── min()/max()/clamp() ────────────────────────────────────────────

#[test]
fn min_function() {
    let expr = parse_math_function("min(10px, 20px, 5px)").unwrap();
    if let MathExpression::Min(args) = &expr {
        assert_eq!(args.len(), 3);
    } else {
        panic!("expected Min, got {:?}", expr);
    }
}

#[test]
fn max_function() {
    let expr = parse_math_function("max(10px, 20px)").unwrap();
    if let MathExpression::Max(args) = &expr {
        assert_eq!(args.len(), 2);
    } else {
        panic!("expected Max, got {:?}", expr);
    }
}

#[test]
fn clamp_three_args() {
    let expr = parse_math_function("clamp(10px, 50px, 100px)").unwrap();
    assert!(matches!(expr, MathExpression::Clamp { .. }));
}

#[test]
fn clamp_rejects_two_args() {
    assert!(parse_math_function("clamp(10px, 50px)").is_err());
}

#[test]
fn clamp_rejects_four_args() {
    assert!(parse_math_function("clamp(10px, 50px, 100px, 200px)").is_err());
}

#[test]
fn min_rejects_empty() {
    assert!(parse_math_function("min()").is_err());
}

#[test]
fn min_with_calc_inside() {
    let expr = parse_math_function("min(calc(10px + 5px), 20px)").unwrap();
    if let MathExpression::Min(args) = &expr {
        assert_eq!(args.len(), 2);
        // 第一个参数应该是 Sum（来自 calc）
        assert!(matches!(args[0], MathExpression::Sum(_, _)));
    } else {
        panic!("expected Min, got {:?}", expr);
    }
}

#[test]
fn max_with_percentages() {
    let expr = parse_math_function("max(50%, 100%)").unwrap();
    if let MathExpression::Max(args) = &expr {
        assert_eq!(args.len(), 2);
    } else {
        panic!("expected Max, got {:?}", expr);
    }
}

#[test]
fn clamp_with_calc_inside() {
    let expr = parse_math_function("clamp(10px, calc(50% - 5px), 100px)").unwrap();
    assert!(matches!(expr, MathExpression::Clamp { .. }));
}

#[test]
fn min_nested_max() {
    let expr = parse_math_function("min(max(10px, 20px), 30px)").unwrap();
    if let MathExpression::Min(args) = &expr {
        assert_eq!(args.len(), 2);
        // 第一个参数应该是嵌套的 Max
        assert!(matches!(args[0], MathExpression::Max(_)));
    } else {
        panic!("expected Min, got {:?}", expr);
    }
}
