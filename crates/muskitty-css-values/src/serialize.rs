//! 序列化 — CSS Values Level 4 §8.1 + §9.7 calc-serialize。
//!
//! 提供 `ToCss` trait 将类型化值序列化回 CSS 字符串。
//! 序列化遵循 §8.1（functional notation）和 §9.7（calc-serialize）规则，
//! 但 CV-5 阶段实现的是简化版（不做 calculation tree 的 sort/simplify）。

use crate::math::{MathConstant, MathExpression};
use crate::numeric::{
    Angle, Frequency, Integer, Length, Number, Percentage, Ratio, Resolution, Time,
};
use crate::textual::{CssString, CustomIdent, DashedIdent, Keyword, Url};
use crate::var::VarReference;
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// 序列化为 CSS 字符串（specified value 序列化，§8.1）。
pub trait ToCss {
    fn to_css_string(&self) -> String;
}

// ── 数值类型 ────────────────────────────────────────────────────────

impl ToCss for Length {
    fn to_css_string(&self) -> String {
        // §8.1: number 后跟单位，无空格
        format!("{}{}", format_number(self.value), self.unit.to_str())
    }
}

impl ToCss for Percentage {
    fn to_css_string(&self) -> String {
        format!("{}%", format_number(self.value))
    }
}

impl ToCss for Number {
    fn to_css_string(&self) -> String {
        format_number(self.value)
    }
}

impl ToCss for Integer {
    fn to_css_string(&self) -> String {
        self.value.to_string()
    }
}

impl ToCss for Angle {
    fn to_css_string(&self) -> String {
        format!("{}{}", format_number(self.value), angle_unit_str(self.unit))
    }
}

impl ToCss for Time {
    fn to_css_string(&self) -> String {
        format!("{}{}", format_number(self.value), time_unit_str(self.unit))
    }
}

impl ToCss for Frequency {
    fn to_css_string(&self) -> String {
        format!(
            "{}{}",
            format_number(self.value),
            frequency_unit_str(self.unit)
        )
    }
}

impl ToCss for Resolution {
    fn to_css_string(&self) -> String {
        format!(
            "{}{}",
            format_number(self.value),
            resolution_unit_str(self.unit)
        )
    }
}

impl ToCss for Ratio {
    fn to_css_string(&self) -> String {
        // §4.7: 单数字时 height 默认 1，但序列化时如果 height==1 只输出 width
        if self.height == 1.0 {
            format_number(self.width)
        } else {
            format!(
                "{} / {}",
                format_number(self.width),
                format_number(self.height)
            )
        }
    }
}

// ── 文本类型 ────────────────────────────────────────────────────────

impl ToCss for Keyword {
    fn to_css_string(&self) -> String {
        self.value.clone()
    }
}

impl ToCss for CustomIdent {
    fn to_css_string(&self) -> String {
        self.value.clone()
    }
}

impl ToCss for DashedIdent {
    fn to_css_string(&self) -> String {
        self.value.clone()
    }
}

impl ToCss for CssString {
    fn to_css_string(&self) -> String {
        // §8.1: string 序列化时用双引号包裹，转义内部双引号和反斜杠
        let escaped = self.value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }
}

impl ToCss for Url {
    fn to_css_string(&self) -> String {
        // §8.1: url() 序列化为 url("value") 形式（quoted）
        let escaped = self.value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("url(\"{escaped}\")")
    }
}

// ── MathExpression ─────────────────────────────────────────────────

impl ToCss for MathExpression {
    fn to_css_string(&self) -> String {
        match self {
            MathExpression::Length(l) => l.to_css_string(),
            MathExpression::Percentage(p) => p.to_css_string(),
            MathExpression::Number(n) => n.to_css_string(),
            MathExpression::Constant(c) => c.to_str().to_string(),
            MathExpression::Var(v) => v.to_css_string(),
            MathExpression::Negate(e) => {
                // §9.7: Negate 序列化为 (-1 * expr)
                format!("(-1 * {})", e.to_css_string())
            }
            MathExpression::Sum(a, b) => {
                // §9.7: + 两侧有空格
                format!("{} + {}", a.to_css_string(), b.to_css_string())
            }
            MathExpression::Product(a, b) => {
                // §9.7: * 两侧无空格
                format!("{} * {}", a.to_css_string(), b.to_css_string())
            }
            MathExpression::Quotient(a, b) => {
                // §9.7: / 两侧无空格
                format!("{} / {}", a.to_css_string(), b.to_css_string())
            }
            MathExpression::Min(args) => {
                format!("min({})", serialize_args(args))
            }
            MathExpression::Max(args) => {
                format!("max({})", serialize_args(args))
            }
            MathExpression::Clamp { min, val, max } => {
                format!(
                    "clamp({}, {}, {})",
                    min.to_css_string(),
                    val.to_css_string(),
                    max.to_css_string()
                )
            }
        }
    }
}

