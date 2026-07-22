//! Grammar trait 接入 — CSS Syntax §5.4.1。
//!
//! 通过实现 `muskitty_css_parser::Grammar`，可使用 `parse_a_grammar`
//! 入口按指定 `ValueKind` 解析类型化 CSS 值。

use crate::math::{parse_math_function_from_cvs, MathExpression};
use crate::numeric::{
    Angle, Frequency, Integer, Length, Number, ParseError as ValuesParseError, Percentage, Ratio,
    Resolution, Time,
};
use crate::textual::{CssString, CustomIdent, DashedIdent, Keyword, Url};
use crate::var::VarReference;
use muskitty_css::parser::{ComponentValue, Grammar, ParseError};

/// CSS Values grammar，用于通过 §5.4.1 `parse_a_grammar` 入口解析类型化值。
pub struct ValuesGrammar {
    pub kind: ValueKind,
}

/// 指定要解析的 CSS 值类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Length,
    Percentage,
    Number,
    Integer,
    Angle,
    Time,
    Frequency,
    Resolution,
    Ratio,
    Keyword,
    CustomIdent,
    DashedIdent,
    String,
    Url,
    /// calc()/min()/max()/clamp() 数学表达式
    Calc,
    /// var() 引用
    Var,
}

/// 解析出的类型化 CSS 值。
#[derive(Debug, Clone)]
pub enum CssValue {
    Length(Length),
    Percentage(Percentage),
    Number(Number),
    Integer(Integer),
    Angle(Angle),
    Time(Time),
    Frequency(Frequency),
    Resolution(Resolution),
    Ratio(Ratio),
    Keyword(Keyword),
    CustomIdent(CustomIdent),
    DashedIdent(DashedIdent),
    String(CssString),
    Url(Url),
    Calc(MathExpression),
    Var(VarReference),
}

impl Grammar for ValuesGrammar {
    type Output = CssValue;

    fn parse(&self, input: &[ComponentValue]) -> Result<Self::Output, ParseError> {
        let result = match self.kind {
            ValueKind::Length => Length::from_cvs(input).map(CssValue::Length),
            ValueKind::Percentage => Percentage::from_cvs(input).map(CssValue::Percentage),
            ValueKind::Number => Number::from_cvs(input).map(CssValue::Number),
            ValueKind::Integer => Integer::from_cvs(input).map(CssValue::Integer),
            ValueKind::Angle => Angle::from_cvs(input).map(CssValue::Angle),
            ValueKind::Time => Time::from_cvs(input).map(CssValue::Time),
            ValueKind::Frequency => Frequency::from_cvs(input).map(CssValue::Frequency),
            ValueKind::Resolution => Resolution::from_cvs(input).map(CssValue::Resolution),
            ValueKind::Ratio => Ratio::from_cvs(input).map(CssValue::Ratio),
            ValueKind::Keyword => Keyword::from_cvs(input).map(CssValue::Keyword),
            ValueKind::CustomIdent => CustomIdent::from_cvs(input).map(CssValue::CustomIdent),
            ValueKind::DashedIdent => DashedIdent::from_cvs(input).map(CssValue::DashedIdent),
            ValueKind::String => CssString::from_cvs(input).map(CssValue::String),
            ValueKind::Url => Url::from_cvs(input).map(CssValue::Url),
            ValueKind::Calc => parse_math_function_from_cvs(input).map(CssValue::Calc),
            ValueKind::Var => VarReference::from_cvs(input).map(CssValue::Var),
        };
        result.map_err(convert_error)
    }
}

/// 将 muskitty-css-values 的 ParseError 转为 muskitty-css-parser 的 ParseError。
fn convert_error(e: ValuesParseError) -> ParseError {
    ParseError::new(e.message)
}
