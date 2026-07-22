//! 数学表达式 AST — CSS Values Level 4 §9。
//!
//! 实现 calc()/min()/max()/clamp() 的类型化 AST 构建。
//! 本阶段仅解析不求值（求值留到 Cascade 阶段）。
//!
//! # 规范依据
//!
//! - §9.1 calc() (L2883)
//! - §9.2 min()/max()/clamp() (L3011)
//! - §9.3 Numeric Keywords (L3961) — e/pi/infinity/-infinity/NaN
//! - §9.7 Syntax (L4072) — `+`/`-` 两侧必须有 whitespace

use crate::numeric::{Length, LengthUnit, Number, ParseError, Percentage};
use crate::var::VarReference;
use muskitty_css::parser::{BlockKind, ComponentValue, Function};
use muskitty_css::tokenizer::Token;

/// CSS 数学表达式 AST (css-values-4 §9)。
///
/// 本阶段只构建 AST，不求值。
#[derive(Debug, Clone)]
pub enum MathExpression {
    /// `<length>` 字面量：`10px`、`1.5em`
    Length(Length),
    /// `<percentage>` 字面量：`50%`
    Percentage(Percentage),
    /// `<number>` 字面量：`3.14`、`42`
    Number(Number),
    /// 数学常量 (§9.3)：`e`、`pi`、`infinity`、`-infinity`、`NaN`
    Constant(MathConstant),
    /// var() 引用（可嵌套在 calc() 内）
    Var(VarReference),
    /// 一元取负：`-expr`
    Negate(Box<MathExpression>),
    /// 加法：`a + b` (左结合)
    Sum(Box<MathExpression>, Box<MathExpression>),
    /// 乘法：`a * b` (左结合)
    Product(Box<MathExpression>, Box<MathExpression>),
    /// 除法：`a / b` (b 必须是 number)
    Quotient(Box<MathExpression>, Box<MathExpression>),
    /// `min(a, b, ...)` (§9.2)
    Min(Vec<MathExpression>),
    /// `max(a, b, ...)` (§9.2)
    Max(Vec<MathExpression>),
    /// `clamp(min, val, max)` (§9.2)
    Clamp {
        min: Box<MathExpression>,
        val: Box<MathExpression>,
        max: Box<MathExpression>,
    },
}

/// 数学常量 (css-values-4 §9.3)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathConstant {
    /// e ≈ 2.71828
    E,
    /// π ≈ 3.14159
    Pi,
    /// +∞
    Infinity,
    /// −∞
    NegInfinity,
    /// NaN
    NaN,
}

impl MathConstant {
    /// 序列化为 CSS 关键字（§9.3 L4037: NaN 必须用规范大小写）。
    pub fn to_str(self) -> &'static str {
        match self {
            Self::E => "e",
            Self::Pi => "pi",
            Self::Infinity => "infinity",
            Self::NegInfinity => "-infinity",
            Self::NaN => "NaN",
        }
    }
}

// ── 顶层解析入口 ────────────────────────────────────────────────────

/// 解析 `calc(...)` 数学表达式。
///
/// 输入应为完整的 `calc(...)` 字符串。内部 calc-sum 按递归下降解析，
/// 遵循 §9.7 优先级规则：`*`/`/` 高于 `+`/`-`，左结合。
pub fn parse_calc(input: &str) -> Result<MathExpression, ParseError> {
    let cvs = muskitty_css::parse_list_of_component_values(input);
    parse_calc_from_cvs(&cvs)
}

/// 从 ComponentValue 列表解析 calc()。
pub fn parse_calc_from_cvs(cvs: &[ComponentValue]) -> Result<MathExpression, ParseError> {
    let cv = crate::numeric::single_non_ws_cv(cvs)?;
    match cv {
        ComponentValue::Function(f) if f.name.eq_ignore_ascii_case("calc") => {
            parse_math_function_body(f)
        }
        _ => Err(ParseError::new("expected calc() function")),
    }
}

/// 解析任意数学函数（calc/min/max/clamp/var）。
///
/// 入口点：从 `ComponentValue::Function` 提取函数名并分发。
pub fn parse_math_function(input: &str) -> Result<MathExpression, ParseError> {
    let cvs = muskitty_css::parse_list_of_component_values(input);
    parse_math_function_from_cvs(&cvs)
}

