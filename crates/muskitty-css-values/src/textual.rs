//! 文本类型 — CSS Values Level 4 §3。
//!
//! - §3.1 Pre-defined Keywords
//! - §3.2 `<custom-ident>`
//! - §3.3 `<dashed-ident>`
//! - §3.4 `<string>`
//! - §3.5 `<url>`

use crate::numeric::ParseError;
use muskitty_css::parser::{ComponentValue, Function};
use muskitty_css::tokenizer::Token;

// ── §3.1 Pre-defined Keywords ──────────────────────────────────────

/// CSS `<keyword>` 值 (§3.1)。
///
/// 一个预定义的标识符（如 `auto`、`block`、`none`）。
/// 关键字是 ASCII case-insensitive 的，但本类型保留原始大小写
/// 供序列化使用；比较时建议用 `eq_ignore_ascii_case`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    pub value: String,
}

impl Keyword {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = crate::numeric::single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Ident(s)) => Ok(Keyword { value: s.clone() }),
            _ => Err(ParseError::new("expected an ident token for keyword")),
        }
    }
}

// ── §3.2 custom-ident ──────────────────────────────────────────────

/// CSS-wide keywords (§3.1) + `default` + `none` — 不能作为 `<custom-ident>`
/// (§3.2 L676-678)。
///
/// `default` 由 §3.2 明确保留；`none` 是常见的 property-specific 排除项。
/// 这里采用保守策略：5 个全部排除。
const CSS_WIDE_OR_RESERVED: &[&str] = &["initial", "inherit", "unset", "default", "none"];

/// CSS `<custom-ident>` 值 (§3.2)。
///
/// 任意作者定义的标识符，不能是 CSS-wide keyword、`default` 或 `none`。
/// 大小写敏感（§3.2 L671-674）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomIdent {
    pub value: String,
}

impl CustomIdent {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = crate::numeric::single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                // §3.2 L676-678: CSS-wide keywords + default 不是合法 custom-ident
                if CSS_WIDE_OR_RESERVED
                    .iter()
                    .any(|kw| s.eq_ignore_ascii_case(kw))
                {
                    return Err(ParseError::new(format!(
                        "custom-ident must not be a CSS-wide keyword or 'none': {s}"
                    )));
                }
                Ok(CustomIdent { value: s.clone() })
            }
            _ => Err(ParseError::new("expected an ident token for custom-ident")),
        }
    }
}

// ── §3.3 dashed-ident ──────────────────────────────────────────────

/// CSS `<dashed-ident>` 值 (§3.3)。
///
/// 必须以 `--` 开头的标识符，主要用于 custom property 名和
/// 现在还处于实验阶段的命名空间标识符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashedIdent {
    pub value: String,
}

impl DashedIdent {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = crate::numeric::single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                if !is_dashed_ident(s) {
                    return Err(ParseError::new(format!(
                        "dashed-ident must start with '--': {s}"
                    )));
                }
                Ok(DashedIdent { value: s.clone() })
            }
            _ => Err(ParseError::new("expected an ident token for dashed-ident")),
        }
    }
}

/// 判断字符串是否是 `<dashed-ident>` 形式（以 `--` 开头且长度 > 2）。
pub fn is_dashed_ident(s: &str) -> bool {
    s.starts_with("--") && s.len() > 2
}

// ── §3.4 string ────────────────────────────────────────────────────

/// CSS `<string>` 值 (§3.4)。
///
/// 已解析的字符串内容（去掉了引号，处理了转义）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssString {
    pub value: String,
}

impl CssString {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = crate::numeric::single_non_ws_cv(cvs)?;
        match cv {
            ComponentValue::PreservedToken(Token::String(s)) => Ok(CssString { value: s.clone() }),
            _ => Err(ParseError::new("expected a string token")),
        }
    }
}

// ── §3.5 url ───────────────────────────────────────────────────────

/// CSS `<url>` 值 (§3.5)。
///
/// 支持两种形式：
/// - `url(unquoted-path)` — 由 tokenizer 输出 `Token::Url`
/// - `url("quoted-path")` 或 `url('quoted-path')` — 由 parser 输出
///   `ComponentValue::Function { name: "url", value: [String(...)] }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    pub value: String,
}

impl Url {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        let cv = crate::numeric::single_non_ws_cv(cvs)?;
        match cv {
            // 形式 1：url(unquoted) → Token::Url
            ComponentValue::PreservedToken(Token::Url(s)) => Ok(Url { value: s.clone() }),
            // 形式 2：url("quoted") → Function("url", [String(...)])
            ComponentValue::Function(f) => Ok(Url {
                value: extract_url_from_function(f)?,
            }),
            _ => Err(ParseError::new("expected a url() token or function")),
        }
    }
}

/// 从 `Function { name: "url", value: [...] }` 提取 URL 字符串。
///
/// 接受 `url("string")` 和 `url('string')` 两种形式。
fn extract_url_from_function(f: &Function) -> Result<String, ParseError> {
    if !f.name.eq_ignore_ascii_case("url") {
        return Err(ParseError::new(format!(
            "expected url() function, got {}()",
            f.name
        )));
    }
    // 从参数中找第一个 String token
    for cv in &f.value {
        if let ComponentValue::PreservedToken(Token::String(s)) = cv {
            return Ok(s.clone());
        }
    }
    // 如果没有 String token，可能是 url(unquoted) 的退化形式
    // （tokenizer 一般会直接产出 Token::Url，但保险起见也支持）
    Err(ParseError::new(
        "url() function must contain a string argument",
    ))
}