impl ToCss for MathConstant {
    fn to_css_string(&self) -> String {
        self.to_str().to_string()
    }
}

fn serialize_args(args: &[MathExpression]) -> String {
    args.iter()
        .map(|a| a.to_css_string())
        .collect::<Vec<_>>()
        .join(", ")
}

// ── VarReference ───────────────────────────────────────────────────

impl ToCss for VarReference {
    fn to_css_string(&self) -> String {
        match &self.fallback {
            None => format!("var({})", self.name),
            Some(fallback) => {
                let s: String = fallback
                    .iter()
                    .map(cv_to_string)
                    .collect::<Vec<_>>()
                    .join("");
                format!("var({}, {})", self.name, s.trim())
            }
        }
    }
}

/// 将单个 ComponentValue 序列化为 CSS 字符串片段。
///
/// 这是一个最小化的序列化器，只覆盖 var() fallback 中常见的 token 类型。
/// 完整的 ComponentValue 序列化应在 css-parser 层实现。
fn cv_to_string(cv: &ComponentValue) -> String {
    match cv {
        ComponentValue::PreservedToken(Token::Ident(s)) => s.clone(),
        ComponentValue::PreservedToken(Token::String(s)) => format!("\"{s}\""),
        ComponentValue::PreservedToken(Token::Number(n)) => format_number(n.value),
        ComponentValue::PreservedToken(Token::Percentage(n)) => {
            format!("{}%", format_number(n.value))
        }
        ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
            format!("{}{}", format_number(n.value), u)
        }
        ComponentValue::PreservedToken(Token::Whitespace) => " ".to_string(),
        ComponentValue::PreservedToken(Token::Comma) => ",".to_string(),
        ComponentValue::PreservedToken(Token::Colon) => ":".to_string(),
        ComponentValue::PreservedToken(Token::Semicolon) => ";".to_string(),
        ComponentValue::PreservedToken(Token::Delim(c)) => c.to_string(),
        ComponentValue::PreservedToken(Token::Hash(s, _)) => format!("#{s}"),
        ComponentValue::Function(f) => {
            let args: String = f
                .value
                .iter()
                .map(cv_to_string)
                .collect::<Vec<_>>()
                .join("");
            format!("{}({})", f.name, args.trim())
        }
        _ => String::new(),
    }
}

// ── 辅助函数 ────────────────────────────────────────────────────────

/// §8.1: 数字序列化——整数无小数点，浮点数保留有效数字。
fn format_number(v: f64) -> String {
    if v == v.trunc() && v.is_finite() {
        format!("{:.0}", v)
    } else {
        format!("{}", v)
    }
}

fn angle_unit_str(u: crate::numeric::AngleUnit) -> &'static str {
    use crate::numeric::AngleUnit::*;
    match u {
        Deg => "deg",
        Grad => "grad",
        Rad => "rad",
        Turn => "turn",
    }
}

fn time_unit_str(u: crate::numeric::TimeUnit) -> &'static str {
    use crate::numeric::TimeUnit::*;
    match u {
        S => "s",
        Ms => "ms",
    }
}

fn frequency_unit_str(u: crate::numeric::FrequencyUnit) -> &'static str {
    use crate::numeric::FrequencyUnit::*;
    match u {
        Hz => "Hz",
        KHz => "kHz",
    }
}

fn resolution_unit_str(u: crate::numeric::ResolutionUnit) -> &'static str {
    use crate::numeric::ResolutionUnit::*;
    match u {
        Dpi => "dpi",
        Dpcm => "dpcm",
        Dppx => "dppx",
    }
}
