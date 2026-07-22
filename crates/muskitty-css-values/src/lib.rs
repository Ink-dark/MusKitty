//! MusKitty CSS Values — CSS Values Level 4 typed value parsing.
//!
//! 实现 CSS Values Level 4 的类型化值解析：数值（length/angle/time/
//! frequency/resolution/ratio/number/integer）、文本类型（keyword/ident/
//! string/url）、数学函数 AST（calc/min/max/clamp）、var() 语法解析。
//!
//! # 设计原则
//!
//! **解析与求值分离**：本 crate 只构建类型化 AST，不做数值计算和
//! var() 替换求值（留到 Cascade 阶段）。
//!
//! # 规范依据
//!
//! - CSS Values Level 4: `d:\csswg\css-values-4\Overview.md`
//! - CSS Variables Level 1: `d:\csswg\css-variables-1\Overview.md`
//!
//! # 快速上手
//!
//! ```
//! use muskitty_css_values::parse_length;
//!
//! let len = parse_length("10px").unwrap();
//! assert_eq!(len.value, 10.0);
//! ```
//!
//! 通过 grammar hook 解析任意类型化值：
//!
//! ```
//! use muskitty_css_values::{parse_value, grammar::ValueKind};
//!
//! let v = parse_value("45deg", ValueKind::Angle).unwrap();
//! println!("{:?}", v);
//! ```

pub mod grammar;
pub mod math;
pub mod numeric;
pub mod serialize;
pub mod textual;
pub mod var;

// ── 顶层便捷函数 ────────────────────────────────────────────────────

/// 解析一个 `<length>` 值。
///
/// ```
/// use muskitty_css_values::parse_length;
/// let len = parse_length("10px").unwrap();
/// assert_eq!(len.value, 10.0);
/// ```
pub fn parse_length(input: &str) -> Result<numeric::Length, numeric::ParseError> {
    numeric::Length::parse(input)
}

/// 解析一个 `<percentage>` 值。
///
/// ```
/// use muskitty_css_values::parse_percentage;
/// let p = parse_percentage("50%").unwrap();
/// assert_eq!(p.value, 50.0);
/// ```
pub fn parse_percentage(input: &str) -> Result<numeric::Percentage, numeric::ParseError> {
    numeric::Percentage::parse(input)
}

/// 解析一个 `<number>` 值。
///
/// ```
/// use muskitty_css_values::parse_number;
/// let n = parse_number("3.14").unwrap();
/// assert!((n.value - 3.14).abs() < f64::EPSILON);
/// ```
pub fn parse_number(input: &str) -> Result<numeric::Number, numeric::ParseError> {
    numeric::Number::parse(input)
}

/// 解析一个 `<integer>` 值。
///
/// ```
/// use muskitty_css_values::parse_integer;
/// let i = parse_integer("42").unwrap();
/// assert_eq!(i.value, 42);
/// ```
pub fn parse_integer(input: &str) -> Result<numeric::Integer, numeric::ParseError> {
    numeric::Integer::parse(input)
}

/// 解析一个 calc() 数学表达式（也接受 min/max/clamp）。
///
/// ```
/// use muskitty_css_values::parse_calc;
/// let expr = parse_calc("calc(10px + 5px)").unwrap();
/// ```
pub fn parse_calc(input: &str) -> Result<math::MathExpression, numeric::ParseError> {
    math::parse_calc(input)
}

/// 解析一个 var() 引用（含 fallback）。
///
/// ```
/// use muskitty_css_values::parse_var;
/// let v = parse_var("var(--foo, 10px)").unwrap();
/// assert_eq!(v.name, "--foo");
/// ```
pub fn parse_var(input: &str) -> Result<var::VarReference, numeric::ParseError> {
    var::VarReference::parse(input)
}

/// 通过 grammar hook（§5.4.1 `parse_a_grammar`）解析任意类型化值。
///
/// ```
/// use muskitty_css_values::{parse_value, grammar::ValueKind};
/// let v = parse_value("auto", ValueKind::Keyword).unwrap();
/// ```
pub fn parse_value(
    input: &str,
    kind: grammar::ValueKind,
) -> Result<grammar::CssValue, muskitty_css::parser::ParseError> {
    muskitty_css::parser::parse_a_grammar(input, &grammar::ValuesGrammar { kind })
}