/// 从 ComponentValue 列表解析任意数学函数。
pub fn parse_math_function_from_cvs(cvs: &[ComponentValue]) -> Result<MathExpression, ParseError> {
    let cv = crate::numeric::single_non_ws_cv(cvs)?;
    match cv {
        ComponentValue::Function(f) => parse_math_function_body(f),
        _ => Err(ParseError::new(
            "expected a math function (calc/min/max/clamp)",
        )),
    }
}

/// 解析函数体（已知是 calc/min/max/clamp/var 之一）。
fn parse_math_function_body(f: &Function) -> Result<MathExpression, ParseError> {
    let name_lower = f.name.to_ascii_lowercase();
    match name_lower.as_str() {
        "calc" => {
            let refs: Vec<&ComponentValue> = f.value.iter().collect();
            let mut parser = CalcParser::new(&refs);
            let expr = parser.parse_calc_sum()?;
            parser.expect_eof()?;
            Ok(expr)
        }
        "min" | "max" => {
            let args = parse_comma_separated_calc_sums(&f.value)?;
            if args.is_empty() {
                return Err(ParseError::new(format!(
                    "{}() requires at least one argument",
                    f.name
                )));
            }
            if name_lower == "min" {
                Ok(MathExpression::Min(args))
            } else {
                Ok(MathExpression::Max(args))
            }
        }
        "clamp" => {
            let args = parse_comma_separated_calc_sums(&f.value)?;
            if args.len() != 3 {
                return Err(ParseError::new(format!(
                    "clamp() requires exactly 3 arguments, got {}",
                    args.len()
                )));
            }
            let mut iter = args.into_iter();
            let min = iter.next().unwrap();
            let val = iter.next().unwrap();
            let max = iter.next().unwrap();
            Ok(MathExpression::Clamp {
                min: Box::new(min),
                val: Box::new(val),
                max: Box::new(max),
            })
        }
        "var" => Ok(MathExpression::Var(VarReference::from_function(f)?)),
        _ => Err(ParseError::new(format!(
            "unsupported math function: {}()",
            f.name
        ))),
    }
}

/// 按逗号分隔 component values，每段解析为 calc-sum。
fn parse_comma_separated_calc_sums(
    cvs: &[ComponentValue],
) -> Result<Vec<MathExpression>, ParseError> {
    let mut args = Vec::new();
    let mut current: Vec<&ComponentValue> = Vec::new();
    for cv in cvs {
        if matches!(cv, ComponentValue::PreservedToken(Token::Comma)) {
            // 解析 current 段
            let mut parser = CalcParser::new(&current);
            let expr = parser.parse_calc_sum()?;
            parser.expect_eof()?;
            args.push(expr);
            current.clear();
        } else {
            current.push(cv);
        }
    }
    // 最后一段（逗号后无内容则 current 为空，min/max/clamp 不允许 trailing comma）
    if !current.is_empty() {
        let mut parser = CalcParser::new(&current);
        let expr = parser.parse_calc_sum()?;
        parser.expect_eof()?;
        args.push(expr);
    }
    Ok(args)
}

// ── 递归下降解析器 ──────────────────────────────────────────────────

/// calc() 内部的递归下降解析器。
///
/// 语法（§9.7）：
/// ```text
/// calc-sum     = calc-product ( ('+' | '-') calc-product )*
/// calc-product = calc-value ( ('*' | '/') calc-value )*
/// calc-value   = <number> | <dimension> | <percentage>
///              | <calc-constant> | '(' calc-sum ')' | <math-function>
/// ```
struct CalcParser<'a> {
    cvs: &'a [&'a ComponentValue],
    pos: usize,
}

