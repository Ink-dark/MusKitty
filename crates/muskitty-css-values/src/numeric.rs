//! 数值类型 — CSS Values Level 4 §4-§6。

use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// 值解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS value parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// 从 ComponentValue 列表中提取唯一的非 whitespace token。
pub fn single_non_ws_cv(cvs: &[ComponentValue]) -> Result<&ComponentValue, ParseError> {
    let filtered: Vec<&ComponentValue> = cvs
        .iter()
        .filter(|cv| !matches!(cv, ComponentValue::PreservedToken(Token::Whitespace)))
        .collect();
    if filtered.len() != 1 {
        return Err(ParseError::new(format!(
            "expected exactly one value, got {}",
            filtered.len()
        )));
    }
    Ok(filtered[0])
}

// ── §5 Length ────────────────────────────────────────────────────────

/// CSS `<length>` 值 (css-values-4 §5)。
///
/// 一个数值 + 长度单位。本阶段不计算绝对长度（如 em→px），
/// 保留原始值和单位，求值留到 Cascade 阶段。
#[derive(Debug, Clone, PartialEq)]
pub struct Length {
    pub value: f64,
    pub unit: LengthUnit,
}

/// 长度单位 (css-values-4 §5.1 相对、§5.2 绝对)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    // 相对 (§5.1)
    Em,
    Rem,
    Ex,
    Ch,
    Vw,
    Vh,
    Vmin,
    Vmax,
    // 绝对 (§5.2)
    Px,
    Cm,
    Mm,
    In,
    Pt,
    Pc,
    Q,
}

impl LengthUnit {
    /// 从字符串解析长度单位（ASCII case-insensitive）。
    pub fn parse_unit(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "em" => Some(Self::Em),
            "rem" => Some(Self::Rem),
            "ex" => Some(Self::Ex),
            "ch" => Some(Self::Ch),
            "vw" => Some(Self::Vw),
            "vh" => Some(Self::Vh),
            "vmin" => Some(Self::Vmin),
            "vmax" => Some(Self::Vmax),
            "px" => Some(Self::Px),
            "cm" => Some(Self::Cm),
            "mm" => Some(Self::Mm),
            "in" => Some(Self::In),
            "pt" => Some(Self::Pt),
            "pc" => Some(Self::Pc),
            "q" => Some(Self::Q),
            _ => None,
        }
    }

    /// 序列化为 CSS 单位字符串。
    pub fn to_str(self) -> &'static str {
        match self {
            Self::Em => "em",
            Self::Rem => "rem",
            Self::Ex => "ex",
            Self::Ch => "ch",
            Self::Vw => "vw",
            Self::Vh => "vh",
            Self::Vmin => "vmin",
            Self::Vmax => "vmax",
            Self::Px => "px",
            Self::Cm => "cm",
            Self::Mm => "mm",
            Self::In => "in",
            Self::Pt => "pt",
            Self::Pc => "pc",
            Self::Q => "Q",
        }
    }
}

impl Length {
    /// 从 CSS 字符串解析一个 `<length>` 值。
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    /// 从 ComponentValue 列表解析。
    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                let unit = LengthUnit::parse_unit(unit)
                    .ok_or_else(|| ParseError::new(format!("unknown length unit: {unit}")))?;
                Ok(Length {
                    value: numeric.value,
                    unit,
                })
            }
            _ => Err(ParseError::new("expected a dimension token for length")),
        }
    }
}

// ── §4.6 Percentage ─────────────────────────────────────────────────

/// CSS `<percentage>` 值 (§4.6)。
#[derive(Debug, Clone, PartialEq)]
pub struct Percentage {
    pub value: f64,
}

impl Percentage {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Percentage(numeric)) => Ok(Percentage {
                value: numeric.value,
            }),
            _ => Err(ParseError::new("expected a percentage token")),
        }
    }
}

// ── §4.4 Number ─────────────────────────────────────────────────────

/// CSS `<number>` 值 (§4.4)。
#[derive(Debug, Clone, PartialEq)]
pub struct Number {
    pub value: f64,
}

impl Number {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Number(numeric)) => Ok(Number {
                value: numeric.value,
            }),
            _ => Err(ParseError::new("expected a number token")),
        }
    }
}

// ── §4.3 Integer ────────────────────────────────────────────────────

/// CSS `<integer>` 值 (§4.3)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integer {
    pub value: i32,
}

impl Integer {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Number(numeric)) if numeric.is_integer => {
                Ok(Integer {
                    value: numeric.value as i32,
                })
            }
            _ => Err(ParseError::new("expected an integer token")),
        }
    }
}

// ── §6.1 Angle ──────────────────────────────────────────────────────

/// CSS `<angle>` 值 (§6.1)。
#[derive(Debug, Clone, PartialEq)]
pub struct Angle {
    pub value: f64,
    pub unit: AngleUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    Deg,
    Grad,
    Rad,
    Turn,
}

impl AngleUnit {
    pub fn parse_unit(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "deg" => Some(Self::Deg),
            "grad" => Some(Self::Grad),
            "rad" => Some(Self::Rad),
            "turn" => Some(Self::Turn),
            _ => None,
        }
    }
}