impl<'a> CalcParser<'a> {
    fn new(cvs: &'a [&'a ComponentValue]) -> Self {
        Self { cvs, pos: 0 }
    }

    fn peek(&self) -> Option<&'a ComponentValue> {
        self.cvs.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<&'a ComponentValue> {
        let cv = self.cvs.get(self.pos).copied()?;
        self.pos += 1;
        Some(cv)
    }

    fn skip_whitespace(&mut self) {
        while let Some(cv) = self.peek() {
            if is_ws(cv) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// 检查剩余是否全是 whitespace（calc 解析完后应到此状态）。
    fn expect_eof(&self) -> Result<(), ParseError> {
        for cv in self.cvs[self.pos..].iter().copied() {
            if !is_ws(cv) {
                return Err(ParseError::new(
                    "unexpected trailing content in math expression",
                ));
            }
        }
        Ok(())
    }

    /// calc-sum = calc-product ( ('+' | '-') calc-product )*
    fn parse_calc_sum(&mut self) -> Result<MathExpression, ParseError> {
        let mut left = self.parse_calc_product()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some(ComponentValue::PreservedToken(Token::Delim('+'))) => {
                    self.advance();
                    let right = self.parse_calc_product()?;
                    left = MathExpression::Sum(Box::new(left), Box::new(right));
                }
                Some(ComponentValue::PreservedToken(Token::Delim('-'))) => {
                    self.advance();
                    let right = self.parse_calc_product()?;
                    // a - b = a + (-b)
                    left = MathExpression::Sum(
                        Box::new(left),
                        Box::new(MathExpression::Negate(Box::new(right))),
                    );
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// calc-product = calc-value ( ('*' | '/') calc-value )*
    fn parse_calc_product(&mut self) -> Result<MathExpression, ParseError> {
        let mut left = self.parse_calc_value()?;
        loop {
            // 保存位置以便回退（* 和 / 两侧 whitespace 可选）
            let saved_pos = self.pos;
            self.skip_whitespace();
            match self.peek() {
                Some(ComponentValue::PreservedToken(Token::Delim('*'))) => {
                    self.advance();
                    let right = self.parse_calc_value()?;
                    left = MathExpression::Product(Box::new(left), Box::new(right));
                }
                Some(ComponentValue::PreservedToken(Token::Delim('/'))) => {
                    self.advance();
                    let right = self.parse_calc_value()?;
                    left = MathExpression::Quotient(Box::new(left), Box::new(right));
                }
                _ => {
                    // 不是 * 或 /，回退保留原 whitespace
                    self.pos = saved_pos;
                    break;
                }
            }
        }
        Ok(left)
    }

    /// calc-value = number | dimension | percentage | constant | '(' sum ')' | function
    fn parse_calc_value(&mut self) -> Result<MathExpression, ParseError> {
        self.skip_whitespace();
        let cv = self
            .peek()
            .ok_or_else(|| ParseError::new("unexpected end of calc expression"))?;
        match cv {
            ComponentValue::PreservedToken(Token::Number(numeric)) => {
                let value = numeric.value;
                self.advance();
                Ok(MathExpression::Number(Number { value }))
            }
            ComponentValue::PreservedToken(Token::Percentage(numeric)) => {
                let value = numeric.value;
                self.advance();
                Ok(MathExpression::Percentage(Percentage { value }))
            }
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                let value = numeric.value;
                let unit = unit.clone();
                self.advance();
                // CV-3 只支持 length dimension（布局场景）
                match LengthUnit::parse_unit(&unit) {
                    Some(length_unit) => Ok(MathExpression::Length(Length {
                        value,
                        unit: length_unit,
                    })),
                    None => Err(ParseError::new(format!(
                        "calc() currently supports only length dimensions, got unit: {unit}"
                    ))),
                }
            }
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                let s_lower = s.to_ascii_lowercase();
                self.advance();
                match s_lower.as_str() {
                    "e" => Ok(MathExpression::Constant(MathConstant::E)),
                    "pi" => Ok(MathExpression::Constant(MathConstant::Pi)),
                    "infinity" => Ok(MathExpression::Constant(MathConstant::Infinity)),
                    "-infinity" => Ok(MathExpression::Constant(MathConstant::NegInfinity)),
                    "nan" => Ok(MathExpression::Constant(MathConstant::NaN)),
                    _ => Err(ParseError::new(format!(
                        "unknown calc constant or identifier: {s}"
                    ))),
                }
            }
            // 括号子表达式：( calc-sum )
            ComponentValue::SimpleBlock(block) if matches!(block.kind, BlockKind::Paren) => {
                self.advance();
                let refs: Vec<&ComponentValue> = block.value.iter().collect();
                let mut inner = CalcParser::new(&refs);
                let expr = inner.parse_calc_sum()?;
                inner.expect_eof()?;
                Ok(expr)
            }
            // 嵌套 math function：calc/min/max/clamp/var
            ComponentValue::Function(f) => {
                let f_ref = f;
                self.advance();
                parse_math_function_body(f_ref)
            }
            _ => Err(ParseError::new("unexpected token in calc expression")),
        }
    }
}

/// 判断 ComponentValue 是否是 whitespace token。
fn is_ws(cv: &ComponentValue) -> bool {
    matches!(cv, ComponentValue::PreservedToken(Token::Whitespace))
}