impl Angle {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                let unit = AngleUnit::parse_unit(unit)
                    .ok_or_else(|| ParseError::new(format!("unknown angle unit: {unit}")))?;
                Ok(Angle {
                    value: numeric.value,
                    unit,
                })
            }
            // §6.1: bare 0 is a valid angle (represents 0deg)
            ComponentValue::PreservedToken(Token::Number(numeric)) if numeric.value == 0.0 => {
                Ok(Angle {
                    value: 0.0,
                    unit: AngleUnit::Deg,
                })
            }
            _ => Err(ParseError::new("expected a dimension or 0 for angle")),
        }
    }
}

// ── §6.2 Time ───────────────────────────────────────────────────────

/// CSS `<time>` 值 (§6.2)。
#[derive(Debug, Clone, PartialEq)]
pub struct Time {
    pub value: f64,
    pub unit: TimeUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    S,
    Ms,
}

impl TimeUnit {
    pub fn parse_unit(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "s" => Some(Self::S),
            "ms" => Some(Self::Ms),
            _ => None,
        }
    }
}

impl Time {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                let unit = TimeUnit::parse_unit(unit)
                    .ok_or_else(|| ParseError::new(format!("unknown time unit: {unit}")))?;
                Ok(Time {
                    value: numeric.value,
                    unit,
                })
            }
            _ => Err(ParseError::new("expected a dimension token for time")),
        }
    }
}

// ── §6.3 Frequency ──────────────────────────────────────────────────

/// CSS `<frequency>` 值 (§6.3)。
#[derive(Debug, Clone, PartialEq)]
pub struct Frequency {
    pub value: f64,
    pub unit: FrequencyUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyUnit {
    Hz,
    KHz,
}

impl FrequencyUnit {
    pub fn parse_unit(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "hz" => Some(Self::Hz),
            "khz" => Some(Self::KHz),
            _ => None,
        }
    }
}

impl Frequency {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                let unit = FrequencyUnit::parse_unit(unit)
                    .ok_or_else(|| ParseError::new(format!("unknown frequency unit: {unit}")))?;
                Ok(Frequency {
                    value: numeric.value,
                    unit,
                })
            }
            _ => Err(ParseError::new("expected a dimension token for frequency")),
        }
    }
}

// ── §6.4 Resolution ─────────────────────────────────────────────────

/// CSS `<resolution>` 值 (§6.4)。
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub value: f64,
    pub unit: ResolutionUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionUnit {
    Dpi,
    Dpcm,
    Dppx,
}

impl ResolutionUnit {
    pub fn parse_unit(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "dpi" => Some(Self::Dpi),
            "dpcm" => Some(Self::Dpcm),
            "dppx" => Some(Self::Dppx),
            _ => None,
        }
    }
}

impl Resolution {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                let unit = ResolutionUnit::parse_unit(unit)
                    .ok_or_else(|| ParseError::new(format!("unknown resolution unit: {unit}")))?;
                Ok(Resolution {
                    value: numeric.value,
                    unit,
                })
            }
            _ => Err(ParseError::new("expected a dimension token for resolution")),
        }
    }
}

// ── §4.7 Ratio ──────────────────────────────────────────────────────

/// CSS `<ratio>` 值 (§4.7)。
///
/// 语法：`<number [0,∞]> [ / <number [0,∞]> ]?`
/// 单数字时 height 默认 1.0。
#[derive(Debug, Clone, PartialEq)]
pub struct Ratio {
    pub width: f64,
    pub height: f64,
}

impl Ratio {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        // 过滤 whitespace，收集非 ws 的 component values
        let filtered: Vec<&ComponentValue> = cvs
            .iter()
            .filter(|cv| !matches!(cv, ComponentValue::PreservedToken(Token::Whitespace)))
            .collect();

        if filtered.is_empty() {
            return Err(ParseError::new("ratio requires at least one number"));
        }

        // 第一个必须是 number
        let width = match filtered[0] {
            ComponentValue::PreservedToken(Token::Number(numeric)) => numeric.value,
            _ => return Err(ParseError::new("ratio first value must be a number")),
        };

        if width < 0.0 {
            return Err(ParseError::new("ratio values must be non-negative"));
        }

        // 只有一个 number → height = 1
        if filtered.len() == 1 {
            return Ok(Ratio { width, height: 1.0 });
        }

        // 两个元素：必须是 `/` + number
        if filtered.len() != 3 {
            return Err(ParseError::new(
                "ratio with second value must be 'number / number'",
            ));
        }

        if !matches!(
            filtered[1],
            ComponentValue::PreservedToken(Token::Delim('/'))
        ) {
            return Err(ParseError::new("expected '/' between ratio numbers"));
        }

        let height = match filtered[2] {
            ComponentValue::PreservedToken(Token::Number(numeric)) => numeric.value,
            _ => return Err(ParseError::new("ratio second value must be a number")),
        };

        if height < 0.0 {
            return Err(ParseError::new("ratio values must be non-negative"));
        }

        Ok(Ratio { width, height })
    }
}
